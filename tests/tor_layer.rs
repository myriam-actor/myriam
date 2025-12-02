#![cfg(feature = "tor")]

//!
//! Tor roundtrip test
//! * extremely slow since we have to establish two separate Tor circuits, so ignored by default
//!

use std::fmt::Display;

use myriam::{
    actors::{
        remote::{
            dencoder::bincode::BincodeDencoder,
            netlayer::tor_layer::TorLayer,
            router::{RemoteHandle, Router, RouterOpts},
            spawn_untyped,
        },
        Actor,
    },
    messaging::{Message, Reply},
};
use serde::{Deserialize, Serialize};

#[ignore]
#[tokio::test]
async fn roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().init();

    let tor_layer = TorLayer::new("actor-1".to_string(), 2050).await?;
    let (_, untyped) = spawn_untyped::<_, _, _, BincodeDencoder>(Mult { a: 15 }).await?;

    let router_opts = RouterOpts::new(60_000, 5_000);
    let router_handle = Router::with_netlayer(tor_layer, Some(router_opts)).await?;
    let address = router_handle.attach(untyped).await?;

    tracing::info!("our address is {address}");

    let tor_layer = TorLayer::new("actor-2".to_string(), 2051).await?;
    let remote_handle =
        RemoteHandle::<u32, u32, SomeError, BincodeDencoder, TorLayer>::new(&address, tor_layer);

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
