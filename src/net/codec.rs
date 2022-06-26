//!
//! libp2p codec defining {de-}serialization for messages and their responses
//!

use async_trait::async_trait;
use libp2p::{
    core::ProtocolName,
    futures::{AsyncReadExt, AsyncWriteExt},
    request_response::RequestResponseCodec,
};

use crate::models::{RawInput, RawOutput};

///
/// protocol identifying messages for our actors
///
#[derive(Debug, Clone)]
pub enum ActorProtocol {
    /// version 1
    V1,
}

impl ProtocolName for ActorProtocol {
    fn protocol_name(&self) -> &[u8] {
        match self {
            Self::V1 => "/myriam/v1".as_bytes(),
        }
    }
}

///
/// libp2p codec for messaging
///
#[derive(Debug, Clone)]
pub struct MessagingCodec;

///
/// Unfortunately we have to keep these implementations completely generic
/// and handle {De-}Serialization outside the codec since we don't know
/// the type of the messages actors are gonna be sending at compile time
///
#[async_trait]
impl RequestResponseCodec for MessagingCodec {
    type Protocol = ActorProtocol;
    type Request = RawInput;
    type Response = RawOutput;

    async fn read_request<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> std::io::Result<Self::Request>
    where
        T: libp2p::futures::AsyncRead + Unpin + Send,
    {
        let mut buffer = vec![];
        io.read_to_end(&mut buffer).await?;

        Ok(buffer)
    }

    async fn read_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> std::io::Result<Self::Response>
    where
        T: libp2p::futures::AsyncRead + Unpin + Send,
    {
        let mut buffer = vec![];
        io.read_to_end(&mut buffer).await?;

        Ok(buffer)
    }

    async fn write_request<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        req: Self::Request,
    ) -> std::io::Result<()>
    where
        T: libp2p::futures::AsyncWrite + Unpin + Send,
    {
        Ok(io.write_all(req.as_slice()).await?)
    }

    async fn write_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        res: Self::Response,
    ) -> std::io::Result<()>
    where
        T: libp2p::futures::AsyncWrite + Unpin + Send,
    {
        Ok(io.write_all(res.as_slice()).await?)
    }
}
