use serde::{Deserialize, Serialize};

pub type RawMessage = Vec<u8>;

pub type RawReply = Vec<u8>;

#[derive(Debug, Serialize, Deserialize)]
pub enum Message<Input> {
    Task(Input),
    Ping,
    Stop,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Reply<Output> {
    Accepted,
    Task(Output),
}

pub type MsgResult<Output, Error> = Result<Reply<Output>, MsgError<Error>>;

#[derive(Debug, thiserror::Error)]
pub enum MsgError<Error: std::error::Error> {
    #[error("failed to send message through channel")]
    Send,

    #[error("failed to receive response from channel")]
    Recv,

    #[error("message processed but task failed")]
    Task(#[from] Error),
}
