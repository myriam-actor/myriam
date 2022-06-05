//!
//! Utilities for creating and manipulating addresses
//!

use std::{
    fmt::Display,
    io,
    net::{IpAddr, TcpListener},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

///
/// Encapsulates a (possibly) valid network address.
/// We really only check that the port is available when creating an address for binding.
///
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Address {
    pub host: String,
    pub port: u16,
}

impl Address {
    ///
    /// Create a new address. Do not check `port` since we could be creating the address for a remote actor.
    ///
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
        }
    }

    ///
    /// Create an address with a random, available port in the current machine.
    ///
    pub fn new_with_random_port(host: &str) -> Result<Self, AddressError> {
        Ok(Self {
            host: host.into(),
            port: random_port()?,
        })
    }

    ///
    /// Create an address, making sure the given port is available in the current machine.
    ///
    pub fn new_with_checked_port(host: &str, port: u16) -> Result<Self, AddressError> {
        Ok(Self {
            host: host.into(),
            port: check_port(port)?,
        })
    }

    ///
    /// Return a new address with a different host, useful for things like publishing to some sort of service announcement.
    ///
    pub fn with_host(&self, new_host: &str) -> Self {
        Self {
            host: new_host.into(),
            port: self.port,
        }
    }

    ///
    /// Return this address' host
    ///
    pub fn host(&self) -> String {
        self.host.clone()
    }
}

impl Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: this might cause trouble with IPv6 addresses
        write!(f, "{}:{}", self.host, self.port)
    }
}

impl TryFrom<Address> for IpAddr {
    type Error = AddressError;

    fn try_from(value: Address) -> Result<Self, Self::Error> {
        Ok(value.host().parse()?)
    }
}

#[derive(Error, Debug)]
pub enum AddressError {
    #[error("failed to adquire free port: {0}")]
    Port(#[from] io::Error),

    #[error("failed to turn addres into an IpAddr")]
    Conversion(#[from] std::net::AddrParseError),
}

fn random_port() -> Result<u16, AddressError> {
    check_port(0)
}

fn check_port(port: u16) -> Result<u16, AddressError> {
    let socket = TcpListener::bind(format!("[::1]:{port}"))?;

    Ok(socket.local_addr()?.port())
}

#[cfg(test)]
mod tests {
    use crate::address::random_port;

    use super::check_port;

    #[test]
    fn can_get_random_port() {
        let port = random_port().unwrap();

        assert!(0 < port && port < 65535);
    }

    #[test]
    fn can_get_specific_port() {
        let port = check_port(3050).unwrap();
        assert_eq!(3050, port);
    }
}
