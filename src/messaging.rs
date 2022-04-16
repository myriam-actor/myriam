use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::address::Address;

///
/// Type to specify whether or not a message to an actor is expected to yield a value or not.
///
#[derive(Serialize, Deserialize)]
pub enum MessageContext {
    Yielding,
    NonYielding,
}

///
/// Types of message an actor accepts.
///
#[derive(Serialize, Deserialize)]
pub enum MessageType<T> {
    Ping,
    Stop,
    Task(T),
}

///
/// Actual type an actor will receive and handle. `sender` is optional because a message
/// _can_ be sent from toplevel, as opposed to sending from within another actor.
///
#[derive(Serialize, Deserialize)]
pub struct Message<T> {
    pub context: MessageContext,
    pub message_type: MessageType<T>,
    pub sender: Option<Address>,
}

///
/// Result of a successful message handled by an actor.
///
#[derive(Serialize, Deserialize)]
pub enum TaskResult<T> {
    Accepted,
    Finished(T),
}

///
/// Result of a failed message in any stage.
///
#[derive(Serialize, Deserialize, Error)]
pub enum MessagingError<E> {
    #[error("could not send data to actor")]
    Send,

    #[error("could not receive data from actor")]
    Recv,

    #[error("task failed: {0}")]
    Task(E),

    #[error("failed to establish or maintain connection to actor")]
    Transport,

    #[error("internal error inside actor")]
    Internal,
}

pub type MessageResult<T, E> = Result<TaskResult<T>, MessagingError<E>>;
