//! Talking to the Hub: its API (`proto/nama/hub/v1/`) and its published key set.

use std::{sync::Arc, time::Duration};

use bytes::Bytes;
use connectrpc::client::{
    ClientBody, ClientConfig, ClientTransport as _, ServiceTransport, full_body,
};
use http::{Request, StatusCode};
use http_body_util::{BodyExt as _, Limited};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use hyper_util::{
    client::legacy::{Client, connect::HttpConnector},
    rt::TokioExecutor,
};
use nama_hub_proto::nama::hub::v1::{
    GetUserLinkRequest, LinkedServerServiceClient, PollServerLinkRequest, PollServerLinkResponse,
    PushWatchStatesRequest, ServerLinkServiceClient, StartServerLinkRequest,
    StartServerLinkResponse, StartUserLinkRequest, StartUserLinkResponse, UnlinkServerRequest,
    UnlinkUserRequest, WatchStateChange,
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
            self.config().map_err(|_unreachable| Polling::Failed)?,
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
            Err(_failed) => Err(Polling::Failed),
        }
    }

    /// Unlinks the Server whose credential is `credential`. One the Hub doesn't know any more is
    /// `REFUSAL_REASON_SERVER_NOT_LINKED`, the call's one `unauthenticated`, so Unlinked already.
    pub(crate) async fn unlink(&self, credential: &str) -> Result<(), PluginError> {
        let unlinked = self
            .linked_server(credential)?
            .unlink_server(UnlinkServerRequest::default())
            .await;
        unlinked_already(unlinked.map(drop))
    }

    /// Starts Linking the User `user_id`, named `name`, of the Server whose credential is
    /// `credential`: `None` if they are Linked already, the Hub's one `already_exists`.
    pub(crate) async fn start_user(
        &self,
        credential: &str,
        user_id: i64,
        name: &str,
    ) -> Result<Option<StartUserLinkResponse>, PluginError> {
        let started = self
            .linked_server(credential)?
            .start_user_link(StartUserLinkRequest {
                user_id,
                name: name.to_owned(),
                ..Default::default()
            })
            .await;
        match started {
            Ok(started) => Ok(Some(started.into_owned())),
            Err(err) if err.code == connectrpc::ErrorCode::AlreadyExists => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    /// The email of the Account the User `user_id` is Linked to, empty while they aren't.
    pub(crate) async fn user_account(
        &self,
        credential: &str,
        user_id: i64,
    ) -> Result<String, PluginError> {
        let got = self
            .linked_server(credential)?
            .get_user_link(GetUserLinkRequest {
                user_id,
                ..Default::default()
            })
            .await?;
        Ok(got.into_owned().account)
    }

    /// Unlinks the User `user_id`. A Server the Hub doesn't know any more has no Linked Users.
    pub(crate) async fn unlink_user(
        &self,
        credential: &str,
        user_id: i64,
    ) -> Result<(), PluginError> {
        let unlinked = self
            .linked_server(credential)?
            .unlink_user(UnlinkUserRequest {
                user_id,
                ..Default::default()
            })
            .await;
        unlinked_already(unlinked.map(drop))
    }

    /// Pushes `changes` to the Watch State of the Accounts the Users of the Server whose credential
    /// is `credential` are Linked to, in order.
    pub(crate) async fn push_watch_states(
        &self,
        credential: &str,
        changes: Vec<WatchStateChange>,
    ) -> Result<(), PluginError> {
        self.linked_server(credential)?
            .push_watch_states(PushWatchStatesRequest {
                changes,
                ..Default::default()
            })
            .await?;
        Ok(())
    }

    /// The Hub's API for the Server whose credential is `credential`.
    fn linked_server(
        &self,
        credential: &str,
    ) -> Result<LinkedServerServiceClient<Transport>, PluginError> {
        let config = self
            .config()?
            .with_default_header("authorization", format!("Bearer {credential}"));
        Ok(LinkedServerServiceClient::new(
            self.transport.clone(),
            config,
        ))
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

/// `unlinked`, where the Hub not knowing the Server's credential, its one `unauthenticated`, means
/// there is nothing left to Unlink.
fn unlinked_already(unlinked: Result<(), connectrpc::ConnectError>) -> Result<(), PluginError> {
    match unlinked {
        Err(err) if err.code != connectrpc::ErrorCode::Unauthenticated => Err(err.into()),
        _ => Ok(()),
    }
}

/// Why a poll didn't say whether the code was entered.
#[derive(Debug)]
pub(crate) enum Polling {
    /// The codes expired or were used: start again.
    Expired,
    /// The Hub couldn't be reached or failed: ask again at the next poll.
    Failed,
}
