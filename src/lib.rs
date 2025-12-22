//!
//! Local and remote actor model implementation with an API heavily inspired in (but not necessarily equivalent to) [Goblins](https://gitlab.com/spritely/guile-goblins).
//!
//! ```rust, no_run
//! # use std::time::Duration;
//! # use std::fmt::Display;
//! #
//! # use serde::{Deserialize, Serialize};
//! # use myriam::{
//! #    actors::{
//! #        Actor,
//! #        remote::{
//! #            self,
//! #            dencoder::bitcode::BitcodeDencoder,
//! #            netlayer::tor_layer::TorLayer,
//! #            router::{RemoteHandle, Router, RouterOpts},
//! #        },
//! #    },
//! #    messaging::{Message, Reply},
//! # };
//! #
//! struct Mult {
//!     pub a: u32,
//! }
//!
//! # #[derive(Debug, Clone, Serialize, Deserialize)]
//! # struct SomeError;
//! #
//! # impl Display for SomeError {
//! #     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//! #         write!(f, "uh oh")
//! #     }
//! # }
//! #
//! impl Actor<u32, u32, SomeError> for Mult {
//!     async fn handler(&self, input: u32) -> Result<u32, SomeError> {
//!         Ok(input * self.a)
//!     }
//! }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // LocalHandle allows for local, type-safe communication
//! // UntypedHandle tries to do the same, but requests and responses are
//! // bag of bytes and have to be {de}serialized
//! let (local_handle, untyped_handle)
//!     = remote::spawn_untyped::<_, _, _, BitcodeDencoder>(Mult { a: 3 }).await?;
//!
//! // create router with a TOR netlayer
//! let layer = TorLayer::new("myriam-foo".to_string(), 8081).await?;
//!
//! let router_handle = Router::with_netlayer(layer, Some(RouterOpts::default())).await?;
//!
//! // routers handle external access to several attached actors
//! // we can think of this exposed actor as a capability
//! // "tor:4ruu43hmgibt5lgg3cvghbrmprotl5m7ts2lral5wnhf5wwkocva@someaddress.onion"
//! let address = router_handle.attach(untyped_handle).await?;
//!
//! let new_layer = TorLayer::new("myriam-bar".to_string(), 8082).await?;
//!
//! let remote_handle
//!     = RemoteHandle::<u32, u32, SomeError, BitcodeDencoder, TorLayer>::new(&address, new_layer);
//! //                     type handle once ^
//!
//! // use RemoteHandle just like a LocalHandle
//! let res = remote_handle.send(Message::Task(42)).await?;
//!
//! // capabilities can be revoked anytime
//! router_handle.revoke(&address).await?;
//!
//! tokio::time::sleep(Duration::from_millis(100));
//!
//! // ...and thus we can't invoke this one anymore
//! remote_handle.send(Message::Ping).await.unwrap_err();
//! #
//! #     Ok(())
//! # }
//! ```
//!
//! check out the [repo examples](https://codeberg.org/arisunz/myriam-rs/src/branch/main/examples) for a more comprehensive demo.
//!
//! ## features
//!
//! * `remote (default)`: support for remote messaging
//! * `tcp (default)`: TCP test-only net layer
//! * `tor (default)`: Tor net layer - requires a running and properly configured Tor router
//!
//! # license
//!
//! Copyright 2024 Ariela Wenner
//!
//! Licensed under the Apache License, Version 2.0 (the "License");
//! you may not use this file except in compliance with the License.
//! You may obtain a copy of the License at <http://www.apache.org/licenses/LICENSE-2.0>
//!
//! Unless required by applicable law or agreed to in writing, software
//! distributed under the License is distributed on an "AS IS" BASIS,
//! WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//! See the License for the specific language governing permissions and
//! limitations under the License.
//!
#![warn(missing_debug_implementations)]
#![warn(missing_docs)]

pub mod actors;
pub mod messaging;

pub(crate) mod utils;

#[cfg(test)]
#[allow(unused)]
mod tests {
    use tokio::sync::OnceCell;

    static TRACING: OnceCell<()> = OnceCell::const_new();

    pub(crate) async fn init_tracing() {
        TRACING
            .get_or_init(|| async {
                tracing_subscriber::fmt::init();
            })
            .await;
    }
}
