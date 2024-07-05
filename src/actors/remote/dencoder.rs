//!
//! if you think "Dencoder" is a dumb name for
//! an encoder that is also a decoder
//!
//! yes. yes it is.
//!

use serde::{de::DeserializeOwned, Serialize};

pub mod bincode;

pub trait Dencoder {
    fn encode<T: Serialize>(value: T) -> Result<Vec<u8>, Error>;
    fn decode<U: DeserializeOwned>(value: Vec<u8>) -> Result<U, Error>;
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to encode message")]
    Encode,

    #[error("failed to decode message")]
    Decode,
}
