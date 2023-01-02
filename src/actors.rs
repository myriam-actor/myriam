//!
//! Main actor module
//!

#[cfg(test)]
mod tests;

use std::{error::Error, sync::Arc};

use async_trait::async_trait;
use libp2p::{
    futures::StreamExt,
    request_response::{RequestResponseEvent, RequestResponseMessage},
    swarm::SwarmEvent,
    Multiaddr, PeerId,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::{sync::mpsc, sync::oneshot, task::JoinHandle};

use crate::{
    actors::swarm_loop::{SwarmCommand, SwarmLoop},
    models::{MessageResult, MessageType, MessagingError, RawOutput, TaskResult},
    net::{behavior::ActorEvent, swarm::new_messaging_swarm},
};

use self::{
    auth::{AccessResolution, AuthHandle},
    opts::SpawnOpts,
    swarm_loop::ActorCommand,
};

pub mod auth;
pub mod opts;
mod swarm_loop;

#[async_trait]
///
/// Main actor trait
///
/// Only missing methods are safe to implement
///
pub trait Actor: Send + 'static {
    /// Input of a message
    type Input: Clone + Send + Serialize + DeserializeOwned + 'static;

    /// Output of a message
    type Output: Clone + Send + Serialize + DeserializeOwned + 'static;

    /// Error in the messaging flow
    type Error: Error + Clone + Send + Serialize + DeserializeOwned + 'static;

    /*
     * Default implementations
     */
    ///
    /// Consume a boxed actor, spawning an instance of it
    ///
    async fn spawn(
        mut self: Box<Self>,
        auth_handle: AuthHandle,
        opts: SpawnOpts,
    ) -> Result<(ActorHandle, JoinHandle<()>), Box<dyn std::error::Error>> {
        let (local_address, mut swarm_receiver, swarm_sender) =
            SwarmLoop::start::<Self::Input, Self::Output, Self::Error>(
                auth_handle.clone(),
                opts.protocol.unwrap_or_default(),
            )
            .await?;

        let peer_id = PeerId::from_public_key(&auth_handle.fetch_keypair().await?.public());
        let self_handle = ActorHandle {
            addr: local_address,
            peer: peer_id,
        };

        let self_handle_inner = self_handle.clone();

        self.on_init().await?;

        let task = tokio::spawn(async move {
            let self_handle = Arc::new(self_handle_inner);
            while let Some(ActorCommand {
                request_id,
                message,
                peer_id,
                address,
            }) = swarm_receiver.recv().await
            {
                let message_type = message.message_type.clone();
                let access_descriptor = (&message_type).into();

                let context = Context {
                    self_addr: self_handle.as_ref().clone(),
                    sender: peer_id,
                    swarm_sender: swarm_sender.clone(),
                };

                if let Ok(r) = auth_handle
                    .resolve(peer_id, address, access_descriptor)
                    .await
                {
                    match r {
                        AccessResolution::Accepted => {
                            match message_type {
                                MessageType::Ping => {
                                    if let Err(err) = swarm_sender
                                        .send(SwarmCommand::Response {
                                            request_id,
                                            response: Ok(TaskResult::Accepted),
                                            peer_id,
                                        })
                                        .await
                                    {
                                        tracing::warn!(
                                            "Failed to relay response back to swarm: {err}"
                                        );
                                    }
                                }
                                MessageType::Stop => {
                                    if let Err(err) = swarm_sender
                                        .send(SwarmCommand::Response {
                                            request_id,
                                            response: Ok(TaskResult::Accepted),
                                            peer_id,
                                        })
                                        .await
                                    {
                                        tracing::warn!(
                                            "Failed to relay response back to swarm: {err}"
                                        );
                                    }

                                    if let Err(err) = swarm_sender.send(SwarmCommand::Stop).await {
                                        tracing::warn!(
                                            "Failed to send stop message to swarm: {err}"
                                        )
                                    }

                                    break;
                                }
                                MessageType::TaskRequest(arg) => {
                                    let response = self
                                        .handle(arg, context)
                                        .await
                                        .map(TaskResult::Finished)
                                        .map_err(MessagingError::Task);

                                    if let Err(err) = swarm_sender
                                        .send(SwarmCommand::Response {
                                            request_id,
                                            response,
                                            peer_id,
                                        })
                                        .await
                                    {
                                        tracing::warn!("Failed to relay request to swarm: {err}");
                                    }
                                }
                                MessageType::Task(arg) => {
                                    if let Err(err) = swarm_sender
                                        .send(SwarmCommand::Response {
                                            request_id,
                                            response: Ok(TaskResult::Accepted),
                                            peer_id,
                                        })
                                        .await
                                    {
                                        tracing::warn!(
                                            "Failed to relay response back to swarm: {err}"
                                        );
                                    }

                                    if let Err(err) = self.handle(arg, context).await {
                                        tracing::warn!(
                                            "Queued task completed with an error: {err}"
                                        );
                                    }
                                }
                            };
                        }
                        AccessResolution::Denied => {
                            if let Err(err) = swarm_sender
                                .send(SwarmCommand::Response {
                                    request_id,
                                    response: Err(MessagingError::Unauthorized),
                                    peer_id,
                                })
                                .await
                            {
                                tracing::warn!("Failed to relay response back to swarm: {err}");
                            }
                        }
                        AccessResolution::Ban => {
                            if let Err(err) = swarm_sender
                                .send(SwarmCommand::Response {
                                    request_id,
                                    response: Err(MessagingError::Banned),
                                    peer_id,
                                })
                                .await
                            {
                                tracing::warn!("Failed to relay response back to swarm: {err}");
                            }
                        }
                    }
                } else {
                    let _ = swarm_sender
                        .send(SwarmCommand::Response {
                            request_id,
                            response: Err(MessagingError::Internal),
                            peer_id,
                        })
                        .await;
                }
            }

            let on_stop = self.on_stop();
            on_stop.await;
        });

        Ok((self_handle.clone(), task))
    }

    ///
    /// Send a message to another actor
    ///
    async fn send<Input, Output, Error>(
        &self,
        handle: ActorHandle,
        message: MessageType<Input>,
        ctx: Context<Self::Output, Self::Error>,
    ) -> MessageResult<Output, Error>
    where
        Input: Clone + Send + Serialize + DeserializeOwned,
        Output: Clone + Send + Serialize + DeserializeOwned,
        Error: Clone + Send + Serialize + DeserializeOwned,
    {
        let (sender, receiver) = oneshot::channel::<RawOutput>();
        let blob = bincode::serialize(&message).map_err(|_| MessagingError::Serialize)?;

        ctx.swarm_sender
            .send(SwarmCommand::Request {
                channel: sender,
                handle,
                message: blob,
            })
            .await
            .map_err(|_| MessagingError::Send)?;

        let blob_response = receiver.await.map_err(|_| MessagingError::Receive)?;

        bincode::deserialize(&blob_response).map_err(|_| MessagingError::Serialize)?
    }

    /*
     * Missing implementations
     */
    ///
    /// Handle an incoming message
    ///
    async fn handle(
        &mut self,
        msg: Self::Input,
        ctx: Context<Self::Output, Self::Error>,
    ) -> Result<Self::Output, Self::Error>;

    ///
    /// Method to execute on actor initialization
    ///
    /// Initialization will fail if this method returns an error
    ///
    async fn on_init(&mut self) -> Result<(), Box<dyn std::error::Error>>;

    ///
    /// Method to execute on actor termination
    ///
    async fn on_stop(&mut self);
}

///
/// Context for the current message being handled
///
#[derive(Debug, Clone)]
pub struct Context<Output, Error>
where
    Output: Clone + Send + Serialize + DeserializeOwned + 'static,
    Error: Clone + Send + Serialize + DeserializeOwned + 'static,
{
    swarm_sender: mpsc::Sender<SwarmCommand<Output, Error>>,
    self_addr: ActorHandle,
    sender: PeerId,
}

impl<Output, Error> Context<Output, Error>
where
    Output: Clone + Send + Serialize + DeserializeOwned,
    Error: Clone + Send + Serialize + DeserializeOwned,
{
    ///
    /// The actor's own address
    ///
    pub fn self_addr(&self) -> ActorHandle {
        self.self_addr.clone()
    }

    ///
    /// The sender's PeerId
    ///
    pub fn sender(&self) -> PeerId {
        self.sender
    }
}

///
/// Handle to message an actor
///
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorHandle {
    /// Address of the actor
    pub addr: Multiaddr,

    /// Peer ID of the actor
    pub peer: PeerId,
}

impl ActorHandle {
    ///
    /// Send a message to an actor from top level
    ///
    pub async fn send_toplevel<Input, Output, Error>(
        &self,
        msg: MessageType<Input>,
        auth_handle: AuthHandle,
    ) -> MessageResult<Output, Error>
    where
        Input: Send + Serialize + DeserializeOwned,
        Output: Send + Serialize + DeserializeOwned,
        Error: Send + Serialize + DeserializeOwned,
    {
        let msg = bincode::serialize(&msg).map_err(|_| MessagingError::Serialize)?;

        let keypair = auth_handle
            .fetch_keypair()
            .await
            .map_err(|_| MessagingError::AuthHandle)?;
        let mut swarm = new_messaging_swarm(keypair)
            .await
            .map_err(|_| MessagingError::Dial)?;

        swarm
            .behaviour_mut()
            .kad
            .add_address(&self.peer, self.addr.clone());

        swarm
            .dial(self.addr.clone())
            .map_err(|_| MessagingError::Dial)?;

        swarm.behaviour_mut().req_rep.send_request(&self.peer, msg);

        tracing::debug!("Request sent to {}, awaiting response...", self.addr);

        loop {
            if let SwarmEvent::Behaviour(event) = swarm.select_next_some().await {
                match event {
                    ActorEvent::ReqRepEvent(event) => match event {
                        RequestResponseEvent::Message {
                            peer: _,
                            message:
                                RequestResponseMessage::Response {
                                    request_id: _,
                                    response,
                                },
                        } => {
                            let response = bincode::deserialize(response.as_slice())
                                .map_err(|_| MessagingError::Serialize)?;

                            return response;
                        }
                        RequestResponseEvent::OutboundFailure {
                            peer,
                            request_id: _,
                            error,
                        } => {
                            tracing::error!("failed to send message or receive response from peer {peer}: {error}");
                            return Err(MessagingError::Receive);
                        }
                        _ => {}
                    },
                    ActorEvent::KademliaEvent(_) => {}
                }
            }
        }
    }
}
