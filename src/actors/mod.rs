use crate::address::Address;

pub mod local;
pub mod remote;

///
/// Struct passed to [Actor::handle] containing the address of itself.
///
#[derive(Debug, Clone)]
pub struct Context {
    pub self_address: Address,
}

pub struct ActorOptions {
    pub host: String,
    pub port: Option<u16>,
    pub read_timeout: Option<u64>,
}
