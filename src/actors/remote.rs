use dencoder::Dencoder;
use serde::{de::DeserializeOwned, Serialize};
use tokio::sync::{mpsc, oneshot};

use crate::messaging::Message;

use super::{
    local::{self, LocalHandle},
    Actor,
};

pub mod dencoder;
pub mod netlayer;

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
        mpsc::channel::<(Vec<u8>, oneshot::Sender<Result<Vec<u8>, Error>>)>(1024);
    let (conf_sender, conf_receiver) = oneshot::channel::<Result<(), Error>>();

    tokio::spawn(async move {
        let _ = conf_sender.send(Ok(()));
        while let Some((msg, sender)) = receiver.recv().await {
            match D::decode(msg) {
                Ok(msg) => {
                    let stop_msg = matches!(msg, Message::<I>::Stop);

                    let res = inner_handle.send(msg).await;
                    let _ = sender.send(D::encode(res).map_err(|_| Error::Encode));

                    if stop_msg {
                        break;
                    }
                }
                Err(err) => {
                    tracing::error!("failed to decode incoming message: {err}");
                    let _ = sender.send(Err(Error::Decode));
                }
            }
        }
    });

    conf_receiver.await.map_err(|_| Error::Spawn)??;

    Ok((local_handle, UntypedHandle { sender }))
}

#[derive(Debug, Clone)]
pub struct UntypedHandle {
    sender: mpsc::Sender<(Vec<u8>, oneshot::Sender<Result<Vec<u8>, Error>>)>,
}

impl UntypedHandle {
    pub async fn send(&self, msg: Vec<u8>) -> Result<Vec<u8>, Error> {
        let (sender, receiver) = oneshot::channel();

        self.sender.send((msg, sender)).await.map_err(|e| {
            tracing::error!("send: {e}");

            Error::Send
        })?;

        receiver.await.map_err(|_| Error::Recv)?
    }
}

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
        messaging::{Message, MsgResult, Reply},
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

        let (_, handle) = super::spawn_untyped::<_, _, _, BincodeDencoder>(mult)
            .await
            .unwrap();

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
}
