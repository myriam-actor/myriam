# myriam-rs

Local and remote actor model implementation with an API heavily inspired (but not necessarily equivalent) in [Goblins](https://gitlab.com/spritely/guile-goblins).

```rust
use std::time::Duration;

use serde::{Deserialize, Serialize};
use myriam::{
    actors::{
        Actor,
        remote::{
            self,
            dencoder::bincode::BincodeDencoder,
            netlayer::tcp_layer::TcpNetLayer,
            router::{RemoteHandle, Router, RouterOpts},
        },
    },
    messaging::{Message, Reply},
};

struct Mult {
    pub a: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[error("uh oh")]
struct SomeError;

impl Actor<u32, u32, SomeError> for Mult {
    async fn handler(&mut self, input: u32) -> Result<u32, SomeError> {
        Ok(input * self.a)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // LocalHandle allows for local, type-safe communication
    // UntypedHandle tries to do the same, but requests and responses are
    // bag of bytes and have to be {de}serialized
    let (local_handle, untyped_handle)
        = remote::spawn_untyped::<_, _, _, BincodeDencoder>(Mult { a: 3 }).await?;
    
    // create router with a TOR netlayer
    let router_handle = Router::with_netlayer(TcpNetLayer::new(), Some(RouterOpts::default())).await?;

    // routers handle external access to several attached actors
    // we can think of this exposed actor as a capability
    // "tor:0139aa9b4d523e1da515ce21a818e579acd005fbd0aea62ef094ac1b845f99e7@someaddress.onion"
    let address = router_handle.attach(untyped_handle).await?;

    let remote_handle
        = RemoteHandle::<u32, u32, SomeError, BincodeDencoder, TcpNetLayer>::new(&address, TcpNetLayer::new());
    //                     type handle once ^

    // use RemoteHandle just like a LocalHandle
    let res = remote_handle.send(Message::Task(42)).await?;

    // capabilities can be revoked anytime
    router_handle.revoke(&address).await?;

    tokio::time::sleep(Duration::from_millis(100));

    // ...and thus we can't invoke this one anymore
    remote_handle.send(Message::Ping).await.unwrap_err();

    Ok(())
}
```

# License

Copyright 2024 Ariela Wenner

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at <http://www.apache.org/licenses/LICENSE-2.0>

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
