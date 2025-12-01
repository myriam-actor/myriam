//!
//! Tor net layer
//!

use std::{fmt::Display, time::Duration};

use arti_client::{DataStream, TorClient, TorClientConfig};
use futures::StreamExt;
use safelog::DisplayRedacted;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tor_cell::relaycell::msg::{Connected, End, EndReason};
use tor_hsservice::{config::OnionServiceConfigBuilder, handle_rend_requests, HsNickname};
use tor_proto::client::stream::IncomingStreamRequest;

use crate::actors::remote::netlayer::{AsyncMsgStream, NetLayer};

///
/// TODO: document me
///
#[derive(Debug)]
pub struct TorLayer {
    channel: mpsc::Sender<(
        TorLayerRequest,
        oneshot::Sender<Result<TorLayerResponse, Error>>,
    )>,
    task: JoinHandle<()>,
}

impl TorLayer {
    ///
    /// boostrap a Tor circuit ready for either making remote connections or creating a new Router
    ///
    pub async fn new(nickname: String) -> Result<Self, Error> {
        let (tx, mut rx) = mpsc::channel::<(
            TorLayerRequest,
            oneshot::Sender<Result<TorLayerResponse, Error>>,
        )>(1024);

        let config = TorClientConfig::default();
        let layer_client = TorClient::create_bootstrapped(config)
            .await
            .map_err(|e| Error::Bootstrap(e.to_string()))?;

        let task = tokio::spawn(async move {
            let mut layer_service = None;
            let mut layer_stream = None;
            let mut layer_address = None;

            while let Some((request, sender)) = rx.recv().await {
                match request {
                    TorLayerRequest::Init => {
                        let nickname = match HsNickname::new(nickname.clone()) {
                            Ok(n) => n,
                            Err(e) => {
                                tracing::error!("{e}");
                                let _ = sender.send(Err(Error::Init(e.to_string())));
                                continue;
                            }
                        };

                        let service_config = match OnionServiceConfigBuilder::default()
                            .nickname(nickname)
                            .build()
                        {
                            Ok(c) => c,
                            Err(e) => {
                                tracing::error!("{e}");
                                let _ = sender.send(Err(Error::Init(e.to_string())));
                                continue;
                            }
                        };

                        let (service, stream) =
                            match layer_client.launch_onion_service(service_config.clone()) {
                                Ok(res) => res,
                                Err(e) => {
                                    tracing::error!("{e}");
                                    let _ = sender.send(Err(Error::Init(e.to_string())));
                                    continue;
                                }
                            };

                        let status_stream = service.status_events();
                        let mut binding = status_stream.filter(|status| {
                            futures::future::ready(status.state().is_fully_reachable())
                        });

                        match tokio::time::timeout(Duration::from_secs(60), binding.next()).await {
                            Ok(Some(_)) => tracing::info!("onion service is fully reachable."),
                            Ok(None) => tracing::info!("status stream ended unexpectedly."),
                            Err(_) => tracing::info!(
                                "timeout waiting for service to become reachable. actor may or may not receive messages."
                            ),
                        };

                        let stream = handle_rend_requests(stream);

                        let redacted = match service.onion_address() {
                            Some(a) => a,
                            None => {
                                tracing::error!("failed to query our own onion address");
                                let _ = sender.send(Err(Error::NotReady));
                                continue;
                            }
                        };
                        let address = format!("{}:80", redacted.display_unredacted());

                        layer_service.replace(service);
                        layer_stream.replace(stream);
                        layer_address.replace(address);

                        let _ = sender.send(Ok(TorLayerResponse::Init));
                    }
                    TorLayerRequest::Accept => {
                        if let Some(requests_stream) = &mut layer_stream {
                            let request_stream = requests_stream.next().await.unwrap();
                            match request_stream.request() {
                                IncomingStreamRequest::Begin(_) => {
                                    match request_stream.accept(Connected::new_empty()).await {
                                        Ok(stream) => {
                                            let _ =
                                                sender.send(Ok(TorLayerResponse::Accept(stream)));
                                        }
                                        Err(e) => {
                                            let _ = sender.send(Err(Error::Accept(e.to_string())));
                                        }
                                    }
                                }
                                _ => {
                                    let _ = request_stream
                                        .reject(End::new_with_reason(EndReason::DONE))
                                        .await;
                                }
                            }
                        } else {
                            let _ = sender.send(Err(Error::NotReady));
                        }
                    }
                    TorLayerRequest::Connect(addr) => {
                        match layer_client.connect(addr.clone()).await {
                            Ok(stream) => {
                                let _ = sender.send(Ok(TorLayerResponse::Connect(stream)));
                            }
                            Err(e) => {
                                tracing::error!("could not connect to {addr}");
                                let _ = sender.send(Err(Error::Connect(e.to_string())));
                            }
                        }
                    }
                    TorLayerRequest::Hostname => {
                        if let Some(address) = &layer_address {
                            let _ = sender.send(Ok(TorLayerResponse::Hostname(address.clone())));
                        } else {
                            let _ = sender.send(Err(Error::NotReady));
                        }
                    }
                }
            }
        });

        Ok(Self { channel: tx, task })
    }

    ///
    /// abort the Tor circuit's event loop, leaving it unusable for further connections
    ///
    pub fn abort(&mut self) {
        self.task.abort();
    }
}

impl NetLayer for TorLayer {
    type Error = Error;

    fn name() -> &'static str {
        "tor"
    }

    async fn connect(&self, addr: &str) -> Result<impl AsyncMsgStream, Self::Error> {
        let (tx, rx) = oneshot::channel();

        self.channel
            .send((TorLayerRequest::Connect(addr.to_string()), tx))
            .await
            .map_err(|_| Error::Connect("actor unavailable".to_string()))?;

        match rx.await {
            Ok(Ok(TorLayerResponse::Connect(stream))) => Ok(stream),
            Ok(Err(e)) => Err(e),
            Err(e) => Err(Error::Accept(e.to_string())),
            _ => Err(Error::Accept("unexpected response".to_string())),
        }
    }

    async fn init(&mut self) -> Result<(), Self::Error> {
        let (tx, rx) = oneshot::channel();

        self.channel
            .send((TorLayerRequest::Init, tx))
            .await
            .map_err(|_| Error::Init("onion service boostrap failed".into()))?;

        match rx.await {
            Ok(Ok(TorLayerResponse::Init)) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(e) => Err(Error::Init(e.to_string())),
            _ => Err(Error::Init("unexpected response".to_string())),
        }
    }

    async fn accept(&self) -> Result<impl AsyncMsgStream, Self::Error> {
        let (tx, rx) = oneshot::channel();

        self.channel
            .send((TorLayerRequest::Accept, tx))
            .await
            .map_err(|_| Error::Accept("actor unavailable".to_string()))?;

        match rx.await {
            Ok(Ok(TorLayerResponse::Accept(stream))) => Ok(stream),
            Ok(Err(e)) => Err(e),
            Err(e) => Err(Error::Accept(e.to_string())),
            _ => Err(Error::Accept("unexpected response".to_string())),
        }
    }

    async fn address(&self) -> Result<String, Self::Error> {
        let (tx, rx) = oneshot::channel();

        self.channel
            .send((TorLayerRequest::Hostname, tx))
            .await
            .map_err(|_| Error::Hostname("actor unavailable".to_string()))?;

        match rx.await {
            Ok(Ok(TorLayerResponse::Hostname(addr))) => Ok(addr),
            Ok(Err(e)) => Err(e),
            Err(e) => Err(Error::Hostname(e.to_string())),
            _ => Err(Error::Connect("unexpected response".to_string())),
        }
    }
}

enum TorLayerRequest {
    Init,
    Accept,
    Connect(String),
    Hostname,
}

enum TorLayerResponse {
    Init,
    Accept(DataStream),
    Connect(DataStream),
    Hostname(String),
}

///
/// errors when binding, accepting and connecting via a Tor net layer
///
#[allow(missing_docs)]
#[derive(Debug)]
pub enum Error {
    Bootstrap(String),
    Init(String),
    Accept(String),
    Connect(String),
    Hostname(String),
    NotReady,
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Init(ctx) => write!(f, "failed to init layer: {ctx}"),
            Error::Accept(ctx) => write!(f, "failed to receive data: {ctx}"),
            Error::Connect(ctx) => write!(f, "failed to connect to endpoint: {ctx}"),
            Error::Hostname(ctx) => write!(f, "failed to recover our hostname: {ctx}"),
            Error::Bootstrap(ctx) => write!(f, "failed to connect to Tor network: {ctx}"),
            Error::NotReady => write!(f, "layer not ready"),
        }
    }
}

impl std::error::Error for Error {}
