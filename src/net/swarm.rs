//!
//! "Request" and "Response" swarms for messaging
//!

use libp2p::{
    futures::StreamExt,
    identity::Keypair,
    kad::{store::MemoryStore, Kademlia},
    request_response::{ProtocolSupport, RequestResponse, RequestResponseConfig},
    swarm::{SwarmBuilder, SwarmEvent},
    Multiaddr, PeerId, Swarm,
};

use crate::actors::opts::Ip;

use super::{
    behavior::ActorBehaviour,
    codec::{ActorProtocol, MessagingCodec},
};

///
/// swarm constructor used inside the inner loop of a remote actor
///
pub(crate) async fn new_actor_swarm(
    keypair: Keypair,
    proto: Ip,
) -> Result<(Swarm<ActorBehaviour>, Multiaddr), Box<dyn std::error::Error>> {
    let peer_id = PeerId::from_public_key(&keypair.public());

    //
    // willfully ignoring the warning about using libp2p::tokio_development_transport
    // as it is exactly what we need
    //
    let transport = libp2p::tokio_development_transport(keypair)?;
    let kad = Kademlia::new(peer_id, MemoryStore::new(peer_id));
    let req_rep = RequestResponse::new(
        MessagingCodec,
        vec![(ActorProtocol::V1, ProtocolSupport::Full)],
        RequestResponseConfig::default(),
    );

    let behavior = ActorBehaviour { req_rep, kad };

    let mut swarm = SwarmBuilder::with_executor(
        transport,
        behavior,
        peer_id,
        Box::new(|fut| {
            tokio::spawn(fut);
        }),
    )
    .build();

    match proto {
        Ip::V4 => {
            swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;
        }
        Ip::V6 => {
            swarm.listen_on("/ip6/::1/tcp/0".parse()?)?;
        }
        Ip::Both => {
            swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;
            swarm.listen_on("/ip6/::1/tcp/0".parse()?)?;
        }
    };

    let (tx, rx) = tokio::sync::oneshot::channel();

    loop {
        if let SwarmEvent::NewListenAddr {
            listener_id: _,
            address,
        } = swarm.select_next_some().await
        {
            let _ = tx.send(address);
            break;
        }
    }

    Ok((swarm, rx.await?))
}

///
/// swarm used for messaging an actor
///
pub(crate) async fn new_messaging_swarm(
    keypair: Keypair,
) -> Result<Swarm<ActorBehaviour>, Box<dyn std::error::Error>> {
    let peer_id = PeerId::from_public_key(&keypair.public());

    //
    // willfully ignoring the warning about using libp2p::tokio_development_transport
    // as it is exactly what we need
    //
    let transport = libp2p::tokio_development_transport(keypair)?;
    let kad = Kademlia::new(peer_id, MemoryStore::new(peer_id));
    let req_rep = RequestResponse::new(
        MessagingCodec,
        vec![(ActorProtocol::V1, ProtocolSupport::Outbound)],
        RequestResponseConfig::default(),
    );

    let behavior = ActorBehaviour { req_rep, kad };
    let swarm = SwarmBuilder::with_executor(
        transport,
        behavior,
        peer_id,
        Box::new(|fut| {
            tokio::spawn(fut);
        }),
    )
    .build();

    Ok(swarm)
}
