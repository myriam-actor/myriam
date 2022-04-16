use std::{io, sync::Arc};

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{mpsc, oneshot},
};

use crate::{
    address::{Address, AddressError},
    auth::{AuthError, AuthHandle},
    identity::PublicIdentity,
    messaging::{Message, MessageContext, MessageResult, MessageType, MessagingError, TaskResult},
    net,
};

#[async_trait]
pub trait Actor {
    ///
    /// Spawn an actor and return a handle to it.
    ///
    /// You are NOT meant to implement this, only [Self::handle].
    ///
    async fn spawn<T, U, E>(
        &'static self,
        opts: ActorOptions,
        auth: AuthHandle,
    ) -> Result<ActorHandle, SpawnError>
    where
        T: DeserializeOwned + Send + 'static,
        U: Serialize + Send + 'static,
        E: Serialize + Send + 'static,
    {
        let address = match opts.port {
            Some(p) => Address::new_with_checked_port(&opts.host, p)?,
            None => Address::new_with_random_port(&opts.host)?,
        };

        let self_identity = auth.fetch_self_identity().await?;
        let public_identity = self_identity.public_identity().clone();

        let context = Arc::new(Context {
            self_address: address.clone(),
        });
        let auth_handle = Arc::new(auth);

        let listener = TcpListener::bind(address.to_string()).await?;
        tokio::spawn(async move {
            let (stop_tx, mut stop_rx) = mpsc::channel::<()>(100);

            // TODO: what if .accept() fails?
            while let Ok((mut socket, addr)) = listener.accept().await {
                if stop_rx.try_recv().is_ok() {
                    break;
                }

                let context = context.clone();
                let auth_handle = auth_handle.clone();
                let self_identity = self_identity.clone();
                let stop_tx = stop_tx.clone();

                tokio::spawn(async move {
                    let (rd, wr) = socket.split();
                    match net::try_read_message::<Message<T>>(rd, &auth_handle, addr.ip()).await {
                        Ok((message, identity)) => {
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
                                                let _ = self
                                                    .handle::<T, U, E>(
                                                        &context,
                                                        message.sender,
                                                        arg,
                                                    )
                                                    .await;
                                            });

                                            let _ = tx.send(Ok(TaskResult::Accepted));
                                        }
                                        MessageContext::Yielding => {
                                            match self
                                                .handle::<T, U, E>(&context, message.sender, arg)
                                                .await
                                            {
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

                            if let Err(e) =
                                net::try_write_message(response, &identity, wr, self_identity).await
                            {
                                // TODO: log the failure and carry on, not much else we can do at this point
                            }
                        }
                        Err(e) => {
                            // TODO: log the failure and carry on
                        }
                    }
                });
            }
        });

        Ok(ActorHandle {
            address,
            identity: public_identity,
        })
    }

    async fn handle<T, U, E>(&self, ctx: &Context, addr: Option<Address>, arg: T) -> Result<U, E>
    where
        T: DeserializeOwned + Send + 'static,
        U: Serialize + Send + 'static,
        E: Serialize + Send + 'static;
}

#[derive(Serialize, Deserialize)]
pub struct ActorHandle {
    address: Address,
    identity: PublicIdentity,
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
        if net::try_write_message::<Message<T>>(message, &self.identity, wr, self_identity)
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
}

#[derive(Clone)]
pub struct Context {
    self_address: Address,
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
