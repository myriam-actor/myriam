//!
//! convenience method for Keypairs and keyfile manipulation
//!

use std::error::Error;

use libp2p::identity::Keypair;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

///
/// read a a file containing a protobuf dump of a Keypair
///
pub async fn read_keyfile(path: &str) -> Result<Keypair, Box<dyn Error>> {
    let mut buffer = vec![];
    let mut keyfile = tokio::fs::File::open(path).await?;
    keyfile.read_to_end(&mut buffer).await?;

    Ok(Keypair::from_protobuf_encoding(buffer.as_ref())?)
}

///
/// dump the given Keypair as protobuf encoding to a file
///
pub async fn dump_keypair(keypair: &Keypair, path: &str) -> Result<(), Box<dyn Error>> {
    let bytes = keypair.to_protobuf_encoding()?;
    let mut keyfile = tokio::fs::File::create(path).await?;
    keyfile.write_all(bytes.as_slice()).await?;

    Ok(())
}
