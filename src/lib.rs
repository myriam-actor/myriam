#![doc = include_str!("../README.md")]
#![allow(unused)]

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
        actors::{self, local::Actor, ActorOptions, Context},
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

    #[derive(Serialize, Deserialize, Debug, Error)]
    enum SomeError {
        #[error("empty input")]
        Empty,
    }

    struct MyActor {
        postfix: String,
    }

    impl MyActor {
        fn new() -> Self {
            Self {
                postfix: "nya".into(),
            }
        }
    }

    #[async_trait]
    impl Actor for MyActor {
        type Input = String;
        type Output = String;
        type Error = SomeError;

        async fn handle(
            &mut self,
            _ctx: Option<Context>,
            _addr: Option<Address>,
            arg: Self::Input,
        ) -> Result<Self::Output, Self::Error> {
            if arg.is_empty() {
                Err(SomeError::Empty)
            } else {
                Ok(format!("{}-{}", arg.to_uppercase(), &self.postfix))
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

        let actor = MyActor::new();
        let (actor_handle, _) =
            actors::remote::spawn(Box::new(actor), opts, actor_auth_handle.clone())
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
                    assert_eq!("SOMETHING-nya", s);
                } else {
                    panic!("expected a value in result, only got confirmation");
                }
            }
            Err(err) => panic!("could not get response: {err}"),
        }
    }

    #[tokio::test]
    async fn actually_stops() {
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
        let (actor_handle, _) =
            actors::remote::spawn(Box::new(actor), opts, actor_auth_handle.clone())
                .await
                .unwrap();

        let stop_response = actor_handle
            .send::<String, String, SomeError>(
                MessageType::Stop,
                MessageContext::Yielding,
                &client_auth_handle,
            )
            .await;

        assert!(matches!(
            stop_response,
            Result::<TaskResult<String>, MessagingError<SomeError>>::Ok(TaskResult::Accepted)
        ));

        let ping_response = actor_handle
            .send::<String, String, SomeError>(
                MessageType::Ping,
                MessageContext::Yielding,
                &client_auth_handle,
            )
            .await;

        assert!(matches!(
            ping_response,
            Result::<TaskResult<String>, MessagingError<SomeError>>::Err(MessagingError::Transport)
        ));
    }
}
