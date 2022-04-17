//!
//! Example independant actor.
//!
//! Make sure to run the keygen example first so we can read the necessary keys.
//!

use async_trait::async_trait;
use myriam::{
    actors::{self, Actor},
    auth::{AccessResolution, AuthActor},
    identity::{PublicIdentity, SelfIdentity},
};
use serde::{Deserialize, Serialize};

struct LocalAuth;

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

#[derive(Debug, Serialize, Deserialize)]
struct SomeError;

struct EchoActor;

#[async_trait]
impl Actor for EchoActor {
    type Input = String;

    type Output = String;

    type Error = SomeError;

    async fn handle(
        &self,
        _ctx: &myriam::actors::Context,
        _addr: Option<myriam::address::Address>,
        arg: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        println!("actor got {arg}, sending response!");

        Ok(arg.chars().rev().collect())
    }
}

#[tokio::main]
async fn main() {
    let self_id = SelfIdentity::read_from_file("actor-secret.key".to_string())
        .await
        .expect("failed to read secret identity from keyfile");

    let auth = LocalAuth::spawn(self_id).await;

    let client_public_id = PublicIdentity::read_from_file("client-public.key".to_string())
        .await
        .expect("failed to read client public identity from file");

    auth.store_identity(client_public_id)
        .await
        .expect("failed to insert public identity in store");

    let opts = actors::ActorOptions {
        host: "::1".into(),
        port: Some(3050),
        read_timeout: None,
    };

    let actor = EchoActor;
    let (_, task) = actors::spawn(Box::new(actor), opts, auth)
        .await
        .expect("failed to spawn actor");

    println!("Actor started! Run a client on another terminal.");

    let _ = task.await;
}
