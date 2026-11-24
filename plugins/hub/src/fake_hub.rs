//! An in-process stand-in for the Hub, so the tests need no network and no PostgreSQL, and for the
//! Server's Link with it, which the Kernel keeps ([`KernelLink`]). It serves one Linked Server, its
//! Users' Links, Users created from Accounts, one sign-in
//! with an Account at a time, its Server Addresses and whether its public port is reached, the
//! Watch State it pushes, which its Linked Users pull back, their Watchlists, its Viewing Events,
//! the titles like The Matrix, what's trending and most watchlisted, two titles recommended to
//! each Linked User, and a key set of one Ed25519 key it signs with, as the Hub's API
//! (`proto/nama/hub/v1/`) says the Hub does.

use std::{
    collections::HashMap,
    future,
    net::SocketAddr,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use connectrpc::{
    ConnectError, ErrorCode, RequestContext, Response, ServiceRequest, ServiceResult,
};
use nama_hub_proto::nama::hub::v1::{
    ExternalId, GetRelayTokenRequest, GetRelayTokenResponse, GetUserLinkRequest,
    GetUserLinkResponse, InviteUserRequest, InviteUserResponse, LinkedServerService,
    ListMostWatchlistedTitlesRequest, ListMostWatchlistedTitlesResponse,
    ListRecommendedTitlesRequest, ListRecommendedTitlesResponse, ListSimilarTitlesRequest,
    ListSimilarTitlesResponse, ListTrendingTitlesRequest, ListTrendingTitlesResponse,
    PollServerSignInRequest, PollServerSignInResponse, PullTrackPreferencesRequest,
    PullTrackPreferencesResponse, PullWatchStatesRequest, PullWatchStatesResponse,
    PushTrackPreferencesRequest, PushTrackPreferencesResponse, PushWatchStatesRequest,
    PushWatchStatesResponse, RecommendedTitle, RecordViewingEventsRequest,
    RecordViewingEventsResponse, SetCertificateChallengeRequest, SetCertificateChallengeResponse,
    SetServerAddressesRequest, SetServerAddressesResponse, SimilarTitle, StartServerSignInRequest,
    StartServerSignInResponse, StartUserLinkRequest, StartUserLinkResponse, SyncWatchlistRequest,
    SyncWatchlistResponse, TitleKind, TrackPreferenceChange, TrendingTitle, UnlinkServerRequest,
    UnlinkServerResponse, UnlinkUserRequest, UnlinkUserResponse, UserViewingEvent,
    WatchStateChange, WatchlistEntry, WatchlistedTitle,
};
use nama_plugin_api::{BoxFuture, PluginError};
use ring::signature::Ed25519KeyPair;
use serde_json::{Value, json};
use tokio::{net::TcpListener, task::JoinHandle};

use crate::KernelLink;

/// The one code the fake gives a User.
pub(super) const USER_LINK_CODE: &str = "CDFG-HJKL";

/// The device code of the one sign-in with an Account the fake starts.
const SIGN_IN_DEVICE_CODE: &str = "sign-in-device-code";

/// The id of that sign-in.
pub(super) const SIGN_IN_ID: &str = "sign-in-id";

/// The one-time code approving that sign-in sends the person back with.
const SIGN_IN_CODE: &str = "sign-in-code";

/// The email of the Account the code is entered by.
pub(super) const EMAIL: &str = "ada@example.com";

/// What the fake Hub knows.
#[derive(Default)]
struct State {
    /// Whether the Server holds a credential the fake knows.
    linked: bool,
    /// How many credentials the fake handed out. Each Link gets a new one, as on the Hub, and only
    /// the last works.
    credentials: usize,
    /// The name each User waiting on a code started with, by id.
    user_codes: HashMap<i64, String>,
    /// The email of the Account each Linked User is Linked to, by id.
    user_accounts: HashMap<i64, String>,
    /// How often a User's Link was asked about.
    user_link_asks: usize,
    /// How many calls came with a Server's credential, known or not.
    server_calls: usize,
    /// Why asking about a User's Link fails, as if the Hub were down, while it does.
    user_link_failure: Option<&'static str>,
    /// Each Viewing Event recorded, in order.
    viewing_events: Vec<UserViewingEvent>,
    /// What recording Viewing Events fails with, if it does.
    recording_failure: Option<ErrorCode>,
    /// Each push's changes, in the order they came.
    pushes: Vec<Vec<WatchStateChange>>,
    /// Why every call with the Linked Server's credential fails, as if the Hub were failing, while
    /// it does.
    server_failure: Option<&'static str>,
    /// Why pushes fail, as if the Hub were down, while they do.
    push_failure: Option<&'static str>,
    /// Each push's Track Preference changes, in the order they came.
    preference_pushes: Vec<Vec<TrackPreferenceChange>>,
    /// Each Watchlist entry Linked Users synced, in the order they came.
    watchlist: Vec<WatchlistEntry>,
    /// The User each Watchlist sync was of, in the order they came.
    watchlist_syncs: Vec<i64>,
    /// The name and email each User created from an Account was invited with, by id.
    invites: HashMap<i64, (String, String)>,
    /// The sign-in with an Account waiting: where it returns to, and the User it signs in as once
    /// approved.
    sign_in: Option<(String, Option<i64>)>,
}

/// The Hub, serving until [`FakeHub::stop`] or its drop.
pub(super) struct FakeHub {
    addr: SocketAddr,
    state: Arc<Mutex<State>>,
    /// The credential the Server keeps, as the Kernel would.
    kept: Arc<Mutex<Option<String>>>,
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
            .add_service(Arc::new(Linked(shared)))
            .into_axum_service();
        let app = axum::Router::new().fallback_service(rpcs);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let serving = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self {
            addr,
            state,
            kept: Arc::default(),
            serving,
        }
    }

    pub(super) fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// The Server Links, as the Kernel does it, getting a new credential.
    pub(super) fn link_server(&self) {
        let mut state = lock(&self.state);
        state.linked = true;
        state.credentials += 1;
        *lock(&self.kept) = Some(credential(state.credentials));
    }

    /// The Server Unlinks, as the Kernel does it, which Unlinks its Users too.
    pub(super) fn unlink_server(&self) {
        self.forget_server();
        *lock(&self.kept) = None;
    }

    /// Whether the Server keeps a Link.
    pub(super) fn server_keeps_link(&self) -> bool {
        lock(&self.kept).is_some()
    }

    /// The Kernel's Link with the fake.
    pub(super) fn kernel_link(&self) -> Arc<dyn KernelLink> {
        Arc::new(KeptLink {
            url: self.url(),
            kept: Arc::clone(&self.kept),
        })
    }

    /// Forgets the Server's credential and its Users' Links, as deleting its Account does.
    pub(super) fn forget_server(&self) {
        let mut state = lock(&self.state);
        state.linked = false;
        state.user_codes.clear();
        state.user_accounts.clear();
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

    /// How many calls the Server made with its credential, whether the fake knew it or not.
    pub(super) fn server_calls(&self) -> usize {
        lock(&self.state).server_calls
    }

    /// Has asking about a User's Link fail, though counted, as if the Hub were down.
    pub(super) fn fail_user_links(&self) {
        lock(&self.state).user_link_failure = Some("the Hub is down");
    }

    /// The User each Watchlist sync was of, in the order they came.
    pub(super) fn watchlist_syncs(&self) -> Vec<i64> {
        lock(&self.state).watchlist_syncs.clone()
    }

    /// Each push's changes, in the order they came.
    pub(super) fn pushes(&self) -> Vec<Vec<WatchStateChange>> {
        lock(&self.state).pushes.clone()
    }

    /// Each push's Track Preference changes, in the order they came.
    pub(super) fn preference_pushes(&self) -> Vec<Vec<TrackPreferenceChange>> {
        lock(&self.state).preference_pushes.clone()
    }

    /// Has pushes fail while `down`, as if the Hub were unreachable, and work again after.
    pub(super) fn set_down(&self, down: bool) {
        lock(&self.state).push_failure = down.then_some("the Hub is down");
    }

    /// Has every call with the Linked Server's credential fail, as a Hub that answers but fails.
    pub(super) fn fail_server_calls(&self) {
        lock(&self.state).server_failure = Some("the Hub is failing");
    }

    /// The name and email the User `user_id` was invited with.
    pub(super) fn invite(&self, user_id: i64) -> Option<(String, String)> {
        lock(&self.state).invites.get(&user_id).cloned()
    }

    /// Where the waiting sign-in returns to.
    pub(super) fn sign_in_returns_to(&self) -> Option<String> {
        lock(&self.state)
            .sign_in
            .as_ref()
            .map(|(url, _)| url.clone())
    }

    /// An Account approves the waiting sign-in as its User `user_id`, and gets the one-time code
    /// that finishes it.
    pub(super) fn approve_sign_in(&self, user_id: i64) -> String {
        if let Some((_, user)) = &mut lock(&self.state).sign_in {
            *user = Some(user_id);
        }
        SIGN_IN_CODE.to_owned()
    }

    /// Each Viewing Event recorded, in order.
    pub(super) fn viewing_events(&self) -> Vec<UserViewingEvent> {
        lock(&self.state).viewing_events.clone()
    }

    /// Has recording Viewing Events fail with `failure`, such as `unavailable` for a Hub that is
    /// down, or work again with `None`.
    pub(super) fn fail_recording(&self, failure: Option<ErrorCode>) {
        lock(&self.state).recording_failure = failure;
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

/// `LinkedServerService`.
struct Linked(Arc<Shared>);

#[expect(
    refining_impl_trait,
    clippy::unused_async_trait_impl,
    reason = "plain `async fn` returning the owned response is the form the generated trait \
              documents, and a fake answers at once"
)]
impl LinkedServerService for Linked {
    async fn unlink_server(
        &self,
        _ctx: RequestContext,
        _request: ServiceRequest<'_, UnlinkServerRequest>,
    ) -> ServiceResult<UnlinkServerResponse> {
        Err(kernels_call())
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
        if let Some(failure) = state.user_link_failure {
            return Err(ConnectError::unavailable(failure));
        }
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

    /// Every change pushed, as the asked User's, if they are Linked: the fake has one Account.
    async fn pull_watch_states(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, PullWatchStatesRequest>,
    ) -> ServiceResult<PullWatchStatesResponse> {
        let state = self.caller(&ctx)?;
        let user_id = request.user_id;
        let watch_states = if state.user_accounts.contains_key(&user_id) {
            (state.pushes.iter().flatten())
                .map(|change| WatchStateChange {
                    user_id,
                    ..change.clone()
                })
                .collect()
        } else {
            Vec::new()
        };
        Response::ok(PullWatchStatesResponse {
            watch_states,
            ..Default::default()
        })
    }

    async fn push_track_preferences(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, PushTrackPreferencesRequest>,
    ) -> ServiceResult<PushTrackPreferencesResponse> {
        let mut state = self.caller(&ctx)?;
        if let Some(failure) = state.push_failure {
            return Err(ConnectError::unavailable(failure));
        }
        state
            .preference_pushes
            .push(request.to_owned_message().changes);
        Response::ok(PushTrackPreferencesResponse::default())
    }

    /// Every Track Preference change pushed, as the asked User's, if they are Linked: the fake
    /// has one Account, and merges nothing.
    async fn pull_track_preferences(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, PullTrackPreferencesRequest>,
    ) -> ServiceResult<PullTrackPreferencesResponse> {
        let state = self.caller(&ctx)?;
        let user_id = request.user_id;
        let preferences = if state.user_accounts.contains_key(&user_id) {
            (state.preference_pushes.iter().flatten())
                .map(|change| TrackPreferenceChange {
                    user_id,
                    ..change.clone()
                })
                .collect()
        } else {
            Vec::new()
        };
        Response::ok(PullTrackPreferencesResponse {
            preferences,
            ..Default::default()
        })
    }

    /// Every entry Linked Users synced, as the asked User's Account's: the fake has one Account.
    async fn sync_watchlist(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SyncWatchlistRequest>,
    ) -> ServiceResult<SyncWatchlistResponse> {
        let mut state = self.caller(&ctx)?;
        let SyncWatchlistRequest {
            user_id, entries, ..
        } = request.to_owned_message();
        state.watchlist_syncs.push(user_id);
        if !state.user_accounts.contains_key(&user_id) {
            return Response::ok(SyncWatchlistResponse::default());
        }
        state.watchlist.extend(entries);
        Response::ok(SyncWatchlistResponse {
            entries: state.watchlist.clone(),
            ..Default::default()
        })
    }

    async fn invite_user(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, InviteUserRequest>,
    ) -> ServiceResult<InviteUserResponse> {
        let invite = (request.name.to_owned(), request.email.to_owned());
        self.caller(&ctx)?.invites.insert(request.user_id, invite);
        Response::ok(InviteUserResponse::default())
    }

    async fn start_server_sign_in(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, StartServerSignInRequest>,
    ) -> ServiceResult<StartServerSignInResponse> {
        self.caller(&ctx)?.sign_in = Some((request.return_url.to_owned(), None));
        Response::ok(StartServerSignInResponse {
            device_code: SIGN_IN_DEVICE_CODE.to_owned(),
            id: SIGN_IN_ID.to_owned(),
            expires_in_seconds: 600,
            ..Default::default()
        })
    }

    async fn poll_server_sign_in(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, PollServerSignInRequest>,
    ) -> ServiceResult<PollServerSignInResponse> {
        let mut state = self.caller(&ctx)?;
        let Some((_, approved)) = state
            .sign_in
            .as_ref()
            .filter(|_| request.device_code == SIGN_IN_DEVICE_CODE)
        else {
            return Err(ConnectError::not_found("the sign-in expired"));
        };
        let Some(user) = *approved else {
            return Response::ok(PollServerSignInResponse::default());
        };
        state.sign_in = None;
        if request.code != SIGN_IN_CODE {
            return Err(ConnectError::not_found("the code is wrong"));
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let claims =
            json!({ "sub": user.to_string(), "aud": "server", "iat": now, "exp": now + 300 });
        Response::ok(PollServerSignInResponse {
            token: sign(&self.0.key, &claims),
            ..Default::default()
        })
    }

    async fn set_certificate_challenge(
        &self,
        _ctx: RequestContext,
        _request: ServiceRequest<'_, SetCertificateChallengeRequest>,
    ) -> ServiceResult<SetCertificateChallengeResponse> {
        Err(kernels_call())
    }

    async fn get_relay_token(
        &self,
        _ctx: RequestContext,
        _request: ServiceRequest<'_, GetRelayTokenRequest>,
    ) -> ServiceResult<GetRelayTokenResponse> {
        Err(kernels_call())
    }

    async fn set_server_addresses(
        &self,
        _ctx: RequestContext,
        _request: ServiceRequest<'_, SetServerAddressesRequest>,
    ) -> ServiceResult<SetServerAddressesResponse> {
        Err(kernels_call())
    }

    async fn record_viewing_events(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, RecordViewingEventsRequest>,
    ) -> ServiceResult<RecordViewingEventsResponse> {
        let mut state = self.caller(&ctx)?;
        if let Some(failure) = state.recording_failure {
            return Err(ConnectError::new(failure, "recording failed"));
        }
        let recorded = request.to_owned_message().events;
        state.viewing_events.extend(recorded);
        Response::ok(RecordViewingEventsResponse::default())
    }

    /// The Matrix's sequel and Alien, by IMDb id, for a Movie asked about by The Matrix's TMDB id;
    /// nothing for any other title.
    async fn list_similar_titles(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, ListSimilarTitlesRequest>,
    ) -> ServiceResult<ListSimilarTitlesResponse> {
        drop(self.caller(&ctx)?);
        let the_matrix = request.kind.as_known() == Some(TitleKind::TITLE_KIND_MOVIE)
            && (request.ids.iter()).any(|id| id.namespace == "tmdb" && id.value == "603");
        let titles = if the_matrix {
            vec![imdb("tt0234215", 0.9), imdb("tt0078748", 0.35)]
        } else {
            vec![]
        };
        Response::ok(ListSimilarTitlesResponse {
            titles,
            ..Default::default()
        })
    }

    async fn list_recommended_titles(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, ListRecommendedTitlesRequest>,
    ) -> ServiceResult<ListRecommendedTitlesResponse> {
        let state = self.caller(&ctx)?;
        let titles = if state.user_accounts.contains_key(&request.user_id) {
            vec![
                recommended(TitleKind::TITLE_KIND_MOVIE, "604", 0.9),
                recommended(TitleKind::TITLE_KIND_SHOW, "1396", 0.4),
            ]
        } else {
            vec![]
        };
        Response::ok(ListRecommendedTitlesResponse {
            titles,
            ..Default::default()
        })
    }

    /// The Matrix, Blade Runner and Toy Story, by TMDB id, most watched first.
    async fn list_trending_titles(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, ListTrendingTitlesRequest>,
    ) -> ServiceResult<ListTrendingTitlesResponse> {
        drop(self.caller(&ctx)?);
        let trending = |value: &str, weight| TrendingTitle {
            id: ExternalId {
                namespace: "tmdb".to_owned(),
                value: value.to_owned(),
                ..Default::default()
            }
            .into(),
            weight,
            ..Default::default()
        };
        Response::ok(ListTrendingTitlesResponse {
            titles: vec![
                trending("603", 1.0),
                trending("78", 0.9),
                trending("862", 0.5),
            ],
            ..Default::default()
        })
    }

    /// Dark, The Matrix and Blade Runner, by TMDB id, most watchlisted first.
    async fn list_most_watchlisted_titles(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, ListMostWatchlistedTitlesRequest>,
    ) -> ServiceResult<ListMostWatchlistedTitlesResponse> {
        drop(self.caller(&ctx)?);
        let watchlisted = |kind: TitleKind, value: &str, weight| WatchlistedTitle {
            kind: kind.into(),
            id: ExternalId {
                namespace: "tmdb".to_owned(),
                value: value.to_owned(),
                ..Default::default()
            }
            .into(),
            weight,
            ..Default::default()
        };
        Response::ok(ListMostWatchlistedTitlesResponse {
            titles: vec![
                watchlisted(TitleKind::TITLE_KIND_SHOW, "70523", 1.0),
                watchlisted(TitleKind::TITLE_KIND_MOVIE, "603", 0.8),
                watchlisted(TitleKind::TITLE_KIND_MOVIE, "78", 0.5),
            ],
            ..Default::default()
        })
    }
}

impl Linked {
    /// What the fake knows, if the call carries the credential of the Server it has Linked.
    fn caller(&self, ctx: &RequestContext) -> Result<MutexGuard<'_, State>, ConnectError> {
        let mut state = lock(&self.0.state);
        state.server_calls += 1;
        let bearer = format!("Bearer {}", credential(state.credentials));
        if !state.linked
            || ctx
                .header("authorization")
                .is_none_or(|sent| sent != bearer.as_str())
        {
            return Err(ConnectError::unauthenticated("no Linked Server has it"));
        }
        if let Some(failure) = state.server_failure {
            return Err(ConnectError::internal(failure));
        }
        Ok(state)
    }
}

fn imdb(value: &str, weight: f32) -> SimilarTitle {
    SimilarTitle {
        id: ExternalId {
            namespace: "imdb".to_owned(),
            value: value.to_owned(),
            ..Default::default()
        }
        .into(),
        weight,
        ..Default::default()
    }
}

fn recommended(kind: TitleKind, tmdb: &str, weight: f32) -> RecommendedTitle {
    RecommendedTitle {
        kind: kind.into(),
        id: ExternalId {
            namespace: "tmdb".to_owned(),
            value: tmdb.to_owned(),
            ..Default::default()
        }
        .into(),
        weight,
        ..Default::default()
    }
}

/// The `nth` credential the fake hands out.
fn credential(nth: usize) -> String {
    format!("credential-{nth}")
}

fn lock<T>(state: &Mutex<T>) -> MutexGuard<'_, T> {
    state.lock().unwrap_or_else(PoisonError::into_inner)
}

const KID: &str = "fake-key";

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

/// What the fake answers a call only the Kernel makes, as the Server's own Link.
fn kernels_call() -> ConnectError {
    ConnectError::unimplemented("the Kernel makes this call, not the Plugin")
}

/// The Kernel's Link with the fake: the credential the Server keeps.
#[derive(Debug)]
struct KeptLink {
    url: String,
    kept: Arc<Mutex<Option<String>>>,
}

impl KernelLink for KeptLink {
    fn hub_url(&self) -> &str {
        &self.url
    }

    fn credential(&self) -> BoxFuture<'_, Result<Option<String>, PluginError>> {
        Box::pin(future::ready(Ok(lock(&self.kept).clone())))
    }

    fn forget<'a>(&'a self, credential: &'a str) -> BoxFuture<'a, Result<(), PluginError>> {
        lock(&self.kept).take_if(|kept| kept == credential);
        Box::pin(future::ready(Ok(())))
    }

    /// The token's `sub`, unchecked: the Kernel checks the signature (`link/token.rs` there).
    fn token_subject<'a>(&'a self, token: &'a str) -> BoxFuture<'a, Result<String, PluginError>> {
        let claims = token.split('.').nth(1).unwrap();
        let claims: Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(claims).unwrap()).unwrap();
        let sub = claims["sub"].as_str().unwrap().to_owned();
        Box::pin(future::ready(Ok(sub)))
    }
}
