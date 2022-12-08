//!
//! libp2p behavior for actors
//!

use libp2p::{
    kad::{store::MemoryStore, Kademlia, KademliaEvent},
    request_response::{RequestResponse, RequestResponseEvent},
    swarm::NetworkBehaviour,
};

use super::codec::MessagingCodec;
use crate::models::{RawInput, RawOutput};

///
/// Network Behavior for our actors
///
#[allow(missing_debug_implementations)]
#[derive(NetworkBehaviour)]
#[behaviour(out_event = "ActorEvent")]
pub(crate) struct ActorBehaviour {
    /// Request-Response network behavior for actors
    pub req_rep: RequestResponse<MessagingCodec>,

    /// Kademlia network behavior for actor discovery by PeerId
    pub kad: Kademlia<MemoryStore>,
}

///
/// Events used by our network behavior
///
#[derive(Debug)]
pub(crate) enum ActorEvent {
    /// Request-Response event for coordinating messages and responses
    ReqRepEvent(RequestResponseEvent<RawInput, RawOutput>),
    /// Kademlia event for peer discovery
    KademliaEvent(KademliaEvent),
}

impl From<RequestResponseEvent<RawInput, RawOutput>> for ActorEvent {
    fn from(ev: RequestResponseEvent<RawInput, RawOutput>) -> Self {
        Self::ReqRepEvent(ev)
    }
}

impl From<KademliaEvent> for ActorEvent {
    fn from(ev: KademliaEvent) -> Self {
        Self::KademliaEvent(ev)
    }
}
