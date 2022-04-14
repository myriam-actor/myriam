use std::net::IpAddr;

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
    messaging::{Message, MessageResult},
};

pub async fn try_read_message<T>(
    mut rd: ReadHalf<'_>,
    auth_handle: &AuthHandle,
    addr: IpAddr,
    self_identity: &SelfIdentity,
) -> Result<(Message<T>, PublicIdentity), ReadError>
where
    T: DeserializeOwned,
{
    let mut key_buffer: [u8; KEY_BYTES] = [0; KEY_BYTES];
    rd.read_exact(&mut key_buffer).await?;

    let alleged_identity = PublicIdentity::from(key_buffer);
    match auth_handle
        .resolve(AccessRequest::new(addr, alleged_identity.clone()))
        .await?
    {
        AccessResolution::Accepted => (),
        AccessResolution::Denied => return Err(ReadError::Unauthorized),
    }

    let public_identity = auth_handle.fetch_identity(alleged_identity.hash()).await?;

    let mut nonce_buffer: [u8; NONCE_BYTES] = [0; NONCE_BYTES];
    rd.read_exact(&mut nonce_buffer).await?;

    let mut size_buffer: [u8; 4] = [0; 4];
    rd.read_exact(&mut size_buffer).await?;

    // TODO: add option to cap size and check against that
    let size = u32::from_be_bytes(size_buffer);
    let mut cipher_buffer: Vec<u8> = vec![0; size as usize];
    rd.read_exact(cipher_buffer.as_mut_slice()).await?;

    let message_bytes = crypto::try_decrypt(
        cipher_buffer,
        &nonce_buffer,
        &public_identity,
        self_identity,
    )?;

    let message = bincode::deserialize(&message_bytes[..])?;

    Ok((message, public_identity))
}

pub async fn write_response<U, E>(
    response: MessageResult<U, E>,
    id: &PublicIdentity,
    mut wr: WriteHalf<'_>,
    self_identity: &SelfIdentity,
) -> Result<(), WriteError>
where
    U: Serialize,
    E: Serialize,
{
    let message_bytes = bincode::serialize(&response)?;
    let (cipher, nonce) = crypto::try_encrypt(&message_bytes, id, self_identity)?;
    let message_size = (cipher.len() as u32).to_be_bytes();

    wr.write_all(self_identity.public_as_bytes()).await?;
    wr.write_all(&nonce).await?;
    wr.write_all(&message_size).await?;
    wr.write_all(&message_bytes).await?;

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

    #[error("public identity missing from store")]
    Missing,
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
