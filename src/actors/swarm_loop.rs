use std::{collections::HashMap, time::Duration};

use libp2p::{
    core::ConnectedPoint,
    futures::StreamExt,
    request_response::{RequestId, RequestResponseEvent, RequestResponseMessage, ResponseChannel},
    swarm::SwarmEvent,
    Multiaddr, PeerId,
};
use serde::{de::DeserializeOwned, Serialize};
use tokio::sync::{mpsc, oneshot};

use crate::{
    models::{Message, MessageResult, MessagingError, RawInput, RawOutput},
    net::{behavior::ActorEvent, swarm::new_actor_swarm},
};

use super::{auth::AuthHandle, opts::Ip, ActorHandle};

//
// Hooookay, so, this one's a bit of a mess, especially so because
// EVERYTHING is kind of out of order, so please bear with me...
//

pub(crate) struct SwarmLoop;

///
/// Actor-to-Swarm messaging
///
#[derive(Debug)]
pub(crate) enum SwarmCommand<Output, Error>
where
    Output: Clone + Send + Serialize + DeserializeOwned + 'static,
    Error: Clone + Send + Serialize + DeserializeOwned + 'static,
{
    /// Local actor is sending a request of unknown type to another actor
    Request {
        handle: ActorHandle,
        message: RawInput,
        channel: oneshot::Sender<RawOutput>,
    },
    /// Local actor is sending a response of known type to another actor's request
    Response {
        request_id: RequestId,
        response: MessageResult<Output, Error>,
        peer_id: PeerId,
    },
    Stop,
}

///
/// Swarm-to-Actor messaging
///
/// Remote actor (or top-level code) is sending a request of known type
///
/// Responses of a remote actor, thus of unknown type, are not handled sent with this. See below.
///
pub(crate) struct ActorCommand<Input>
where
    Input: Clone + Send + Serialize + DeserializeOwned + 'static,
{
    pub request_id: RequestId,
    pub message: Message<Input>,
    pub peer_id: PeerId,
    pub address: Option<Multiaddr>,
}

impl SwarmLoop {
    ///
    /// Start the loop and return our address and two channels:
    ///
    /// * A receiver for the actor to receive messages form the swarm
    /// * A sender for the actor to send messages to the swarm
    ///
    pub(crate) async fn start<Input, Output, Error>(
        auth_handle: AuthHandle,
        proto: Ip,
    ) -> Result<
        (
            Multiaddr,
            mpsc::Receiver<ActorCommand<Input>>,
            mpsc::Sender<SwarmCommand<Output, Error>>,
        ),
        SwarmLoopError,
    >
    where
        Input: Clone + Send + Serialize + DeserializeOwned + 'static,
        Output: Clone + Send + Serialize + DeserializeOwned + 'static,
        Error: Clone + Send + Serialize + DeserializeOwned + 'static,
    {
        let keypair = auth_handle
            .fetch_keypair()
            .await
            .map_err(|_| SwarmLoopError::Keypair)?;

        let (mut swarm, address) = new_actor_swarm(keypair, proto)
            .await
            .map_err(|_| SwarmLoopError::Swarm)?;

        let (actor_sender, actor_receiver) = mpsc::channel::<ActorCommand<Input>>(1024);
        let (swarm_sender, mut swarm_receiver) = mpsc::channel::<SwarmCommand<Output, Error>>(1024);

        //
        // incoming requests are stored with their ids and a response channel to send, well, a response
        //
        let mut inbound_requests: HashMap<RequestId, ResponseChannel<RawOutput>> = HashMap::new();

        //
        // outbound requests are stored with their ids and a channel to send responses back to the actor handle
        //
        let mut outbound_requests: HashMap<RequestId, oneshot::Sender<RawOutput>> = HashMap::new();

        //
        // known addresses are stored so we can pass them on to the actor
        //
        let mut known_addresses: HashMap<PeerId, Multiaddr> = HashMap::new();

        tokio::spawn(async move {
            //
            // we loop and select either messages from our actor or from outside
            //
            loop {
                tokio::select! {
                    // actor => swarm => remote
                    command = swarm_receiver.recv() => {
                        if let Some(command) = command {
                            match command {
                                //
                                // our actor is sending out a request
                                //
                                // store its id along with a channel to send the result later
                                //
                                SwarmCommand::Request {
                                    handle,
                                    message,
                                    channel,
                                } => {
                                    swarm.behaviour_mut().kad.add_address(&handle.peer, handle.addr);

                                    let result = swarm.behaviour_mut().req_rep.send_request(&handle.peer, message);
                                    outbound_requests.insert(result, channel);
                                }
                                //
                                // our actor is sending out a response of known type, serialize and yeet
                                //
                                // we can remove the request from the inbound requests store
                                //
                                SwarmCommand::Response {
                                    request_id,
                                    response,
                                    peer_id
                                } => {
                                    if let Some(channel) = inbound_requests.remove(&request_id) {
                                        let serialize = bincode::serialize(&response);
                                        if let Ok(blob) = serialize {
                                            let _ = swarm.behaviour_mut().req_rep.send_response(channel, blob);
                                        } else {
                                            tracing::error!("Failed to serialize response with id {} for {}", request_id, peer_id);
                                        }
                                    }

                                    if let Err(MessagingError::Banned) = response {
                                        // wait a little for the message to be sent so the other party knows it has been banned
                                        tokio::time::sleep(Duration::from_millis(10)).await;
                                        swarm.ban_peer_id(peer_id);
                                    }
                                },
                                SwarmCommand::Stop => {
                                    // if we break immediately, we drop our swarm (and the current connection) before we can send a reply
                                    tracing::info!("Stopping swarm for actor with peer ID {}", swarm.local_peer_id());
                                    tokio::time::sleep(Duration::from_millis(10)).await;
                                    break;
                                },
                            }
                        }
                    }
                    // remote => swarm => actor
                    event = swarm.select_next_some() => {
                        match event {
                            //
                            // we established a connection,
                            // store the incoming address in the auth actor with its peer id as the key
                            //
                            // we need this so the we can fetch this address later during authorization
                            //
                            SwarmEvent::ConnectionEstablished {
                                peer_id: peer,
                                endpoint:
                                    ConnectedPoint::Listener {
                                        local_addr: _,
                                        send_back_addr: addr,
                                    },
                                num_established: _,
                                concurrent_dial_errors: _,
                            } => {
                                let _ = known_addresses.insert(peer, addr);
                            },
                            SwarmEvent::Behaviour(ActorEvent::ReqRepEvent(event)) => {
                                match event {
                                    RequestResponseEvent::Message {
                                            peer,
                                            message
                                        } => match message {
                                            //
                                            // incoming request of known type, deserialize and handle as usual
                                            //
                                            RequestResponseMessage::Request {
                                                request_id,
                                                request,
                                                channel
                                            } => {
                                                tracing::debug!("Incoming request, asigned ID {}. Handling...", request_id);
                                                let deserialize = bincode::deserialize(&request);
                                                if let Ok(message) = deserialize {
                                                    let address = known_addresses.get(&peer).map(|a| a.to_owned());
                                                    let result = actor_sender.send(ActorCommand { request_id, message, peer_id: peer, address }).await;
                                                    if result.is_ok() {
                                                        inbound_requests.insert(request_id, channel);
                                                    } else {
                                                        tracing::warn!("Failed to relay request to actor!");
                                                    }

                                                } else {
                                                    tracing::error!("Failed to deserialize request from peer {}", peer);
                                                }
                                            },
                                            //
                                            // incoming response of unknown type, pass it on to the actor
                                            // if it still wants it
                                            //
                                            RequestResponseMessage::Response {
                                                request_id,
                                                response
                                            } => {
                                                if let Some(channel) = outbound_requests.remove(&request_id) {
                                                    // not much we can do if this send fails
                                                    if channel.send(response).is_err() {
                                                        tracing::warn!("Failed to relay incoming response with id {} and peer ID {}", request_id, peer);
                                                    }
                                                } else {
                                                    tracing::warn!("Incoming response with unknown ID!");
                                                }

                                            },
                                        },
                                    //
                                    // we remove the request ids from their corresponding stores on failure
                                    //
                                    RequestResponseEvent::OutboundFailure {
                                        peer: _,
                                        request_id,
                                        error
                                    } => {
                                        tracing::error!("failed to send request: {error}");
                                        let _ = outbound_requests.remove(&request_id);
                                    },
                                    RequestResponseEvent::InboundFailure {
                                        peer: _,
                                        request_id,
                                        error
                                    } => {
                                        tracing::error!("failed to receive incoming request: {error}");
                                        let _ = inbound_requests.remove(&request_id);
                                    },
                                    RequestResponseEvent::ResponseSent {
                                        peer: _,
                                        request_id: _
                                    } => {
                                        // Nothing here for now
                                    },
                                }
                            },
                            _ => {}
                        }
                    }
                }
            }
        });

        Ok((address, actor_receiver, swarm_sender))
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum SwarmLoopError {
    #[error("could not fetch keypair from auth actor")]
    Keypair,

    #[error("failed to initialize swarm")]
    Swarm,
}
