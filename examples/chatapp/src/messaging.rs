use myriam::{
    actors::{
        local::LocalHandle,
        remote::{
            address::ActorAddress, dencoder::bitcode::BitcodeDencoder,
            netlayer::tor_layer::TorLayer, router::RemoteHandle,
        },
        Actor,
    },
    messaging::Message,
};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Sender;

use crate::{AppError, Report};

pub type MessengerHandle = LocalHandle<MessengerCmd, (), AppError>;
pub type MessengerRemoteHandle =
    RemoteHandle<MessengerCmd, (), AppError, BitcodeDencoder, TorLayer>;

pub struct Messenger {
    name: String,
    tui_sender: Sender<Report>,
    addr: Option<ActorAddress>,
    peer: Option<MessengerRemoteHandle>,
}

impl Messenger {
    pub fn new(name: String, tui_sender: Sender<Report>) -> Self {
        Self {
            name,
            tui_sender,
            addr: None,
            peer: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessengerCmd {
    Init(ActorAddress),
    Hello(ActorAddress),
    Register(ActorAddress),
    Incoming(String),
    Outgoing(String),
}

//
// actor impls
//
impl Actor<MessengerCmd, (), AppError> for Messenger {
    async fn handler(&self, input: MessengerCmd) -> Result<(), AppError> {
        match input {
            MessengerCmd::Incoming(msg) => {
                if self.peer.is_some() {
                    let _ = self.tui_sender.send(Report::peer_msg(msg)).await;
                    Ok(())
                } else {
                    Err(AppError::NotAllowed)
                }
            }
            MessengerCmd::Outgoing(msg) => {
                if let Some(ref peer) = self.peer {
                    if let Err(err) = peer.send(Message::Task(MessengerCmd::Incoming(msg))).await {
                        let _ = self.tui_sender.send(Report::error(err.to_string())).await;
                    }

                    Ok(())
                } else {
                    Err(AppError::NotAllowed)
                }
            }
            _ => Err(AppError::NotAllowed),
        }
    }

    async fn handler_mut(&mut self, input: MessengerCmd) -> Result<Option<()>, AppError> {
        match input {
            MessengerCmd::Register(addr) => {
                if self.peer.is_some() {
                    Err(AppError::NotAllowed)
                } else {
                    let handle = RemoteHandle::new(
                        &addr,
                        TorLayer::new_for_client(self.name.clone())
                            .await
                            .map_err(|_| AppError::NotReady)?,
                    );

                    if let Err(err) = handle
                        .send(Message::TaskMut(MessengerCmd::Hello(
                            self.addr.as_ref().expect("failed to init").clone(),
                        )))
                        .await
                    {
                        let _ = self.tui_sender.send(Report::error(err.to_string())).await;
                        Err(AppError::PeerMsg)
                    } else {
                        self.peer.replace(handle);
                        let _ = self
                            .tui_sender
                            .send(Report::info(format!("connected to peer: {addr}")))
                            .await;
                        Ok(None)
                    }
                }
            }
            MessengerCmd::Hello(addr) => {
                if self.peer.is_some() {
                    Err(AppError::NotAllowed)
                } else {
                    let handle = RemoteHandle::new(
                        &addr,
                        TorLayer::new_for_client(self.name.clone())
                            .await
                            .map_err(|_| AppError::NotReady)?,
                    );

                    self.peer.replace(handle);

                    let _ = self
                        .tui_sender
                        .send(Report::info(format!("new peer: {addr}")))
                        .await;

                    Ok(None)
                }
            }
            MessengerCmd::Init(addr) => {
                self.addr.replace(addr);
                Ok(None)
            }
            _ => Err(AppError::NotAllowed),
        }
    }
}
