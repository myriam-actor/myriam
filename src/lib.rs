#![doc = include_str!("../README.md")]

pub mod actors;
pub mod address;
pub mod auth;
mod crypto;
pub mod identity;
pub mod messaging;
mod net;

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use serde::{Deserialize, Serialize};
    use thiserror::Error;
    use tracing_subscriber::EnvFilter;

    use crate::{
        actors::{Actor, ActorOptions, Context},
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
            _address_store: &AddressStore,
        ) -> AccessResolution {
            let addr = request.address.to_string();
            if addr == "127.0.0.1" || addr == "::1" {
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
                Err(SomeError::Empty)
            } else {
                Ok(arg.to_uppercase())
            }
        }
    }

    #[tokio::test]
    async fn spawn_and_message() {
        let filter = EnvFilter::from_default_env();
        tracing_subscriber::fmt().with_env_filter(filter).init();

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

        let actor_handle = MyActor::spawn(opts, actor_auth_handle.clone())
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
}
