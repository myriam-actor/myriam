# myriam-rs

Remote actors with e2e encryption.

Displaimer: This API is a. EXTREMELY unstable at the moment and b. not guaranteed to be secure. Please don't even think of letting this get close to a production system, or anything you even remotely care about for that matter.

# Usage

## Configuration

* Message size cap (in bytes): can be set with via env var `MYRIAM_MAX_MSG_SIZE`. Default is `8_388_608`.
* Message recv timeout (in milliseconds): can be set via `ActorOpts` when spawning or globally with the env var `MYRIAM_READ_TIMEOUT`.

# Example

```rust
    use async_trait::async_trait;
    use serde::{Deserialize, Serialize};
    use thiserror::Error;

    use myriam::{
        actors::{self, Actor, ActorOptions, Context},
        address::Address,
        auth::{AccessRequest, AccessResolution, AddressStore, AuthActor, IdentityStore},
        identity::SelfIdentity,
        messaging::{MessageContext, MessageType, TaskResult},
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

    #[derive(Serialize, Deserialize, Debug, Error)]
    enum SomeError {
        #[error("empty input")]
        Empty,
    }

    struct MyActor;
    impl MyActor {
        fn new() -> Self {
            Self
        }
    }

    #[async_trait]
    impl Actor for MyActor {
        type Input = String;
        type Output = String;
        type Error = SomeError;

        async fn handle(
            &self,
            _ctx: &Context,
            _addr: Option<Address>,
            arg: Self::Input,
        ) -> Result<Self::Output, Self::Error> {
            if arg.is_empty() {
                Err(SomeError::Empty)
            } else {
                Ok(arg.to_uppercase())
            }
        }
    }

    async fn example() {
        let actor_self_identity = SelfIdentity::new();
        let actor_auth_handle = Autho::spawn(actor_self_identity.clone()).await;

        let client_self_identity = SelfIdentity::new();
        let client_auth_handle = Autho::spawn(client_self_identity.clone()).await;

        actor_auth_handle
            .store_identity(client_self_identity.public_identity().clone())
            .await
            .unwrap();

        client_auth_handle
            .store_identity(actor_self_identity.public_identity().clone())
            .await
            .unwrap();

        let opts = ActorOptions {
            host: "::1".into(),
            port: None,
            read_timeout: Some(2000),
        };

        let actor = MyActor::new();
        let actor_handle = actors::spawn(Box::new(actor), opts, actor_auth_handle.clone())
            .await
            .unwrap();

        let response = actor_handle
            .send::<String, String, SomeError>(
                MessageType::Task("something".to_string()),
                MessageContext::Yielding,
                &client_auth_handle,
            )
            .await;

        match response {
            Ok(res) => {
                if let TaskResult::Finished(s) = res {
                    tracing::info!("response: {s}");
                } else {
                    panic!("expected a value in result, only got confirmation");
                }
            }
            Err(err) => panic!("could not get response: {err}"),
        }
    }

```

# Caveats/TODOs

* This is all far from being ergonomic, despite simplicity and ease of use being my main focus. If you have any suggestions for improvements, please open an issue!

# License

(c) Ariela Wenner - 2022

Licensed under the [Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0.txt).