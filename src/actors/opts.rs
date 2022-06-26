//!
//! spawn options for actors
//!

///
/// Spawn options for remote actors
///
#[derive(Debug, Default)]
pub struct SpawnOpts {
    /// Protocol to use, defaults to IPv4
    pub protocol: Option<Ip>,
}

///
/// Specify whether we should use IPv4 or IPv6
///
#[derive(Debug)]
pub enum Ip {
    /// Listen in IPv4 address
    V4,

    /// Listen in IPv6 address
    V6,

    /// Listen in both
    Both,
}

impl Default for Ip {
    fn default() -> Self {
        Self::V4
    }

    // don't mind me, jusy sprinkling some ,*. Wishful Thinking '*,
    // fn default() -> Self {
    //     Self::V6
    // }
}
