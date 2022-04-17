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

    use crate::{
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

    #[tokio::test]
    async fn spawn_and_message() {
        let actor_self_identity = SelfIdentity::new();
        let actor_auth_handle = Autho::spawn(actor_self_identity).await;

        let opts = ActorOptions {
            host: "localhost".into(),
            port: None,
        };

        let actor_handle = MyActor::spawn(opts, actor_auth_handle.clone())
            .await
            .unwrap();

        let client_self_identity = SelfIdentity::new();
        let client_auth_handle = Autho::spawn(client_self_identity).await;

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
}
