//!
//! Decoder/Encoder trait and implementations
//!
//! if you think "Dencoder" is a dumb name for
//! an encoder that is also a decoder
//!
//! yes. yes it is.
//!

use serde::{de::DeserializeOwned, Serialize};

pub mod bincode;

///
/// trait for abstracting message coder/decoder
///
pub trait Dencoder {
    /// try to encode given value to a bag of bytes
    fn encode<T: Serialize>(value: T) -> Result<Vec<u8>, Error>;

    /// try to decode a bag of bytes as a value
    fn decode<U: DeserializeOwned>(value: Vec<u8>) -> Result<U, Error>;
}

///
/// possible errors when de/coding values
///
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum Error {
    #[error("failed to encode message")]
    Encode,

    #[error("failed to decode message")]
    Decode,
}
