//!
//! abstractions for network layers and implementations
//!

use std::future::Future;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[cfg(feature = "tcp")]
pub mod tcp_layer;

#[cfg(feature = "tor")]
pub mod tor_layer;

///
/// trait for AsyncRead + AsyncWrite streams used in routers
///
pub trait AsyncMsgStream: AsyncReadExt + AsyncWriteExt + Unpin + Send + 'static {}

impl<T> AsyncMsgStream for T where T: AsyncReadExt + AsyncWriteExt + Unpin + Send + 'static {}

///
/// net layer trait abstracting over async streams
///
pub trait NetLayer {
    /// errors during net layer operation
    type Error: std::error::Error;

    ///
    /// net layer's identifier, using during actor address construction
    ///
    fn name() -> &'static str;

    ///
    /// connect to the given address, returning a stream
    ///
    fn connect(
        &self,
        addr: &str,
    ) -> impl Future<Output = Result<impl AsyncMsgStream, Self::Error>> + Send;

    ///
    /// initialize this net layer for accepting connections
    ///
    fn init(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send;

    ///
    /// wait for and accept the next connection
    ///
    fn accept(&self) -> impl Future<Output = Result<impl AsyncMsgStream, Self::Error>> + Send;

    ///
    /// this net layer's exposed address
    ///
    fn address(&self) -> Result<String, Self::Error>;
}
