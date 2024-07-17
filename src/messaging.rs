//!
//! models for local and remote messaging
//!

#[cfg(feature = "remote")]
use serde::{Deserialize, Serialize};

///
/// actor message
///
#[derive(Debug)]
#[cfg_attr(feature = "remote", derive(Serialize, Deserialize))]
pub enum Message<Input> {
    /// task request with known input
    Task(Input),

    /// ping this actor for liveness
    Ping,

    /// stop this actor
    Stop,
}

///
/// message reply from actor
///
#[derive(Debug)]
#[cfg_attr(feature = "remote", derive(Serialize, Deserialize))]
pub enum Reply<Output> {
    /// message processed, no output returned
    Accepted,

    /// task output
    Task(Output),
}

///
/// [`Result`] wrapped over [`Reply`] and [`MsgError`], returned by send operations.
///
pub type MsgResult<Output, Error> = Result<Reply<Output>, MsgError<Error>>;

///
/// errors that could arise from actor messaging
///
#[allow(missing_docs)]
#[derive(Debug, thiserror::Error)]
#[cfg_attr(feature = "remote", derive(Serialize, Deserialize))]
pub enum MsgError<Error: std::error::Error> {
    #[error("failed to send message through channel")]
    Send,

    #[error("failed to receive response from channel")]
    Recv,

    #[error("message processed but task failed")]
    Task(#[from] Error),
}
