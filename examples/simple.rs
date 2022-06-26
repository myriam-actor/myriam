//!
//! "Simple" example of a minimal actor which mutates its own state
//!

use async_trait::async_trait;
use libp2p::{identity::Keypair, multiaddr::Protocol};
use myriam::{
    actors::{
        auth::{AccessRequest, AccessResolution, AddrStore, AuthActor, PeerStore},
        opts::SpawnOpts,
        Actor, Context,
    },
    models::{MessageType, TaskResult},
};
use serde::{Deserialize, Serialize};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[error("failed to initialize counter")]
struct CounterError;

struct Counter {
    count: i32,
}

impl Counter {
    fn new(init: i32) -> Self {
        Self { count: init }
    }
}

#[async_trait]
impl Actor for Counter {
    type Input = i32;
    type Output = i32;
    type Error = CounterError;

    async fn handle(
        &mut self,
        arg: Self::Input,
        _: Context<Self::Output, Self::Error>,
    ) -> Result<Self::Output, Self::Error> {
        self.count += arg;

        Ok(self.count)
    }

    async fn on_init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.count != 0 {
            Err(Box::new(CounterError))
        } else {
            Ok(())
        }
    }

    async fn on_stop(&self) {}
}

struct Autho;

#[async_trait]
impl AuthActor for Autho {
    async fn resolve(
        &mut self,
        request: AccessRequest,
        _: &mut AddrStore,
        _: &mut PeerStore,
    ) -> AccessResolution {
        //
        // only accept incoming requests from this machine
        //
        if let Some(address) = request.addr {
            if address.iter().any(|p| match p {
                Protocol::Ip4(a) => a.is_loopback(),
                Protocol::Ip6(a) => a.is_loopback(),
                _ => false,
            }) {
                AccessResolution::Accepted
            } else {
                AccessResolution::Denied
            }
        } else {
            AccessResolution::Denied
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let filter = EnvFilter::from_default_env();
    tracing_subscriber::fmt().with_env_filter(filter).init();

    // create the keypair for this host
    let keypair = Keypair::generate_ed25519();

    // spawn an instance of our authenticator
    let auth_handle = AuthActor::spawn(Box::new(Autho), keypair).await;

    // spawn an actor using our authorization actor
    // share this handle however you want
    let actor = Box::new(Counter::new(0));
    let (actor_handle, _) = actor
        .spawn(auth_handle.clone(), SpawnOpts::default())
        .await?;

    //
    // ... now, on another machine ...
    //

    let client_keypair = Keypair::generate_ed25519();
    let client_auth_handle = AuthActor::spawn(Box::new(Autho), client_keypair).await;

    let result = actor_handle
        .send_toplevel::<i32, i32, CounterError>(MessageType::TaskRequest(11), client_auth_handle)
        .await
        .expect("failed to get a result");

    match result {
        TaskResult::Accepted => panic!("expected a value!"),
        TaskResult::Finished(s) => {
            assert_eq!(11, s);
            tracing::info!("Response is: {s}");
        }
    }

    Ok(())
}
