//!
//! Tor net layer
//!
//! Requires properly configured Tor router with a hidden service per router in your application
//!

use std::{fmt::Display, path::Path};

use tokio::{
    io::BufStream,
    net::{TcpListener, TcpStream},
};

use super::NetLayer;

///
/// Tor net layer
///
#[derive(Debug)]
pub struct TorNetLayer {
    proxy_address: String,
    tordata_dir: String,
    local_address: Option<String>,
    hostname: Option<String>,
    listener: Option<TcpListener>,
}

impl TorNetLayer {
    ///
    /// Create a new Tor layer with the necessary setup for connecting to other actors
    ///
    pub fn new<S>(proxy_address: S, tordata_dir: S) -> Self
    where
        S: Into<String>,
    {
        Self {
            proxy_address: proxy_address.into(),
            tordata_dir: tordata_dir.into(),
            local_address: None,
            hostname: None,
            listener: None,
        }
    }

    ///
    /// creates a new Tor layer with the required setup for
    /// exposing this actor to the network
    ///
    pub async fn new_for_service<S>(
        proxy_address: S,
        local_address: S,
        tordata_dir: S,
    ) -> Result<Self, Error>
    where
        S: Into<String>,
    {
        Self::new(proxy_address, tordata_dir)
            .as_service(local_address)
            .await
    }

    ///
    /// create a new layer from this one, capable of accepting connections
    ///
    pub async fn as_service<S>(mut self, local_address: S) -> Result<Self, Error>
    where
        S: Into<String>,
    {
        let hostname = self.hostname().await?;
        self.hostname.replace(hostname);
        self.local_address.replace(local_address.into());

        Ok(self)
    }

    ///
    /// this layer's proxy address
    ///
    pub fn proxy_address(&self) -> &str {
        &self.proxy_address
    }

    ///
    /// this layer's Tor data directory
    ///
    pub fn tordata_dir(&self) -> &str {
        &self.tordata_dir
    }

    ///
    /// this layer's .onion address
    ///
    pub async fn hostname(&self) -> Result<String, Error> {
        let path = Path::new(&self.tordata_dir)
            .join("hostname")
            .canonicalize()
            .map_err(|e| {
                tracing::error!("tor layer - hostname : {e}");
                Error::Hostname(e.to_string())
            })?;

        tokio::fs::read_to_string(path).await.map_err(|e| {
            tracing::error!("tor layer - hostname: {e}");
            Error::Hostname(e.to_string())
        })
    }
}

impl NetLayer for TorNetLayer {
    type Error = Error;

    fn name() -> &'static str {
        "tor"
    }

    async fn connect(&self, addr: &str) -> Result<impl super::AsyncMsgStream, Self::Error> {
        let proxy = TcpStream::connect(&self.proxy_address)
            .await
            .map_err(|err| {
                tracing::error!("tor socket: proxy connect - {err}");
                Error::Connect(err.to_string())
            })?;

        let mut stream = BufStream::new(proxy);
        socks5_impl::client::connect(&mut stream, (addr, 80), None)
            .await
            .map_err(|err| {
                tracing::error!("tor socket: connect - {err}");
                Error::Connect(err.to_string())
            })?;

        Ok(stream)
    }

    async fn init(&mut self) -> Result<(), Self::Error> {
        self.listener.replace(
            TcpListener::bind(&self.local_address.as_ref().ok_or(Error::NotReady)?)
                .await
                .map_err(|e| {
                    tracing::error!("tor socket - init: {e}");
                    Error::Init(e.to_string())
                })?,
        );

        Ok(())
    }

    async fn accept(&self) -> Result<impl super::AsyncMsgStream, Self::Error> {
        self.listener
            .as_ref()
            .ok_or(Error::NotReady)?
            .accept()
            .await
            .map_err(|e| {
                tracing::error!("tor layer: failed to accept - {e}");
                Error::Recv(e.to_string())
            })
            .map(|s| s.0)
    }

    fn address(&self) -> Result<String, Self::Error> {
        self.hostname
            .to_owned()
            .ok_or(Error::NotReady)
            .map(|s| s.trim().to_owned())
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
