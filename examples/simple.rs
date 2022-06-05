//!
//! "Simple" example of a minimal actor which mutates its own state
//!

use async_trait::async_trait;
use myriam::{
    actors::{self, local::Actor, remote::ActorHandle},
    address::Address,
    auth::{AccessRequest, AccessResolution, AddressStore, AuthActor, IdentityStore},
    identity::{PublicIdentity, SelfIdentity},
    messaging::{MessageContext, MessageType, TaskResult},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct CounterError;

struct Counter {
    count: i32,
    address: Address,
}

impl Counter {
    fn new(init: i32) -> Self {
        Self {
            count: init,
            address: Address::new_with_random_port("::1").expect("failed to parse address"),
        }
    }
}

#[async_trait]
impl Actor for Counter {
    type Input = i32;
    type Output = i32;
    type Error = CounterError;

    async fn handle(
        &mut self,
        _addr: Option<Address>,
        arg: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        self.count += arg;
        Ok(self.count)
    }

    fn get_self(&self) -> Address {
        self.address.clone()
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

    let actor = Counter::new(0);
    let opts = actor.spawn_options(None);

    actors::remote::spawn(Box::new(actor), opts, handle)
        .await
        .expect("failed to spawn actor")
        .0
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
        .send::<i32, i32, CounterError>(
            MessageType::Task(11),
            MessageContext::Yielding,
            &auth_handle,
        )
        .await
        .expect("failed to get a result");

    match result {
        TaskResult::Accepted => panic!("expected a value!"),
        TaskResult::Finished(s) => assert_eq!(11, s),
    }
}
