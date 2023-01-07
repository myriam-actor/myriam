//!
//! Generate a keypair for using with client-server examples
//!

use myriam::prelude::*;

const ACTOR_KEYFILE_NAME: &str = "actor-secret.key";
const CLIENT_KEYFILE_NAME: &str = "client-secret.key";

#[tokio::main]
async fn main() {
    let actor_keypair = Keypair::generate_ed25519();
    dump_keypair(&actor_keypair, ACTOR_KEYFILE_NAME)
        .await
        .expect("failed to dump actor keyfile");

    let client_keypair = Keypair::generate_ed25519();
    dump_keypair(&client_keypair, CLIENT_KEYFILE_NAME)
        .await
        .expect("failed to dump client keyfile");
}
