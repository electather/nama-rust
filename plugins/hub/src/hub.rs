//! Talking to the Hub's API (`proto/nama/hub/v1/`) as the Linked Server, but for the Server's own
//! Link, which the Kernel keeps (ADR 0105). What its API accepts is in `limits.rs`.

pub(crate) mod limits;

use std::{error::Error, fmt, net::Ipv4Addr, sync::Arc, time::Duration};

use bytes::Bytes;
use connectrpc::client::{
    ClientBody, ClientConfig, ClientTransport as _, ServiceTransport, full_body,
};
use http::{
    Request,
    header::{AUTHORIZATION, CONTENT_TYPE},
};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use hyper_util::{
    client::legacy::{Client, connect::HttpConnector},
    rt::TokioExecutor,
};
use nama_hub_proto::nama::hub::v1::{
    LinkedServerServiceClient, ListMostWatchlistedTitlesRequest, ListMostWatchlistedTitlesResponse,
    ListRecommendedTitlesRequest, ListSimilarTitlesRequest, ListSimilarTitlesResponse,
    ListTrendingTitlesRequest, ListTrendingTitlesResponse, PullTrackPreferencesRequest,
    PullWatchStatesRequest, PushTrackPreferencesRequest, PushWatchStatesRequest, RecommendedTitle,
    RecordViewingEventsRequest, SyncWatchlistRequest, TrackPreferenceChange, UserViewingEvent,
    WatchStateChange, WatchlistEntry,
};
use nama_plugin_api::{PluginError, diagnostics_sharer::Signal};

const TIMEOUT: Duration = Duration::from_secs(20);

type Transport = ServiceTransport<Client<HttpsConnector<HttpConnector>, ClientBody>>;

/// The Hub at one address.
#[derive(Clone)]
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
        let mut http = HttpConnector::new();
        http.enforce_http(false);
        // IPv4 only: the Hub checks the Server's public port at the address its calls come from,
        // and the router opens that port on its IPv4 address alone (ADR 0057).
        http.set_local_address(Some(Ipv4Addr::UNSPECIFIED.into()));
        let connector = builder.https_or_http().enable_http1().wrap_connector(http);
        let client = Client::builder(TokioExecutor::new()).build(connector);
        Ok(Self {
            url,
            transport: ServiceTransport::new(client),
        })
    }

    /// The Hub's API for the Linked Server whose credential is `credential`. Only the Link holder
    /// has one to give.
    pub(crate) fn linked(&self, credential: &str) -> LinkedHub {
        LinkedHub {
            hub: self.clone(),
            credential: credential.to_owned(),
        }
    }

    fn config(&self) -> Result<ClientConfig, PluginError> {
        Ok(ClientConfig::new(self.url.parse()?).with_default_timeout(TIMEOUT))
    }
}

/// The Hub's API for one Linked Server, bound to its credential. Every RPC answers
/// [`LinkDropped`] if the Hub doesn't know the credential any more,
/// `REFUSAL_REASON_SERVER_NOT_LINKED`, its one `unauthenticated` for a call with one, such as
/// after the Server's Account was deleted. Each call takes the handle, as the Link holder hands
/// out one per call.
pub(crate) struct LinkedHub {
    hub: Hub,
    credential: String,
}

impl LinkedHub {
    /// Pushes `changes` to the Watch State of the Accounts the Server's Users are Linked to, in
    /// order.
    pub(crate) async fn push_watch_states(
        self,
        changes: Vec<WatchStateChange>,
    ) -> Result<(), PluginError> {
        (self.client()?)
            .push_watch_states(PushWatchStatesRequest {
                changes,
                ..Default::default()
            })
            .await
            .map_err(link_dropped)?;
        Ok(())
    }

    /// The Watch State of the Account the User `user_id` is Linked to, each title by each id
    /// apart; empty while they aren't Linked.
    pub(crate) async fn pull_watch_states(
        self,
        user_id: i64,
    ) -> Result<Vec<WatchStateChange>, PluginError> {
        let pulled = (self.client()?)
            .pull_watch_states(PullWatchStatesRequest {
                user_id,
                ..Default::default()
            })
            .await
            .map_err(link_dropped)?;
        Ok(pulled.into_owned().watch_states)
    }

    /// Merges `changes` to the Track Preferences of the Server's Users into their Accounts'.
    pub(crate) async fn push_track_preferences(
        self,
        changes: Vec<TrackPreferenceChange>,
    ) -> Result<(), PluginError> {
        (self.client()?)
            .push_track_preferences(PushTrackPreferencesRequest {
                changes,
                ..Default::default()
            })
            .await
            .map_err(link_dropped)?;
        Ok(())
    }

    /// The Track Preferences of the Account the User `user_id` is Linked to; empty while they
    /// aren't Linked.
    pub(crate) async fn pull_track_preferences(
        self,
        user_id: i64,
    ) -> Result<Vec<TrackPreferenceChange>, PluginError> {
        let pulled = (self.client()?)
            .pull_track_preferences(PullTrackPreferencesRequest {
                user_id,
                ..Default::default()
            })
            .await
            .map_err(link_dropped)?;
        Ok(pulled.into_owned().preferences)
    }

    /// Merges `entries`, the whole Watchlist of the User `user_id`, into the Account they are
    /// Linked to, and answers the Account's, each title by each id apart; empty while they aren't
    /// Linked.
    pub(crate) async fn sync_watchlist(
        self,
        user_id: i64,
        entries: Vec<WatchlistEntry>,
    ) -> Result<Vec<WatchlistEntry>, PluginError> {
        let synced = (self.client()?)
            .sync_watchlist(SyncWatchlistRequest {
                user_id,
                entries,
                ..Default::default()
            })
            .await
            .map_err(link_dropped)?;
        Ok(synced.into_owned().entries)
    }

    /// Records `events`, the Server's Viewing Events.
    pub(crate) async fn record_viewing_events(
        self,
        events: Vec<UserViewingEvent>,
    ) -> Result<(), PluginError> {
        (self.client()?)
            .record_viewing_events(RecordViewingEventsRequest {
                events,
                ..Default::default()
            })
            .await
            .map_err(link_dropped)?;
        Ok(())
    }

    /// Posts `batch`, an OTLP/HTTP protobuf export request of `signal`, to the Hub as the Server,
    /// for it to forward to Nama's developers (ADR 0054).
    pub(crate) async fn share(self, signal: Signal, batch: Vec<u8>) -> Result<(), PluginError> {
        let request = Request::post(format!("{}/otlp/{}", self.hub.url, signal.path()))
            .header(CONTENT_TYPE, "application/x-protobuf")
            .header(AUTHORIZATION, format!("Bearer {}", self.credential))
            .body(full_body(Bytes::from(batch)))?;
        let status = tokio::time::timeout(TIMEOUT, self.hub.transport.send(request))
            .await??
            .status();
        if !status.is_success() {
            return Err(format!("the Hub answered {status} for shared Diagnostics").into());
        }
        Ok(())
    }

    /// The titles like the one `request` names.
    pub(crate) async fn similar(
        self,
        request: ListSimilarTitlesRequest,
    ) -> Result<ListSimilarTitlesResponse, PluginError> {
        let similar = (self.client()?)
            .list_similar_titles(request)
            .await
            .map_err(link_dropped)?;
        Ok(similar.into_owned())
    }

    /// The titles the Hub recommends to the User `request` names.
    pub(crate) async fn recommended(
        self,
        request: ListRecommendedTitlesRequest,
    ) -> Result<Vec<RecommendedTitle>, PluginError> {
        let recommended = (self.client()?)
            .list_recommended_titles(request)
            .await
            .map_err(link_dropped)?;
        Ok(recommended.into_owned().titles)
    }

    /// The Movies trending across Nama's Accounts.
    pub(crate) async fn trending(self) -> Result<ListTrendingTitlesResponse, PluginError> {
        let trending = (self.client()?)
            .list_trending_titles(ListTrendingTitlesRequest::default())
            .await
            .map_err(link_dropped)?;
        Ok(trending.into_owned())
    }

    /// The titles on most of Nama's Accounts' Watchlists.
    pub(crate) async fn most_watchlisted(
        self,
    ) -> Result<ListMostWatchlistedTitlesResponse, PluginError> {
        let watchlisted = (self.client()?)
            .list_most_watchlisted_titles(ListMostWatchlistedTitlesRequest::default())
            .await
            .map_err(link_dropped)?;
        Ok(watchlisted.into_owned())
    }

    fn client(&self) -> Result<LinkedServerServiceClient<Transport>, PluginError> {
        let config = (self.hub.config()?)
            .with_default_header("authorization", format!("Bearer {}", self.credential));
        Ok(LinkedServerServiceClient::new(
            self.hub.transport.clone(),
            config,
        ))
    }
}

/// The Hub doesn't know the Server's credential any more, `REFUSAL_REASON_SERVER_NOT_LINKED`,
/// such as after its Account was deleted: the Server's Link is gone.
#[derive(Debug)]
pub(crate) struct LinkDropped;

impl fmt::Display for LinkDropped {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the Hub doesn't know the Server's Link any more")
    }
}

impl Error for LinkDropped {}

/// `err`, as [`LinkDropped`] if it is the Hub not knowing the Server's credential, its one
/// `unauthenticated` for a call with one.
fn link_dropped(err: connectrpc::ConnectError) -> PluginError {
    if err.code == connectrpc::ErrorCode::Unauthenticated {
        LinkDropped.into()
    } else {
        err.into()
    }
}
