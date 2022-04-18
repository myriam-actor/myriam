//!
//! Code implementing messaging and myriam's wire format.
//!
//! Our current wire format is as follows:
//!
//! `| 1       | 2    | 3    | 4    | 5 | 6                           ... |`
//!
//! 1. public key bytes
//! 2. timestamp nonce
//! 3. timestamp cipher
//! 4. message ciphertext nonce
//! 5. size of the message ciphertext as a network order u32
//! 6. ciphetext of the serialized message
//!
//! We receive the message in chunks. After reading (1), we send that along with the
//! origin IP of the message to the auth actor. Only if we get the OK from that, we keep
//! reading the rest of the message.
//!

use std::{net::IpAddr, sync::Arc};

use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::tcp::{ReadHalf, WriteHalf},
};

use crate::{
    auth::{AccessRequest, AccessResolution, AuthError, AuthHandle},
    crypto::{self, DecryptionError, KEY_BYTES, NONCE_BYTES, TIMESTAMP_CIPHER_BYTES},
    identity::{PublicIdentity, SelfIdentity},
};

const MAX_MSG_SIZE_VAR_NAME: &str = "MYRIAM_MAX_MSG_SIZE";
const DELTA_TOLERANCE_VAR_NAME: &str = "MYRIAM_MESSAGE_AGE_TOLERANCE";

/// max size of an incoming message cipher in bytes
const DEFAULT_MAX_MESSAGE_SIZE: u32 = 8_388_608;

const DEFAULT_DELTA_TOLERANCE: u64 = 10_000;

pub async fn try_read_message<Message>(
    mut rd: ReadHalf<'_>,
    auth_handle: &AuthHandle,
    addr: IpAddr,
) -> Result<(Message, Arc<PublicIdentity>), ReadError>
where
    Message: DeserializeOwned,
{
    //
    // start by trying to read a public key, and verify we have a copy if it
    //
    let mut key_buffer: [u8; KEY_BYTES] = [0; KEY_BYTES];
    rd.read_exact(&mut key_buffer).await?;

    let alleged_identity = PublicIdentity::from(key_buffer);
    tracing::debug!(
        "Got incoming request with hash ID {}.",
        alleged_identity.hash()
    );

    match auth_handle
        .resolve(AccessRequest::new(addr, alleged_identity.clone()))
        .await?
    {
        AccessResolution::Accepted => {
            tracing::debug!("Request with ID hash {} accepted.", alleged_identity.hash())
        }
        AccessResolution::Denied => return Err(ReadError::Unauthorized),
    }

    //
    // ignore the key given to us and fetch it again from our own store
    //
    let public_identity = match auth_handle.fetch_identity(alleged_identity.hash()).await {
        Ok(id) => id,
        Err(e) => {
            tracing::warn!("Could not fetch identity from store -- {e}. Dropping.");
            return Err(ReadError::Auth(e));
        }
    };

    //
    // try to decrypt and read a timestamp, and verify it is recent enough
    //
    let mut timestamp_nonce_buffer: [u8; NONCE_BYTES] = [0; NONCE_BYTES];
    rd.read_exact(&mut timestamp_nonce_buffer).await?;

    let mut timestamp_buffer: [u8; TIMESTAMP_CIPHER_BYTES] = [0; TIMESTAMP_CIPHER_BYTES];
    rd.read_exact(&mut timestamp_buffer).await?;

    let self_identity = auth_handle.fetch_self_identity().await?;

    let timestamp: i64 = match crypto::try_decrypt(
        &timestamp_buffer,
        &timestamp_nonce_buffer,
        &public_identity,
        &self_identity,
    ) {
        Ok(vec) => match vec.as_slice().try_into() {
            Ok(bytes) => i64::from_be_bytes(bytes),
            Err(_) => return Err(ReadError::InvalidMessage),
        },
        Err(_) => return Err(ReadError::Unauthorized),
    };

    let tolerance: u64 = match std::env::var(DELTA_TOLERANCE_VAR_NAME) {
        Ok(s) => match s.parse::<u64>() {
            Ok(s) => s,
            Err(_) => DEFAULT_DELTA_TOLERANCE,
        },
        Err(_) => DEFAULT_DELTA_TOLERANCE,
    };

    let now = chrono::Utc::now().timestamp_millis();
    if now.abs_diff(timestamp) > tolerance {
        return Err(ReadError::Unauthorized);
    }

    //
    // now read the nonce, the size of the message cipher, and the message cipher itself
    // so we can try to decrypt it
    //
    let mut nonce_buffer: [u8; NONCE_BYTES] = [0; NONCE_BYTES];
    rd.read_exact(&mut nonce_buffer).await?;

    let mut size_buffer: [u8; 4] = [0; 4];
    rd.read_exact(&mut size_buffer).await?;

    let max_size: u32 = match std::env::var(MAX_MSG_SIZE_VAR_NAME) {
        Ok(s) => match s.parse::<u32>() {
            Ok(s) => s,
            Err(_) => DEFAULT_MAX_MESSAGE_SIZE,
        },
        Err(_) => DEFAULT_MAX_MESSAGE_SIZE,
    };

    let size = u32::from_be_bytes(size_buffer);
    if size > max_size {
        tracing::warn!(
            "Incoming message with ID hash {} exceeded allowed cipher size. Dropping.",
            alleged_identity.hash(),
        );
        return Err(ReadError::InvalidMessage);
    }

    tracing::debug!(
        "Reading message cipher from {} with {size} bytes.",
        alleged_identity.hash()
    );

    let mut cipher_buffer: Vec<u8> = vec![0; size as usize];
    rd.read_exact(cipher_buffer.as_mut_slice()).await?;

    tracing::debug!(
        "Done reading message cipher from {}. Attempting to decrypt.",
        alleged_identity.hash()
    );

    let message_bytes = crypto::try_decrypt(
        cipher_buffer.as_slice(),
        &nonce_buffer,
        &public_identity,
        self_identity.as_ref(),
    )?;

    let message = bincode::deserialize(&message_bytes[..])?;

    tracing::debug!(
        "Message for {} decryped and decoded successfully.",
        alleged_identity.hash()
    );

    Ok((message, public_identity))
}

pub async fn try_write_message<Message>(
    message: Message,
    mut wr: WriteHalf<'_>,
    public_identity: &PublicIdentity,
    self_identity: Arc<SelfIdentity>,
) -> Result<(), WriteError>
where
    Message: Serialize,
{
    let message_bytes = bincode::serialize(&message)?;
    let (cipher, nonce) =
        crypto::try_encrypt(&message_bytes, public_identity, self_identity.as_ref())?;
    let message_size = (cipher.len() as u32).to_be_bytes();

    let timestamp = chrono::Utc::now().timestamp_millis();

    let (timestamp_cipher, timestamp_nonce) = crypto::try_encrypt(
        &timestamp.to_be_bytes(),
        public_identity,
        self_identity.as_ref(),
    )?;

    tracing::debug!(
        "Sending message to {} with size {}.",
        public_identity.hash(),
        cipher.len()
    );

    wr.write_all(self_identity.public_as_bytes()).await?;
    wr.write_all(&timestamp_nonce).await?;
    wr.write_all(&timestamp_cipher).await?;
    wr.write_all(&nonce).await?;
    wr.write_all(&message_size).await?;
    wr.write_all(&cipher).await?;

    Ok(())
}

#[derive(Debug, Error)]
pub enum ReadError {
    #[error("failed to read response: {0}")]
    Recv(#[from] io::Error),

    #[error("received message from unauthorized identity")]
    Unauthorized,

    #[error("failed to get a response from authorization actor")]
    Auth(#[from] AuthError),

    #[error("failed to decrypt incoming message")]
    Decryption(#[from] DecryptionError),

    #[error("failed to decode incoming message")]
    Decoding(#[from] std::boxed::Box<bincode::ErrorKind>),

    #[error("incoming message is invalid")]
    InvalidMessage,
}

#[derive(Debug, Error)]
pub enum WriteError {
    #[error("failed to send response: {0}")]
    Send(#[from] io::Error),

    #[error("failed to encode message")]
    Encode(#[from] std::boxed::Box<bincode::ErrorKind>),

    #[error("failed to encrypt message")]
    Encrypt(#[from] crypto::EncryptionError),
}
