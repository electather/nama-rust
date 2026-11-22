//! An in-process stand-in for the Hub, so the tests need no network and no PostgreSQL. It serves
//! one Server's device flow, Unlinking, its Users' Links, its Server Addresses, the Watch State it
//! pushes, and a key set of one Ed25519 key it signs with, as the Hub's API
//! (`proto/nama/hub/v1/link.proto`) says the Hub
//! does, and takes shared Diagnostics.

use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    body::Bytes,
    extract::Path,
    http::{HeaderMap, StatusCode},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use nama_hub_proto::nama::hub::v1::{
    GetUserLinkRequest, GetUserLinkResponse, LinkedServerService, PollServerLinkRequest,
    PollServerLinkResponse, PushWatchStatesRequest, PushWatchStatesResponse, ServerLinkService,
    SetServerAddressesRequest, SetServerAddressesResponse, StartServerLinkRequest,
    StartServerLinkResponse, StartUserLinkRequest, StartUserLinkResponse, UnlinkServerRequest,
    UnlinkServerResponse, UnlinkUserRequest, UnlinkUserResponse, WatchStateChange,
};
use ring::signature::{Ed25519KeyPair, KeyPair as _};
use serde_json::{Value, json};
use tokio::{net::TcpListener, task::JoinHandle};

/// The one device code the fake hands out.
const DEVICE_CODE: &str = "device-code";

/// The one code the fake shows.
pub(super) const USER_CODE: &str = "BDFH-JKLM";

/// The one code the fake gives a User.
pub(super) const USER_LINK_CODE: &str = "CDFG-HJKL";

/// The email of the Account the code is entered by.
pub(super) const EMAIL: &str = "ada@example.com";

/// What the fake Hub knows.
#[derive(Default)]
struct State {
    /// The name the Server started with, while its code waits.
    starting: Option<String>,
    entered: bool,
    /// Whether the Server holds a credential the fake knows.
    linked: bool,
    /// How many credentials the fake handed out. Each Link gets a new one, as on the Hub, and only
    /// the last works.
    credentials: usize,
    key_set_fetches: usize,
    /// What the key set answers in place of the keys, if anything.
    key_set_failure: Option<StatusCode>,
    /// Each batch of shared Diagnostics, with the signal its path named.
    shared: Vec<(String, Vec<u8>)>,
    /// The name each User waiting on a code started with, by id.
    user_codes: HashMap<i64, String>,
    /// The email of the Account each Linked User is Linked to, by id.
    user_accounts: HashMap<i64, String>,
    /// How often a User's Link was asked about.
    user_link_asks: usize,
    /// Each push's changes, in the order they came.
    pushes: Vec<Vec<WatchStateChange>>,
    /// Why pushes fail, as if the Hub were down, while they do.
    push_failure: Option<&'static str>,
    /// Each Server URL and LAN addresses the Server reported, in order.
    reported_addresses: Vec<(String, Vec<String>)>,
    /// How many of the next reports of Server Addresses fail.
    addresses_failures: usize,
}

/// The Hub, serving until [`FakeHub::stop`] or its drop.
pub(super) struct FakeHub {
    addr: SocketAddr,
    state: Arc<Mutex<State>>,
    key: Arc<Ed25519KeyPair>,
    serving: JoinHandle<()>,
}

impl FakeHub {
    pub(super) async fn start() -> Self {
        let state = Arc::default();
        let key = Arc::new(Ed25519KeyPair::from_seed_unchecked(&[7; 32]).unwrap());
        let shared = Arc::new(Shared {
            state: Arc::clone(&state),
            key: Arc::clone(&key),
        });
        let rpcs = connectrpc::Router::new()
            .add_service(Arc::new(Starting(Arc::clone(&shared))))
            .add_service(Arc::new(Linked(shared)))
            .into_axum_service();
        let key_set = key_set(&key);
        let key_set_state = Arc::clone(&state);
        let app = axum::Router::new()
            .route(
                "/.well-known/jwks.json",
                axum::routing::get(async move || {
                    let mut fetched = lock(&key_set_state);
                    fetched.key_set_fetches += 1;
                    match fetched.key_set_failure {
                        Some(failure) => (failure, String::new()),
                        None => (StatusCode::OK, key_set.to_string()),
                    }
                }),
            )
            .route(
                "/otlp/v1/{signal}",
                axum::routing::post({
                    let state = Arc::clone(&state);
                    async move |Path(signal): Path<String>, headers: HeaderMap, batch: Bytes| {
                        take_shared(&state, signal, &headers, &batch)
                    }
                }),
            )
            .fallback_service(rpcs);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let serving = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self {
            addr,
            state,
            key,
            serving,
        }
    }

    pub(super) fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Someone enters the code on the Hub.
    pub(super) fn enter_code(&self) {
        lock(&self.state).entered = true;
    }

    /// The User `user_id` enters their code on the Hub, signed in to [`EMAIL`]'s Account.
    pub(super) fn enter_user_code(&self, user_id: i64) {
        let mut state = lock(&self.state);
        if state.user_codes.remove(&user_id).is_some() {
            state.user_accounts.insert(user_id, EMAIL.to_owned());
        }
    }

    /// An Account Links the User `user_id` on the Hub, as through a code another Device showed.
    pub(super) fn link_user_on_hub(&self, user_id: i64) {
        let mut state = lock(&self.state);
        state.user_codes.remove(&user_id);
        state.user_accounts.insert(user_id, EMAIL.to_owned());
    }

    /// The Account Unlinks the User `user_id` on the Hub.
    pub(super) fn unlink_user_on_hub(&self, user_id: i64) {
        lock(&self.state).user_accounts.remove(&user_id);
    }

    /// The name the User `user_id` started Linking as, while their code waits.
    pub(super) fn user_starting(&self, user_id: i64) -> Option<String> {
        lock(&self.state).user_codes.get(&user_id).cloned()
    }

    /// Whether the Hub has the User `user_id` Linked.
    pub(super) fn user_linked(&self, user_id: i64) -> bool {
        lock(&self.state).user_accounts.contains_key(&user_id)
    }

    /// How often the Server asked about a User's Link.
    pub(super) fn user_link_asks(&self) -> usize {
        lock(&self.state).user_link_asks
    }

    /// Each push's changes, in the order they came.
    pub(super) fn pushes(&self) -> Vec<Vec<WatchStateChange>> {
        lock(&self.state).pushes.clone()
    }

    /// Has pushes fail while `down`, as if the Hub were unreachable, and work again after.
    pub(super) fn set_down(&self, down: bool) {
        lock(&self.state).push_failure = down.then_some("the Hub is down");
    }

    /// Each Server URL and LAN addresses the Server reported, in order.
    pub(super) fn reported_addresses(&self) -> Vec<(String, Vec<String>)> {
        lock(&self.state).reported_addresses.clone()
    }

    /// Has the next report of Server Addresses fail, as if the Hub blinked.
    pub(super) fn fail_next_addresses(&self) {
        lock(&self.state).addresses_failures += 1;
    }

    /// Has the key set fail, as if the Hub went down right after handing out a credential.
    pub(super) fn break_key_set(&self) {
        lock(&self.state).key_set_failure = Some(StatusCode::INTERNAL_SERVER_ERROR);
    }

    /// The name the waiting Server started with.
    pub(super) fn starting(&self) -> Option<String> {
        lock(&self.state).starting.clone()
    }

    /// Whether the Server holds a credential the Hub still knows.
    pub(super) fn linked(&self) -> bool {
        lock(&self.state).linked
    }

    /// How often the key set was fetched.
    pub(super) fn key_set_fetches(&self) -> usize {
        lock(&self.state).key_set_fetches
    }

    /// Each batch of shared Diagnostics the Hub took, with the signal its path named.
    pub(super) fn shared(&self) -> Vec<(String, Vec<u8>)> {
        lock(&self.state).shared.clone()
    }

    /// `claims` signed as the Hub signs them.
    pub(super) fn sign(&self, claims: &Value) -> String {
        sign(&self.key, claims)
    }

    /// Goes down, as if unreachable.
    pub(super) fn stop(&self) {
        self.serving.abort();
    }
}

impl Drop for FakeHub {
    fn drop(&mut self) {
        self.serving.abort();
    }
}

struct Shared {
    state: Arc<Mutex<State>>,
    key: Arc<Ed25519KeyPair>,
}

/// `ServerLinkService`.
struct Starting(Arc<Shared>);

/// `LinkedServerService`.
struct Linked(Arc<Shared>);

#[expect(
    refining_impl_trait,
    clippy::unused_async_trait_impl,
    reason = "plain `async fn` returning the owned response is the form the generated trait \
              documents, and a fake answers at once"
)]
impl ServerLinkService for Starting {
    async fn start_server_link(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, StartServerLinkRequest>,
    ) -> ServiceResult<StartServerLinkResponse> {
        lock(&self.0.state).starting = Some(request.name.to_owned());
        Response::ok(StartServerLinkResponse {
            device_code: DEVICE_CODE.to_owned(),
            user_code: USER_CODE.to_owned(),
            expires_in_seconds: 600,
            // So a test's every poll asks.
            interval_seconds: 0,
            ..Default::default()
        })
    }

    async fn poll_server_link(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, PollServerLinkRequest>,
    ) -> ServiceResult<PollServerLinkResponse> {
        let mut state = lock(&self.0.state);
        if request.device_code != DEVICE_CODE || state.starting.is_none() {
            return Err(ConnectError::not_found("the code expired"));
        }
        if !state.entered {
            return Response::ok(PollServerLinkResponse::default());
        }
        state.starting = None;
        state.linked = true;
        state.credentials += 1;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let token = sign(
            &self.0.key,
            &json!({ "sub": "server", "email": EMAIL, "iat": now, "exp": now + 300 }),
        );
        Response::ok(PollServerLinkResponse {
            linked: true,
            credential: credential(state.credentials),
            token,
            ..Default::default()
        })
    }
}

#[expect(
    refining_impl_trait,
    clippy::unused_async_trait_impl,
    reason = "plain `async fn` returning the owned response is the form the generated trait \
              documents, and a fake answers at once"
)]
impl LinkedServerService for Linked {
    async fn unlink_server(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, UnlinkServerRequest>,
    ) -> ServiceResult<UnlinkServerResponse> {
        let mut state = self.caller(&ctx)?;
        state.linked = false;
        state.user_codes.clear();
        state.user_accounts.clear();
        Response::ok(UnlinkServerResponse::default())
    }

    async fn start_user_link(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, StartUserLinkRequest>,
    ) -> ServiceResult<StartUserLinkResponse> {
        let mut state = self.caller(&ctx)?;
        if state.user_accounts.contains_key(&request.user_id) {
            return Err(ConnectError::already_exists("the User is Linked already"));
        }
        state
            .user_codes
            .insert(request.user_id, request.name.to_owned());
        Response::ok(StartUserLinkResponse {
            user_code: USER_LINK_CODE.to_owned(),
            expires_in_seconds: 600,
            ..Default::default()
        })
    }

    async fn get_user_link(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, GetUserLinkRequest>,
    ) -> ServiceResult<GetUserLinkResponse> {
        let mut state = self.caller(&ctx)?;
        state.user_link_asks += 1;
        Response::ok(GetUserLinkResponse {
            account: state
                .user_accounts
                .get(&request.user_id)
                .cloned()
                .unwrap_or_default(),
            ..Default::default()
        })
    }

    async fn unlink_user(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, UnlinkUserRequest>,
    ) -> ServiceResult<UnlinkUserResponse> {
        let mut state = self.caller(&ctx)?;
        state.user_codes.remove(&request.user_id);
        state.user_accounts.remove(&request.user_id);
        Response::ok(UnlinkUserResponse::default())
    }

    async fn push_watch_states(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, PushWatchStatesRequest>,
    ) -> ServiceResult<PushWatchStatesResponse> {
        let mut state = self.caller(&ctx)?;
        if let Some(failure) = state.push_failure {
            return Err(ConnectError::unavailable(failure));
        }
        state.pushes.push(request.to_owned_message().changes);
        Response::ok(PushWatchStatesResponse::default())
    }

    async fn set_server_addresses(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SetServerAddressesRequest>,
    ) -> ServiceResult<SetServerAddressesResponse> {
        let mut state = self.caller(&ctx)?;
        if state.addresses_failures > 0 {
            state.addresses_failures -= 1;
            return Err(ConnectError::unavailable("the fake Hub blinked"));
        }
        let lan = request
            .lan_addresses
            .iter()
            .map(|&address| address.to_owned());
        state
            .reported_addresses
            .push((request.url.to_owned(), lan.collect()));
        Response::ok(SetServerAddressesResponse::default())
    }
}

impl Linked {
    /// What the fake knows, if the call carries the credential of the Server it has Linked.
    fn caller(&self, ctx: &RequestContext) -> Result<MutexGuard<'_, State>, ConnectError> {
        let state = lock(&self.0.state);
        let bearer = format!("Bearer {}", credential(state.credentials));
        if !state.linked
            || ctx
                .header("authorization")
                .is_none_or(|sent| sent != bearer.as_str())
        {
            return Err(ConnectError::unauthenticated("no Linked Server has it"));
        }
        Ok(state)
    }
}

/// Keeps a batch of shared Diagnostics from the Server holding the credential, as the Hub forwards
/// it, and refuses anyone else.
fn take_shared(
    state: &Mutex<State>,
    signal: String,
    headers: &HeaderMap,
    batch: &Bytes,
) -> StatusCode {
    let mut state = lock(state);
    let bearer = format!("Bearer {}", credential(state.credentials));
    if !state.linked || headers["authorization"] != bearer.as_str() {
        return StatusCode::UNAUTHORIZED;
    }
    state.shared.push((signal, batch.to_vec()));
    StatusCode::OK
}

/// The `nth` credential the fake hands out.
fn credential(nth: usize) -> String {
    format!("credential-{nth}")
}

fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    state.lock().unwrap_or_else(PoisonError::into_inner)
}

const KID: &str = "fake-key";

fn key_set(key: &Ed25519KeyPair) -> Value {
    json!({ "keys": [{
        "kty": "OKP",
        "crv": "Ed25519",
        "x": URL_SAFE_NO_PAD.encode(key.public_key()),
        "kid": KID,
        "alg": "EdDSA",
        "use": "sig",
    }]})
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
