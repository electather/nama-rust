//! Calling the Hub as the Server's Link needs: its API (`proto/nama/hub/v1/`) for Linking the
//! Server and its Users, signing in with an Account, the Server Addresses, the certificate
//! challenge, the Relay token, syncing Linked Users and recording their Viewing Events, what it
//! accepts of each, its published key set, and its OTLP/HTTP paths taking shared Diagnostics.

// ponytail: the client is copied from the Hub Plugin's `hub.rs`, which keeps its own for the calls
// it still makes; one goes once the rest of the Plugin's Linking moves here (#2203).

use std::{error::Error, fmt, net::Ipv4Addr, ops::RangeInclusive, sync::Arc, time::Duration};

use bytes::Bytes;
use connectrpc::client::{
    ClientBody, ClientConfig, ClientTransport as _, ServiceTransport, full_body,
};
use http::{
    Request, StatusCode,
    header::{AUTHORIZATION, CONTENT_TYPE},
};
use http_body_util::{BodyExt as _, Limited};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use hyper_util::{
    client::legacy::{Client, connect::HttpConnector},
    rt::TokioExecutor,
};
use nama_hub_proto::nama::hub::v1::{
    GetRelayTokenRequest, GetUserLinkRequest, InviteUserRequest, LinkedServerServiceClient,
    PollServerLinkRequest, PollServerLinkResponse, PollServerSignInRequest,
    PullTrackPreferencesRequest, PullWatchStatesRequest, PushTrackPreferencesRequest,
    PushWatchStatesRequest, RecordViewingEventsRequest, ServerLinkServiceClient,
    SetCertificateChallengeRequest, SetServerAddressesRequest, StartServerLinkRequest,
    StartServerLinkResponse, StartServerSignInRequest, StartServerSignInResponse,
    StartUserLinkRequest, StartUserLinkResponse, SyncWatchlistRequest, TrackPreferenceChange,
    UnlinkServerRequest, UnlinkUserRequest, UserViewingEvent, WatchStateChange, WatchlistEntry,
};
use nama_plugin_api::server_link::{Reachability, ServerAddresses};

use crate::diagnostics::{OTLP_PROTOBUF, Signal};

/// Why a call to the Hub failed, in words for the log.
pub(super) type Failure = Box<dyn Error + Send + Sync>;

const TIMEOUT: Duration = Duration::from_secs(20);

/// The most of the key set read, so a misbehaving Hub can't exhaust memory.
const MAX_KEY_SET_BYTES: usize = 64 * 1024;

/// The most External IDs a `WatchStateChange` carries.
pub(super) const WATCH_STATE_IDS: usize = 20;

/// The most changes a `PushWatchStatesRequest` or a `PushTrackPreferencesRequest` holds; a
/// `RecordViewingEventsRequest` holds twice as many.
pub(super) const PUSH_CHANGES: usize = 500;

/// The most entries a `SyncWatchlistRequest` holds.
pub(super) const WATCHLIST_ENTRIES: usize = 5000;

/// The most characters a `TrackPreferenceChange.language` has.
pub(super) const TRACK_LANGUAGE_CHARS: usize = 100;

/// The most characters a `TrackPreferenceChange.title` has.
pub(super) const TRACK_TITLE_CHARS: usize = 1000;

/// The characters an External ID has, such as `tmdb-348`.
const EXTERNAL_ID_CHARS: RangeInclusive<usize> = 1..=100;

/// When a change may say it was made, in milliseconds since the Unix epoch: from 1970 to the end of
/// 9999.
const CHANGED_AT_MS: RangeInclusive<i64> = 0..=253_402_300_799_999;

/// Whether the Hub takes `id` as an External ID: past any of these limits it refuses the whole call.
pub(super) fn takes_external_id(id: &str) -> bool {
    EXTERNAL_ID_CHARS.contains(&id.chars().count())
}

/// Whether the Hub takes `changed_at_ms` as when a change was made.
pub(super) fn takes_changed_at(changed_at_ms: i64) -> bool {
    CHANGED_AT_MS.contains(&changed_at_ms)
}

/// Whether the Hub takes `duration_ms` as how long a Viewing Event's title is.
pub(super) fn takes_duration(duration_ms: i64) -> bool {
    duration_ms > 0
}

/// Where the Hub takes a Viewing Event that stopped `stop_position_ms` into a title `duration_ms`
/// long to have stopped: never before its start or past its end.
pub(super) fn stop_position_taken(stop_position_ms: i64, duration_ms: i64) -> i64 {
    stop_position_ms.min(duration_ms).max(0)
}

type Transport = ServiceTransport<Client<HttpsConnector<HttpConnector>, ClientBody>>;

/// The Hub at one address.
#[derive(Clone)]
pub(super) struct Hub {
    url: String,
    /// The origin of [`Self::url`] as a browser sends it, `None` for a URL with no such origin.
    origin: Option<String>,
    transport: Transport,
}

impl fmt::Debug for Hub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Hub")
            .field("url", &self.url)
            .finish_non_exhaustive()
    }
}

impl Hub {
    /// The Hub at `url`, such as `https://hub.example`.
    pub(super) fn new(url: &str) -> Result<Self, Failure> {
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
        // Opaque, it would read `null`, which any sandboxed page can send as its origin.
        let origin = (url::Url::parse(&url).ok())
            .map(|url| url.origin())
            .filter(url::Origin::is_tuple)
            .map(|origin| origin.ascii_serialization());
        Ok(Self {
            url,
            origin,
            transport: ServiceTransport::new(client),
        })
    }

    /// The address of the Hub's pages, where a person enters a Server's code.
    pub(super) fn url(&self) -> &str {
        &self.url
    }

    /// The origin the Hub serves its web app from, such as `https://hub.example`.
    pub(super) fn origin(&self) -> Option<&str> {
        self.origin.as_deref()
    }

    /// The Hub's API for the Linked Server whose credential is `credential`.
    pub(super) fn linked(&self, credential: &str) -> LinkedHub {
        LinkedHub {
            hub: self.clone(),
            credential: credential.to_owned(),
        }
    }

    fn config(&self) -> Result<ClientConfig, Failure> {
        Ok(ClientConfig::new(self.url.parse()?).with_default_timeout(TIMEOUT))
    }

    /// Starts registering this Server as `name`, whom the Hub's page sends back to `return_url`
    /// once approved, when it isn't empty.
    pub(super) async fn start(
        &self,
        name: &str,
        return_url: &str,
    ) -> Result<StartServerLinkResponse, Failure> {
        let client = ServerLinkServiceClient::new(self.transport.clone(), self.config()?);
        let started = client
            .start_server_link(StartServerLinkRequest {
                name: name.to_owned(),
                return_url: return_url.to_owned(),
                ..Default::default()
            })
            .await?;
        Ok(started.into_owned())
    }

    /// Whether the code the Server shows was entered. A device code that expired or was used is
    /// `REFUSAL_REASON_LINK_EXPIRED`, the call's one `not_found`.
    pub(super) async fn poll(&self, device_code: &str) -> Result<PollServerLinkResponse, Polling> {
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

    /// The Hub's published key set, as JSON.
    pub(super) async fn key_set(&self) -> Result<Bytes, Failure> {
        let request = Request::get(format!("{}/.well-known/jwks.json", self.url))
            .body(full_body(Bytes::new()))?;
        let fetched = async {
            let response = self.transport.send(request).await?;
            let status = response.status();
            let body = Limited::new(response.into_body(), MAX_KEY_SET_BYTES)
                .collect()
                .await?
                .to_bytes();
            Ok::<_, Failure>((status, body))
        };
        let (status, body) = tokio::time::timeout(TIMEOUT, fetched).await??;
        if status != StatusCode::OK {
            return Err(format!("the Hub answered {status} for its key set").into());
        }
        Ok(body)
    }
}

/// The Hub's API for one Linked Server, bound to its credential. Every RPC answers
/// [`LinkDropped`] if the Hub doesn't know the credential any more,
/// `REFUSAL_REASON_SERVER_NOT_LINKED`, its one `unauthenticated` for a call with one, such as
/// after the Server's Account was deleted.
pub(super) struct LinkedHub {
    hub: Hub,
    credential: String,
}

impl LinkedHub {
    /// Unlinks the Server.
    pub(super) async fn unlink(self) -> Result<(), Failure> {
        (self.client()?)
            .unlink_server(UnlinkServerRequest::default())
            .await
            .map_err(link_dropped)?;
        Ok(())
    }

    /// Tells the Hub `addresses` are where clients reach the Server, and answers whether the Hub
    /// reached its public port, if it has one, and the names it gave the Server, if any.
    pub(super) async fn set_addresses(
        self,
        addresses: ServerAddresses,
    ) -> Result<Option<Reachability>, Failure> {
        let checked = (self.client()?)
            .set_server_addresses(SetServerAddressesRequest {
                url: addresses.url,
                lan_addresses: addresses.lan,
                public_port: addresses.public_port.map_or(0, u32::from),
                relay: addresses
                    .relay
                    .map_or_else(String::new, |relay| relay.to_string()),
                ..Default::default()
            })
            .await
            .map_err(link_dropped)?
            .into_owned();
        let found = !checked.public_address.is_empty() || !checked.names.is_empty();
        Ok(found.then_some(Reachability {
            address: checked.public_address,
            reachable: checked.reachable,
            names: checked.names,
        }))
    }

    /// Has the Hub publish `values` as the Server's certificate challenge, or remove it when empty.
    pub(super) async fn set_certificate_challenge(
        self,
        values: Vec<String>,
    ) -> Result<(), Failure> {
        (self.client()?)
            .set_certificate_challenge(SetCertificateChallengeRequest {
                values,
                ..Default::default()
            })
            .await
            .map_err(link_dropped)?;
        Ok(())
    }

    /// A token proving to a Relay that the Server holds its own name.
    pub(super) async fn relay_token(self) -> Result<String, Failure> {
        let got = (self.client()?)
            .get_relay_token(GetRelayTokenRequest::default())
            .await
            .map_err(link_dropped)?;
        Ok(got.into_owned().token)
    }

    /// Posts `batch`, an OTLP/HTTP protobuf export request of `signal`, for the Hub to forward to
    /// Nama's developers (ADR 0054).
    pub(super) async fn share(self, signal: Signal, batch: Bytes) -> Result<(), Failure> {
        let request = Request::post(format!("{}/otlp/{}", self.hub.url, signal.path()))
            .header(CONTENT_TYPE, OTLP_PROTOBUF)
            .header(AUTHORIZATION, format!("Bearer {}", self.credential))
            .body(full_body(batch))?;
        let status = tokio::time::timeout(TIMEOUT, self.hub.transport.send(request))
            .await??
            .status();
        if !status.is_success() {
            return Err(format!("the Hub answered {status} for shared Diagnostics").into());
        }
        Ok(())
    }

    /// Starts Linking the User `user_id`, named `name`: `None` if they are Linked already, the
    /// Hub's one `already_exists`.
    pub(super) async fn start_user(
        self,
        user_id: i64,
        name: &str,
    ) -> Result<Option<StartUserLinkResponse>, Failure> {
        let started = (self.client()?)
            .start_user_link(StartUserLinkRequest {
                user_id,
                name: name.to_owned(),
                ..Default::default()
            })
            .await;
        match started {
            Ok(started) => Ok(Some(started.into_owned())),
            Err(err) if err.code == connectrpc::ErrorCode::AlreadyExists => Ok(None),
            Err(err) => Err(link_dropped(err)),
        }
    }

    /// The email of the Account the User `user_id` is Linked to, empty while they aren't.
    pub(super) async fn user_account(self, user_id: i64) -> Result<String, Failure> {
        let got = (self.client()?)
            .get_user_link(GetUserLinkRequest {
                user_id,
                ..Default::default()
            })
            .await
            .map_err(link_dropped)?;
        Ok(got.into_owned().account)
    }

    /// Unlinks the User `user_id`.
    pub(super) async fn unlink_user(self, user_id: i64) -> Result<(), Failure> {
        (self.client()?)
            .unlink_user(UnlinkUserRequest {
                user_id,
                ..Default::default()
            })
            .await
            .map_err(link_dropped)?;
        Ok(())
    }

    /// Asks the Account with `email`, if any, to accept the User `user_id`, named `name`.
    pub(super) async fn invite_user(
        self,
        user_id: i64,
        name: &str,
        email: &str,
    ) -> Result<(), Failure> {
        (self.client()?)
            .invite_user(InviteUserRequest {
                user_id,
                name: name.to_owned(),
                email: email.to_owned(),
                ..Default::default()
            })
            .await
            .map_err(link_dropped)?;
        Ok(())
    }

    /// Starts a sign-in with an Account to the Server, which the Hub's page sends back to
    /// `return_url`.
    pub(super) async fn start_sign_in(
        self,
        return_url: &str,
    ) -> Result<StartServerSignInResponse, Failure> {
        let started = (self.client()?)
            .start_server_sign_in(StartServerSignInRequest {
                return_url: return_url.to_owned(),
                ..Default::default()
            })
            .await
            .map_err(link_dropped)?;
        Ok(started.into_owned())
    }

    /// The token naming the User the sign-in polled with `device_code` signs in as, once an
    /// Account approved it and `code` is the one-time code the Hub sent the person back with;
    /// `None` before, and for one that expired or was used or a wrong code, the call's one
    /// `not_found`.
    pub(super) async fn poll_sign_in(
        self,
        device_code: &str,
        code: &str,
    ) -> Result<Option<String>, Failure> {
        let polled = (self.client()?)
            .poll_server_sign_in(PollServerSignInRequest {
                device_code: device_code.to_owned(),
                code: code.to_owned(),
                ..Default::default()
            })
            .await;
        match polled {
            Ok(polled) => Ok(Some(polled.into_owned().token).filter(|token| !token.is_empty())),
            Err(err) if err.code == connectrpc::ErrorCode::NotFound => Ok(None),
            Err(err) => Err(link_dropped(err)),
        }
    }

    /// Pushes `changes` to the Watch State of the Accounts the Server's Users are Linked to.
    pub(super) async fn push_watch_states(
        self,
        changes: Vec<WatchStateChange>,
    ) -> Result<(), Failure> {
        (self.client()?)
            .push_watch_states(PushWatchStatesRequest {
                changes,
                ..Default::default()
            })
            .await
            .map_err(link_dropped)?;
        Ok(())
    }

    /// Records `events`, the Server's Users' Viewing Events, under the Accounts they are Linked to.
    pub(super) async fn record_viewing_events(
        self,
        events: Vec<UserViewingEvent>,
    ) -> Result<(), Failure> {
        (self.client()?)
            .record_viewing_events(RecordViewingEventsRequest {
                events,
                ..Default::default()
            })
            .await
            .map_err(link_dropped)?;
        Ok(())
    }

    /// The Watch State of the Account the User `user_id` is Linked to, each title by each id
    /// apart; empty while they aren't Linked.
    pub(super) async fn pull_watch_states(
        self,
        user_id: i64,
    ) -> Result<Vec<WatchStateChange>, Failure> {
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
    pub(super) async fn push_track_preferences(
        self,
        changes: Vec<TrackPreferenceChange>,
    ) -> Result<(), Failure> {
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
    pub(super) async fn pull_track_preferences(
        self,
        user_id: i64,
    ) -> Result<Vec<TrackPreferenceChange>, Failure> {
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
    pub(super) async fn sync_watchlist(
        self,
        user_id: i64,
        entries: Vec<WatchlistEntry>,
    ) -> Result<Vec<WatchlistEntry>, Failure> {
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

    fn client(&self) -> Result<LinkedServerServiceClient<Transport>, Failure> {
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
pub(super) struct LinkDropped;

impl fmt::Display for LinkDropped {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the Hub doesn't know the Server's Link any more")
    }
}

impl Error for LinkDropped {}

/// `err`, as [`LinkDropped`] if it is the Hub not knowing the Server's credential, its one
/// `unauthenticated` for a call with one.
fn link_dropped(err: connectrpc::ConnectError) -> Failure {
    if err.code == connectrpc::ErrorCode::Unauthenticated {
        LinkDropped.into()
    } else {
        err.into()
    }
}

/// Why a poll didn't say whether the code was entered.
#[derive(Debug)]
pub(super) enum Polling {
    /// The codes expired or were used: start again.
    Expired,
    /// The Hub couldn't be reached or failed: ask again at the next poll.
    Failed,
}
