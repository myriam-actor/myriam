//!
//! models for local and remote messaging
//!

use std::fmt::Display;

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

    /// task request requiring mutation
    TaskMut(Input),

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
#[derive(Debug)]
#[cfg_attr(feature = "remote", derive(Serialize, Deserialize))]
pub enum MsgError<Error> {
    Send(String),
    Recv(String),
    Task(Error),
    NotAllowed,
}

impl<E> Display for MsgError<E>
where
    E: Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MsgError::Send(ctx) => write!(f, "failed to send message: {ctx}"),
            MsgError::Recv(ctx) => write!(f, "failed to receive message: {ctx}"),
            MsgError::Task(err) => write!(f, "task failed: {err}"),
            MsgError::NotAllowed => write!(f, "message not allowed"),
        }
    }
}

impl<E> std::error::Error for MsgError<E> where E: Display + core::fmt::Debug {}
