use std::fmt::Display;

use rand::Rng;

use super::netlayer::NetLayer;

#[derive(Debug, Clone)]
pub struct ActorAddress {
    proto_id: String,
    peer_id: String,
    host: String,
}

impl ActorAddress {
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

        let peer_id = hex::encode(bytes);

        Ok(Self {
            proto_id: proto_id.to_owned(),
            host: host.to_owned(),
            peer_id,
        })
    }

    pub fn try_parse(value: String) -> Result<Self, Error> {
        let peer_sep = value.find(':').ok_or(Error::Malformed)?;
        let host_sep = value.find('@').ok_or(Error::Malformed)?;

        if peer_sep == 0 || host_sep == 0 || peer_sep >= host_sep {
            return Err(Error::Malformed);
        }

        Ok(Self {
            proto_id: value[0..peer_sep].to_owned(),
            peer_id: value[peer_sep + 1..host_sep].to_owned(),
            host: value[host_sep + 1..value.len()].to_owned(),
        })
    }

    pub fn proto_id(&self) -> &str {
        &self.proto_id
    }

    pub fn peer_id(&self) -> &str {
        &self.peer_id
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn to_string(&self) -> String {
        format!("{}:{}@{}", self.proto_id, self.peer_id, self.host)
    }
}

impl TryFrom<String> for ActorAddress {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_parse(value)
    }
}

impl From<ActorAddress> for String {
    fn from(value: ActorAddress) -> Self {
        value.to_string()
    }
}

impl Display for ActorAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("malformed actor address")]
    Malformed,

    #[error("failed to generate peer ID")]
    Id,
}

#[cfg(test)]
mod tests {
    use crate::actors::remote::netlayer::tcp_layer::TcpNetLayer;

    use super::ActorAddress;

    #[test]
    fn can_generate() {
        let addr = ActorAddress::new::<TcpNetLayer>("127.0.0.1").unwrap();

        assert_eq!("tcp", addr.proto_id());
        assert_eq!("127.0.0.1", addr.host());
        assert_eq!(64, addr.peer_id().len());
        assert_eq!(32, hex::decode(addr.peer_id()).unwrap().len());
    }

    #[test]
    fn can_parse() {
        let addr_str = "tcp:c0ffee@example.com";

        let addr = ActorAddress::try_parse(addr_str.into()).unwrap();

        assert_eq!("tcp", addr.proto_id());
        assert_eq!("c0ffee", addr.peer_id());
        assert_eq!("example.com", addr.host());
    }

    #[test]
    fn can_parse_host_and_port() {
        let addr_str = "tcp:c0ffee@example.com:8037";

        let addr = ActorAddress::try_parse(addr_str.into()).unwrap();

        assert_eq!("tcp", addr.proto_id());
        assert_eq!("c0ffee", addr.peer_id());
        assert_eq!("example.com:8037", addr.host());
    }

    #[test]
    fn empty_address_fails() {
        ActorAddress::try_parse(":@".into()).unwrap_err();
    }

    #[test]
    fn malformed_address_fails() {
        ActorAddress::try_parse("jkfd@fdk:asdj".into()).unwrap_err();
    }
}
