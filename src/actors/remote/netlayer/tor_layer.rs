//!
//! Tor net layer
//!

use std::sync::Arc;
use std::{fmt::Display, time::Duration};

use arti_client::{TorClient, TorClientConfig};
use futures::lock::Mutex;
use futures::{Stream, StreamExt};
use safelog::DisplayRedacted;
use tor_cell::relaycell::msg::Connected;
use tor_hsservice::config::OnionServiceConfigBuilder;
use tor_hsservice::{RunningOnionService, StreamRequest};
use tor_proto::client::stream::IncomingStreamRequest;
use tor_rtcompat::PreferredRuntime;

use crate::actors::remote::netlayer::{AsyncMsgStream, NetLayer};
use crate::utils;

///
/// Tor netlayer powered by Arti
///
#[allow(missing_debug_implementations)]
pub struct TorLayer {
    client: TorClient<PreferredRuntime>,
    nickname: String,
    port: Option<u16>,
    address: Option<String>,
    service: Option<Arc<RunningOnionService>>,

    // here lies a testament to my inadequacy
    stream: Option<Arc<Mutex<Box<dyn Stream<Item = StreamRequest> + Send + Unpin>>>>,
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
            port: Some(port),
            address: None,
            service: None,
            stream: None,
        })
    }

    ///
    /// bootstrap a Tor circuit for making connections. note that a layer created this
    /// way will get a port assigned at random. if you want to chose the port, use
    /// [Self::new] instead.
    ///
    pub async fn new_for_client(nickname: String) -> Result<Self, Error> {
        let client = TorClient::create_bootstrapped(TorClientConfig::default())
            .await
            .map_err(|e| Error::Bootstrap(e.to_string()))?;

        Ok(Self {
            client,
            nickname,
            port: None,
            address: None,
            service: None,
            stream: None,
        })
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

        if self.port.is_none() {
            let port = self.port.unwrap_or(
                utils::random_unused_port()
                    .await
                    .map_err(|e| Error::Hostname(e.to_string()))?,
            );

            self.port.replace(port);
        }

        let redacted = match service.onion_address() {
            Some(a) => a,
            None => {
                return Err(Error::Init(
                    "failed to query our own onion address".to_string(),
                ));
            }
        };
        let address = format!(
            "{}:{}",
            redacted.display_unredacted(),
            self.port.expect("valid port should be set")
        );

        let requests_stream = tor_hsservice::handle_rend_requests(requests_stream);

        self.service.replace(service);
        self.stream
            .replace(Arc::new(Mutex::new(Box::new(requests_stream))));
        self.address.replace(address);

        Ok(())
    }

    async fn accept(&self) -> Result<impl AsyncMsgStream, Self::Error> {
        loop {
            if let Some(stream) = &self.stream {
                if let Some(request) = stream.lock().await.next().await {
                    match request.request() {
                        IncomingStreamRequest::Begin(begin)
                            if begin.port() == self.port.expect("valid port should be set") =>
                        {
                            return request
                                .accept(Connected::new_empty())
                                .await
                                .map_err(|e| Error::Accept(e.to_string()));
                        }
                        _ => {
                            let _ = request.shutdown_circuit();
                            continue;
                        }
                    }
                } else {
                    return Err(Error::NotReady);
                }
            } else {
                return Err(Error::NotReady);
            }
        }
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
