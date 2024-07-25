//!
//! local actors, with no net dependencies
//!

use tokio::sync::{mpsc, oneshot};

use crate::messaging::{Message, MsgError, MsgResult, Reply};

use super::Actor;

///
/// consume an actor and return a handle to it
///
pub async fn spawn<I, O, E>(
    mut actor: impl Actor<I, O, E> + Send + 'static,
) -> Result<LocalHandle<I, O, E>, Error>
where
    I: Send + 'static,
    O: Send + 'static,
    E: Send + std::error::Error + 'static,
{
    // TODO: non-arbitrary channel bound
    let (sender, mut receiver) =
        mpsc::channel::<(Message<I>, oneshot::Sender<MsgResult<O, E>>)>(1024);
    let (conf_sender, conf_receiver) = oneshot::channel::<Result<(), Error>>();

    tokio::spawn(async move {
        let _ = conf_sender.send(Ok(()));
        while let Some((msg, sender)) = receiver.recv().await {
            match msg {
                Message::Task(input) => {
                    let result = match actor.handler(input).await {
                        Ok(res) => Ok(Reply::Task(res)),
                        Err(err) => Err(MsgError::Task(err)),
                    };

                    try_send_reply(sender, result);
                }
                Message::TaskMut(input) => {
                    let result = match actor.handler_mut(input).await {
                        Ok(Some(res)) => Ok(Reply::Task(res)),
                        Ok(None) => Ok(Reply::Accepted),
                        Err(err) => Err(MsgError::Task(err)),
                    };

                    try_send_reply(sender, result);
                }
                Message::Ping => {
                    try_send_reply(sender, Ok(Reply::Accepted));
                }
                Message::Stop => {
                    try_send_reply(sender, Ok(Reply::Accepted));
                    break;
                }
            }
        }
    });

    // first error is oneshot sender being dropped prematurely
    conf_receiver.await.map_err(|_| Error::Spawn)??;

    Ok(LocalHandle { sender })
}

fn try_send_reply<O, E>(sender: oneshot::Sender<MsgResult<O, E>>, reply: MsgResult<O, E>)
where
    E: std::error::Error,
{
    if let Err(_) = sender.send(reply) {
        tracing::error!("local: failed to send reply");
    }
}

///
/// handle for a locally spawned actor
///
#[derive(Debug, Clone)]
pub struct LocalHandle<I, O, E: std::error::Error> {
    sender: mpsc::Sender<(Message<I>, oneshot::Sender<MsgResult<O, E>>)>,
}

impl<I, O, E> LocalHandle<I, O, E>
where
    E: std::error::Error,
{
    ///
    /// attempt to send a message to this actor
    ///
    pub async fn send(&self, msg: Message<I>) -> MsgResult<O, E> {
        let (sender, receiver) = oneshot::channel();

        self.sender
            .send((msg, sender))
            .await
            .map_err(|_| MsgError::Send)?;

        receiver.await.map_err(|_| MsgError::Recv)?
    }

    ///
    /// attempt to send a message to this actor
    ///
    /// this is a convenience method to use outside async contexts
    ///
    pub fn blocking_send(&self, msg: Message<I>) -> MsgResult<O, E> {
        let (sender, receiver) = oneshot::channel();

        self.sender
            .blocking_send((msg, sender))
            .map_err(|_| MsgError::Send)?;

        receiver.blocking_recv().map_err(|_| MsgError::Recv)?
    }
}

///
/// Errors when spawning an actor
///
#[allow(missing_docs)]
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to spawn this actor")]
    Spawn,
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::{runtime::Runtime, sync::oneshot};

    use crate::{
        actors::tests::Mult,
        messaging::{Message, Reply},
    };

    #[tokio::test]
    async fn spawning_and_messaging() {
        let mult = Mult { a: 2 };

        let handle = super::spawn(mult).await.unwrap();

        let reply = handle.send(Message::Task(15)).await.unwrap();

        assert!(matches!(reply, Reply::Task(30)));
    }

    #[tokio::test]
    async fn ping() {
        let mult = Mult { a: 2 };

        let handle = super::spawn(mult).await.unwrap();

        let reply = handle.send(Message::Ping).await.unwrap();

        assert!(matches!(reply, Reply::Accepted));
    }

    #[tokio::test]
    async fn stop() {
        let mult = Mult { a: 2 };
        let handle = super::spawn(mult).await.unwrap();

        let reply = handle.send(Message::Stop).await.unwrap();

        assert!(matches!(reply, Reply::Accepted));

        let _ = tokio::time::sleep(Duration::from_millis(10)).await;

        handle.send(Message::Ping).await.unwrap_err();
    }

    #[test]
    fn blocking_send() {
        let rt = Runtime::new().unwrap();

        let (handler_sender, handler_receiver) = oneshot::channel();
        let (compl_sender, compl_receiver) = oneshot::channel();

        std::thread::spawn(move || {
            rt.block_on(async move {
                let mult = Mult { a: 2 };
                let handle = super::spawn(mult).await.unwrap();

                handler_sender.send(handle).unwrap();
                let _ = compl_receiver.await;
            });
        });

        let handle = handler_receiver.blocking_recv().unwrap();
        let reply = handle.blocking_send(Message::Task(15)).unwrap();

        assert!(matches!(reply, Reply::Task(30)));

        compl_sender.send(()).unwrap();
    }
}
