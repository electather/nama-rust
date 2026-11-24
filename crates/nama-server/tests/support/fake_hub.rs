//! An in-process stand-in for the Hub a Server's config names, answering the calls the Kernel
//! makes as the Server's Link (ADR 0105): its device flow, Unlinking, the Server Addresses, whose
//! public port it reaches only at [`OPEN_PORT`], the Relay token, the key set it signs tokens with,
//! and syncing Users' Watch State, Track Preferences and Watchlists, all as one [`Account`]'s, which
//! a fake for another Server can share. It speaks the Connect protocol's unary calls with protobuf
//! bodies, as the Kernel's client does, so it needs none of the Hub API's other calls.

use std::{
    any, future,
    net::SocketAddr,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    Router,
    body::Bytes,
    http::{HeaderMap, header::CONTENT_TYPE},
    response::{IntoResponse as _, Response},
    routing::{MethodRouter, get, post},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use buffa::Message;
use connectrpc::{ConnectError, client::HttpClient};
use nama_hub_proto::nama::hub::v1::{
    GetRelayTokenRequest, GetRelayTokenResponse, PollServerLinkRequest, PollServerLinkResponse,
    PullTrackPreferencesRequest, PullTrackPreferencesResponse, PullWatchStatesRequest,
    PullWatchStatesResponse, PushTrackPreferencesRequest, PushTrackPreferencesResponse,
    PushWatchStatesRequest, PushWatchStatesResponse, SetServerAddressesRequest,
    SetServerAddressesResponse, StartServerLinkRequest, StartServerLinkResponse,
    SyncWatchlistRequest, SyncWatchlistResponse, TrackPreferenceChange, UnlinkServerRequest,
    UnlinkServerResponse, WatchStateChange, WatchlistEntry,
};
use nama_proto::nama::v1::{
    GetServerLinkRequest, ServerServiceClient, StartServerLinkRequest as Start,
};
use nama_server::config::Config;
use ring::signature::{Ed25519KeyPair, KeyPair as _};
use serde_json::{Value, json};
use tokio::{net::TcpListener, task::JoinHandle};

use crate::server::TestServer;

/// The code the fake shows a Server to Link.
pub(crate) const CODE: &str = "BDFH-JKLM";

/// The email of the Account the code is entered by.
pub(crate) const EMAIL: &str = "ada@example.com";

/// The name the fake gives every Server that reports its addresses.
pub(crate) const OWN: &str = "abc.namadirect.test";

/// The IP address the fake sees the Server's calls come from.
pub(crate) const PUBLIC_IP: &str = "203.0.113.7";

/// The one public port the fake reaches the Server at.
pub(crate) const OPEN_PORT: u16 = 7980;

const DEVICE_CODE: &str = "device-code";

const KID: &str = "hub";

/// What the fake knows.
#[derive(Default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each is a switch a test flips on its own, not states of one thing"
)]
struct State {
    /// The name the Server started Linking as, while its code waits.
    starting: Option<String>,
    entered: bool,
    /// Whether the Server holds a credential the fake knows.
    linked: bool,
    /// How many credentials the fake handed out; only the last works.
    credentials: usize,
    /// Each set of Server Addresses the Server reported, in order.
    told: Vec<SetServerAddressesRequest>,
    /// Whether every call waits forever, as a Hub slow to time out.
    hanging: bool,
    /// Whether every call fails `unavailable`, as a Hub that is down.
    down: bool,
    /// The type of each request that failed so.
    refused: Vec<&'static str>,
    account: Arc<Mutex<Account>>,
}

/// What the one Account the fake syncs every User with keeps: of each title and Preference, the
/// change made last, as the Hub does.
#[derive(Default)]
pub(crate) struct Account {
    pub(crate) watch_states: Vec<WatchStateChange>,
    pub(crate) preferences: Vec<TrackPreferenceChange>,
    pub(crate) watchlist: Vec<WatchlistEntry>,
    /// Each Watch State push (`"push"`) and pull (`"pull"`), in the order they came.
    pub(crate) asked: Vec<&'static str>,
}

impl Account {
    /// Keeps `change` unless one made later of the same title is kept.
    pub(crate) fn keep_watch_state(&mut self, change: WatchStateChange) {
        let title = |kept: &WatchStateChange| (kept.episode, kept.ids.clone());
        keep_latest(&mut self.watch_states, change, title, |kept| {
            kept.changed_at_ms
        });
    }

    /// Keeps `change` unless one made later of the same Preference is kept.
    pub(crate) fn keep_preference(&mut self, change: TrackPreferenceChange) {
        let preference =
            |kept: &TrackPreferenceChange| (kept.audio, kept.title_kind, kept.title_id.clone());
        keep_latest(&mut self.preferences, change, preference, |kept| {
            kept.changed_at_ms
        });
    }

    /// Keeps `entry` unless one made later of the same title is kept.
    pub(crate) fn keep_entry(&mut self, entry: WatchlistEntry) {
        let title = |kept: &WatchlistEntry| (kept.kind, kept.id.clone());
        keep_latest(&mut self.watchlist, entry, title, |kept| kept.changed_at_ms);
    }
}

/// Puts `item` in `kept` in place of the one of the same `key`, unless that one was made later.
fn keep_latest<T, K: PartialEq>(
    kept: &mut Vec<T>,
    item: T,
    key: impl Fn(&T) -> K,
    at: impl Fn(&T) -> i64,
) {
    match kept.iter_mut().find(|old| key(old) == key(&item)) {
        Some(old) if at(old) > at(&item) => {}
        Some(old) => *old = item,
        None => kept.push(item),
    }
}

/// The Hub, serving until [`FakeHub::stop`] or its drop.
pub(crate) struct FakeHub {
    addr: SocketAddr,
    state: Arc<Mutex<State>>,
    account: Arc<Mutex<Account>>,
    serving: JoinHandle<()>,
}

impl FakeHub {
    pub(crate) async fn start() -> Self {
        Self::keeping(Arc::default()).await
    }

    /// The Hub as another Server Linked to the same Account sees it, sharing what it syncs.
    pub(crate) async fn for_another_server(&self) -> Self {
        Self::keeping(Arc::clone(&self.account)).await
    }

    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    async fn keeping(account: Arc<Mutex<Account>>) -> Self {
        let state = Arc::new(Mutex::new(State {
            account: Arc::clone(&account),
            ..State::default()
        }));
        let key = Arc::new(Ed25519KeyPair::from_seed_unchecked(&[7; 32]).unwrap());
        let key_set = json!({ "keys": [{
            "kty": "OKP",
            "crv": "Ed25519",
            "kid": KID,
            "x": URL_SAFE_NO_PAD.encode(key.public_key()),
        }] })
        .to_string();
        let linking = "/nama.hub.v1.ServerLinkService";
        let linked = "/nama.hub.v1.LinkedServerService";
        let app = Router::new()
            .route("/.well-known/jwks.json", get(async move || key_set))
            .route(
                &format!("{linking}/StartServerLink"),
                unary(&state, &key, start),
            )
            .route(
                &format!("{linking}/PollServerLink"),
                unary(&state, &key, poll),
            )
            .route(
                &format!("{linked}/UnlinkServer"),
                unary(&state, &key, unlink),
            )
            .route(
                &format!("{linked}/SetServerAddresses"),
                unary(&state, &key, tell),
            )
            .route(
                &format!("{linked}/GetRelayToken"),
                unary(&state, &key, relay_token),
            )
            .route(
                &format!("{linked}/PushWatchStates"),
                unary(&state, &key, push_watch_states),
            )
            .route(
                &format!("{linked}/PullWatchStates"),
                unary(&state, &key, pull_watch_states),
            )
            .route(
                &format!("{linked}/PushTrackPreferences"),
                unary(&state, &key, push_track_preferences),
            )
            .route(
                &format!("{linked}/PullTrackPreferences"),
                unary(&state, &key, pull_track_preferences),
            )
            .route(
                &format!("{linked}/SyncWatchlist"),
                unary(&state, &key, sync_watchlist),
            );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let serving = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self {
            addr,
            state,
            account,
            serving,
        }
    }

    pub(crate) fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// The config of a Server that Links with the fake.
    pub(crate) fn config(&self) -> Config {
        Config {
            hub_url: Some(self.url()),
            ..Config::default()
        }
    }

    /// Someone enters the code on the Hub.
    pub(crate) fn enter_code(&self) {
        lock(&self.state).entered = true;
    }

    /// The name the waiting Server started Linking as.
    pub(crate) fn started_as(&self) -> Option<String> {
        lock(&self.state).starting.clone()
    }

    /// Whether the Server holds a credential the fake still knows.
    pub(crate) fn linked(&self) -> bool {
        lock(&self.state).linked
    }

    /// Forgets the Server's credential, as deleting its Account does.
    pub(crate) fn forget_server(&self) {
        lock(&self.state).linked = false;
    }

    /// The Server Addresses the Server reported last.
    pub(crate) fn last_told(&self) -> Option<SetServerAddressesRequest> {
        lock(&self.state).told.last().cloned()
    }

    /// Has every call from now on wait forever.
    pub(crate) fn hang(&self) {
        lock(&self.state).hanging = true;
    }

    /// What the Account keeps.
    pub(crate) fn account(&self) -> MutexGuard<'_, Account> {
        lock(&self.account)
    }

    /// Fails every call from now on `unavailable`, as a Hub that is down, or answers again.
    pub(crate) fn set_down(&self, down: bool) {
        lock(&self.state).down = down;
    }

    /// How many calls of `request`, such as `PushWatchStatesRequest`, failed while it was down.
    pub(crate) fn refused(&self, request: &str) -> usize {
        let state = lock(&self.state);
        (state.refused.iter())
            .filter(|refused| refused.ends_with(request))
            .count()
    }

    /// Goes down, as if unreachable.
    pub(crate) fn stop(&self) {
        self.serving.abort();
    }

    /// Links `server` to [`EMAIL`]'s Account, as its Admin does in Settings.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    pub(crate) async fn link(&self, server: &TestServer) {
        let settings = ServerServiceClient::new(HttpClient::plaintext(), server.client_config());
        settings.start_server_link(Start::default()).await.unwrap();
        self.enter_code();
        let linked = settings
            .get_server_link(GetServerLinkRequest::default())
            .await
            .unwrap()
            .into_owned()
            .link
            .into_option()
            .unwrap();
        assert_eq!(linked.account, EMAIL);
    }
}

impl Drop for FakeHub {
    fn drop(&mut self) {
        self.serving.abort();
    }
}

/// How the fake answers one call: what the request asks, signed with the key where it signs.
type Answer<Req, Res> =
    fn(&mut State, &HeaderMap, &Req, &Ed25519KeyPair) -> Result<Res, ConnectError>;

/// A Connect unary call `answer` answers.
fn unary<Req, Res>(
    state: &Arc<Mutex<State>>,
    key: &Arc<Ed25519KeyPair>,
    answer: Answer<Req, Res>,
) -> MethodRouter
where
    Req: Message + Send + 'static,
    Res: Message + 'static,
{
    let (state, key) = (Arc::clone(state), Arc::clone(key));
    post(async move |headers: HeaderMap, body: Bytes| -> Response {
        let hanging = lock(&state).hanging;
        if hanging {
            future::pending::<()>().await;
        }
        let down = {
            let mut state = lock(&state);
            if state.down {
                state.refused.push(any::type_name::<Req>());
            }
            state.down
        };
        let answered = Req::decode_from_slice(&body)
            .map_err(|_unreadable| ConnectError::invalid_argument("not a request"))
            .and_then(|request| {
                if down {
                    return Err(ConnectError::unavailable("the Hub is down"));
                }
                answer(&mut lock(&state), &headers, &request, &key)
            });
        match answered {
            Ok(response) => (
                [(CONTENT_TYPE, "application/proto")],
                response.encode_to_vec(),
            )
                .into_response(),
            Err(err) => (
                err.http_status(),
                [(CONTENT_TYPE, "application/json")],
                err.to_json(),
            )
                .into_response(),
        }
    })
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "every call's answer has the one type `unary` takes"
)]
fn start(
    state: &mut State,
    _: &HeaderMap,
    request: &StartServerLinkRequest,
    _: &Ed25519KeyPair,
) -> Result<StartServerLinkResponse, ConnectError> {
    state.starting = Some(request.name.clone());
    Ok(StartServerLinkResponse {
        device_code: DEVICE_CODE.to_owned(),
        user_code: CODE.to_owned(),
        expires_in_seconds: 600,
        // So a test's every look asks.
        interval_seconds: 0,
        ..Default::default()
    })
}

fn poll(
    state: &mut State,
    _: &HeaderMap,
    request: &PollServerLinkRequest,
    key: &Ed25519KeyPair,
) -> Result<PollServerLinkResponse, ConnectError> {
    if request.device_code != DEVICE_CODE || state.starting.is_none() {
        return Err(ConnectError::not_found("the code expired"));
    }
    if !state.entered {
        return Ok(PollServerLinkResponse::default());
    }
    state.starting = None;
    state.entered = false;
    state.linked = true;
    state.credentials += 1;
    Ok(PollServerLinkResponse {
        linked: true,
        credential: credential(state.credentials),
        token: sign(
            key,
            &json!({ "sub": "server", "email": EMAIL, "exp": in_5_minutes() }),
        ),
        ..Default::default()
    })
}

fn unlink(
    state: &mut State,
    headers: &HeaderMap,
    _: &UnlinkServerRequest,
    _: &Ed25519KeyPair,
) -> Result<UnlinkServerResponse, ConnectError> {
    caller(state, headers)?;
    state.linked = false;
    Ok(UnlinkServerResponse::default())
}

fn tell(
    state: &mut State,
    headers: &HeaderMap,
    request: &SetServerAddressesRequest,
    _: &Ed25519KeyPair,
) -> Result<SetServerAddressesResponse, ConnectError> {
    caller(state, headers)?;
    let mut names = vec![OWN.to_owned()];
    if !request.relay.is_empty() {
        names.push(format!("relay.{OWN}"));
    }
    let port = request.public_port;
    state.told.push(request.clone());
    if port == 0 {
        return Ok(SetServerAddressesResponse {
            names,
            ..Default::default()
        });
    }
    Ok(SetServerAddressesResponse {
        public_address: format!("http://{PUBLIC_IP}:{port}"),
        reachable: port == u32::from(OPEN_PORT),
        names,
        ..Default::default()
    })
}

fn relay_token(
    state: &mut State,
    headers: &HeaderMap,
    _: &GetRelayTokenRequest,
    key: &Ed25519KeyPair,
) -> Result<GetRelayTokenResponse, ConnectError> {
    caller(state, headers)?;
    Ok(GetRelayTokenResponse {
        token: sign(
            key,
            &json!({ "sub": OWN, "aud": "relay", "exp": in_5_minutes() }),
        ),
        ..Default::default()
    })
}

fn push_watch_states(
    state: &mut State,
    headers: &HeaderMap,
    request: &PushWatchStatesRequest,
    _: &Ed25519KeyPair,
) -> Result<PushWatchStatesResponse, ConnectError> {
    caller(state, headers)?;
    let mut account = lock(&state.account);
    account.asked.push("push");
    for change in &request.changes {
        account.keep_watch_state(change.clone());
    }
    Ok(PushWatchStatesResponse::default())
}

fn pull_watch_states(
    state: &mut State,
    headers: &HeaderMap,
    _: &PullWatchStatesRequest,
    _: &Ed25519KeyPair,
) -> Result<PullWatchStatesResponse, ConnectError> {
    caller(state, headers)?;
    let mut account = lock(&state.account);
    account.asked.push("pull");
    Ok(PullWatchStatesResponse {
        watch_states: account.watch_states.clone(),
        ..Default::default()
    })
}

fn push_track_preferences(
    state: &mut State,
    headers: &HeaderMap,
    request: &PushTrackPreferencesRequest,
    _: &Ed25519KeyPair,
) -> Result<PushTrackPreferencesResponse, ConnectError> {
    caller(state, headers)?;
    let mut account = lock(&state.account);
    for change in &request.changes {
        account.keep_preference(change.clone());
    }
    Ok(PushTrackPreferencesResponse::default())
}

fn pull_track_preferences(
    state: &mut State,
    headers: &HeaderMap,
    _: &PullTrackPreferencesRequest,
    _: &Ed25519KeyPair,
) -> Result<PullTrackPreferencesResponse, ConnectError> {
    caller(state, headers)?;
    Ok(PullTrackPreferencesResponse {
        preferences: lock(&state.account).preferences.clone(),
        ..Default::default()
    })
}

fn sync_watchlist(
    state: &mut State,
    headers: &HeaderMap,
    request: &SyncWatchlistRequest,
    _: &Ed25519KeyPair,
) -> Result<SyncWatchlistResponse, ConnectError> {
    caller(state, headers)?;
    let mut account = lock(&state.account);
    for entry in &request.entries {
        account.keep_entry(entry.clone());
    }
    Ok(SyncWatchlistResponse {
        entries: account.watchlist.clone(),
        ..Default::default()
    })
}

/// Refuses a call without the credential of the Server the fake has Linked, as the Hub does.
fn caller(state: &State, headers: &HeaderMap) -> Result<(), ConnectError> {
    let bearer = format!("Bearer {}", credential(state.credentials));
    let sent = headers
        .get("authorization")
        .and_then(|sent| sent.to_str().ok());
    if state.linked && sent == Some(bearer.as_str()) {
        Ok(())
    } else {
        Err(ConnectError::unauthenticated("no Linked Server has it"))
    }
}

/// The `nth` credential the fake hands out.
fn credential(nth: usize) -> String {
    format!("credential-{nth}")
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn in_5_minutes() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 300
}

fn sign(key: &Ed25519KeyPair, claims: &Value) -> String {
    let header = json!({ "alg": "EdDSA", "typ": "JWT", "kid": KID });
    let signed = format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(header.to_string()),
        URL_SAFE_NO_PAD.encode(claims.to_string()),
    );
    format!(
        "{signed}.{}",
        URL_SAFE_NO_PAD.encode(key.sign(signed.as_bytes()))
    )
}

fn lock<T>(locked: &Mutex<T>) -> MutexGuard<'_, T> {
    locked.lock().unwrap_or_else(PoisonError::into_inner)
}
