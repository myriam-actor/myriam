//!
//! Generate a keypair for using with client-server examples
//!

use myriam::identity::SelfIdentity;

#[tokio::main]
async fn main() {
    let actor_id = SelfIdentity::new();
    let client_id = SelfIdentity::new();

    actor_id
        .dump_keyfile("actor-secret.key".to_string())
        .await
        .expect("falied to dump actor keyfile");

    actor_id
        .public_identity()
        .dump_keyfile("actor-public.key".to_string())
        .await
        .expect("failed to dump actor public keyfile");

    client_id
        .dump_keyfile("client-secret.key".to_string())
        .await
        .expect("failed to dump client keyfile");

    client_id
        .public_identity()
        .dump_keyfile("client-public.key".to_string())
        .await
        .expect("failed to dump client public keyfile");
}
