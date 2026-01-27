//!
//! TCP actor net layer
//!
//! !WARNING! for testing only! nothing going through these is encrypted!
//!

use std::fmt::Display;

use tokio::net::{TcpListener, TcpStream};

use super::{AsyncMsgStream, NetLayer};

///
/// simple TCP net layer
///
/// Unencrypted! Do not use for anything sensitive!
///
#[derive(Debug)]
pub struct TcpNetLayer {
    listener: Option<TcpListener>,
}

impl TcpNetLayer {
    ///
    /// create a new (not yet listening) TCP net layer
    ///
    pub fn new() -> Self {
        Self {
            listener: Option::None,
        }
    }
}

impl Default for TcpNetLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl NetLayer for TcpNetLayer {
    type Error = TcpError;

    fn name() -> &'static str {
        "tcp"
    }

    async fn connect(&self, addr: &str) -> Result<impl AsyncMsgStream, Self::Error> {
        TcpStream::connect(addr).await.map_err(|e| {
            tracing::error!("connect error {e}");

            TcpError::Connect(e.to_string())
        })
    }

    async fn init(&mut self) -> Result<(), Self::Error> {
        self.listener
            .replace(TcpListener::bind("0.0.0.0:0").await.map_err(|e| {
                tracing::error!("bind error: {e}");

                TcpError::Bind(e.to_string())
            })?);

        Ok(())
    }

    async fn accept(&self) -> Result<impl AsyncMsgStream, Self::Error> {
        Ok(self
            .listener
            .as_ref()
            .ok_or(TcpError::NotReady)?
            .accept()
            .await
            .map_err(|e| {
                tracing::error!("accept error: {e}");

                TcpError::Accept(e.to_string())
            })?
            .0)
    }

    async fn address(&self) -> Result<String, Self::Error> {
        Ok(self
            .listener
            .as_ref()
            .ok_or(TcpError::NotReady)?
            .local_addr()
            .map_err(|_| TcpError::NotReady)?
            .to_string())
    }
}

///
/// Errors when binding, connecting or accepting connections
///
#[allow(missing_docs)]
#[derive(Debug)]
pub enum TcpError {
    NotReady,
    Bind(String),
    Accept(String),
    Connect(String),
}

impl Display for TcpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TcpError::NotReady => write!(f, "net layer not ready"),
            TcpError::Bind(ctx) => write!(f, "failed to bind to address: {ctx}"),
            TcpError::Accept(ctx) => write!(f, "failed to accept connection: {ctx}"),
            TcpError::Connect(ctx) => write!(f, "failed to connect to address: {ctx}"),
        }
    }
}

impl std::error::Error for TcpError {}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::actors::remote::netlayer::{NetLayer, tcp_layer::TcpNetLayer};

    #[tokio::test]
    async fn listen() {
        let mut nl = TcpNetLayer::new();
        nl.init().await.unwrap();
    }

    #[tokio::test]
    async fn accept() {
        let mut nl = TcpNetLayer::new();
        nl.init().await.unwrap();

        let addr = nl.address().await.unwrap();

        let listen = tokio::spawn(async move { nl.accept().await.map(|_| ()) });
        tokio::spawn(async move {
            let _ = TcpNetLayer::new().connect(&addr).await;
        });

        tokio::time::timeout(Duration::from_millis(1000), listen)
            .await
            .unwrap()
            .unwrap()
            .unwrap(); // lmao
    }

    #[tokio::test]
    async fn connect() {
        let mut nl = TcpNetLayer::new();
        nl.init().await.unwrap();

        let addr = nl.address().await.unwrap();

        tokio::spawn(async move {
            let _ = nl.accept().await;
        });
        let connect =
            tokio::spawn(async move { TcpNetLayer::new().connect(&addr).await.map(|_| ()) });

        tokio::time::timeout(Duration::from_millis(1000), connect)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }
}
