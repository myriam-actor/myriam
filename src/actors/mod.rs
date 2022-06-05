pub mod local;
pub mod remote;

pub struct ActorOptions {
    pub host: String,
    pub port: Option<u16>,
    pub read_timeout: Option<u64>,
}
