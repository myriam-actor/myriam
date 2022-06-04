use async_trait::async_trait;
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};

use crate::address::Address;

use super::Context;

#[async_trait]
pub trait Actor: 'static {
    type Input: Send + 'static;
    type Output: Send + 'static;
    type Error: Send + 'static;

    async fn spawn(mut self: Box<Self>) -> LocalHandle<Self::Input, Self::Output, Self::Error> {
        let (tx, mut rx) =
            mpsc::channel::<LocalMessage<Self::Input, Self::Output, Self::Error>>(1024);

        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                let LocalMessage {
                    body,
                    return_channel,
                    context,
                    sender,
                } = msg;
                let res = self
                    .handle(context, sender, body)
                    .await
                    .map_err(LocalMessagingError::Task);

                let _ = return_channel.send(res);
            }
        });

        LocalHandle::new(tx)
    }

    async fn handle(
        &mut self,
        ctx: Option<Context>,
        addr: Option<Address>,
        arg: Self::Input,
    ) -> Result<Self::Output, Self::Error>;
}

#[derive(Debug)]
struct LocalMessage<I: Send + 'static, O: Send + 'static, E: Send + 'static> {
    body: I,
    context: Option<Context>,
    sender: Option<Address>,
    return_channel: oneshot::Sender<Result<O, LocalMessagingError<E>>>,
}

impl<I, O, E> LocalMessage<I, O, E>
where
    I: Send + 'static,
    O: Send + 'static,
    E: Send + 'static,
{
    fn new(
        i: I,
        ctx: Option<Context>,
        sender: Option<Address>,
    ) -> (Self, oneshot::Receiver<Result<O, LocalMessagingError<E>>>) {
        let (tx, rx) = oneshot::channel();
        (
            Self {
                body: i,
                return_channel: tx,
                context: ctx,
                sender,
            },
            rx,
        )
    }
}

#[derive(Debug, Error)]
pub enum LocalMessagingError<E: Send + 'static> {
    #[error("failed to reach local actor")]
    Send,

    #[error("{0}")]
    Recv(#[from] oneshot::error::RecvError),

    #[error("{0}")]
    Task(E),
}

#[derive(Debug, Clone)]
pub struct LocalHandle<I, O, E>
where
    I: Send + 'static,
    O: Send + 'static,
    E: Send + 'static,
{
    sender: mpsc::Sender<LocalMessage<I, O, E>>,
}

impl<I, O, E> LocalHandle<I, O, E>
where
    I: Send + 'static,
    O: Send + 'static,
    E: Send + 'static,
{
    fn new(sender: mpsc::Sender<LocalMessage<I, O, E>>) -> Self {
        Self { sender }
    }

    pub async fn send_local(
        &self,
        body: I,
        context: Option<Context>,
        sender: Option<Address>,
    ) -> Result<O, LocalMessagingError<E>> {
        let (msg, channel) = LocalMessage::new(body, context, sender);
        self.sender
            .send(msg)
            .await
            .map_err(|_| LocalMessagingError::Send)?;

        channel.await?
    }

    pub async fn send(&self, body: I) -> Result<O, LocalMessagingError<E>> {
        self.send_local(body, None, None).await
    }
}
