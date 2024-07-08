use std::future::Future;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub mod tcp_layer;

pub trait AsyncReadWriteExt: AsyncReadExt + AsyncWriteExt + Unpin + Send {}

impl<T> AsyncReadWriteExt for T where T: AsyncReadExt + AsyncWriteExt + Unpin + Send {}

pub trait NetLayer {
    type Error: std::error::Error;

    fn name() -> &'static str;

    fn connect(
        addr: &str,
    ) -> impl Future<Output = Result<impl AsyncReadWriteExt, Self::Error>> + Send;

    fn init(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send;
    fn accept(&self) -> impl Future<Output = Result<impl AsyncReadWriteExt, Self::Error>> + Send;
    fn address(&self) -> Result<String, Self::Error>;
}
