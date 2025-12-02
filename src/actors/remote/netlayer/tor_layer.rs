//!
//! Tor net layer
//!

use std::sync::Arc;
use std::{fmt::Display, time::Duration};

use arti_client::{DataStream, TorClient, TorClientConfig};
use futures::StreamExt;
use safelog::DisplayRedacted;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tor_cell::relaycell::msg::{Connected, End, EndReason};
use tor_hsservice::config::OnionServiceConfigBuilder;
use tor_hsservice::RunningOnionService;
use tor_proto::client::stream::IncomingStreamRequest;
use tor_rtcompat::PreferredRuntime;

use crate::actors::remote::netlayer::{AsyncMsgStream, NetLayer};

///
/// TODO: document me
///
#[allow(missing_debug_implementations)]
pub struct TorLayer {
    client: TorClient<PreferredRuntime>,
    nickname: String,
    port: u16,
    address: Option<String>,
    service: Option<Arc<RunningOnionService>>,
    channel: Option<mpsc::Sender<oneshot::Sender<Result<DataStream, Error>>>>,
    task: Option<JoinHandle<()>>,
}

impl TorLayer {
    ///
    /// boostrap a Tor circuit ready for either making remote connections or creating a new Router
    ///
    pub async fn new(nickname: String, port: u16) -> Result<Self, Error> {
        let client = TorClient::create_bootstrapped(TorClientConfig::default())
            .await
            .map_err(|e| Error::Bootstrap(e.to_string()))?;

        Ok(Self {
            client,
            nickname,
            port,
            address: None,
            service: None,
            channel: None,
            task: None,
        })
    }

    ///
    /// abort the layer's request stream event loop, if started
    ///
    pub fn abort(&mut self) -> Result<(), Error> {
        self.task.as_ref().ok_or(Error::NotReady)?.abort();
        Ok(())
    }
}

impl NetLayer for TorLayer {
    type Error = Error;

    fn name() -> &'static str {
        "tor"
    }

    async fn connect(&self, addr: &str) -> Result<impl AsyncMsgStream, Self::Error> {
        self.client
            .connect(addr)
            .await
            .map_err(|e| Error::Connect(e.to_string()))
    }

    async fn init(&mut self) -> Result<(), Self::Error> {
        let service_config = OnionServiceConfigBuilder::default()
            .nickname(
                self.nickname
                    .parse()
                    .map_err(|_| Error::Init("invalid nickname".to_string()))?,
            )
            .build()
            .map_err(|e| Error::Init(e.to_string()))?;

        let (service, requests_stream) = self
            .client
            .launch_onion_service(service_config)
            .map_err(|e| Error::Init(e.to_string()))?
            .ok_or(Error::Init("could not launch onion service".to_string()))?;

        let status_stream = service.status_events();
        let mut binding = status_stream
            .filter(|status| futures::future::ready(status.state().is_fully_reachable()));

        match tokio::time::timeout(Duration::from_secs(60), binding.next()).await {
                            Ok(Some(_)) => tracing::info!("onion service is fully reachable."),
                            Ok(None) => tracing::warn!("status stream ended unexpectedly."),
                            Err(_) => tracing::warn!(
                                "timeout waiting for service to become reachable. actor may or may not receive messages."
                            ),
                        };

        let redacted = match service.onion_address() {
            Some(a) => a,
            None => {
                return Err(Error::Init(
                    "failed to query our own onion address".to_string(),
                ));
            }
        };
        let address = format!("{}:{}", redacted.display_unredacted(), self.port);

        let mut requests_stream = tor_hsservice::handle_rend_requests(requests_stream);

        let port = self.port;
        let (tx, mut rx) = mpsc::channel::<oneshot::Sender<Result<DataStream, Error>>>(1024);
        let task = tokio::spawn(async move {
            while let Some(request_stream) = requests_stream.next().await {
                match request_stream.request() {
                    IncomingStreamRequest::Begin(begin) if begin.port() == port => {
                        if let Some(sender) = rx.recv().await {
                            let stream = request_stream
                                .accept(Connected::new_empty())
                                .await
                                .map_err(|e| Error::Accept(e.to_string()));

                            if sender.send(stream).is_err() {
                                tracing::error!("could not send DataStream, channel dropped?");
                            }
                        }
                    }
                    _ => {
                        let _ = request_stream
                            .reject(End::new_with_reason(EndReason::DONE))
                            .await;
                    }
                }
            }
        });

        self.service.replace(service);
        self.task.replace(task);
        self.address.replace(address);
        self.channel.replace(tx);

        Ok(())
    }

    async fn accept(&self) -> Result<impl AsyncMsgStream, Self::Error> {
        let (tx, rx) = oneshot::channel::<Result<DataStream, Error>>();

        if let Some(channel) = &self.channel {
            channel.send(tx).await.map_err(|_| {
                Error::Accept("failed to receive response from onion service loop".to_string())
            })?;
        } else {
            return Err(Error::NotReady);
        }

        rx.await.map_err(|e| Error::Accept(e.to_string()))?
    }

    async fn address(&self) -> Result<String, Self::Error> {
        self.address.to_owned().ok_or(Error::NotReady)
    }
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
