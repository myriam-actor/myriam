//!
//! Example actor consumer.
//!
//! Make sure to run the keygen example first so we can read the necessary keys.
//!

use async_trait::async_trait;
use myriam::{
    actors::remote::ActorHandle,
    address::Address,
    auth::{AccessResolution, AuthActor},
    identity::{PublicIdentity, SelfIdentity},
    messaging::{MessageContext, MessageType, TaskResult},
};
use serde::{Deserialize, Serialize};

struct LocalAuth;

// ideally, this would be part of a shared lib instead of copy-pasted here
#[derive(Debug, Serialize, Deserialize)]
struct SomeError;

#[async_trait]
impl AuthActor for LocalAuth {
    async fn handle(
        request: myriam::auth::AccessRequest,
        _id_store: &myriam::auth::IdentityStore,
        _address_store: &myriam::auth::AddressStore,
    ) -> myriam::auth::AccessResolution {
        let addr = request.address.to_string();
        if addr == "127.0.0.1" || addr == "::1" || addr == "::0" {
            AccessResolution::Accepted
        } else {
            AccessResolution::Denied
        }
    }
}

#[tokio::main]
async fn main() {
    let args = std::env::args();
    let message = args.skip(1).fold(String::new(), |res, s| res + " " + &s);

    let self_id = SelfIdentity::read_from_file("client-secret.key".to_string())
        .await
        .expect("failed to read secret identity from keyfile");

    let auth = LocalAuth::spawn(self_id).await;

    let actor_public_id = PublicIdentity::read_from_file("actor-public.key".to_string())
        .await
        .expect("failed to read client public identity from file");

    auth.store_identity(actor_public_id.clone())
        .await
        .expect("failed to insert public identity in store");

    let handle = ActorHandle {
        address: Address::new("::1", 3050),
        identity: actor_public_id,
    };

    let result = handle
        .send::<String, String, SomeError>(
            MessageType::Task(message),
            MessageContext::Yielding,
            &auth,
        )
        .await
        .expect("failed to receive response from actor");

    if let TaskResult::Finished(response) = result {
        println!("Response: {response}");
    } else {
        eprintln!("Failed to receive response from actor...");
    }
}
