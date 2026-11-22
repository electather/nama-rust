//! Talking to the Hub: its API (`proto/nama/hub/v1/`) and its published key set.

use std::{sync::Arc, time::Duration};

use bytes::Bytes;
use connectrpc::{
    ConnectError,
    client::{ClientBody, ClientConfig, ClientTransport as _, ServiceTransport, full_body},
};
use http::{Request, StatusCode};
use http_body_util::{BodyExt as _, Limited};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use hyper_util::{
    client::legacy::{Client, connect::HttpConnector},
    rt::TokioExecutor,
};
use nama_hub_proto::nama::hub::v1::{
    LinkedServerServiceClient, PollServerLinkRequest, PollServerLinkResponse,
    ServerLinkServiceClient, StartServerLinkRequest, StartServerLinkResponse, UnlinkServerRequest,
};
use nama_plugin_api::PluginError;

const TIMEOUT: Duration = Duration::from_secs(20);

/// The most of the key set read, so a misbehaving Hub can't exhaust memory.
const MAX_KEY_SET_BYTES: usize = 64 * 1024;

type Transport = ServiceTransport<Client<HttpsConnector<HttpConnector>, ClientBody>>;

/// The Hub at one address.
pub(crate) struct Hub {
    url: String,
    transport: Transport,
}

impl Hub {
    /// The Hub at `url`, such as `https://hub.example`, without a trailing slash.
    pub(crate) fn new(url: &str) -> Result<Self, PluginError> {
        let url = url.trim_end_matches('/').to_owned();
        let provider = rustls::crypto::ring::default_provider();
        let builder = HttpsConnectorBuilder::new();
        // A plain-HTTP Hub, as in development, needs no trust roots, and loading them can fail.
        let builder = if url.starts_with("http://") {
            let tls = rustls::ClientConfig::builder_with_provider(Arc::new(provider))
                .with_safe_default_protocol_versions()?
                .with_root_certificates(rustls::RootCertStore::empty())
                .with_no_client_auth();
            builder.with_tls_config(tls)
        } else {
            builder.with_provider_and_native_roots(provider)?
        };
        let connector = builder.https_or_http().enable_http1().build();
        let client = Client::builder(TokioExecutor::new()).build(connector);
        Ok(Self {
            url,
            transport: ServiceTransport::new(client),
        })
    }

    /// The address of the Hub's pages, where a person enters a Server's code.
    pub(crate) fn url(&self) -> &str {
        &self.url
    }

    fn config(&self) -> Result<ClientConfig, PluginError> {
        Ok(ClientConfig::new(self.url.parse()?).with_default_timeout(TIMEOUT))
    }

    /// Starts registering this Server as `name`.
    pub(crate) async fn start(&self, name: &str) -> Result<StartServerLinkResponse, PluginError> {
        let client = ServerLinkServiceClient::new(self.transport.clone(), self.config()?);
        let started = client
            .start_server_link(StartServerLinkRequest {
                name: name.to_owned(),
                ..Default::default()
            })
            .await?;
        Ok(started.into_owned())
    }

    /// Whether the code the Server shows was entered. A device code that expired or was used is
    /// `REFUSAL_REASON_LINK_EXPIRED`, the call's one `not_found`.
    pub(crate) async fn poll(&self, device_code: &str) -> Result<PollServerLinkResponse, Polling> {
        let client = ServerLinkServiceClient::new(
            self.transport.clone(),
            self.config().map_err(Polling::Failed)?,
        );
        let polled = client
            .poll_server_link(PollServerLinkRequest {
                device_code: device_code.to_owned(),
                ..Default::default()
            })
            .await;
        match polled {
            Ok(polled) => Ok(polled.into_owned()),
            Err(err) if err.code == connectrpc::ErrorCode::NotFound => Err(Polling::Expired),
            Err(err) => Err(Polling::Failed(err.into())),
        }
    }

    /// Unlinks the Server whose credential is `credential`. One the Hub doesn't know any more is
    /// `REFUSAL_REASON_SERVER_NOT_LINKED`, the call's one `unauthenticated`, so Unlinked already.
    pub(crate) async fn unlink(&self, credential: &str) -> Result<(), PluginError> {
        let config = self
            .config()?
            .with_default_header("authorization", format!("Bearer {credential}"));
        let client = LinkedServerServiceClient::new(self.transport.clone(), config);
        match client.unlink_server(UnlinkServerRequest::default()).await {
            Ok(_) => Ok(()),
            Err(err) if err.code == connectrpc::ErrorCode::Unauthenticated => Ok(()),
            Err(err) => Err(err.into()),
        }
    }

    /// The Hub's published key set, as JSON.
    pub(crate) async fn key_set(&self) -> Result<Bytes, PluginError> {
        let request = Request::get(format!("{}/.well-known/jwks.json", self.url))
            .body(full_body(Bytes::new()))?;
        let fetched = async {
            let response = self.transport.send(request).await?;
            let status = response.status();
            let body = Limited::new(response.into_body(), MAX_KEY_SET_BYTES)
                .collect()
                .await?
                .to_bytes();
            Ok::<_, PluginError>((status, body))
        };
        let (status, body) = tokio::time::timeout(TIMEOUT, fetched).await??;
        if status != StatusCode::OK {
            return Err(format!("the Hub answered {status} for its key set").into());
        }
        Ok(body)
    }
}

/// Why a poll didn't say whether the code was entered.
#[derive(Debug)]
pub(crate) enum Polling {
    /// The codes expired or were used: start again.
    Expired,
    Failed(PluginError),
}

impl From<ConnectError> for Polling {
    fn from(err: ConnectError) -> Self {
        Self::Failed(err.into())
    }
}
