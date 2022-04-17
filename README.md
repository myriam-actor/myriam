# myriam-rs

Minimal stateless remote actors with e2e encryption.

# Usage

## Configuration

* Message size cap (in bytes): can be set with via env var `MYRIAM_MAX_MSG_SIZE`. Default is `8_388_608`.
* Message recv timeout (in milliseconds): can be set via `ActorOpts` when spawning or globally with the env var `MYRIAM_READ_TIMEOUT`.

# Example

```rust
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use myriam::{
    actors::{Actor, ActorOptions, Context},
    address::Address,
    auth::{AccessRequest, AccessResolution, AddressStore, AuthActor, IdentityStore},
    identity::SelfIdentity,
    messaging::{MessageContext, MessageType, MessagingError, TaskResult},
};

struct Autho;

#[async_trait]
impl AuthActor for Autho {
    async fn handle(
        request: AccessRequest,
        _id_store: &IdentityStore,
        address_store: &AddressStore,
    ) -> AccessResolution {
        if address_store.contains(&request.address) {
            AccessResolution::Accepted
        } else {
            AccessResolution::Denied
        }
    }
}

#[derive(Serialize, Deserialize)]
struct SomeError;

struct MyActor;

#[async_trait]
impl Actor for MyActor {
    type Input = String;
    type Output = String;
    type Error = SomeError;

    async fn handle(
        _ctx: &Context,
        _addr: Option<Address>,
        arg: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        if arg.is_empty() {
            Err(SomeError)
        } else {
            Ok(arg.to_uppercase())
        }
    }
}

async fn foo() {
    // Machine A
    let actor_self_identity = SelfIdentity::new();
    let actor_auth_handle = Autho::spawn(actor_self_identity).await;

    // ... populate auth actor with addresses/identities ...

    let opts = ActorOptions {
        host: "::1".into(),
        port: None,
        read_timeout: None
    };

    // suppose we somehow publish this handle in an service for discoverability.
    let actor_handle = MyActor::spawn(opts, actor_auth_handle.clone())
        .await
        .unwrap();

    // now in machine B, with a handle to the actor.
    let client_self_identity = SelfIdentity::new();
    let client_auth_handle = Autho::spawn(client_self_identity).await;

    // ... pupulate the client auth_handle with (at least) the remote actor's identity ...

    let _response = actor_handle
        .send::<String, String, SomeError>(
            MessageType::Task("something".to_string()),
            MessageContext::Yielding,
            &client_auth_handle,
        )
        .await;

    assert!(matches!(
        Ok::<_, MessagingError<SomeError>>(TaskResult::Finished("SOMETHING")),
        _response
    ));
}
```

# License

(c) Ariela Wenner - 2022

Licensed under the [Apache License 2.0](LICENSE).