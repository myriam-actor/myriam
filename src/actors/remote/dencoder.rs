//!
//! Decoder/Encoder trait and implementations
//!
//! if you think "Dencoder" is a dumb name for
//! an encoder that is also a decoder
//!
//! yes. yes it is.
//!

use std::fmt::Display;

use serde::{de::DeserializeOwned, Serialize};

pub mod bincode;

pub mod bitcode;

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
#[derive(Debug)]
#[allow(missing_docs)]
pub enum Error {
    Encode(String),
    Decode(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Encode(ctx) => write!(f, "failed to encode message: {ctx}"),
            Error::Decode(ctx) => write!(f, "failed to decode message: {ctx}"),
        }
    }
}

impl std::error::Error for Error {}
