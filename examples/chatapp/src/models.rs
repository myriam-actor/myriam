use std::{fmt::Display, str::FromStr};

use myriam::actors::remote::address::ActorAddress;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct Report {
    kind: ReportKind,
    body: String,
}

impl Report {
    pub fn echo(msg: String) -> Self {
        Self {
            kind: ReportKind::Echo,
            body: msg,
        }
    }

    pub fn peer_msg(msg: String) -> Self {
        Self {
            kind: ReportKind::Peer,
            body: msg,
        }
    }

    pub fn info(msg: String) -> Self {
        Self {
            kind: ReportKind::Info,
            body: msg,
        }
    }

    pub fn error(msg: String) -> Self {
        Self {
            kind: ReportKind::Error,
            body: msg,
        }
    }

    pub fn kind(&self) -> &ReportKind {
        &self.kind
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub fn marker(&self) -> &str {
        match self.kind() {
            ReportKind::Echo => "> ",
            ReportKind::Peer => "*> ",
            ReportKind::Info => "[*] ",
            ReportKind::Error => "[!] ",
        }
    }
}

#[derive(Debug)]
pub enum Command {
    Msg(String),
    Hello(ActorAddress),
    Quit,
}

#[derive(Debug)]
pub enum ReportKind {
    Echo,
    Peer,
    Info,
    Error,
}

impl FromStr for Command {
    type Err = AppError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.trim() {
            "!q" => Ok(Self::Quit),
            cmd if cmd.starts_with("!add") => {
                let addr = cmd
                    .split(' ')
                    .nth(1)
                    .ok_or(AppError::InvalidCmd(s.to_string()))?
                    .parse()
                    .map_err(|_| AppError::InvalidCmd(s.to_string()))?;
                Ok(Command::Hello(addr))
            }
            _ => Ok(Command::Msg(s.trim().to_string())),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppError {
    MissingArg(String),
    InvalidCmd(String),
    PeerMsg,
    NotReady,
    NotAllowed,
    InvalidArg(String),
}

impl Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::MissingArg(arg) => write!(f, "missing required arg: {arg}"),
            AppError::InvalidCmd(cmd) => write!(f, "invalid command: {cmd}"),
            AppError::PeerMsg => write!(f, "failed to send message to peer"),
            AppError::NotReady => write!(f, "actor is not ready to handle this message"),
            AppError::NotAllowed => write!(f, "message is currently forbidden by this actor"),
            AppError::InvalidArg(msg) => write!(f, "invalid arg: {msg}"),
        }
    }
}

impl std::error::Error for AppError {}
