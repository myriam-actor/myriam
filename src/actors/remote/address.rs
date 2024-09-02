//!
//! remote address struct and utils
//!

use std::{fmt::Display, str::FromStr};

use rand::{Rng, RngCore};
use serde::{Deserialize, Serialize};

use super::netlayer::NetLayer;

///
/// revocable address to a remote actor
///
/// addresses have the format `<protocol>:<peer id>@<host>`
///
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorAddress {
    proto_id: String,
    peer_id: PeerId,
    host: String,
}

impl ActorAddress {
    ///
    /// create a new address from this host and `NetLayer` parameter
    ///
    pub fn new<N>(host: &str) -> Result<Self, Error>
    where
        N: NetLayer,
    {
        let proto_id = N::name();
        let mut bytes = [0u8; 32];

        let mut rng = rand::thread_rng();
        rng.try_fill(&mut bytes).map_err(|err| {
            tracing::error!("could not fill ID buffer - {err}");
            Error::Id
        })?;

        Ok(Self {
            proto_id: proto_id.to_owned(),
            host: host.to_owned(),
            peer_id: PeerId::new()?,
        })
    }

    ///
    /// create a new address using this host, [`PeerID`] and [`NetLayer`] param
    ///
    pub fn new_with_peer_id<N>(host: &str, peer_id: PeerId) -> Self
    where
        N: NetLayer,
    {
        let proto_id = N::name();

        Self {
            proto_id: proto_id.to_owned(),
            host: host.to_owned(),
            peer_id,
        }
    }

    ///
    /// try to parse an address from the given string
    ///
    pub fn try_parse(value: &str) -> Result<Self, Error> {
        let peer_sep = value.find(':').ok_or(Error::Malformed)?;
        let host_sep = value.find('@').ok_or(Error::Malformed)?;

        if peer_sep == 0 || host_sep == 0 || peer_sep >= host_sep {
            return Err(Error::Malformed);
        }

        let peer_id = PeerId::try_parse(&value[peer_sep + 1..host_sep])?;

        Ok(Self {
            proto_id: value[0..peer_sep].to_owned(),
            host: value[host_sep + 1..value.len()].to_owned(),
            peer_id,
        })
    }

    ///
    /// this actor's protocol ID
    ///
    pub fn proto_id(&self) -> &str {
        &self.proto_id
    }

    ///
    /// this actor's peer ID in the router
    ///
    pub fn peer_id(&self) -> &PeerId {
        &self.peer_id
    }

    ///
    /// this actor's host address, or its protocol-related equivalent
    ///
    pub fn host(&self) -> &str {
        &self.host
    }
}

impl FromStr for ActorAddress {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_parse(s)
    }
}

impl TryFrom<String> for ActorAddress {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_parse(&value)
    }
}

impl From<ActorAddress> for String {
    fn from(value: ActorAddress) -> Self {
        value.to_string()
    }
}

impl Display for ActorAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}@{}", self.proto_id, self.peer_id, self.host)
    }
}

///
/// wrapper over a bag of bytes, acting as a unique identifier
/// inside a [`Router`]
///
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct PeerId(Vec<u8>);

impl PeerId {
    /// generate a new random PeerId
    pub fn new() -> Result<Self, Error> {
        let mut rng = rand::thread_rng();

        let mut buffer = [0u8; 32];
        rng.fill_bytes(&mut buffer);

        Ok(Self(buffer.to_vec()))
    }

    /// generate a new PeerId using `bytes`
    pub fn new_from_bytes(bytes: &[u8]) -> Self {
        Self(bytes.to_vec())
    }

    ///
    /// try to parse a string as a PeerId
    ///
    /// fails if `value` is not an hex-encoded string
    ///
    pub fn try_parse(value: &str) -> Result<Self, Error> {
        Ok(Self::new_from_bytes(
            &hex::decode(value).map_err(|_| Error::Id)?,
        ))
    }

    /// returns a reference to the bytes making up this PeerId
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }

    /// returns the length of the byte buffer making up this PeerId
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// returns true if the PeerId's byte buffer is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Display for PeerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(&self.0))
    }
}

///
/// Errors when creating a new address
///
#[allow(missing_docs)]
#[derive(Debug)]
pub enum Error {
    Malformed,
    Id,
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Malformed => write!(f, "malformed actor address"),
            Error::Id => write!(f, "failed to generate peer ID"),
        }
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use crate::actors::remote::netlayer::tcp_layer::TcpNetLayer;

    use super::ActorAddress;

    #[test]
    fn can_generate() {
        let addr = ActorAddress::new::<TcpNetLayer>("127.0.0.1").unwrap();

        assert_eq!("tcp", addr.proto_id());
        assert_eq!("127.0.0.1", addr.host());
        assert_eq!(32, addr.peer_id().len());
        assert_eq!(64, addr.peer_id().to_string().len());
    }

    #[test]
    fn can_parse() {
        let addr_str = "tcp:c0ffee@example.com";

        let addr = ActorAddress::try_parse(addr_str).unwrap();

        assert_eq!("tcp", addr.proto_id());
        assert_eq!("c0ffee", addr.peer_id().to_string());
        assert_eq!("example.com", addr.host());
    }

    #[test]
    fn can_parse_host_and_port() {
        let addr_str = "tcp:c0ffee@example.com:8037";

        let addr = ActorAddress::try_parse(addr_str).unwrap();

        assert_eq!("tcp", addr.proto_id());
        assert_eq!("c0ffee", addr.peer_id().to_string());
        assert_eq!("example.com:8037", addr.host());
    }

    #[test]
    fn empty_address_fails() {
        ActorAddress::try_parse(":@").unwrap_err();
    }

    #[test]
    fn malformed_address_fails() {
        ActorAddress::try_parse("jkfd@fdk:asdj").unwrap_err();
    }
}
