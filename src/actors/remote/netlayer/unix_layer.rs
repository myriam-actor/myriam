use tokio::net::{UnixListener, UnixStream};

use super::{AsyncMsgStream, NetLayer};

#[derive(Debug)]
pub struct UnixNetLayer {
    path: String,
    listener: Option<UnixListener>,
}

impl UnixNetLayer {
    pub fn new(path: String) -> Self {
        Self {
            path,
            listener: None,
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }
}

impl NetLayer for UnixNetLayer {
    type Error = Error;

    fn name() -> &'static str {
        "unix"
    }

    async fn connect(&self, _: &str) -> Result<impl AsyncMsgStream, Self::Error> {
        Ok(UnixStream::connect(&self.path).await.map_err(|err| {
            tracing::error!("unix netlayer: {err}");
            Error::Connect
        })?)
    }

    async fn init(&mut self) -> Result<(), Self::Error> {
        self.listener
            .replace(UnixListener::bind(&self.path).map_err(|err| {
                tracing::error!("unix netlayer: {err}");
                Error::Init
            })?);

        Ok(())
    }

    async fn accept(&self) -> Result<impl AsyncMsgStream, Self::Error> {
        Ok(self
            .listener
            .as_ref()
            .ok_or(Error::NotReady)?
            .accept()
            .await
            .map_err(|err| {
                tracing::error!("unix netlayer: could not accept connection {err}");
                Error::Accept
            })?
            .0)
    }

    fn address(&self) -> Result<String, Self::Error> {
        // "address" makes no sense with Unix sockets,
        // and we don't want to accidentally expose local paths
        Ok(String::from("_"))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to connect to the given address")]
    Connect,

    #[error("failed to accept connection")]
    Accept,

    #[error("failed to bind unix socket")]
    Init,

    #[error("failed to obtain address")]
    Address,

    #[error("listener not ready")]
    NotReady,
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use rand::RngCore;

    use crate::actors::remote::netlayer::{unix_layer::UnixNetLayer, NetLayer};

    fn random_socket() -> String {
        let mut rng = rand::thread_rng();

        let mut buf = vec![0; 8];
        rng.fill_bytes(&mut buf);

        format!("/tmp/myriam-{}.sock", hex::encode(buf))
    }

    #[tokio::test]
    async fn listen() {
        crate::tests::init_tracing().await;

        let mut nl = UnixNetLayer::new(random_socket());
        nl.init().await.unwrap();
    }

    #[tokio::test]
    async fn accept() {
        crate::tests::init_tracing().await;

        let socket = random_socket();

        let mut nl = UnixNetLayer::new(socket.clone());
        nl.init().await.unwrap();

        let listen = tokio::spawn(async move { nl.accept().await.map(|_| ()) });
        tokio::spawn(async move {
            let _ = UnixNetLayer::new(socket.clone()).connect("").await;
        });

        tokio::time::timeout(Duration::from_millis(1000), listen)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn connect() {
        crate::tests::init_tracing().await;

        let socket = random_socket();

        let mut nl = UnixNetLayer::new(socket.clone());
        nl.init().await.unwrap();

        tokio::spawn(async move {
            let _ = nl.accept().await;
        });
        let connect = tokio::spawn(async move {
            UnixNetLayer::new(socket.clone())
                .connect("")
                .await
                .map(|_| ())
        });

        tokio::time::timeout(Duration::from_millis(1000), connect)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }
}
