use std::error::Error;

use crate::{
    actors::opts::Ip,
    models::{MessageResult, MessageType, MessagingError, TaskResult},
};

use super::{
    auth::{AccessRequest, AccessResolution, AddrStore, AuthActor, PeerStore},
    opts::SpawnOpts,
    Actor, ActorHandle, Context,
};
use async_trait::async_trait;
use libp2p::identity::Keypair;
use serde::{Deserialize, Serialize};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Default)]
struct Counter {
    count: usize,
}

#[derive(Debug)]
struct Proxy(ActorHandle);

#[derive(Debug, Clone, Serialize, Deserialize)]
enum CounterMessage {
    Get,
    Increment(usize),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProxyGetCounterMessage;

#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[error("proxy failure")]
struct ProxyError;

#[derive(Debug, Clone, thiserror::Error, Serialize, Deserialize)]
enum CounterError {
    #[error("init value should be zero")]
    Init,
    #[error("something else went wrong")]
    InvalidRequest,
}

struct Autho;

#[async_trait]
impl AuthActor for Autho {
    async fn resolve(
        &mut self,
        request: AccessRequest,
        _addr_store: &mut AddrStore,
        _peer_store: &mut PeerStore,
    ) -> AccessResolution {
        if let Some(addr) = request.addr {
            match addr.iter().find(|p| match p {
                libp2p::multiaddr::Protocol::Ip4(a) => a.is_loopback(),
                libp2p::multiaddr::Protocol::Ip6(a) => a.is_loopback(),
                _ => false,
            }) {
                Some(_) => AccessResolution::Accepted,
                None => AccessResolution::Denied,
            }
        } else {
            AccessResolution::Denied
        }
    }
}

struct BanningAuth;

#[async_trait]
impl AuthActor for BanningAuth {
    async fn resolve(
        &mut self,
        _: AccessRequest,
        _: &mut AddrStore,
        _: &mut PeerStore,
    ) -> AccessResolution {
        AccessResolution::Ban
    }
}

#[async_trait]
impl Actor for Counter {
    type Input = CounterMessage;
    type Output = usize;
    type Error = CounterError;

    async fn handle(
        &mut self,
        msg: Self::Input,
        _: Context<Self::Output, Self::Error>,
    ) -> Result<Self::Output, Self::Error> {
        match msg {
            CounterMessage::Get => Ok(self.count),
            CounterMessage::Increment(n) => {
                self.count += n;
                Ok(self.count)
            }
        }
    }

    async fn on_init(&mut self) -> Result<(), Box<dyn Error>> {
        if self.count == 0 {
            Ok(())
        } else {
            Err(Box::new(CounterError::Init))
        }
    }

    async fn on_stop(&self) {}
}

#[async_trait]
impl Actor for Proxy {
    type Input = ProxyGetCounterMessage;
    type Output = usize;
    type Error = ProxyError;

    async fn handle(
        &mut self,
        _: Self::Input,
        ctx: Context<Self::Output, Self::Error>,
    ) -> Result<Self::Output, Self::Error> {
        if let Ok(TaskResult::Finished(n)) = self
            .send::<CounterMessage, usize, CounterError>(
                self.0.clone(),
                MessageType::TaskRequest(CounterMessage::Get),
                ctx,
            )
            .await
        {
            Ok(n)
        } else {
            Err(ProxyError)
        }
    }

    async fn on_init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    async fn on_stop(&self) {}
}

#[tokio::test]
async fn spawn_and_message() -> Result<(), Box<dyn std::error::Error>> {
    let filter = EnvFilter::from_default_env();
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let counter = Box::new(Counter::default());
    let their_keypair = Keypair::generate_ed25519();
    let their_auth = Box::new(Autho).spawn(their_keypair).await;
    let (handle, task) = counter
        .spawn(
            their_auth,
            SpawnOpts {
                protocol: Some(Ip::V4),
            },
        )
        .await
        .unwrap();

    let our_keypair = Keypair::generate_ed25519();
    let our_auth = Box::new(Autho).spawn(our_keypair).await;
    let response = handle
        .send_toplevel::<CounterMessage, usize, CounterError>(
            MessageType::TaskRequest(CounterMessage::Increment(10)),
            our_auth.clone(),
        )
        .await
        .expect("failed to send message!");

    match response {
        TaskResult::Accepted => panic!("failed to receive a value in response!"),
        TaskResult::Finished(n) => assert_eq!(n, 10),
    }

    let _ = handle
        .send_toplevel::<CounterMessage, usize, CounterError>(MessageType::Stop, our_auth)
        .await
        .expect("failed to stop actor!");
    let _ = task.await;

    Ok(())
}

#[tokio::test]
async fn actor_to_actor() -> Result<(), Box<dyn std::error::Error>> {
    let counter = Box::new(Counter::default());
    let counter_keypair = Keypair::generate_ed25519();
    let counter_auth = Box::new(Autho).spawn(counter_keypair).await;
    let (counter_handle, _) = counter
        .spawn(
            counter_auth,
            SpawnOpts {
                protocol: Some(Ip::V4),
            },
        )
        .await?;

    let proxy = Box::new(Proxy(counter_handle.clone()));
    let proxy_keypair = Keypair::generate_ed25519();
    let proxy_auth = Box::new(Autho).spawn(proxy_keypair).await;
    let (proxy_handle, _) = proxy
        .spawn(
            proxy_auth,
            SpawnOpts {
                protocol: Some(Ip::V4),
            },
        )
        .await?;

    let our_keypair = Keypair::generate_ed25519();
    let our_auth = Box::new(Autho).spawn(our_keypair).await;

    counter_handle
        .send_toplevel::<CounterMessage, usize, CounterError>(
            MessageType::TaskRequest(CounterMessage::Increment(42)),
            our_auth.clone(),
        )
        .await
        .expect("failed to set counter");

    let proxy_response = proxy_handle
        .send_toplevel::<ProxyGetCounterMessage, usize, ProxyError>(
            MessageType::TaskRequest(ProxyGetCounterMessage),
            our_auth.clone(),
        )
        .await
        .expect("failed to send message!");

    match proxy_response {
        TaskResult::Accepted => panic!("failed to receive a value in response!"),
        TaskResult::Finished(n) => assert_eq!(n, 42),
    }

    Ok(())
}

#[tokio::test]
async fn messaging_from_banning_fails() {
    let counter = Box::new(Counter::default());
    let counter_keypair = Keypair::generate_ed25519();
    let counter_auth = Box::new(BanningAuth).spawn(counter_keypair).await;
    let (counter_handle, _) = counter
        .spawn(counter_auth, Default::default())
        .await
        .expect("failed to spawn counter");

    let our_keypair = Keypair::generate_ed25519();
    let our_auth = Box::new(Autho).spawn(our_keypair).await;

    let result = counter_handle
        .send_toplevel::<CounterMessage, usize, CounterError>(
            MessageType::TaskRequest(CounterMessage::Increment(1)),
            our_auth.clone(),
        )
        .await;

    assert!(matches!(result, MessageResult::Err(MessagingError::Banned)));

    let _ = counter_handle
        .send_toplevel::<CounterMessage, usize, CounterError>(
            MessageType::TaskRequest(CounterMessage::Increment(1)),
            our_auth,
        )
        .await
        .expect_err("message was supposed to fail");
}
