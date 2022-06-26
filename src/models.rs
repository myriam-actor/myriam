//!
//! Common models (and their implementations) used throughout myriam
//!

use serde::{Deserialize, Serialize};

///
/// Message to an actor
///
#[derive(Debug, Serialize, Deserialize)]
pub struct Message<T> {
    /// possible type of message
    pub message_type: MessageType<T>,
}

///
/// Possible types of messages an actor is capable of handling
///
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType<T> {
    /// "Ping" message, useful for health checks
    Ping,

    /// Signal an actor to stop itself
    Stop,

    /// Carry out a task and send a result back
    TaskRequest(T),

    /// Carry out a task and comfirm it has accepted it -- we don't care about the result
    Task(T),
}

///
/// Result of a successful message
///
#[derive(Debug, Serialize, Deserialize)]
pub enum TaskResult<T> {
    /// The message was accepted, no concrete value returned
    Accepted,

    /// The task requested has finished and a value was returned
    Finished(T),
}

///
/// Possible errors that could arise at any stage of communication with an actor
///
#[derive(Debug, Serialize, Deserialize)]
pub enum MessagingError<E> {
    /// The message was accepted, but the task itself returned an error
    Task(E),

    /// Communication with AuthHandle failed
    AuthHandle,

    /// We couldn't connect to the remote actor
    Dial,

    /// We failed to send a message
    Send,

    /// A message was sent, but we failed to receive a response
    Receive,

    /// An internal, unrecoverable error arised
    Internal,

    /// Requester is unauthorized to perform the task solicited
    Unauthorized,

    /// Requester has been marked as abusive and consequently banned
    Banned,

    /// Incorrect type for {De-}Serialization
    Serialize,
}

///
/// Type alias for the response of an actor
///
pub type MessageResult<T, E> = Result<TaskResult<T>, MessagingError<E>>;

pub(crate) type RawInput = Vec<u8>;
pub(crate) type RawOutput = Vec<u8>;
