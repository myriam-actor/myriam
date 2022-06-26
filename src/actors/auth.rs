//!
//! authorization helpers for remote actors
//!

use std::collections::HashSet;

use async_trait::async_trait;
use libp2p::{identity::Keypair, Multiaddr, PeerId};
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};

use crate::models::MessageType;

/// Type alias for store of Multiaddr used by AuthActor
pub type AddrStore = HashSet<Multiaddr>;

/// Type alias for store of PeerId used by AuthActor
pub type PeerStore = HashSet<PeerId>;

///
/// Custom local actor for handling authorization requests
///
#[async_trait]
pub trait AuthActor {
    ///
    /// Spawn an instance
    ///
    async fn spawn(mut self: Box<Self>, keypair: Keypair) -> AuthHandle
    where
        Self: 'static,
    {
        let (tx, mut rx) = mpsc::channel(1024);
        tokio::spawn(async move {
            let mut peers = HashSet::new();
            let mut addrs = HashSet::new();
            while let Some(request) = rx.recv().await {
                match request {
                    AuthCommand::PutPeer(p) => {
                        peers.insert(p);
                    }
                    AuthCommand::PutAddr(addr) => {
                        addrs.insert(addr);
                    }
                    AuthCommand::GetKeypair { sender } => {
                        let _ = sender.send(keypair.clone());
                    }
                    AuthCommand::Resolve { request, sender } => {
                        let _ = sender.send(self.resolve(request, &mut addrs, &mut peers).await);
                    }
                    AuthCommand::Stop => break,
                }
            }
        });

        AuthHandle { sender: tx }
    }

    ///
    /// resolve the incoming authorization request
    ///
    async fn resolve(
        &mut self,
        request: AccessRequest,
        addr_store: &mut AddrStore,
        peer_store: &mut PeerStore,
    ) -> AccessResolution;
}

///
/// Handle to message this AuthActor
///
#[derive(Debug, Clone)]
pub struct AuthHandle
where
    Self: 'static,
{
    sender: mpsc::Sender<AuthCommand>,
}

impl AuthHandle {
    ///
    /// Insert a new PeerId inside the AuthActor's PeerStore
    ///
    pub async fn store_peer(&self, peer: PeerId) -> Result<(), AuthError> {
        Ok(self.sender.send(AuthCommand::PutPeer(peer)).await?)
    }

    ///
    /// Insert a new MultiAddr inside the AuthActor's AddrStore
    ///
    pub async fn store_addr(&self, addr: Multiaddr) -> Result<(), AuthError> {
        Ok(self.sender.send(AuthCommand::PutAddr(addr)).await?)
    }

    ///
    /// Fetch our Keypair from this AuthActor
    ///
    pub async fn fetch_keypair(&self) -> Result<Keypair, AuthError> {
        let (sender, receiver) = oneshot::channel();
        self.sender.send(AuthCommand::GetKeypair { sender }).await?;

        Ok(receiver.await?)
    }

    ///
    /// Resolve an incoming AuthRequest
    ///
    pub(crate) async fn resolve(
        &self,
        peer: PeerId,
        addr: Option<Multiaddr>,
        access: AccessDescription,
    ) -> Result<AccessResolution, AuthError> {
        let request = AccessRequest { addr, peer, access };

        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(AuthCommand::Resolve { request, sender })
            .await?;

        Ok(receiver.await?)
    }

    ///
    /// Stop this AuthActor, should you have any sane reason to do so. Note that this will happen
    /// automatically when all handles are dropped anyway.
    ///
    /// **WARNING**: stopping an AuthActor prematurely WILL cripple ALL other actors depending on it.
    /// You have been warned!
    ///
    pub async fn stop(&self) -> Result<(), AuthError> {
        Ok(self.sender.send(AuthCommand::Stop).await?)
    }
}

#[derive(Debug)]
enum AuthCommand {
    PutPeer(PeerId),
    PutAddr(Multiaddr),
    GetKeypair {
        sender: oneshot::Sender<Keypair>,
    },
    Resolve {
        request: AccessRequest,
        sender: oneshot::Sender<AccessResolution>,
    },
    Stop,
}

///
/// Information about the peer trying to access our resources
///
#[derive(Debug)]
pub struct AccessRequest {
    /// Multiaddr of the peer
    pub addr: Option<Multiaddr>,

    /// PeerId of the peer
    pub peer: PeerId,

    /// Incoming message of this request
    pub access: AccessDescription,
}

///
/// Access type required by a message
///
#[derive(Debug, Clone, Copy)]
pub enum AccessDescription {
    /// Simple Ping request
    Ping,

    /// Stop request
    Stop,

    /// Task request
    Task,
}

impl<T> From<&MessageType<T>> for AccessDescription {
    fn from(m: &MessageType<T>) -> Self {
        match m {
            MessageType::Ping => Self::Ping,
            MessageType::Stop => Self::Stop,
            _ => Self::Task,
        }
    }
}

///
/// Whether to accept or deny an incoming request
///
#[derive(Debug)]
pub enum AccessResolution {
    /// Request accepted
    Accepted,

    /// Request denied
    Denied,

    /// Request denied -- ban this peer
    Ban,
}

///
/// Possible errors while messaging an AuthActor
///
#[derive(Debug, Error)]
pub enum AuthError {
    /// Failed to send request to the actor
    #[error("failed to send request to actor")]
    Send,

    /// Failed to receive a response from the actor
    #[error("failed to receive response from actor")]
    Recv(#[from] oneshot::error::RecvError),
}

impl From<mpsc::error::SendError<AuthCommand>> for AuthError {
    fn from(_: mpsc::error::SendError<AuthCommand>) -> Self {
        AuthError::Send
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use libp2p::{identity::Keypair, Multiaddr, PeerId};

    use super::{
        AccessDescription, AccessRequest, AccessResolution, AddrStore, AuthActor, PeerStore,
    };

    struct Auth;

    #[async_trait]
    impl AuthActor for Auth {
        async fn resolve(
            &mut self,
            request: AccessRequest,
            addr_store: &mut AddrStore,
            peer_store: &mut PeerStore,
        ) -> AccessResolution {
            // use stores as allow-lists and allow incoming request if
            // we have either their PeerId or Multiaddr registered
            if peer_store.contains(&request.peer) {
                AccessResolution::Accepted
            } else {
                match &request.addr {
                    Some(addr) if addr_store.contains(addr) => AccessResolution::Accepted,
                    _ => AccessResolution::Denied,
                }
            }
        }
    }

    #[tokio::test]
    async fn allow_peer() {
        let auth_keypair = Keypair::generate_ed25519();
        let auth = Box::new(Auth).spawn(auth_keypair).await;

        let our_keypair = Keypair::generate_ed25519();
        let our_peer_id = PeerId::from(our_keypair.public());

        auth.store_peer(our_peer_id)
            .await
            .expect("Failed to store peer in address");

        let res = auth
            .resolve(our_peer_id, None, AccessDescription::Task)
            .await;

        assert!(matches!(res, Ok(AccessResolution::Accepted)));
    }

    #[tokio::test]
    async fn deny_peer() {
        let auth_keypair = Keypair::generate_ed25519();
        let auth = Box::new(Auth).spawn(auth_keypair).await;

        let our_keypair = Keypair::generate_ed25519();
        let our_peer_id = PeerId::from(our_keypair.public());

        let res = auth
            .resolve(our_peer_id, None, AccessDescription::Task)
            .await;

        assert!(matches!(res, Ok(AccessResolution::Denied)));
    }

    #[tokio::test]
    async fn allow_addr() {
        let auth_keypair = Keypair::generate_ed25519();
        let auth = Box::new(Auth).spawn(auth_keypair).await;

        let our_keypair = Keypair::generate_ed25519();
        let our_peer_id = PeerId::from(our_keypair.public());

        let addr = "/ip4/127.0.0.1/tcp/3050"
            .parse::<Multiaddr>()
            .expect("failed to parse multiaddr....?");

        auth.store_addr(addr.clone())
            .await
            .expect("failed to store address");

        let res = auth
            .resolve(our_peer_id, Some(addr), AccessDescription::Task)
            .await;

        assert!(matches!(res, Ok(AccessResolution::Accepted)));
    }

    #[tokio::test]
    async fn deny_addr() {
        let auth_keypair = Keypair::generate_ed25519();
        let auth = Box::new(Auth).spawn(auth_keypair).await;

        let our_keypair = Keypair::generate_ed25519();
        let our_peer_id = PeerId::from(our_keypair.public());

        let addr = "/ip4/127.0.0.1/tcp/3050"
            .parse::<Multiaddr>()
            .expect("failed to parse multiaddr....?");

        let res = auth
            .resolve(our_peer_id, Some(addr), AccessDescription::Task)
            .await;

        assert!(matches!(res, Ok(AccessResolution::Denied)));
    }
}
