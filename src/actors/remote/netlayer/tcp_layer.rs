use tokio::net::{TcpListener, TcpStream};

use super::{AsyncMsgStream, NetLayer};

#[derive(Debug)]
pub struct TcpNetLayer {
    listener: Option<TcpListener>,
}

impl TcpNetLayer {
    pub fn new() -> Self {
        Self {
            listener: Option::None,
        }
    }
}

impl NetLayer for TcpNetLayer {
    type Error = TcpError;

    fn name() -> &'static str {
        "tcp"
    }

    async fn connect(&self, addr: &str) -> Result<impl AsyncMsgStream, Self::Error> {
        Ok(TcpStream::connect(addr).await.map_err(|e| {
            tracing::error!("connect error {e}");

            TcpError::Connect
        })?)
    }

    async fn init(&mut self) -> Result<(), Self::Error> {
        self.listener
            .replace(TcpListener::bind("0.0.0.0:0").await.map_err(|e| {
                tracing::error!("bind error: {e}");

                TcpError::Bind
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

                TcpError::Accept
            })?
            .0)
    }

    fn address(&self) -> Result<String, Self::Error> {
        Ok(self
            .listener
            .as_ref()
            .ok_or(TcpError::NotReady)?
            .local_addr()
            .map_err(|_| TcpError::NotReady)?
            .to_string())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TcpError {
    #[error("net layer not ready")]
    NotReady,

    #[error("failed to bind net layer to address")]
    Bind,

    #[error("failed to accept new connection")]
    Accept,

    #[error("failed to connect to address")]
    Connect,
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::actors::remote::netlayer::{tcp_layer::TcpNetLayer, NetLayer};

    #[tokio::test]
    async fn listen() {
        let mut nl = TcpNetLayer::new();
        nl.init().await.unwrap();
    }

    #[tokio::test]
    async fn accept() {
        let mut nl = TcpNetLayer::new();
        nl.init().await.unwrap();

        let addr = nl.address().unwrap();

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

        let addr = nl.address().unwrap();

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
