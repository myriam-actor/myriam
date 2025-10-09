#![cfg(feature = "tor")]

//!
//! TODO
//!

use std::fmt::Display;

use myriam::actors::Actor;
use serde::{Deserialize, Serialize};

#[ignore]
#[tokio::test]
async fn roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    todo!()
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
