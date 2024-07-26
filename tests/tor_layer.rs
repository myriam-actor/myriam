#![cfg(feature = "tor")]

//!
//! To run:
//! 1. create the /tmp/myriam/test directory
//! 2. give it the appropriate permissions, e.g. with `chmod -R 700 /tmp/myriam/test`
//! 3. start tor with the torrc provided in this directory, like `tor -f tests/torrc`
//!

use std::fmt::Display;

use myriam::{
    actors::{
        remote::{
            self,
            dencoder::bincode::BincodeDencoder,
            netlayer::tor_layer::TorNetLayer,
            router::{RemoteHandle, Router, RouterOpts},
        },
        Actor,
    },
    messaging::{Message, Reply},
};
use serde::{Deserialize, Serialize};

#[ignore]
#[tokio::test]
async fn roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    let layer =
        TorNetLayer::new_for_service("127.0.0.1.9050", "127.0.0.1:8080", "/tmp/myriam/test")
            .await?;

    let (_, handle) = remote::spawn_untyped::<_, _, _, BincodeDencoder>(Mult { a: 3 })
        .await
        .unwrap();

    let router = Router::with_netlayer(
        layer,
        Some(RouterOpts {
            msg_read_timeout: 60_000,
            ..Default::default()
        }),
    )
    .await
    .unwrap();

    let addr = router.attach(handle).await.unwrap();

    let client_layer = TorNetLayer::new("127.0.0.1:9050", "/tmp/myrian/test");

    let remote =
        RemoteHandle::<u32, u32, SomeError, BincodeDencoder, TorNetLayer>::new(&addr, client_layer);

    let res = remote.send(Message::Task(5)).await.unwrap();
    assert!(matches!(res, Ok(Reply::Task(15))));

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
