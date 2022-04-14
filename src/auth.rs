use std::{collections::HashMap, net::IpAddr};

use thiserror::Error;
use tokio::sync::{mpsc, oneshot};

use crate::identity::PublicIdentity;

pub type IdentityStore = HashMap<String, PublicIdentity>;
pub type AddressStore = Vec<IpAddr>;

trait AuthActor {
    /// Default implementation. Not really meant to be overriden.
    fn spawn() -> AuthHandle {
        let (tx, mut rx) = mpsc::channel::<AuthCommand>(1024);
        tokio::spawn(async move {
            let mut identity_store: HashMap<String, PublicIdentity> = HashMap::new();
            let mut address_store: Vec<IpAddr> = Vec::new();

            while let Some(request) = rx.recv().await {
                match request {
                    AuthCommand::PutAddress(a) => {
                        address_store.push(a);
                    }
                    AuthCommand::GetIdentity { hash, sender } => match identity_store.get(&hash) {
                        Some(id) => {
                            let _ = sender.send(Some(id.to_owned()));
                        }
                        None => {
                            let _ = sender.send(None);
                        }
                    },
                    AuthCommand::PutIdentity(i) => {
                        identity_store.insert(i.hash(), i);
                    }
                    AuthCommand::Resolve { request, sender } => {
                        let res = Self::handle(request, &identity_store, &address_store);
                        let _ = sender.send(res);
                    }
                    AuthCommand::Stop => break,
                }
            }
        });

        AuthHandle { sender: tx }
    }

    fn handle(
        request: AccessRequest,
        id_store: &IdentityStore,
        address_store: &AddressStore,
    ) -> AccessResolution;
}

#[derive(Clone)]
pub struct AuthHandle {
    sender: mpsc::Sender<AuthCommand>,
}

impl AuthHandle {
    pub async fn store_address(&self, addr: IpAddr) -> Result<(), AuthError> {
        Ok(self.sender.send(AuthCommand::PutAddress(addr)).await?)
    }

    pub async fn store_identity(&self, identity: PublicIdentity) -> Result<(), AuthError> {
        Ok(self.sender.send(AuthCommand::PutIdentity(identity)).await?)
    }

    pub async fn fetch_identity(&self, hash: String) -> Result<PublicIdentity, AuthError> {
        let (sender, receiver) = oneshot::channel();

        self.sender
            .send(AuthCommand::GetIdentity { hash, sender })
            .await?;

        match receiver.await? {
            Some(id) => Ok(id),
            None => Err(AuthError::NotFound),
        }
    }

    pub async fn resolve(&self, request: AccessRequest) -> Result<AccessResolution, AuthError> {
        let (sender, receiver) = oneshot::channel();

        self.sender
            .send(AuthCommand::Resolve { request, sender })
            .await?;

        Ok(receiver.await?)
    }

    pub async fn stop(&self) -> Result<(), AuthError> {
        Ok(self.sender.send(AuthCommand::Stop).await?)
    }
}

#[derive(Debug)]
enum AuthCommand {
    PutAddress(IpAddr),
    GetIdentity {
        hash: String,
        sender: oneshot::Sender<Option<PublicIdentity>>,
    },
    PutIdentity(PublicIdentity),
    Resolve {
        request: AccessRequest,
        sender: oneshot::Sender<AccessResolution>,
    },
    Stop,
}

#[derive(Debug)]
pub struct AccessRequest {
    address: IpAddr,
    identity: PublicIdentity,
}

impl AccessRequest {
    pub fn new(address: IpAddr, identity: PublicIdentity) -> Self {
        Self { address, identity }
    }
}

#[derive(Debug)]
pub enum AccessResolution {
    Accepted,
    Denied,
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("failed to send request to actor")]
    Send,

    #[error("failed to receive response from actor")]
    Recv(#[from] oneshot::error::RecvError),

    #[error("could not found the requested identity in the store")]
    NotFound,
}

impl From<mpsc::error::SendError<AuthCommand>> for AuthError {
    fn from(_: mpsc::error::SendError<AuthCommand>) -> Self {
        AuthError::Send
    }
}
