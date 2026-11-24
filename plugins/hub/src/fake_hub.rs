//! An in-process stand-in for the Hub, so the tests need no network and no PostgreSQL, and for the
//! Server's Link with it and its Users', which the Kernel keeps ([`KernelLink`]). It serves one
//! Linked Server, the Watch State it pushes, which its Linked Users pull back, their Watchlists,
//! its Viewing Events, the titles like The Matrix, what's trending and most watchlisted, and two
//! titles recommended to each Linked User, as the Hub's API (`proto/nama/hub/v1/`) says the Hub
//! does, and takes shared Diagnostics.

use std::{
    collections::HashMap,
    future,
    net::SocketAddr,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use axum::{
    body::Bytes,
    extract::Path,
    http::{HeaderMap, StatusCode},
};
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
use tokio::{net::TcpListener, task::JoinHandle};

use crate::KernelLink;

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
    /// Each batch of shared Diagnostics, with the signal its path named.
    shared: Vec<(String, Vec<u8>)>,
    /// The email of the Account each Linked User is Linked to, by id.
    user_accounts: HashMap<i64, String>,
    /// How many calls came with a Server's credential, known or not.
    server_calls: usize,
    /// Why the Kernel can't tell whether a User is Linked, while it can't.
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
        let shared = Arc::new(Shared {
            state: Arc::clone(&state),
        });
        let rpcs = connectrpc::Router::new()
            .add_service(Arc::new(Linked(shared)))
            .into_axum_service();
        let app = axum::Router::new()
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

    /// Whether the Server keeps a Link.
    pub(super) fn server_keeps_link(&self) -> bool {
        lock(&self.kept).is_some()
    }

    /// The Kernel's Link with the fake.
    pub(super) fn kernel_link(&self) -> Arc<dyn KernelLink> {
        Arc::new(KeptLink {
            url: self.url(),
            kept: Arc::clone(&self.kept),
            state: Arc::clone(&self.state),
        })
    }

    /// Forgets the Server's credential, as deleting its Account does. The Kernel still has its
    /// Users Linked, as it does until it next asks the Hub.
    pub(super) fn forget_server(&self) {
        lock(&self.state).linked = false;
    }

    /// An Account Links the User `user_id` on the Hub, as through a code another Device showed.
    pub(super) fn link_user_on_hub(&self, user_id: i64) {
        lock(&self.state)
            .user_accounts
            .insert(user_id, EMAIL.to_owned());
    }

    /// How many calls the Server made with its credential, whether the fake knew it or not.
    pub(super) fn server_calls(&self) -> usize {
        lock(&self.state).server_calls
    }

    /// Has the Kernel fail to tell whether a User is Linked.
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

    /// Each Viewing Event recorded, in order.
    pub(super) fn viewing_events(&self) -> Vec<UserViewingEvent> {
        lock(&self.state).viewing_events.clone()
    }

    /// Has recording Viewing Events fail with `failure`, such as `unavailable` for a Hub that is
    /// down, or work again with `None`.
    pub(super) fn fail_recording(&self, failure: Option<ErrorCode>) {
        lock(&self.state).recording_failure = failure;
    }

    /// Each batch of shared Diagnostics the Hub took, with the signal its path named.
    pub(super) fn shared(&self) -> Vec<(String, Vec<u8>)> {
        lock(&self.state).shared.clone()
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
        _ctx: RequestContext,
        _request: ServiceRequest<'_, StartUserLinkRequest>,
    ) -> ServiceResult<StartUserLinkResponse> {
        Err(kernels_call())
    }

    async fn get_user_link(
        &self,
        _ctx: RequestContext,
        _request: ServiceRequest<'_, GetUserLinkRequest>,
    ) -> ServiceResult<GetUserLinkResponse> {
        Err(kernels_call())
    }

    async fn unlink_user(
        &self,
        _ctx: RequestContext,
        _request: ServiceRequest<'_, UnlinkUserRequest>,
    ) -> ServiceResult<UnlinkUserResponse> {
        Err(kernels_call())
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
        _ctx: RequestContext,
        _request: ServiceRequest<'_, InviteUserRequest>,
    ) -> ServiceResult<InviteUserResponse> {
        Err(kernels_call())
    }

    async fn start_server_sign_in(
        &self,
        _ctx: RequestContext,
        _request: ServiceRequest<'_, StartServerSignInRequest>,
    ) -> ServiceResult<StartServerSignInResponse> {
        Err(kernels_call())
    }

    async fn poll_server_sign_in(
        &self,
        _ctx: RequestContext,
        _request: ServiceRequest<'_, PollServerSignInRequest>,
    ) -> ServiceResult<PollServerSignInResponse> {
        Err(kernels_call())
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

fn lock<T>(state: &Mutex<T>) -> MutexGuard<'_, T> {
    state.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What the fake answers a call only the Kernel makes, as the Server's own Link.
fn kernels_call() -> ConnectError {
    ConnectError::unimplemented("the Kernel makes this call, not the Plugin")
}

/// The Kernel's Link with the fake: the credential the Server keeps, and its Users' Links as the
/// fake has them.
struct KeptLink {
    url: String,
    kept: Arc<Mutex<Option<String>>>,
    state: Arc<Mutex<State>>,
}

impl std::fmt::Debug for KeptLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeptLink")
            .field("url", &self.url)
            .finish_non_exhaustive()
    }
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

    /// Whether the fake has `user_id` Linked, as the Kernel last heard: the Kernel's caching is
    /// its own to test.
    fn user_linked(&self, user_id: i64) -> BoxFuture<'_, Result<bool, PluginError>> {
        let state = lock(&self.state);
        let linked = match state.user_link_failure {
            Some(failure) => Err(failure.into()),
            None => Ok(lock(&self.kept).is_some() && state.user_accounts.contains_key(&user_id)),
        };
        Box::pin(future::ready(linked))
    }
}
