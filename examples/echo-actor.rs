//!
//! Example independant actor.
//!
//! Make sure to run the keygen example first so we can read the necessary keys.
//!

use async_trait::async_trait;
use libp2p::multiaddr::Protocol;
use myriam::{
    actors::{
        auth::{AccessRequest, AccessResolution, AddrStore, AuthActor, PeerStore},
        Actor, Context,
    },
    net::keys::read_keyfile,
};
use serde::{Deserialize, Serialize};

struct LocalAuth;

#[async_trait]
impl AuthActor for LocalAuth {
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

#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[error("something went wrong")]
struct SomeError;

struct EchoActor;

#[async_trait]
impl Actor for EchoActor {
    type Input = String;

    type Output = String;

    type Error = SomeError;

    async fn handle(
        &mut self,
        msg: Self::Input,
        _: Context<Self::Output, Self::Error>,
    ) -> Result<Self::Output, Self::Error> {
        Ok(msg.chars().rev().collect())
    }

    async fn on_init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    async fn on_stop(&mut self) {}
}

#[tokio::main]
async fn main() {
    let keypair = read_keyfile("actor-secret.key")
        .await
        .expect("failed to read actor's keyfile");

    let auth = AuthActor::spawn(Box::new(LocalAuth), keypair).await;

    let actor = Box::new(EchoActor);
    let (handle, task) = actor
        .spawn(auth, Default::default())
        .await
        .expect("failed to spawn actor");

    println!("Address: {}", handle.addr);
    println!("PeerId: {}", handle.peer);
    println!("Actor started! Run a client on another terminal with our address and peer id.");

    let _ = task.await;
}
