//!
//! Example actor consumer.
//!
//! Make sure to run the keygen example first so we can read the necessary keys.
//!
//! Usage: cargo run --example echo-client -- <address> <Peer ID> <message>
//!

use async_trait::async_trait;
use libp2p::multiaddr::Protocol;
use myriam::{
    actors::{
        auth::{AccessRequest, AccessResolution, AddrStore, AuthActor, PeerStore},
        ActorHandle,
    },
    models::{MessageType, TaskResult},
    net::keys::read_keyfile,
};
use serde::{Deserialize, Serialize};

struct LocalAuth;

// ideally, this would be part of a shared lib instead of copy-pasted here
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[error("Something went wrong")]
struct SomeError;

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

#[tokio::main]
async fn main() {
    let mut args = std::env::args();

    let addr = args.nth(1).expect("missing address");
    let peer_id = args.next().expect("missing peer id");
    let message = args.collect();

    println!("dialing addr: {addr}");
    println!("dialing peer_id: {peer_id}");

    let keypair = read_keyfile("client-secret.key")
        .await
        .expect("failed to read keyfile");
    let auth = AuthActor::spawn(Box::new(LocalAuth), keypair).await;

    let handle = ActorHandle {
        addr: addr.parse().expect("failed to parse given address"),
        peer: peer_id.parse().expect("failed to parse peer id"),
    };

    let result = handle
        .send_toplevel::<String, String, SomeError>(MessageType::TaskRequest(message), auth)
        .await
        .expect("failed to receive response from actor");

    if let TaskResult::Finished(response) = result {
        println!("Response: {response}");
    } else {
        eprintln!("Failed to receive response from actor...");
    }
}
