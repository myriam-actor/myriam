//!
//! support for remote access to local actors
//!

use dencoder::Dencoder;
use serde::{de::DeserializeOwned, Serialize};
use tokio::sync::{mpsc, oneshot};

use crate::messaging::{Message, MsgError, MsgResult};

use super::{
    local::{self, LocalHandle},
    Actor,
};

pub mod address;
pub mod dencoder;
pub mod netlayer;
pub mod router;

///
/// spawn an actor, wrapping it behind an untyped handle.
///
/// necessary for registering with a local router.
///
pub async fn spawn_untyped<I, O, E, D>(
    actor: impl Actor<I, O, E> + Send + 'static,
) -> Result<(LocalHandle<I, O, E>, UntypedHandle), Error>
where
    I: Clone + Send + DeserializeOwned + 'static,
    O: Clone + Send + Serialize + 'static,
    E: Clone + Send + Serialize + std::error::Error + 'static,
    D: Dencoder,
{
    let local_handle = local::spawn(actor).await?;
    let inner_handle = local_handle.clone();
    let (sender, mut receiver) =
        mpsc::channel::<(Vec<u8>, HandleOpts, oneshot::Sender<Result<Vec<u8>, Error>>)>(1024);
    let (conf_sender, conf_receiver) = oneshot::channel::<Result<(), Error>>();

    tokio::spawn(async move {
        let _ = conf_sender.send(Ok(()));
        while let Some((msg, opts, sender)) = receiver.recv().await {
            match D::decode::<Message<I>>(msg) {
                Ok(msg) => {
                    if let Err(err) = opts.validate::<I, E>(&msg) {
                        let err: MsgResult<O, E> = Err(err);
                        let res = D::encode(err).map_err(|_| Error::Encode);
                        let _ = sender.send(res);
                        continue;
                    }

                    let stop_msg = matches!(msg, Message::<I>::Stop);

                    let res = inner_handle.send(msg).await;
                    match D::encode(res).map_err(|_| Error::Encode) {
                        Ok(enc) => {
                            if let Err(_) = sender.send(Ok(enc)) {
                                tracing::warn!("untyped: failed to send reply");
                            }

                            if stop_msg {
                                break;
                            }
                        }
                        Err(err) => {
                            tracing::error!("untyped: failed to encode reply");
                            let _ = sender.send(Err(err)).inspect_err(|_| {
                                tracing::warn!("untyped: failed to send reply");
                            });
                        }
                    }
                }
                Err(err) => {
                    tracing::error!("untyped: failed to decode incoming message: {err}");
                    let _ = sender.send(Err(Error::Decode)).inspect_err(|_| {
                        tracing::warn!("untyped: failed to send reply");
                    });
                }
            }
        }
    });

    conf_receiver.await.map_err(|_| Error::Spawn)??;

    Ok((
        local_handle,
        UntypedHandle {
            sender,
            opts: HandleOpts::new(),
        },
    ))
}

///
/// options for this handle
///
#[derive(Debug, Clone)]
pub struct HandleOpts {
    allow_mut: bool,
    allow_stop: bool,
}

impl HandleOpts {
    ///
    /// new option set with defaults:
    ///
    /// * allow mutation: false
    /// * allow stopping: false
    ///
    pub fn new() -> Self {
        Self {
            allow_mut: false,
            allow_stop: false,
        }
    }

    ///
    /// validate message according to this option set
    ///
    pub fn validate<I, E: std::error::Error>(&self, msg: &Message<I>) -> Result<(), MsgError<E>> {
        match msg {
            Message::TaskMut(_) if !self.allow_mut => Err(MsgError::Mut),
            Message::Stop if !self.allow_stop => Err(MsgError::Stop),
            _ => Ok(()),
        }
    }

    /// whether this handle relays messages requiring mutation
    pub fn allow_mut(&self) -> bool {
        self.allow_mut
    }

    /// whether this handle relays `Stop` messages
    pub fn allow_stop(&self) -> bool {
        self.allow_stop
    }
}

///
/// untyped handle for remote messaging, when types aren't available.
///
#[derive(Debug, Clone)]
pub struct UntypedHandle {
    sender: mpsc::Sender<(Vec<u8>, HandleOpts, oneshot::Sender<Result<Vec<u8>, Error>>)>,
    opts: HandleOpts,
}

impl UntypedHandle {
    ///
    /// attempt to message this actor with an encoded message, getting an encoded response in return.
    ///
    pub async fn send(&self, msg: Vec<u8>) -> Result<Vec<u8>, Error> {
        let (sender, receiver) = oneshot::channel();

        self.sender
            .send((msg, self.opts.clone(), sender))
            .await
            .map_err(|e| {
                tracing::error!("untyped send: {e}");

                Error::Send
            })?;

        receiver.await.map_err(|e| {
            tracing::error!("untyped recv: {e}");
            Error::Recv
        })?
    }

    ///
    /// whether to allow this handle to relay messages requiring mutation.
    ///
    /// off by default.
    ///
    pub fn allow_mut(&mut self, allow: bool) {
        self.opts.allow_mut = allow;
    }

    ///
    /// whether to allow this handle to relay `Stop` messages.
    ///
    /// off by default.
    ///
    pub fn allow_stop(&mut self, allow: bool) {
        self.opts.allow_stop = allow;
    }
}

///
/// errors when spawning an actor or messaging through an [`UntypedHandle`]
///
#[allow(missing_docs)]
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to spawn local actor")]
    Local(#[from] local::Error),

    #[error("failed to spawn dencoder actor")]
    Spawn,

    #[error("failed to send message to dencoder actor")]
    Send,

    #[error("failed to receive reply from dencoder actor")]
    Recv,

    #[error("failed to decode message")]
    Decode,

    #[error("failed to encode message")]
    Encode,
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{
        actors::{
            remote::dencoder::{bincode::BincodeDencoder, Dencoder},
            tests::*,
        },
        messaging::{Message, MsgError, MsgResult, Reply},
    };

    #[tokio::test]
    async fn spawning_and_messaging() {
        let mult = Mult { a: 2 };

        let (_, handle) = super::spawn_untyped::<_, _, _, BincodeDencoder>(mult)
            .await
            .unwrap();

        let msg = BincodeDencoder::encode(Message::Task(14u32)).unwrap();

        let raw = handle.send(msg).await.unwrap();
        let res = BincodeDencoder::decode::<MsgResult<u32, SomeError>>(raw)
            .unwrap()
            .unwrap();

        assert!(matches!(res, Reply::Task(28)));
    }

    #[tokio::test]
    async fn ping() {
        let mult = Mult { a: 2 };

        let (_, handle) = super::spawn_untyped::<_, _, _, BincodeDencoder>(mult)
            .await
            .unwrap();

        let msg = BincodeDencoder::encode(Message::<u32>::Ping).unwrap();

        let raw = handle.send(msg).await.unwrap();
        let res = BincodeDencoder::decode::<MsgResult<u32, SomeError>>(raw)
            .unwrap()
            .unwrap();

        assert!(matches!(res, Reply::Accepted));
    }

    #[tokio::test]
    async fn stop() {
        let mult = Mult { a: 2 };

        let (_, mut handle) = super::spawn_untyped::<_, _, _, BincodeDencoder>(mult)
            .await
            .unwrap();

        handle.allow_stop(true);

        let msg = BincodeDencoder::encode(Message::<u32>::Stop).unwrap();

        let raw = handle.send(msg).await.unwrap();
        let res = BincodeDencoder::decode::<MsgResult<u32, SomeError>>(raw)
            .unwrap()
            .unwrap();

        assert!(matches!(res, Reply::Accepted));

        tokio::time::sleep(Duration::from_millis(10)).await;

        let msg = BincodeDencoder::encode(Message::<u32>::Ping).unwrap();

        handle.send(msg).await.unwrap_err();
    }

    #[tokio::test]
    async fn disallow_mut() {
        let mult = Mult { a: 2 };

        let (_, handle) = super::spawn_untyped::<_, _, _, BincodeDencoder>(mult)
            .await
            .unwrap();

        let msg = BincodeDencoder::encode(Message::<u32>::TaskMut(6)).unwrap();

        let raw = handle.send(msg).await.unwrap();
        let res = BincodeDencoder::decode::<MsgResult<u32, SomeError>>(raw)
            .unwrap()
            .unwrap_err();

        assert!(matches!(res, MsgError::Mut));
    }
}
