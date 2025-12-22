use std::fmt::Display;

use tokio::net::TcpListener;

pub async fn random_unused_port() -> Result<u16, Error> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|_| Error::Port)?;

    Ok(listener.local_addr().map_err(|_| Error::Port)?.port())
}

#[derive(Debug)]
pub enum Error {
    Port,
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Port => write!(f, "failed to obtain an unused port"),
        }
    }
}
