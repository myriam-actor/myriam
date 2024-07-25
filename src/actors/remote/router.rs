//!
//! event loop and associated handlers for remote messaging
//!
//! # Protocol
//!
//! The wire protocol is defined as follows:
//!
//! ## Message
//! ` N_id | Id[N_id] | N_m | M[N_m] `
//!
//! where
//!
//! * `N_id`: 2 bytes -> `u16`
//! * `Id[N_id]`: `N_id` bytes -> `[u8; N_id]`
//! * `N_m`: 4 bytes -> `u32`
//! * `M[N_m]`: `N_m` bytes -> `[u8; N_m]`
//!
//! ## Reply
//! `N_r | R[N_r]`
//!
//! where
//!
//! * `N_r`: 4 bytes -> `u32`
//! * `R[N_r]`: `N_m` bytes -> `[u8; N_r]`
//!

use std::{collections::HashMap, marker::PhantomData, sync::Arc, time::Duration};

use serde::{de::DeserializeOwned, Serialize};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{mpsc, oneshot, RwLock},
};

use crate::{
    actors::remote::UntypedHandle,
    messaging::{Message, MsgResult},
};

use super::{
    address::{self, ActorAddress},
    dencoder::{self, Dencoder},
    netlayer::{AsyncMsgStream, NetLayer},
};

///
/// router for exposing actors under a given net layer
///
#[derive(Debug)]
pub struct Router;

impl Router {
    ///
    /// spawn a new router event loop using the given net layer, and return a handle to it
    ///
    pub async fn with_netlayer<N>(
        mut netlayer: N,
        opts: Option<RouterOpts>,
    ) -> Result<RouterHandle, Error>
    where
        N: NetLayer + Send + 'static,
        <N as NetLayer>::Error: Send + std::fmt::Display,
    {
        let opts = opts.unwrap_or_default();

        netlayer.init().await.map_err(|e| {
            tracing::error!("router init: {e}");
            Error::Init
        })?;

        let host_address = netlayer.address().map_err(|e| {
            tracing::error!("router init: failed to obtain address - {e}");
            Error::Init
        })?;

        let host_address_inner = host_address.clone();

        let peers: HashMap<String, UntypedHandle> = HashMap::new();

        let (sender, mut receiver) =
            mpsc::channel::<(RouterMessage, oneshot::Sender<Result<RouterReply, Error>>)>(1024);
        let (conf_sender, conf_receiver) = oneshot::channel::<Result<(), Error>>();

        tokio::spawn(async move {
            let opts = Arc::new(opts);
            let peers = Arc::new(RwLock::new(peers));
            let _ = conf_sender.send(Ok(()));

            loop {
                tokio::select! {
                    Some((command, sender)) = receiver.recv() => {
                        match command {
                            RouterMessage::Stop => {
                                let _ = sender.send(Ok(RouterReply::Accepted));
                                return;
                            },
                            RouterMessage::Attach(handle) => {
                                let addr = if let Ok(addr) = ActorAddress::new::<N>(&host_address_inner) {
                                    addr
                                } else {
                                    continue;
                                };

                                peers.write().await.insert(addr.peer_id().to_owned(), handle);

                                let _ = sender.send(Ok(RouterReply::Address(addr)));
                            },
                            RouterMessage::Revoke(addr) => {
                                peers.write().await.remove(addr.peer_id());

                                let _ = sender.send(Ok(RouterReply::Address(addr)));
                            },
                        }
                    },
                    Ok(mut stream) = netlayer.accept() => {
                        let opts = opts.clone();
                        let peers = peers.clone();

                        tokio::spawn(async move {
                            let _ = tokio::time::timeout(
                                Duration::from_millis(opts.msg_read_timeout()),
                                async move {
                                    let id = match try_read_id(&mut stream).await {
                                        Ok(id) => id,
                                        Err(_) => {
                                            return;
                                        },
                                    };

                                    let handle = match peers.read().await.get(&id) {
                                        Some(handle) => handle.clone(),
                                        None => {
                                            tracing::warn!("router: recv - unknown peer {id}");
                                            return;
                                        },
                                    };

                                    let _ = try_handle_message(stream, handle, opts.as_ref()).await;
                                }).await;
                        });
                    }
                }
            }
        });

        conf_receiver.await.map_err(|_| Error::Init)??;

        Ok(RouterHandle {
            sender,
            host_address,
        })
    }
}

async fn try_read_id<S>(stream: &mut S) -> Result<String, Error>
where
    S: AsyncReadExt + Unpin,
{
    let size = stream.read_u16().await.map_err(|e| {
        tracing::error!("router: could not read id size - {e}");
        Error::Recv
    })?;

    let mut id_buffer: Vec<u8> = vec![0; size as usize];
    stream.read_exact(&mut id_buffer).await.map_err(|err| {
        tracing::error!("router: recv - {err}");
        Error::Recv
    })?;

    Ok(hex::encode(id_buffer))
}

async fn try_handle_message<S>(
    mut stream: S,
    handle: UntypedHandle,
    opts: &RouterOpts,
) -> Result<(), Error>
where
    S: AsyncMsgStream,
{
    let msg_size = stream.read_u32().await.map_err(|e| {
        tracing::error!("router: recv - could not read msg size - {e}");
        Error::Recv
    })?;

    if msg_size > opts.max_msg_size() {
        tracing::warn!("router: recv - incoming message body exceeds size limit; dropping");
        Err(Error::Recv)?
    }

    let mut msg_buffer = vec![0; msg_size as usize];
    stream.read_exact(&mut msg_buffer).await.map_err(|e| {
        tracing::error!("router: recv - could not read msg - {e}");
        Error::Recv
    })?;

    let res = handle.send(msg_buffer).await.map_err(|err| {
        tracing::error!("router: msg error - {err}");
        Error::Send
    })?;

    stream.write_u32(res.len() as u32).await.map_err(|err| {
        tracing::error!("router: could not send response size - {err}");
        Error::Send
    })?;

    stream.write_all(&res).await.map_err(|err| {
        tracing::error!("router: could not send response - {err}");
        Error::Send
    })?;

    Ok(())
}

///
/// router configuration
///
#[derive(Debug)]
pub struct RouterOpts {
    ///
    /// timeout in milliseconds for reading messages from the net layer's stream.
    ///
    /// default is 5000.
    ///
    pub msg_read_timeout: u64,

    ///
    /// timeout in milliseconds for reading messages from the net layer's stream.
    ///
    /// default is 5000.
    ///
    pub max_msg_size: u32,
}

impl RouterOpts {
    /// create a new set of router options
    pub fn new(msg_read_timeout: u64, max_msg_size: u32) -> Self {
        Self {
            msg_read_timeout,
            max_msg_size,
        }
    }

    /// get the message read timeout
    pub fn msg_read_timeout(&self) -> u64 {
        self.msg_read_timeout
    }

    /// get the max message size.
    pub fn max_msg_size(&self) -> u32 {
        self.max_msg_size
    }
}

impl Default for RouterOpts {
    fn default() -> Self {
        Self {
            msg_read_timeout: 5000,
            max_msg_size: 4194304,
        }
    }
}

///
/// handle for router messaging
///
#[derive(Debug)]
pub struct RouterHandle {
    host_address: String,
    sender: mpsc::Sender<(RouterMessage, oneshot::Sender<Result<RouterReply, Error>>)>,
}

impl RouterHandle {
    ///
    /// register an actor, getting a new address for it.
    ///
    /// this address can be seen as a capability, and revoked at any time. see [`Self::revoke()`].
    ///
    pub async fn attach(&self, handle: UntypedHandle) -> Result<ActorAddress, Error> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send((RouterMessage::Attach(handle), sender))
            .await
            .map_err(|e| {
                tracing::error!("router: {e}");

                Error::Send
            })?;

        match receiver.await.map_err(|e| {
            tracing::error!("router: {e}");
            Error::Recv
        })?? {
            RouterReply::Accepted => panic!("expected Address variant"),
            RouterReply::Address(a) => Ok(a),
        }
    }

    ///
    /// revoke this address. any incoming requests using thereafter will be dropped.
    ///
    pub async fn revoke(&self, address: &ActorAddress) -> Result<ActorAddress, Error> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send((RouterMessage::Revoke(address.clone()), sender))
            .await
            .map_err(|e| {
                tracing::error!("router: {e}");

                Error::Send
            })?;

        match receiver.await.map_err(|e| {
            tracing::error!("router: {e}");
            Error::Recv
        })?? {
            RouterReply::Accepted => panic!("expected Address variant"),
            RouterReply::Address(a) => Ok(a),
        }
    }

    ///
    /// stop this router, dropping all registered addresses.
    ///
    pub async fn stop(&self) -> Result<(), Error> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send((RouterMessage::Stop, sender))
            .await
            .map_err(|e| {
                tracing::error!("router: {e}");

                Error::Send
            })?;

        match receiver.await.map_err(|e| {
            tracing::error!("router: {e}");
            Error::Recv
        })?? {
            RouterReply::Accepted => Ok(()),
            RouterReply::Address(_) => panic!("expected Accepted variant"),
        }
    }

    ///
    /// this router's exposed host address.
    ///
    /// equivalent to calling its net layer's [`NetLayer::address()`]
    ///
    pub fn host_address(&self) -> &str {
        &self.host_address
    }
}

///
/// handle for messaging a remote actor with a given capability.
///
#[derive(Debug, Clone)]
pub struct RemoteHandle<I, O, E, D: Dencoder, N: NetLayer> {
    address: ActorAddress,
    netlayer: N,

    _ipd: PhantomData<I>,
    _opd: PhantomData<O>,
    _epd: PhantomData<E>,
    _dpd: PhantomData<D>,
}

impl<I, O, E, D, N> RemoteHandle<I, O, E, D, N>
where
    I: Serialize + DeserializeOwned,
    O: Serialize + DeserializeOwned,
    E: Serialize + DeserializeOwned,
    D: Dencoder,
    N: NetLayer,
{
    ///
    /// create a new handle from this address and net layer for messaging
    ///
    pub fn new(address: &ActorAddress, netlayer: N) -> Self {
        Self {
            address: address.to_owned(),
            netlayer,
            _ipd: PhantomData::default(),
            _opd: PhantomData::default(),
            _epd: PhantomData::default(),
            _dpd: PhantomData::default(),
        }
    }

    ///
    /// try to message the actor behind our address
    ///
    pub async fn send(&self, msg: Message<I>) -> Result<MsgResult<O, E>, Error>
    where
        <N as NetLayer>::Error: std::fmt::Display,
    {
        let mut stream = self
            .netlayer
            .connect(self.address.host())
            .await
            .map_err(|err| {
                tracing::error!("remote handle: failed to connect - {err}");
                Error::Connect
            })?;

        let id = hex::decode(self.address.peer_id()).map_err(|err| {
            tracing::error!("remote handle: invalid id - {err}");
            Error::Connect
        })?;

        let id_len = id.len() as u16;

        stream.write_u16(id_len).await.map_err(|err| {
            tracing::error!("remote handle: failed to send peer ID size - {err}");
            Error::Send
        })?;

        stream.write_all(&id).await.map_err(|err| {
            tracing::error!("remote handle: failed to send peer ID - {err}");
            Error::Send
        })?;

        let bytes = D::encode(msg)?;
        stream.write_u32(bytes.len() as u32).await.map_err(|err| {
            tracing::error!("remote handle: failed to send message size - {err}");
            Error::Send
        })?;

        stream.write_all(&bytes).await.map_err(|err| {
            tracing::error!("remote handle: failed to send message - {err}");
            Error::Send
        })?;

        stream.flush().await.map_err(|err| {
            tracing::error!("remote handle: failed to send message - {err}");
            Error::Send
        })?;

        let size = stream.read_u32().await.map_err(|err| {
            tracing::error!("remote handle: failed to receive message size - {err}");
            Error::Recv
        })?;

        let mut res_buffer = vec![0; size as usize];
        stream.read_exact(&mut res_buffer).await.map_err(|err| {
            tracing::error!("remote handle: failed to receive message - {err}");
            Error::Recv
        })?;

        Ok(D::decode(res_buffer)?)
    }

    /// [`ActorAddress`] pointed to by this handle
    pub fn addr(&self) -> &ActorAddress {
        &self.address
    }
}

#[derive(Debug)]
enum RouterMessage {
    Stop,
    Attach(UntypedHandle),
    Revoke(ActorAddress),
}

enum RouterReply {
    Accepted,
    Address(ActorAddress),
}

///
/// errors when creating a routing, or messaging an actor with it
///
#[allow(missing_docs)]
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to init router")]
    Init,

    #[error("failed to connect to host")]
    Connect,

    #[error("failed to de/serialize message")]
    Serialize(#[from] dencoder::Error),

    #[error("failed to send message to router")]
    Send,

    #[error("failed to receive response from router")]
    Recv,

    #[error("{0}")]
    Address(#[from] address::Error),
}

#[cfg(test)]
mod tests {
    use crate::{
        actors::{
            remote::{
                self,
                dencoder::bincode::BincodeDencoder,
                netlayer::tcp_layer::TcpNetLayer,
                router::{RemoteHandle, Router, RouterOpts},
            },
            tests::{Mult, SomeError},
        },
        messaging::{Message, Reply},
    };

    #[tokio::test]
    async fn spawn_and_message() {
        let (_, handle) = remote::spawn_untyped::<_, _, _, BincodeDencoder>(Mult { a: 3 })
            .await
            .unwrap();

        let router = Router::with_netlayer(TcpNetLayer::new(), Some(RouterOpts::default()))
            .await
            .unwrap();

        let addr = router.attach(handle).await.unwrap();

        let remote = RemoteHandle::<u32, u32, SomeError, BincodeDencoder, TcpNetLayer>::new(
            &addr,
            TcpNetLayer::new(),
        );

        let res = remote.send(Message::Task(5)).await.unwrap();
        assert!(matches!(res, Ok(Reply::Task(15))));
    }

    #[tokio::test]
    async fn ping() {
        let (_, handle) = remote::spawn_untyped::<_, _, _, BincodeDencoder>(Mult { a: 3 })
            .await
            .unwrap();

        let router = Router::with_netlayer(TcpNetLayer::new(), Some(RouterOpts::default()))
            .await
            .unwrap();

        let addr = router.attach(handle).await.unwrap();

        let remote = RemoteHandle::<u32, u32, SomeError, BincodeDencoder, TcpNetLayer>::new(
            &addr,
            TcpNetLayer::new(),
        );

        let res = remote.send(Message::Ping).await.unwrap();
        assert!(matches!(res, Ok(Reply::Accepted)));
    }

    #[tokio::test]
    async fn stop() {
        let (_, mut handle) = remote::spawn_untyped::<_, _, _, BincodeDencoder>(Mult { a: 3 })
            .await
            .unwrap();

        handle.allow_stop(true);

        let router = Router::with_netlayer(TcpNetLayer::new(), Some(RouterOpts::default()))
            .await
            .unwrap();

        let addr = router.attach(handle).await.unwrap();

        let remote = RemoteHandle::<u32, u32, SomeError, BincodeDencoder, TcpNetLayer>::new(
            &addr,
            TcpNetLayer::new(),
        );

        let res = remote.send(Message::Stop).await.unwrap();
        assert!(matches!(res, Ok(Reply::Accepted)));

        remote.send(Message::Ping).await.unwrap_err();
    }

    #[tokio::test]
    async fn revoke() {
        let (_, handle) = remote::spawn_untyped::<_, _, _, BincodeDencoder>(Mult { a: 3 })
            .await
            .unwrap();

        let router = Router::with_netlayer(TcpNetLayer::new(), Some(RouterOpts::default()))
            .await
            .unwrap();

        let addr = router.attach(handle).await.unwrap();

        let remote = RemoteHandle::<u32, u32, SomeError, BincodeDencoder, TcpNetLayer>::new(
            &addr,
            TcpNetLayer::new(),
        );

        let res = remote.send(Message::Ping).await.unwrap();
        assert!(matches!(res, Ok(Reply::Accepted)));

        router.revoke(&addr).await.unwrap();

        remote.send(Message::Ping).await.unwrap_err();
    }
}
