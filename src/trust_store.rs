//!
//! Local actor managing access to trusted public identities.
//!

use std::collections::HashMap;

use thiserror::Error;
use tokio::sync::{mpsc, oneshot};

use crate::identity::PublicIdentity;

///
/// Storage service for trusted public identity. Many instances may be spawned to offer different access policies.
///
pub struct TrustStore;

impl TrustStore {
    ///
    /// Spawn a TrustStore and return a handle to it.
    /// You may pass `None` as `ids` if you intend to store identities later on.
    ///
    pub fn spawn(ids: Option<Vec<PublicIdentity>>) -> TrustStoreHandle {
        let (tx, mut rx) = mpsc::channel::<TrustStoreRequest>(1024);
        tokio::spawn(async move {
            let mut store: HashMap<String, PublicIdentity> = HashMap::new();

            if let Some(ids) = ids {
                for id in ids.iter() {
                    store.insert(id.hash(), id.to_owned());
                }
            }

            while let Some(request) = rx.recv().await {
                match request {
                    TrustStoreRequest::Get { hash, sender } => {
                        match store.get(&hash) {
                            Some(x) => {
                                let _ = sender.send(Some(x.to_owned()));
                            }
                            None => {
                                let _ = sender.send(None);
                            }
                        };
                    }
                    TrustStoreRequest::Query { hash, sender } => {
                        let _ = sender.send(store.contains_key(&hash));
                    }
                    TrustStoreRequest::Put(id) => {
                        store.insert(id.hash(), id);
                    }
                    TrustStoreRequest::Stop => break,
                };
            }
        });

        TrustStoreHandle { sender: tx }
    }
}

///
/// Handle to a TrustStore. Fetching and inserting of identities is made through this struct.
///
#[derive(Clone)]
pub struct TrustStoreHandle {
    sender: mpsc::Sender<TrustStoreRequest>,
}

impl TrustStoreHandle {
    ///
    /// Attempt to fetch an identity with the given hash.
    /// Result will be `Err(TrustStoreError)` if the associated TrustStore has been stopped,
    /// or `Ok(None)` if no such identity exists.
    ///
    pub async fn fetch(&self, hash: String) -> Result<Option<PublicIdentity>, TrustStoreError> {
        let (sender, rx) = oneshot::channel::<Option<PublicIdentity>>();
        self.sender
            .send(TrustStoreRequest::Get { hash, sender })
            .await?;

        Ok(rx.await?)
    }

    pub async fn exists(&self, hash: String) -> Result<bool, TrustStoreError> {
        let (sender, rx) = oneshot::channel::<bool>();
        self.sender
            .send(TrustStoreRequest::Query { hash, sender })
            .await?;

        Ok(rx.await?)
    }

    ///
    /// Insert a new identity on the TrustStore, replacing it if such an identity with the same hash exists.
    ///
    pub async fn store(&self, id: PublicIdentity) -> Result<(), TrustStoreError> {
        Ok(self.sender.send(TrustStoreRequest::Put(id.clone())).await?)
    }

    ///
    /// Stop the associated trust store. Further requests will fail.
    ///
    pub async fn stop(&mut self) -> Result<(), TrustStoreError> {
        Ok(self.sender.send(TrustStoreRequest::Stop).await?)
    }
}

#[derive(Debug)]
pub enum TrustStoreRequest {
    Get {
        hash: String,
        sender: oneshot::Sender<Option<PublicIdentity>>,
    },
    Query {
        hash: String,
        sender: oneshot::Sender<bool>,
    },
    Put(PublicIdentity),
    Stop,
}

#[derive(Debug, Error)]
pub enum TrustStoreError {
    #[error("failed to send TrustStore request: {0}")]
    Send(#[from] mpsc::error::SendError<TrustStoreRequest>),

    #[error("failed to receive response from TrustStore: {0}")]
    Recv(#[from] oneshot::error::RecvError),
}
