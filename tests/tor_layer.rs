#![cfg(feature = "tor")]

//!
//! Tor roundtrip test
//! * extremely slow since we have to establish two separate Tor circuits, so ignored by default
//!

use std::fmt::Display;

use myriam::{
    actors::{
        Actor,
        remote::{
            address::PeerId,
            dencoder::bitcode::BitcodeDencoder,
            netlayer::tor_layer::{TorLayer, TorLayerConfig, TorLayerDirectories},
            router::{RemoteHandle, Router, RouterOpts},
            spawn_untyped,
        },
    },
    messaging::{Message, Reply},
};
use serde::{Deserialize, Serialize};

#[ignore]
#[tokio::test]
async fn roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().init();

    let peer_id = PeerId::new()?;
    let tor_dir = TorLayerDirectories::new(
        format!("/tmp/myriam/test/{peer_id}/state"),
        format!("/tmp/myriam/test/{peer_id}/cache"),
    );

    let tor_layer =
        TorLayer::new("actor-1".to_string(), TorLayerConfig::new(2050, tor_dir)).await?;
    let (_, untyped) = spawn_untyped::<_, _, _, BitcodeDencoder>(Mult { a: 15 }).await?;

    let router_opts = RouterOpts::new(60_000, 5_000);
    let router_handle = Router::with_netlayer(tor_layer, Some(router_opts)).await?;
    let address = router_handle.attach_with_id(untyped, peer_id).await?;

    tracing::info!("our address is {address}");

    let tor_layer = TorLayer::new_for_client("actor-2".to_string()).await?;
    let remote_handle =
        RemoteHandle::<u32, u32, SomeError, BitcodeDencoder, TorLayer>::new(&address, tor_layer);

    let response = remote_handle.send(Message::Task(3)).await??;
    assert!(matches!(response, Reply::Task(45)));

    Ok(())
}

struct Mult {
    pub a: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SomeError;

impl Display for SomeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "uh oh")
    }
}

impl Actor<u32, u32, SomeError> for Mult {
    async fn handler(&self, input: u32) -> Result<u32, SomeError> {
        Ok(input * self.a)
    }
}
