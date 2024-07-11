use std::future::Future;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[cfg(feature = "tcp")]
pub mod tcp_layer;

pub trait AsyncMsgStream: AsyncReadExt + AsyncWriteExt + Unpin + Send + 'static {}

impl<T> AsyncMsgStream for T where T: AsyncReadExt + AsyncWriteExt + Unpin + Send + 'static {}

pub trait NetLayer {
    type Error: std::error::Error;

    fn name() -> &'static str;

    fn connect(
        &self,
        addr: &str,
    ) -> impl Future<Output = Result<impl AsyncMsgStream, Self::Error>> + Send;

    fn init(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send;
    fn accept(&self) -> impl Future<Output = Result<impl AsyncMsgStream, Self::Error>> + Send;
    fn address(&self) -> Result<String, Self::Error>;
}
