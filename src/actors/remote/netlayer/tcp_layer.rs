use tokio::net::{TcpListener, TcpStream};

use super::{AsyncReadWriteExt, NetLayer};

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

    async fn connect(addr: &str) -> Result<impl AsyncReadWriteExt, Self::Error> {
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

    async fn accept(&self) -> Result<impl AsyncReadWriteExt, Self::Error> {
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
