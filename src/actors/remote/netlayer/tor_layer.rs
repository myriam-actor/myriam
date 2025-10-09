//!
//! Tor net layer
//!
//! Requires properly configured Tor router with a hidden service per router in your application
//!

use std::fmt::Display;

///
/// Tor net layer
///
#[derive(Debug)]
pub struct TorNetLayer;

impl TorNetLayer {
    pub fn new() -> Result<Self, Error> {
        todo!()
    }
}

///
/// errors when binding, accepting and connecting via a Tor net layer
///
#[allow(missing_docs)]
#[derive(Debug)]
pub enum Error {
    Init(String),
    Recv(String),
    Connect(String),
    Hostname(String),
    NotReady,
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Init(ctx) => write!(f, "failed to init layer: {ctx}"),
            Error::Recv(ctx) => write!(f, "failed to receive data: {ctx}"),
            Error::Connect(ctx) => write!(f, "failed to connect to endpoint: {ctx}"),
            Error::Hostname(ctx) => write!(f, "failed to recover our hostname: {ctx}"),
            Error::NotReady => write!(f, "layer not ready"),
        }
    }
}

impl std::error::Error for Error {}
