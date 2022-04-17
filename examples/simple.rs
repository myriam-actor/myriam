use async_trait::async_trait;
use myriam::{
    actors::{self, Actor, ActorHandle, ActorOptions, Context},
    address::Address,
    auth::{AccessRequest, AccessResolution, AddressStore, AuthActor, IdentityStore},
    identity::{PublicIdentity, SelfIdentity},
    messaging::{MessageContext, MessageType, TaskResult},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct SomeError;

struct Kawaiifier {
    postfix: String,
}

impl Kawaiifier {
    fn new(postfix: String) -> Self {
        Self { postfix }
    }
}

#[async_trait]
impl Actor for Kawaiifier {
    type Input = String;
    type Output = String;
    type Error = SomeError;

    async fn handle(
        &self,
        _ctx: &Context,
        _addr: Option<Address>,
        arg: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        Ok(format!("{}-{}", arg, self.postfix))
    }
}

struct Authenticator;

#[async_trait]
impl AuthActor for Authenticator {
    async fn handle(
        request: AccessRequest,
        id_store: &IdentityStore,
        _address_store: &AddressStore,
    ) -> AccessResolution {
        if id_store.contains_key(&request.identity.hash()) {
            AccessResolution::Accepted
        } else {
            AccessResolution::Denied
        }
    }
}

async fn spawn(sender_id: PublicIdentity) -> ActorHandle {
    let self_id = SelfIdentity::new();
    let handle = Authenticator::spawn(self_id).await;
    handle
        .store_identity(sender_id)
        .await
        .expect("failed to insert sender public identity in store");

    let opts = ActorOptions {
        host: "::1".to_string(),
        port: None,
        read_timeout: None,
    };

    let actor = Kawaiifier::new("nya".into());
    actors::spawn(Box::new(actor), opts, handle)
        .await
        .expect("failed to spawn actor")
}

#[tokio::main]
async fn main() {
    // create the identity for this host
    let client_id = SelfIdentity::new();
    let client_public_id = client_id.public_identity().clone();

    // spawn an instance of our authenticator
    let auth_handle = Authenticator::spawn(client_id).await;

    // pass our public id to the actor on some other machine and get a handle to the actor
    let actor_handle = spawn(client_public_id.clone()).await;

    // store the public identity of the actor
    auth_handle
        .store_identity(actor_handle.identity.clone())
        .await
        .expect("failed to insert actor public identity in store");

    let result = actor_handle
        .send::<String, String, SomeError>(
            MessageType::Task("something".into()),
            MessageContext::Yielding,
            &auth_handle,
        )
        .await
        .expect("failed to get a result");

    match result {
        TaskResult::Accepted => panic!("expected a value!"),
        TaskResult::Finished(s) => println!("Result from actor response: {s}"),
    }
}
