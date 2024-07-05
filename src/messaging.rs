use serde::{Deserialize, Serialize};

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

#[derive(Debug, Serialize, Deserialize, thiserror::Error)]
pub enum MsgError<Error: std::error::Error> {
    #[error("failed to send message through channel")]
    Send,

    #[error("failed to receive response from channel")]
    Recv,

    #[error("message processed but task failed")]
    Task(#[from] Error),
}
