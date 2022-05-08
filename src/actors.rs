//!
//! Here you will find the "core" of this implementation. Particularly, the [Actor] trait, with a generic handler and a spawning function.
//! Additionally, we define a handler which is itself used to send messages to an actor.
//!

use std::{io, sync::Arc, time::Duration};

use async_trait::async_trait;
use lazy_static::lazy_static;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

use crate::{
    address::{Address, AddressError},
    auth::{AuthError, AuthHandle},
    identity::PublicIdentity,
    messaging::{Message, MessageContext, MessageResult, MessageType, MessagingError, TaskResult},
    net,
};

const MAX_READ_TIMEOUT_VAR_NAME: &str = "MYRIAM_READ_TIMEOUT";

/// max time to wait for incoming message in milliseconds
const DEFAULT_MAX_READ_TIMEOUT: u64 = 30_000;
lazy_static! {
    static ref MAX_READ_TIMEOUT: u64 = match std::env::var(MAX_READ_TIMEOUT_VAR_NAME) {
        Ok(s) => match s.parse::<u64>() {
            Ok(t) => t,
            Err(_) => DEFAULT_MAX_READ_TIMEOUT,
        },
        Err(_) => DEFAULT_MAX_READ_TIMEOUT,
    };
}

///
/// Actor is a trait. You only have to implement a handle method for your type.
///
/// [Actor::Input], [Actor::Output] and [Actor::Error] correspond to the input and result types of the task implemented as [Actor::handle]
///
#[async_trait]
pub trait Actor {
    type Input: DeserializeOwned + Send + 'static;
    type Output: Serialize + Send + 'static;
    type Error: Serialize + Send + 'static;

    ///
    /// Handling method for incoming messages.
    ///
    /// `ctx` contains the actors' own address, while `addr` (if present) correspond to the address of the actor that sent the message.
    ///
    async fn handle(
        &self,
        ctx: &Context,
        addr: Option<Address>,
        arg: Self::Input,
    ) -> Result<Self::Output, Self::Error>;
}

///
/// Take ownership of a dyn Actor though a Box and spawn an instance of it.
///
/// If all goes well, returns a handle to the actor and the `JoinHandle<()>` of its internal task.
///
pub async fn spawn<T, U, E>(
    actor: Box<dyn Actor<Input = T, Output = U, Error = E> + Send + Sync>,
    opts: ActorOptions,
    auth: AuthHandle,
) -> Result<(ActorHandle, JoinHandle<()>), SpawnError>
where
    T: DeserializeOwned + Send + 'static,
    U: Serialize + Send + 'static,
    E: Serialize + Send + 'static,
{
    let address = match opts.port {
        Some(p) => Address::new_with_checked_port(&opts.host, p)?,
        None => Address::new_with_random_port(&opts.host)?,
    };

    let read_timeout: u64 = match opts.read_timeout {
        Some(t) => t,
        None => *MAX_READ_TIMEOUT,
    };

    let self_identity = auth.fetch_self_identity().await?;
    let public_identity = self_identity.public_identity().clone();

    let context = Arc::new(Context {
        self_address: address.clone(),
    });
    let auth_handle = Arc::new(auth);

    let listener = TcpListener::bind(address.to_string()).await?;
    tracing::debug!("new actor listening on {}", listener.local_addr().unwrap());
    let task_handle = tokio::spawn(async move {
        let (stop_tx, mut stop_rx) = mpsc::channel::<()>(100);
        let actor = Arc::new(actor);

        // TODO: what if .accept() fails?
        while let Ok((mut socket, addr)) = listener.accept().await {
            if stop_rx.try_recv().is_ok() {
                break;
            }

            let context = context.clone();
            let auth_handle = auth_handle.clone();
            let self_identity = self_identity.clone();
            let stop_tx = stop_tx.clone();
            let actor = actor.clone();

            tokio::spawn(async move {
                let (rd, wr) = socket.split();
                if let Ok(Ok((message, identity))) = tokio::time::timeout(
                    Duration::from_millis(read_timeout),
                    net::try_read_message::<Message<T>>(rd, &auth_handle, addr.ip()),
                )
                .await
                {
                    let (tx, rx) = oneshot::channel::<MessageResult<U, E>>();

                    tokio::spawn(async move {
                        match message.message_type {
                            MessageType::Ping => {
                                let _ = tx.send(Ok(TaskResult::Accepted));
                            }
                            MessageType::Stop => {
                                let _ = stop_tx.send(());
                                let _ = tx.send(Ok(TaskResult::Accepted));
                            }
                            MessageType::Task(arg) => match message.context {
                                MessageContext::NonYielding => {
                                    tokio::spawn(async move {
                                        let _ = actor.handle(&context, message.sender, arg).await;
                                    });

                                    let _ = tx.send(Ok(TaskResult::Accepted));
                                }
                                MessageContext::Yielding => {
                                    match actor.handle(&context, message.sender, arg).await {
                                        Ok(res) => {
                                            let _ = tx.send(Ok(TaskResult::Finished(res)));
                                        }
                                        Err(err) => {
                                            let _ = tx.send(Err(MessagingError::Task(err)));
                                        }
                                    }
                                }
                            },
                        }
                    });

                    let response = match rx.await {
                        Ok(res) => res,
                        Err(_) => return,
                    };

                    if (net::try_write_message(response, wr, &identity, self_identity).await)
                        .is_err()
                    {
                        tracing::warn!("Failed to write response to trusted request. Maybe the reader dropped?");
                    }
                } else {
                    tracing::warn!(
                        "Got an incoming message, but failed to read it entirely. Dropping connection."
                    );
                }
            });
        }
    });

    Ok((
        ActorHandle {
            address,
            identity: public_identity,
        },
        task_handle,
    ))
}

///
/// Handle to message an actor. [ActorHandle::identity] corresponds to the public identity (public key) of said actor.
///
/// We want it to derive {De}Serialize for stuff like actor discovery, etc.
///
#[derive(Serialize, Deserialize)]
pub struct ActorHandle {
    pub address: Address,
    pub identity: PublicIdentity,
}

impl ActorHandle {
    pub async fn send_from<T, U, E>(
        &self,
        msg: MessageType<T>,
        ctx: MessageContext,
        from: Option<Address>,
        auth_handle: &AuthHandle,
    ) -> MessageResult<U, E>
    where
        T: Serialize,
        U: DeserializeOwned,
        E: DeserializeOwned,
    {
        let message = Message {
            context: ctx,
            message_type: msg,
            sender: from,
        };

        let connection = TcpStream::connect(self.address.to_string()).await;
        let mut stream = match connection {
            Ok(s) => s,
            Err(_) => return Err(MessagingError::Transport),
        };

        let self_identity = match auth_handle.fetch_self_identity().await {
            Ok(id) => id,
            Err(_) => return Err(MessagingError::Internal),
        };

        let (rd, wr) = stream.split();
        if net::try_write_message::<Message<T>>(message, wr, &self.identity, self_identity)
            .await
            .is_err()
        {
            return Err(MessagingError::Send);
        }

        let addr = match self.address.clone().try_into() {
            Ok(a) => a,
            Err(_) => return Err(MessagingError::Internal),
        };

        match net::try_read_message::<MessageResult<U, E>>(rd, auth_handle, addr).await {
            Ok((res, _)) => res,
            Err(_) => Err(MessagingError::Recv),
        }
    }

    pub async fn send<T, U, E>(
        &self,
        msg: MessageType<T>,
        ctx: MessageContext,
        auth_handle: &AuthHandle,
    ) -> MessageResult<U, E>
    where
        T: Serialize,
        U: DeserializeOwned,
        E: DeserializeOwned,
    {
        self.send_from(msg, ctx, None, auth_handle).await
    }
}

pub struct ActorOptions {
    pub host: String,
    pub port: Option<u16>,
    pub read_timeout: Option<u64>,
}

///
/// Struct passed to [Actor::handle] containing the address of itself.
///
#[derive(Clone)]
pub struct Context {
    pub self_address: Address,
}

#[derive(Debug, Error)]
pub enum SpawnError {
    #[error("{0}")]
    Address(#[from] AddressError),

    #[error("failed to open listener: {0}")]
    Listener(#[from] io::Error),

    #[error("{0}")]
    Auth(#[from] AuthError),
}
