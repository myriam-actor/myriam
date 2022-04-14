//!
//! Utilities for creating and managing identities
//!

use crypto_box::{PublicKey, SecretKey};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::{
    fs::File,
    io::{self, AsyncReadExt, AsyncWriteExt},
};

use crate::crypto::KEY_BYTES;

///
/// Encapsulates the identity (keys and hash) of an actor, or group of actors
///
#[derive(Clone)]
pub struct SelfIdentity {
    public_identity: PublicIdentity,
    secret_key: SecretKey,
}

impl SelfIdentity {
    ///
    /// Create a SelfIdentity from scratch, generating its public and secret keys.
    ///
    pub fn new() -> Self {
        let mut rng = crypto_box::rand_core::OsRng;
        let secret_key = SecretKey::generate(&mut rng);
        let public_key = secret_key.public_key();

        Self {
            public_identity: PublicIdentity::from(public_key),
            secret_key,
        }
    }

    ///
    /// attempt to read a secret key from a _keyfile_, which is a file whose size in bytes is exactly crate::crypto::KEY_BYTES (32)
    ///
    pub async fn read_from_file(filename: String) -> Result<Self, IdentityError> {
        let key_bytes = read_key_bytes(filename).await?;

        Ok(Self::from(SecretKey::from(key_bytes)))
    }

    ///
    /// get this identity's public key as bytes
    ///
    pub fn public_as_bytes(&self) -> &[u8] {
        self.public_identity.public_key.as_bytes()
    }

    ///
    /// get this identity's secret key as bytes
    ///
    pub fn secret_as_bytes(&self) -> &[u8] {
        self.secret_key.as_bytes()
    }

    ///
    /// get the hash of this identity's public key
    ///
    pub fn hash(&self) -> String {
        self.public_identity.hash.clone()
    }

    pub fn public_identity(&self) -> &PublicIdentity {
        &self.public_identity
    }

    pub fn secret(&self) -> &SecretKey {
        &self.secret_key
    }

    ///
    /// dump this identity as a _keyfile_ -- see [Self::read_from_file]
    ///
    pub async fn dump_keyfile(&self, filename: String) -> Result<(), IdentityError> {
        let mut f = File::create(filename).await?;

        if let Err(e) = f.write_all(self.secret_key.as_bytes()).await {
            return Err(e.into());
        }

        Ok(())
    }
}

impl From<SecretKey> for SelfIdentity {
    fn from(secret_key: SecretKey) -> Self {
        let public_key = secret_key.public_key();

        Self {
            public_identity: PublicIdentity::from(public_key),
            secret_key,
        }
    }
}

impl From<[u8; KEY_BYTES]> for SelfIdentity {
    fn from(key_bytes: [u8; KEY_BYTES]) -> Self {
        Self::from(SecretKey::from(key_bytes))
    }
}

impl Default for SelfIdentity {
    fn default() -> Self {
        Self::new()
    }
}

///
/// Encapsulates a public identity (public key + hash), which you can share as you please.
///
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicIdentity {
    hash: String,
    public_key: PublicKey,
}

impl PublicIdentity {
    pub fn key_as_bytes(&self) -> &[u8] {
        self.public_key.as_bytes()
    }

    ///
    /// get this identity public key's hash
    ///
    pub fn hash(&self) -> String {
        self.hash.clone()
    }

    pub fn key(&self) -> &PublicKey {
        &self.public_key
    }

    ///
    /// attempt to read a public key from a _keyfile_, which is a file whose size in bytes is exactly crate::crypto::KEY_BYTES (32)
    ///
    pub async fn read_from_file(filename: String) -> Result<Self, IdentityError> {
        let key_bytes = read_key_bytes(filename).await?;

        Ok(Self::from(PublicKey::from(key_bytes)))
    }

    ///
    /// dump this identity's public key as a _keyfile_ -- see [Self::read_from_file]
    ///
    pub async fn dump_keyfile(&self, filename: String) -> Result<(), IdentityError> {
        let mut f = File::create(filename).await?;

        if let Err(e) = f.write_all(self.public_key.as_bytes()).await {
            return Err(e.into());
        }

        Ok(())
    }
}

impl From<PublicKey> for PublicIdentity {
    fn from(public_key: PublicKey) -> Self {
        let hash = sha256::digest_bytes(public_key.as_bytes());

        Self { hash, public_key }
    }
}

impl From<SelfIdentity> for PublicIdentity {
    fn from(s: SelfIdentity) -> Self {
        s.public_identity
    }
}

impl From<[u8; KEY_BYTES]> for PublicIdentity {
    fn from(secret_key_bytes: [u8; KEY_BYTES]) -> Self {
        Self::from(PublicKey::from(secret_key_bytes))
    }
}

async fn read_key_bytes(filename: String) -> Result<[u8; KEY_BYTES], IdentityError> {
    let mut f = File::open(filename).await?;
    let mut buffer = [0u8; KEY_BYTES];

    if f.read_exact(&mut buffer).await.is_err() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid keyfile").into());
    }

    if let Ok(0) = f.read(&mut buffer).await {
        Ok(buffer)
    } else {
        Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid keyfile").into())
    }
}

#[derive(Debug, Error)]
pub enum IdentityError {
    #[error("error creating identity from file: {0}")]
    Io(#[from] io::Error),
}
