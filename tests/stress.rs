//!
//! In this test we spawn 10 actors to message a single actor at once with 1000 messages
//!
//! Running with `cargo test --test stress --profile=bench` in my i7 laptop
//! gives me:
//!
//! ```
//! running 1 test
//! test stress ... ok
//!
//! test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.40s
//! ```
//!
//! which is not terrible for 10,000 Request-Replies, I think
//!

use async_trait::async_trait;
use libp2p::identity::Keypair;
use myriam::{
    actors::{
        auth::{AccessRequest, AccessResolution, AddrStore, AuthActor, PeerStore},
        opts::{Ip, SpawnOpts},
        Actor, ActorHandle, Context,
    },
    models::{MessageType, TaskResult},
};
use serde::{Deserialize, Serialize};

const NUM_TASKS: usize = 1000;
const NUM_ACTORS: usize = 10;

#[derive(Debug, Default)]
struct Counter {
    count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum CounterMessage {
    Get,
    Increment(usize),
}

#[derive(Debug, Clone, thiserror::Error, Serialize, Deserialize)]
enum CounterError {
    #[error("init value should be zero")]
    Init,
    #[error("something else went wrong")]
    InvalidRequest,
}

#[derive(Debug)]
struct Profiler;

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

    async fn on_init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.count == 0 {
            Ok(())
        } else {
            Err(Box::new(CounterError::Init))
        }
    }

    async fn on_stop(&self) {}
}

#[async_trait]
impl Actor for Profiler {
    type Input = ActorHandle;
    type Output = ();
    type Error = CounterError;

    async fn handle(
        &mut self,
        msg: Self::Input,
        ctx: Context<Self::Output, Self::Error>,
    ) -> Result<Self::Output, Self::Error> {
        for _ in 0..NUM_TASKS {
            let counter_handle = msg.clone();
            let _ = self
                .send::<CounterMessage, usize, CounterError>(
                    counter_handle,
                    MessageType::TaskRequest(CounterMessage::Increment(1)),
                    ctx.clone(),
                )
                .await;
        }

        Ok(())
    }

    async fn on_init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    async fn on_stop(&self) {}
}

#[tokio::test]
#[ignore]
async fn stress() -> Result<(), Box<dyn std::error::Error>> {
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

    let our_keypair = Keypair::generate_ed25519();
    let our_auth = Box::new(Autho).spawn(our_keypair).await;

    let mut profilers = vec![];
    for _ in 0..NUM_ACTORS {
        let profiler = Box::new(Profiler);
        let profiler_keypair = Keypair::generate_ed25519();
        let profiler_auth = Box::new(Autho).spawn(profiler_keypair).await;
        let (profiler_handle, _) = profiler
            .spawn(
                profiler_auth,
                SpawnOpts {
                    protocol: Some(Ip::V4),
                },
            )
            .await?;

        profilers.push(profiler_handle);
    }

    let mut handles = vec![];
    for profiler in profilers {
        let counter_handle = counter_handle.clone();
        let our_auth = our_auth.clone();
        let handle = tokio::spawn(async move {
            profiler
                .send_toplevel::<ActorHandle, (), CounterError>(
                    MessageType::TaskRequest(counter_handle.clone()),
                    our_auth,
                )
                .await
        });

        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.await;
    }

    let response = counter_handle
        .send_toplevel::<CounterMessage, usize, CounterError>(
            MessageType::TaskRequest(CounterMessage::Get),
            our_auth.clone(),
        )
        .await
        .expect("failed to send message!");

    if let TaskResult::Finished(n) = response {
        assert_eq!(NUM_TASKS * NUM_ACTORS, n);
    } else {
        panic!("query failed!");
    }

    Ok(())
}
