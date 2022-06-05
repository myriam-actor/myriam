use async_trait::async_trait;
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};

use crate::address::Address;

use super::ActorOptions;

///
/// Main Actor trait.
/// Can technically be used for local actors, but internally we wrap around them to offer remote actors.
///
#[async_trait]
pub trait Actor: Send + 'static {
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
                    sender,
                } = msg;
                let res = self
                    .handle(sender, body)
                    .await
                    .map_err(LocalMessagingError::Task);

                let _ = return_channel.send(res);
            }
        });

        LocalHandle::new(tx)
    }

    ///
    /// Convenience method to get the (remote) spawn options for this actor
    ///
    fn spawn_options(&self, timeout: Option<u64>) -> ActorOptions {
        let Address { host, port } = self.get_self();
        ActorOptions {
            host,
            port: Some(port),
            read_timeout: timeout,
        }
    }

    async fn handle(
        &mut self,
        addr: Option<Address>,
        arg: Self::Input,
    ) -> Result<Self::Output, Self::Error>;

    ///
    /// Objects implementing this trait should have some way to query
    /// their own address
    ///
    fn get_self(&self) -> Address;
}

#[derive(Debug)]
struct LocalMessage<I: Send + 'static, O: Send + 'static, E: Send + 'static> {
    body: I,
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
        sender: Option<Address>,
    ) -> (Self, oneshot::Receiver<Result<O, LocalMessagingError<E>>>) {
        let (tx, rx) = oneshot::channel();
        (
            Self {
                body: i,
                return_channel: tx,
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
        sender: Option<Address>,
    ) -> Result<O, LocalMessagingError<E>> {
        let (msg, channel) = LocalMessage::new(body, sender);
        self.sender
            .send(msg)
            .await
            .map_err(|_| LocalMessagingError::Send)?;

        channel.await?
    }

    pub async fn send(&self, body: I) -> Result<O, LocalMessagingError<E>> {
        self.send_local(body, None).await
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use crate::address::Address;

    use super::Actor;

    struct TestActor {
        counter: i32,
        self_address: Address,
    }

    impl TestActor {
        fn new(initial_count: i32) -> Self {
            Self {
                counter: initial_count,
                self_address: Address::new_with_random_port("127.0.0.1").unwrap(),
            }
        }
    }

    #[derive(Debug)]
    struct SomeError;

    #[async_trait]
    impl Actor for TestActor {
        type Input = i32;

        type Output = i32;

        type Error = SomeError;

        async fn handle(
            &mut self,
            _addr: Option<crate::address::Address>,
            arg: Self::Input,
        ) -> Result<Self::Output, Self::Error> {
            self.counter *= arg;
            Ok(self.counter)
        }

        fn get_self(&self) -> Address {
            self.self_address.clone()
        }
    }

    #[tokio::test]
    async fn spawn_and_message() {
        let actor = TestActor::new(15);
        let handle = Box::new(actor).spawn().await;

        let result = handle.send(3).await.unwrap();

        assert_eq!(45, result);
    }
}
