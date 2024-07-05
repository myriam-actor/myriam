use std::future::Future;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub mod tcp_layer;

pub trait NetLayer: Sized {
    type Error: std::error::Error;

    fn name() -> &'static str;
    fn init(&mut self) -> impl Future<Output = Result<(), Self::Error>>;
    fn accept(&self) -> impl Future<Output = Result<impl AsyncReadExt, Self::Error>>;
    fn connect(&self, addr: &str) -> impl Future<Output = Result<impl AsyncWriteExt, Self::Error>>;
    fn address(&self) -> Result<String, Self::Error>;
}
