//!
//! Code implementing messaging and myriam's wire format.
//!
//! Our current wire format is as follows:
//!
//! | 1     | 2      | 3 | 4                     ... |
//!
//! 1. public key bytes
//! 2. nonce bytes
//! 3. size of the following message as a network order u32
//! 4. ciphetext of the serialized message
//!
//! We receive the message in chunks. After receiving (1), we send that along with the
//! origin IP of the message to the auth actor. Only if we get the OK from that, we keep
//! receiving the rest of the message.
//!
//! BIG TODO here: matching (1) against our internal store does NOT actually
//! verify the identity of the actor sending the message, since this key is public.
//! When receiving a public key that is in our store but whose corresponding secret key was
//! not used to encrypt (4), decryption will fail, of course, but we might still read hundreds
//! of thousands of bytes before finding that out. Send enough fake messages and your host gets DoS'ed.
//!
//! Long story short, we need to find a way to add a signature somewhere before (3) to ensure
//! the key actually comes from where it says it comes from. Limiting how big (3) can be helps,
//! but it is a copout for this particular problem.
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
    crypto::{self, DecryptionError, KEY_BYTES, NONCE_BYTES},
    identity::{PublicIdentity, SelfIdentity},
};

const MAX_MSG_SIZE_VAR_NAME: &str = "MYRIAM_MAX_MSG_SIZE";

/// max size of an incoming message cipher in bytes
const DEFAULT_MAX_MESSAGE_SIZE: u32 = 8_388_608;

pub async fn try_read_message<Message>(
    mut rd: ReadHalf<'_>,
    auth_handle: &AuthHandle,
    addr: IpAddr,
) -> Result<(Message, Arc<PublicIdentity>), ReadError>
where
    Message: DeserializeOwned,
{
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

    // ignore the key given to us and fetch it again from our own store
    let public_identity = match auth_handle.fetch_identity(alleged_identity.hash()).await {
        Ok(id) => id,
        Err(e) => {
            tracing::warn!("Could not fetch identity from store -- {e}. Dropping.");
            return Err(ReadError::Auth(e));
        }
    };

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

    let self_identity = auth_handle.fetch_self_identity().await?;

    let message_bytes = crypto::try_decrypt(
        cipher_buffer,
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
    id: &PublicIdentity,
    mut wr: WriteHalf<'_>,
    self_identity: Arc<SelfIdentity>,
) -> Result<(), WriteError>
where
    Message: Serialize,
{
    let message_bytes = bincode::serialize(&message)?;
    let (cipher, nonce) = crypto::try_encrypt(&message_bytes, id, self_identity.as_ref())?;
    let message_size = (cipher.len() as u32).to_be_bytes();

    tracing::debug!(
        "Sending message to {} with size {}.",
        id.hash(),
        cipher.len()
    );

    wr.write_all(self_identity.public_as_bytes()).await?;
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
