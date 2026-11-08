//! Requesting Movies and Seasons through the generated `RequestService` client, over a real
//! socket, with the TMDB Plugin enabled against a fake TMDB.

#[expect(dead_code, reason = "these tests fetch only whole files")]
#[path = "support/http.rs"]
mod http;
#[path = "support/refused.rs"]
mod refused;
#[expect(dead_code, reason = "the harness's plain `start` has no use here")]
#[path = "support/server.rs"]
mod server;

use std::{
    collections::{BTreeMap, HashMap},
    fs,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use connectrpc::{
    ConnectError, ErrorCode,
    client::{ClientConfig, HttpClient, ServerStream},
};
use http::fetch;
use nama_plugin_api::{
    BoxFuture, Plugin, PluginError, PluginId, Priority, Registrar,
    fulfiller::{Download, Fulfiller, Movies},
    settings::{SettingValue, Settings},
};
use nama_plugin_tmdb::{
    Tmdb,
    testing::{API_KEY, FakeTmdb},
};
use nama_proto::nama::v1::{
    __buffa::view::SubscribeResponseView, ApproveRequestRequest, BrowseServiceClient,
    CancelRequestRequest, CreateLibraryRequest, CreateRequestRequest, CreateSeasonRequestsRequest,
    CreateUserRequest, DeclineRequestRequest, EventServiceClient, EventType, FoundMovie,
    GetRequestRequest, LibraryKind, LibraryServiceClient, ListMoviesRequest, ListRequestsRequest,
    ListShowsRequest, LookUpMovieRequest, Permission, RefusalReason, Request, RequestServiceClient,
    RequestState, RetryRequestRequest, ScanLibraryRequest, SearchMoviesRequest, SessionDelivery,
    SetPermissionsRequest, SignInRequest, SignInServiceClient, SubscribeRequest, SubscribeResponse,
    UserServiceClient, WithdrawRequestRequest, event, subscribe_response,
};
use nama_server::{config::Config, db::Database, plugins::Registry};
use refused::assert_refused;
use server::{PASSWORD, TestServer};

/// Movies the fake TMDB knows.
const ALIEN: i64 = 348;
const HEAT: i64 = 949;
const DUNE: i64 = 438_631;

/// A Show the fake TMDB knows, whose Season 1 has three aired Episodes and Season 2 one.
const DARK: i64 = 70523;
const DARK_S01E01: &str = "Dark {tmdb-70523}/Season 01/Dark - S01E01.mkv";
const DARK_S02E01: &str = "Dark {tmdb-70523}/Season 02/Dark - S02E01.mkv";
const THE_OFFICE: i64 = 2316;

const WAIT: Duration = Duration::from_secs(10);

/// What a new User gets: `REQUEST` and `VIEW_REQUESTS`.
const NEW_USER: [Permission; 2] = [Permission::Request, Permission::ViewRequests];

/// A Server with the TMDB Plugin enabled against a fake TMDB, and an Admin.
struct Fixture {
    server: TestServer,
    db: Arc<Database>,
    tmdb: FakeTmdb,
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    async fn start() -> Self {
        Self::start_with(Registry::new()).await
    }

    /// A Fixture whose Movie Fulfiller is `fulfiller`.
    async fn with_fulfiller(fulfiller: &FakeFulfiller) -> Self {
        let plugins = Registry::new();
        plugins
            .enable(Arc::new(FulfillerPlugin(fulfiller.clone())))
            .unwrap();
        Self::start_with(plugins).await
    }

    /// A Fixture with the Plugins enabled in `plugins` too.
    async fn start_with(plugins: Registry) -> Self {
        let tmdb = FakeTmdb::start().await;
        let settings = Settings::new(BTreeMap::from([(
            "api_key".to_owned(),
            SettingValue::Text(API_KEY.to_owned()),
        )]));
        plugins
            .enable_with_settings(Arc::new(Tmdb::with_endpoints(tmdb.endpoints())), settings)
            .unwrap();
        let (server, db) = TestServer::start_with_db(Config::default(), plugins).await;
        Self { server, db, tmdb }
    }

    /// The reference the Fulfiller gave for the Request `id`, as the Server stored it.
    async fn fulfiller_reference(&self, id: i64) -> Option<String> {
        sqlx::query_scalar("SELECT fulfiller_reference FROM requests WHERE id = ?")
            .bind(id)
            .fetch_one(self.db.reader())
            .await
            .unwrap()
    }

    /// Approves the Request `id` as the Admin, and returns it as the approval left it.
    async fn approve(&self, id: i64) -> Request {
        requests(self.server.client_config())
            .approve_request(ApproveRequestRequest {
                id,
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned()
            .request
            .into_option()
            .unwrap()
    }

    /// Creates a User named `name` with `permissions`, signs them in, and returns their ID and
    /// client settings that call as them.
    async fn user(&self, name: &str, permissions: &[Permission]) -> (i64, ClientConfig) {
        let as_admin = UserServiceClient::new(HttpClient::plaintext(), self.server.client_config());
        let id = as_admin
            .create_user(CreateUserRequest {
                name: name.to_owned(),
                password: PASSWORD.to_owned(),
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned()
            .id;
        as_admin
            .set_permissions(SetPermissionsRequest {
                user_id: id,
                permissions: permissions.iter().map(|&p| p.into()).collect(),
                ..Default::default()
            })
            .await
            .unwrap();
        let token = SignInServiceClient::new(
            HttpClient::plaintext(),
            self.server.anonymous_client_config(),
        )
        .sign_in(SignInRequest {
            name: name.to_owned(),
            password: PASSWORD.to_owned(),
            device_name: "Phone".to_owned(),
            delivery: SessionDelivery::SESSION_DELIVERY_TOKEN.into(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .token;
        (id, self.server.client_config_as(&token))
    }

    /// Creates a Movies Library holding Alien, scans it, and waits until Alien is Matched.
    /// Returns the folder and the Library's ID.
    async fn library_with_alien(&self) -> (tempfile::TempDir, i64) {
        self.library_with(
            LibraryKind::LIBRARY_KIND_MOVIES,
            &["Alien (1979)/Alien (1979).mkv"],
        )
        .await
    }

    /// Creates a Library of `kind` holding `files`, scans it, and waits until a title is Matched.
    /// Returns the folder and the Library's ID.
    async fn library_with(&self, kind: LibraryKind, files: &[&str]) -> (tempfile::TempDir, i64) {
        let media = tempfile::tempdir().unwrap();
        let folder = media.path().canonicalize().unwrap();
        for file in files {
            let file = folder.join(file);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, b"video").unwrap();
        }
        let library =
            LibraryServiceClient::new(HttpClient::plaintext(), self.server.client_config())
                .create_library(CreateLibraryRequest {
                    name: "Media".to_owned(),
                    kind: kind.into(),
                    folders: vec![folder.to_str().unwrap().to_owned()],
                    ..Default::default()
                })
                .await
                .unwrap()
                .into_owned()
                .library
                .into_option()
                .unwrap()
                .id;
        self.scan_until(library, kind, true).await;
        (media, library)
    }

    /// Scans the Library `library` of `kind`, and waits until it lists a title if `has_titles`, or
    /// none.
    async fn scan_until(&self, library: i64, kind: LibraryKind, has_titles: bool) {
        let config = self.server.client_config();
        LibraryServiceClient::new(HttpClient::plaintext(), config.clone())
            .scan_library(ScanLibraryRequest {
                id: library,
                ..Default::default()
            })
            .await
            .unwrap();
        let browse = BrowseServiceClient::new(HttpClient::plaintext(), config);
        tokio::time::timeout(WAIT, until_listed(&browse, library, kind, has_titles))
            .await
            .unwrap();
    }
}

/// Waits until the Library `library` of `kind` lists a Movie or a Show if `has_titles`, or none.
/// Matching stores a Show's Episodes together, so they are all Matched once it lists the Show.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn until_listed(
    browse: &BrowseServiceClient<HttpClient>,
    library: i64,
    kind: LibraryKind,
    has_titles: bool,
) {
    loop {
        let listed = if kind == LibraryKind::LIBRARY_KIND_MOVIES {
            let listing = ListMoviesRequest {
                library_id: library,
                ..Default::default()
            };
            browse
                .list_movies(listing)
                .await
                .unwrap()
                .into_owned()
                .movies
                .len()
        } else {
            let listing = ListShowsRequest {
                library_id: library,
                ..Default::default()
            };
            browse
                .list_shows(listing)
                .await
                .unwrap()
                .into_owned()
                .shows
                .len()
        };
        if (listed > 0) == has_titles {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

fn requests(config: ClientConfig) -> RequestServiceClient<HttpClient> {
    RequestServiceClient::new(HttpClient::plaintext(), config)
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn ask(client: &RequestServiceClient<HttpClient>, tmdb_id: i64) -> Request {
    client
        .create_request(CreateRequestRequest {
            movie_tmdb_id: tmdb_id,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .request
        .into_option()
        .unwrap()
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn ask_seasons(
    client: &RequestServiceClient<HttpClient>,
    show: i64,
    seasons: &[u32],
) -> Vec<Request> {
    client
        .create_season_requests(season_request(show, seasons))
        .await
        .unwrap()
        .into_owned()
        .requests
}

/// Asks for the Seasons `seasons` of the Show `show`.
fn season_request(show: i64, seasons: &[u32]) -> CreateSeasonRequestsRequest {
    CreateSeasonRequestsRequest {
        show_tmdb_id: show,
        season_numbers: seasons.to_vec(),
        ..Default::default()
    }
}

/// The Show title and Season number `request` asks for, if it asks for a Season.
fn season_of(request: &Request) -> Option<(&str, u32)> {
    let season = request.season.as_option()?;
    Some((season.show_title.as_str(), season.season_number))
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn list(client: &RequestServiceClient<HttpClient>) -> Vec<Request> {
    client
        .list_requests(ListRequestsRequest::default())
        .await
        .unwrap()
        .into_owned()
        .requests
}

/// The Movies a search for "Alien" finds, as the User `config` calls as sees them.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn search_alien(config: ClientConfig) -> Vec<FoundMovie> {
    requests(config)
        .search_movies(SearchMoviesRequest {
            title: "Alien".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .movies
}

fn requester_ids(request: &Request) -> Vec<i64> {
    request
        .requesters
        .iter()
        .map(|requester| requester.user_id)
        .collect()
}

#[tokio::test]
async fn two_users_asking_for_the_same_movie_share_one_request_with_both_as_requesters() {
    let fixture = Fixture::start().await;
    let (ada, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let (grace, as_grace) = fixture.user("Grace", &NEW_USER).await;

    let first = ask(&requests(as_ada), HEAT).await;
    let second = ask(&requests(as_grace.clone()), HEAT).await;
    let again = ask(&requests(as_grace.clone()), HEAT).await;

    assert_eq!(second.id, first.id);
    assert_eq!(requester_ids(&second), [ada, grace]);
    assert_eq!(again, second, "asking twice changes nothing");
    let movie = second.movie.as_option().map(|movie| movie.title.as_str());
    assert_eq!(movie, Some("Heat"), "{second:?}");
    assert_eq!(second.requesters[1].name, "Grace");
    assert_eq!(list(&requests(as_grace)).await, [second]);
}

#[tokio::test]
async fn a_user_who_may_auto_approve_movies_opens_an_approved_request_and_one_who_may_not_a_pending_one()
 {
    let fixture = Fixture::start().await;
    let (_, auto) = fixture
        .user(
            "Ada",
            &[Permission::RequestMovie, Permission::AutoApproveMovie],
        )
        .await;
    let (_, covered) = fixture
        .user("Linus", &[Permission::Request, Permission::AutoApprove])
        .await;
    let (_, as_grace) = fixture.user("Grace", &NEW_USER).await;

    let approved = ask(&requests(auto), HEAT).await;
    let also_approved = ask(&requests(covered), ALIEN).await;
    let pending = ask(&requests(as_grace), DUNE).await;

    assert_eq!(approved.state, RequestState::Approved);
    assert_eq!(also_approved.state, RequestState::Approved);
    assert_eq!(pending.state, RequestState::Pending);
}

#[tokio::test]
async fn a_user_without_request_movie_is_refused_naming_the_permission() {
    let fixture = Fixture::start().await;
    let (_, as_viewer) = fixture.user("Ada", &[Permission::ViewRequests]).await;

    let refused = requests(as_viewer)
        .create_request(CreateRequestRequest {
            movie_tmdb_id: HEAT,
            ..Default::default()
        })
        .await;

    let refusal = assert_refused(
        refused,
        ErrorCode::PermissionDenied,
        RefusalReason::REFUSAL_REASON_PERMISSION_REQUIRED,
    );
    let permission = refusal.params.get("permission").map(String::as_str);
    assert_eq!(permission, Some("PERMISSION_REQUEST_MOVIE"));
}

#[tokio::test]
async fn requesting_a_movie_matched_in_a_library_is_refused() {
    let fixture = Fixture::start().await;
    let (_media, _) = fixture.library_with_alien().await;
    let (_, as_grace) = fixture.user("Grace", &NEW_USER).await;

    let refused = requests(as_grace)
        .create_request(CreateRequestRequest {
            movie_tmdb_id: ALIEN,
            ..Default::default()
        })
        .await;

    let refusal = assert_refused(
        refused,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_ALREADY_IN_LIBRARY,
    );
    assert_eq!(refusal.field, "movie_tmdb_id");
}

#[tokio::test]
async fn requesting_a_movie_the_metadata_source_doesnt_have_is_refused() {
    let fixture = Fixture::start().await;
    let (_, as_grace) = fixture.user("Grace", &NEW_USER).await;

    let refused = requests(as_grace)
        .create_request(CreateRequestRequest {
            movie_tmdb_id: 1,
            ..Default::default()
        })
        .await;

    let refusal = assert_refused(
        refused,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_UNKNOWN_TITLE,
    );
    let title = refusal.params.get("titleId").map(String::as_str);
    assert_eq!(title, Some("tmdb-1"));
}

#[tokio::test]
async fn without_view_requests_a_user_lists_and_gets_only_their_own_requests() {
    let fixture = Fixture::start().await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let (_, as_grace) = fixture.user("Grace", &[Permission::RequestMovie]).await;
    let adas = ask(&requests(as_ada.clone()), HEAT).await;
    ask(&requests(as_ada.clone()), DUNE).await;
    let grace = requests(as_grace);
    let shared = ask(&grace, DUNE).await;

    let listed = list(&grace).await;
    let got_shared = grace
        .get_request(GetRequestRequest {
            id: shared.id,
            ..Default::default()
        })
        .await;
    let got_adas = grace
        .get_request(GetRequestRequest {
            id: adas.id,
            ..Default::default()
        })
        .await;

    assert_eq!(listed, std::slice::from_ref(&shared));
    let got_shared = got_shared.map(|response| response.into_owned().request.into_option());
    assert_eq!(got_shared.ok().flatten(), Some(shared.clone()));
    assert_refused(
        got_adas,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_REQUEST_NOT_FOUND,
    );
    assert_eq!(
        list(&requests(as_ada)).await,
        [shared, adas],
        "newest first"
    );
}

#[tokio::test]
async fn without_view_requests_the_events_stream_shows_only_the_users_own_requests() {
    let fixture = Fixture::start().await;
    let (ada, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let (_, as_grace) = fixture.user("Grace", &[Permission::RequestMovie]).await;
    let mut graces_events = subscribe_to_requests(as_grace.clone()).await;

    ask(&requests(as_ada.clone()), HEAT).await;
    let shared = ask(&requests(as_ada), DUNE).await;
    ask(&requests(as_grace), DUNE).await;
    let event = next_event(&mut graces_events).await;

    let event::Event::RequesterAdded(added) = event else {
        panic!("Ada's Requests reached Grace first: {event:?}");
    };
    assert_eq!(added.request.as_option().map(|r| r.id), Some(shared.id));
    assert_eq!(
        added.request.as_option().map(requester_ids),
        Some(vec![ada, added.user_id])
    );
}

#[tokio::test]
async fn with_view_requests_the_events_stream_shows_everyones_requests() {
    let fixture = Fixture::start().await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let (grace, as_grace) = fixture.user("Grace", &[Permission::RequestMovie]).await;
    let mut adas_events = subscribe_to_requests(as_ada).await;

    let graces = ask(&requests(as_grace), HEAT).await;
    let event = next_event(&mut adas_events).await;

    let event::Event::RequestCreated(created) = event else {
        panic!("not Grace's new Request: {event:?}");
    };
    assert_eq!(created.request.as_option(), Some(&graces));
    assert_eq!(requester_ids(&graces), [grace]);
}

#[tokio::test]
async fn an_admin_approves_a_members_pending_request_and_the_member_sees_it_live() {
    let fixture = Fixture::start().await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let pending = ask(&requests(as_ada.clone()), HEAT).await;
    let mut adas_events = subscribe_to_requests(as_ada).await;

    let approved = fixture.approve(pending.id).await;
    let event = next_event(&mut adas_events).await;

    assert_eq!(approved.state, RequestState::Approved);
    let event::Event::RequestStateChanged(changed) = event else {
        panic!("not the approval: {event:?}");
    };
    assert_eq!(changed.request.as_option(), Some(&approved));
}

#[tokio::test]
async fn an_approved_request_becomes_available_once_a_scan_matches_its_movie_and_the_member_sees_it_live()
 {
    let fixture = Fixture::start().await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let ada = requests(as_ada.clone());
    let pending = ask(&ada, ALIEN).await;
    fixture.approve(pending.id).await;
    let mut adas_events = subscribe_to_requests(as_ada).await;

    let _media = fixture.library_with_alien().await;
    let event = next_event(&mut adas_events).await;

    let event::Event::RequestStateChanged(changed) = event else {
        panic!("not the Request becoming Available: {event:?}");
    };
    assert_eq!(changed.request.state, RequestState::Available);
    assert_eq!(list(&ada).await, [changed.request.into_option().unwrap()]);
}

/// A Movie Fulfiller that takes every Request it is handed, giving `movie-<TMDB id>` as its
/// reference, unless it is refusing them all.
#[derive(Clone, Default)]
struct FakeFulfiller {
    /// The TMDB id of each Movie it was handed, in order.
    handed: Arc<Mutex<Vec<i64>>>,
    refuses: Arc<AtomicBool>,
}

impl FakeFulfiller {
    fn refusing() -> Self {
        let fulfiller = Self::default();
        fulfiller.refuses.store(true, Ordering::Relaxed);
        fulfiller
    }

    /// Takes every Request it is handed from now on.
    fn recover(&self) {
        self.refuses.store(false, Ordering::Relaxed);
    }

    fn handed(&self) -> Vec<i64> {
        self.handed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Fulfiller<Movies> for FakeFulfiller {
    fn fulfill<'a>(&'a self, request: &'a Request) -> BoxFuture<'a, Result<String, PluginError>> {
        let tmdb_id = request.movie.tmdb_id;
        self.handed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(tmdb_id);
        let refuses = self.refuses.load(Ordering::Relaxed);
        Box::pin(async move {
            if refuses {
                return Err("Radarr is unreachable".into());
            }
            Ok(format!("movie-{tmdb_id}"))
        })
    }

    fn progress<'a>(
        &'a self,
        _references: &'a [String],
    ) -> BoxFuture<'a, Result<HashMap<String, Vec<Download>>, PluginError>> {
        Box::pin(async { Ok(HashMap::new()) })
    }
}

/// A test Plugin providing a [`FakeFulfiller`] for Movies.
struct FulfillerPlugin(FakeFulfiller);

impl Plugin for FulfillerPlugin {
    fn id(&self) -> PluginId {
        PluginId::from_static("fake-fulfiller")
    }

    fn version(&self) -> &'static str {
        "0.0.0"
    }

    fn register(&self, registrar: &mut Registrar) {
        registrar.provide::<dyn Fulfiller<Movies>>(Priority::DEFAULT, Arc::new(self.0.clone()));
    }
}

#[tokio::test]
async fn approving_a_pending_request_hands_it_to_the_movie_fulfiller_and_the_member_sees_it_processing_live()
 {
    let fulfiller = FakeFulfiller::default();
    let fixture = Fixture::with_fulfiller(&fulfiller).await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let ada = requests(as_ada.clone());
    let pending = ask(&ada, HEAT).await;
    let mut adas_events = subscribe_to_requests(as_ada).await;

    let approved = fixture.approve(pending.id).await;
    let events = [
        next_event(&mut adas_events).await,
        next_event(&mut adas_events).await,
    ];

    assert_eq!(approved.state, RequestState::Processing);
    assert_eq!(fulfiller.handed(), [HEAT]);
    assert_eq!(
        fixture.fulfiller_reference(pending.id).await.as_deref(),
        Some("movie-949")
    );
    let [
        event::Event::RequestStateChanged(was_approved),
        event::Event::RequestStateChanged(processing),
    ] = events
    else {
        panic!("not the approval, then the hand-off: {events:?}");
    };
    assert_eq!(was_approved.request.state, RequestState::Approved);
    assert_eq!(processing.request.as_option(), Some(&approved));
    assert_eq!(list(&ada).await, [approved]);
}

#[tokio::test]
async fn a_request_opened_approved_goes_to_the_movie_fulfiller_and_joining_it_does_not_again() {
    let fulfiller = FakeFulfiller::default();
    let fixture = Fixture::with_fulfiller(&fulfiller).await;
    let may_auto_approve = [Permission::Request, Permission::AutoApproveMovie];
    let (_, as_ada) = fixture.user("Ada", &may_auto_approve).await;
    let (_, as_grace) = fixture.user("Grace", &may_auto_approve).await;

    let opened = ask(&requests(as_ada), HEAT).await;
    let joined = ask(&requests(as_grace), HEAT).await;

    assert_eq!(opened.state, RequestState::Processing);
    assert_eq!(joined.state, RequestState::Processing);
    assert_eq!(fulfiller.handed(), [HEAT]);
    assert_eq!(
        fixture.fulfiller_reference(opened.id).await.as_deref(),
        Some("movie-949")
    );
}

#[tokio::test]
async fn a_request_the_movie_fulfiller_refuses_fails_with_its_reason_and_the_member_sees_it_live() {
    let fulfiller = FakeFulfiller::refusing();
    let fixture = Fixture::with_fulfiller(&fulfiller).await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let ada = requests(as_ada.clone());
    let pending = ask(&ada, HEAT).await;
    let mut adas_events = subscribe_to_requests(as_ada).await;

    let failed = fixture.approve(pending.id).await;
    let events = [
        next_event(&mut adas_events).await,
        next_event(&mut adas_events).await,
    ];

    assert_eq!(failed.state, RequestState::Failed);
    assert_eq!(
        failed.failure_reason.as_deref(),
        Some("Radarr is unreachable")
    );
    assert_eq!(fulfiller.handed(), [HEAT]);
    assert_eq!(fixture.fulfiller_reference(pending.id).await, None);
    let [_, event::Event::RequestStateChanged(changed)] = events else {
        panic!("not the approval, then the failure: {events:?}");
    };
    assert_eq!(changed.request.as_option(), Some(&failed));
    assert_eq!(list(&ada).await, [failed]);
}

#[tokio::test]
async fn retrying_a_failed_request_once_the_movie_fulfiller_recovers_moves_it_to_processing() {
    let fulfiller = FakeFulfiller::refusing();
    let fixture = Fixture::with_fulfiller(&fulfiller).await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let ada = requests(as_ada);
    let failed = fixture.approve(ask(&ada, HEAT).await.id).await;

    fulfiller.recover();
    let retried = retry(&fixture, failed.id).await.unwrap();

    assert_eq!(retried.state, RequestState::Processing);
    assert_eq!(retried.failure_reason, None);
    assert_eq!(fulfiller.handed(), [HEAT, HEAT]);
    assert_eq!(
        fixture.fulfiller_reference(failed.id).await.as_deref(),
        Some("movie-949")
    );
    assert_eq!(list(&ada).await, [retried]);
}

#[tokio::test]
async fn retrying_a_request_that_isnt_failed_is_refused() {
    let fixture = Fixture::with_fulfiller(&FakeFulfiller::default()).await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let ada = requests(as_ada);
    let pending = ask(&ada, HEAT).await;
    let processing = fixture.approve(ask(&ada, ALIEN).await.id).await;

    for id in [pending.id, processing.id] {
        let refusal = assert_refused(
            retry(&fixture, id).await,
            ErrorCode::FailedPrecondition,
            RefusalReason::REFUSAL_REASON_REQUEST_NOT_FAILED,
        );
        assert_eq!(refusal.field, "id");
    }
    assert_eq!(
        states(&list(&ada).await),
        [
            (processing.id, RequestState::Processing),
            (pending.id, RequestState::Pending)
        ]
    );
}

/// Retries the Request `id` as the Admin.
async fn retry(fixture: &Fixture, id: i64) -> Result<Request, ConnectError> {
    let retried = requests(fixture.server.client_config())
        .retry_request(RetryRequestRequest {
            id,
            ..Default::default()
        })
        .await?;
    Ok(retried
        .into_owned()
        .request
        .into_option()
        .unwrap_or_default())
}

#[tokio::test]
async fn an_approved_season_request_is_not_handed_to_the_movie_fulfiller() {
    let fulfiller = FakeFulfiller::default();
    let fixture = Fixture::with_fulfiller(&fulfiller).await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let pending = ask_seasons(&requests(as_ada), DARK, &[1]).await;

    let approved = fixture.approve(pending[0].id).await;

    assert_eq!(approved.state, RequestState::Approved);
    assert_eq!(fulfiller.handed(), Vec::<i64>::new());
}

#[tokio::test]
async fn a_processing_request_becomes_available_once_a_scan_matches_its_movie() {
    let fixture = Fixture::with_fulfiller(&FakeFulfiller::default()).await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let ada = requests(as_ada);
    let processing = fixture.approve(ask(&ada, ALIEN).await.id).await;
    assert_eq!(processing.state, RequestState::Processing);

    let _media = fixture.library_with_alien().await;

    assert_eq!(
        states(&list(&ada).await),
        [(processing.id, RequestState::Available)]
    );
}

#[tokio::test]
async fn a_scan_makes_a_pending_request_available_and_leaves_a_declined_one_declined() {
    let fixture = Fixture::start().await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let ada = requests(as_ada);
    let declined = decline(&fixture, ask(&ada, ALIEN).await.id, "").await;
    let pending = ask(&ada, ALIEN).await;

    let _media = fixture.library_with_alien().await;

    assert_eq!(
        states(&list(&ada).await),
        [
            (pending.id, RequestState::Available),
            (declined.id, RequestState::Declined)
        ]
    );
}

#[tokio::test]
async fn an_available_request_stays_available_after_its_movie_leaves_the_library_and_can_be_requested_again()
 {
    let fixture = Fixture::start().await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let ada = requests(as_ada);
    let first = ask(&ada, ALIEN).await;
    let (media, library) = fixture.library_with_alien().await;

    fs::remove_dir_all(media.path().join("Alien (1979)")).unwrap();
    fixture
        .scan_until(library, LibraryKind::LIBRARY_KIND_MOVIES, false)
        .await;
    let again = ask(&ada, ALIEN).await;

    assert_eq!(
        states(&list(&ada).await),
        [
            (again.id, RequestState::Pending),
            (first.id, RequestState::Available)
        ]
    );
}

fn states(requests: &[Request]) -> Vec<(i64, RequestState)> {
    requests
        .iter()
        .map(|request| (request.id, request.state.as_known().unwrap_or_default()))
        .collect()
}

#[tokio::test]
async fn a_declined_request_shows_its_reason_to_its_requesters_and_asking_again_opens_a_new_one() {
    let fixture = Fixture::start().await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let ada = requests(as_ada);
    let pending = ask(&ada, HEAT).await;

    let declined = decline(&fixture, pending.id, "Not in 4K yet").await;
    let again = ask(&ada, HEAT).await;

    assert_eq!(declined.state, RequestState::Declined);
    assert_eq!(declined.decline_reason.as_deref(), Some("Not in 4K yet"));
    assert_ne!(again.id, pending.id);
    assert_eq!(again.state, RequestState::Pending);
    assert_eq!(again.decline_reason, None);
    assert_eq!(list(&ada).await, [again, declined], "newest first");
}

#[tokio::test]
async fn declining_with_a_blank_reason_gives_none() {
    let fixture = Fixture::start().await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let pending = ask(&requests(as_ada), HEAT).await;

    let declined = decline(&fixture, pending.id, "  ").await;

    assert_eq!(declined.decline_reason, None);
}

#[tokio::test]
async fn a_decline_reason_is_kept_without_its_surrounding_spaces() {
    let fixture = Fixture::start().await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let pending = ask(&requests(as_ada), HEAT).await;

    let declined = decline(&fixture, pending.id, "  Not in 4K yet\n").await;

    assert_eq!(declined.decline_reason.as_deref(), Some("Not in 4K yet"));
}

#[tokio::test]
async fn cancelling_removes_a_request_and_tells_its_requesters() {
    let fixture = Fixture::start().await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let approved = ask(&requests(fixture.server.client_config()), HEAT).await;
    let joined = ask(&requests(as_ada.clone()), HEAT).await;
    let mut adas_events = subscribe_to_requests(as_ada.clone()).await;

    requests(fixture.server.client_config())
        .cancel_request(CancelRequestRequest {
            id: approved.id,
            ..Default::default()
        })
        .await
        .unwrap();
    let event = next_event(&mut adas_events).await;

    let event::Event::RequestCancelled(cancelled) = event else {
        panic!("not the cancelling: {event:?}");
    };
    assert_eq!(cancelled.request.as_option(), Some(&joined));
    assert_eq!(list(&requests(as_ada)).await, []);
}

#[tokio::test]
async fn approving_a_request_that_isnt_pending_is_refused() {
    let fixture = Fixture::start().await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let pending = ask(&requests(as_ada), HEAT).await;
    decline(&fixture, pending.id, "").await;

    let refused = requests(fixture.server.client_config())
        .approve_request(ApproveRequestRequest {
            id: pending.id,
            ..Default::default()
        })
        .await;

    let refusal = assert_refused(
        refused,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_REQUEST_NOT_PENDING,
    );
    assert_eq!(refusal.field, "id");
}

#[tokio::test]
async fn approving_a_request_that_doesnt_exist_is_refused() {
    let fixture = Fixture::start().await;

    let refused = requests(fixture.server.client_config())
        .approve_request(ApproveRequestRequest {
            id: 7,
            ..Default::default()
        })
        .await;

    assert_refused(
        refused,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_REQUEST_NOT_FOUND,
    );
}

#[tokio::test]
async fn a_user_without_manage_requests_is_refused_each_action_naming_the_permission() {
    let fixture = Fixture::start().await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let ada = requests(as_ada);
    let id = ask(&ada, HEAT).await.id;

    let approve = ada
        .approve_request(ApproveRequestRequest {
            id,
            ..Default::default()
        })
        .await
        .map(drop);
    let decline = ada
        .decline_request(DeclineRequestRequest {
            id,
            ..Default::default()
        })
        .await
        .map(drop);
    let cancel = ada
        .cancel_request(CancelRequestRequest {
            id,
            ..Default::default()
        })
        .await
        .map(drop);
    let retry = ada
        .retry_request(RetryRequestRequest {
            id,
            ..Default::default()
        })
        .await
        .map(drop);

    for refused in [approve, decline, cancel, retry] {
        let refusal = assert_refused(
            refused,
            ErrorCode::PermissionDenied,
            RefusalReason::REFUSAL_REASON_PERMISSION_REQUIRED,
        );
        let permission = refusal.params.get("permission").map(String::as_str);
        assert_eq!(permission, Some("PERMISSION_MANAGE_REQUESTS"));
    }
    assert_eq!(list(&ada).await[0].state, RequestState::Pending);
}

#[tokio::test]
async fn one_of_two_requesters_withdraws_and_the_request_stays_with_the_other() {
    let fixture = Fixture::start().await;
    let (ada, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let (grace, as_grace) = fixture.user("Grace", &NEW_USER).await;
    let asked = ask(&requests(as_ada.clone()), HEAT).await;
    ask(&requests(as_grace.clone()), HEAT).await;
    let mut adas_events = subscribe_to_requests(as_ada.clone()).await;
    let mut graces_events = subscribe_to_requests(as_grace.clone()).await;

    withdraw(&requests(as_ada), asked.id).await.unwrap();

    for event in [
        next_event(&mut adas_events).await,
        next_event(&mut graces_events).await,
    ] {
        let event::Event::RequesterRemoved(removed) = event else {
            panic!("not the withdrawal: {event:?}");
        };
        assert_eq!(removed.user_id, ada);
    }
    let remaining = list(&requests(as_grace)).await;
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, asked.id);
    assert_eq!(requester_ids(&remaining[0]), [grace]);
}

#[tokio::test]
async fn the_last_requester_withdrawing_removes_the_request_and_the_movie_can_be_requested_again() {
    let fixture = Fixture::start().await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let ada = requests(as_ada.clone());
    let asked = ask(&ada, HEAT).await;
    let mut adas_events = subscribe_to_requests(as_ada).await;

    withdraw(&ada, asked.id).await.unwrap();
    let event = next_event(&mut adas_events).await;
    let gone = list(&ada).await;
    let again = ask(&ada, HEAT).await;

    let event::Event::RequestWithdrawn(withdrawn) = event else {
        panic!("not the withdrawal: {event:?}");
    };
    assert_eq!(withdrawn.request.as_option(), Some(&asked));
    assert_eq!(gone, []);
    assert_ne!(again.id, asked.id);
    assert_eq!(again.state, RequestState::Pending);
}

#[tokio::test]
async fn withdrawing_from_an_approved_request_is_refused() {
    let fixture = Fixture::start().await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let ada = requests(as_ada);
    let asked = ask(&ada, HEAT).await;
    requests(fixture.server.client_config())
        .approve_request(ApproveRequestRequest {
            id: asked.id,
            ..Default::default()
        })
        .await
        .unwrap();

    let refused = withdraw(&ada, asked.id).await;

    let refusal = assert_refused(
        refused,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_REQUEST_NOT_PENDING,
    );
    assert_eq!(refusal.field, "id");
    assert_eq!(list(&ada).await[0].state, RequestState::Approved);
}

#[tokio::test]
async fn withdrawing_from_a_request_the_user_isnt_a_requester_of_is_refused() {
    let fixture = Fixture::start().await;
    let (ada, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let (_, as_grace) = fixture.user("Grace", &NEW_USER).await;
    let asked = ask(&requests(as_ada.clone()), HEAT).await;

    let refused = withdraw(&requests(as_grace), asked.id).await;

    let refusal = assert_refused(
        refused,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_NOT_A_REQUESTER,
    );
    assert_eq!(refusal.field, "id");
    assert_eq!(requester_ids(&list(&requests(as_ada)).await[0]), [ada]);
}

#[tokio::test]
async fn without_view_requests_withdrawing_from_someone_elses_request_is_refused_as_not_found() {
    let fixture = Fixture::start().await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let (_, as_grace) = fixture.user("Grace", &[Permission::Request]).await;
    let asked = ask(&requests(as_ada), HEAT).await;

    let refused = withdraw(&requests(as_grace), asked.id).await;

    assert_refused(
        refused,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_REQUEST_NOT_FOUND,
    );
}

/// Withdraws the caller of `client` from the Request `id`.
async fn withdraw(client: &RequestServiceClient<HttpClient>, id: i64) -> Result<(), ConnectError> {
    client
        .withdraw_request(WithdrawRequestRequest {
            id,
            ..Default::default()
        })
        .await
        .map(drop)
}

/// Declines the Request `id` as the Admin, giving `reason`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn decline(fixture: &Fixture, id: i64, reason: &str) -> Request {
    requests(fixture.server.client_config())
        .decline_request(DeclineRequestRequest {
            id,
            reason: reason.to_owned(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .request
        .into_option()
        .unwrap()
}

type Subscription = ServerStream<hyper::body::Incoming, SubscribeResponseView<'static>>;

/// Subscribes to Request events, as the User `config` calls as.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn subscribe_to_requests(config: ClientConfig) -> Subscription {
    let only_requests = SubscribeRequest {
        types: vec![
            EventType::EVENT_TYPE_REQUEST_CREATED.into(),
            EventType::EVENT_TYPE_REQUESTER_ADDED.into(),
            EventType::EVENT_TYPE_REQUEST_STATE_CHANGED.into(),
            EventType::EVENT_TYPE_REQUEST_CANCELLED.into(),
            EventType::EVENT_TYPE_REQUESTER_REMOVED.into(),
            EventType::EVENT_TYPE_REQUEST_WITHDRAWN.into(),
        ],
        ..Default::default()
    };
    EventServiceClient::new(HttpClient::plaintext(), config)
        .subscribe(only_requests)
        .await
        .unwrap()
}

#[expect(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    reason = "a test helper fails its test by panicking"
)]
async fn next_event(subscription: &mut Subscription) -> event::Event {
    let next = tokio::time::timeout(WAIT, subscription.message::<SubscribeResponse>())
        .await
        .expect("timed out waiting for an event")
        .unwrap()
        .expect("the stream ended")
        .to_owned_message()
        .message;
    let Some(subscribe_response::Message::Event(event)) = next else {
        panic!("not an event: {next:?}");
    };
    event.event.expect("an event with nothing in it")
}

#[tokio::test]
async fn a_search_finds_movies_with_their_year_and_poster_and_no_library_or_request() {
    let fixture = Fixture::start().await;
    let (_, as_grace) = fixture.user("Grace", &NEW_USER).await;

    let found = search_alien(as_grace).await;

    let titles: Vec<_> = found.iter().map(|movie| movie.title.as_str()).collect();
    assert_eq!(titles, ["Alien", "Alien: Romulus", "Aliens"]);
    let alien = &found[0];
    assert_eq!(alien.tmdb_id, ALIEN);
    assert_eq!(alien.year, Some(1979));
    assert_eq!(
        alien.poster_url.as_deref(),
        Some("/media/artwork/tmdb/%2FvfrQk5IPloGg1v9Rzbh2Eg3VGyM.jpg")
    );
    assert_eq!(alien.library_id, None);
    assert!(alien.request.is_unset(), "{alien:?}");
}

#[tokio::test]
async fn a_search_shows_a_movie_matched_in_a_library_as_in_it() {
    let fixture = Fixture::start().await;
    let (_media, _) = fixture.library_with_alien().await;
    let (_, as_grace) = fixture.user("Grace", &NEW_USER).await;

    let found = search_alien(as_grace).await;

    assert!(found[0].library_id.is_some(), "{:?}", found[0]);
    assert_eq!(found[1].library_id, None, "Alien: Romulus isn't in it");
}

#[tokio::test]
async fn a_movie_someone_else_requested_shows_their_request_and_asking_joins_it() {
    let fixture = Fixture::start().await;
    let (ada, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let (grace, as_grace) = fixture.user("Grace", &NEW_USER).await;
    let adas = ask(&requests(as_ada), ALIEN).await;

    let found = search_alien(as_grace.clone()).await;
    let joined = ask(&requests(as_grace.clone()), ALIEN).await;
    let looked_up = requests(as_grace)
        .look_up_movie(LookUpMovieRequest {
            movie_tmdb_id: ALIEN,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .movie
        .into_option()
        .unwrap();

    assert_eq!(found[0].request.as_option(), Some(&adas));
    assert_eq!(adas.state, RequestState::Pending);
    assert!(
        found[1].request.is_unset(),
        "nobody asked for Alien: Romulus"
    );
    assert_eq!(requester_ids(&joined), [ada, grace]);
    assert_eq!(looked_up.request.as_option(), Some(&joined));
    assert_eq!(looked_up.title, "Alien");
    assert_eq!(looked_up.year, Some(1979));
    assert_eq!(looked_up.runtime_minutes, Some(117));
    assert!(looked_up.overview.is_some());
    assert!(looked_up.backdrop_url.is_some());
}

#[tokio::test]
async fn without_view_requests_a_search_hides_other_peoples_requests() {
    let fixture = Fixture::start().await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let (_, as_grace) = fixture.user("Grace", &[Permission::RequestMovie]).await;
    ask(&requests(as_ada), ALIEN).await;

    let found = search_alien(as_grace).await;

    assert!(found[0].request.is_unset(), "{:?}", found[0]);
}

#[tokio::test]
async fn searching_for_a_blank_title_is_refused() {
    let fixture = Fixture::start().await;
    let (_, as_grace) = fixture.user("Grace", &NEW_USER).await;

    let refused = requests(as_grace)
        .search_movies(SearchMoviesRequest {
            title: " ".to_owned(),
            ..Default::default()
        })
        .await;

    let refusal = assert_refused(
        refused,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
    );
    assert_eq!(refusal.field, "title");
}

#[tokio::test]
async fn looking_up_a_movie_the_metadata_source_doesnt_have_is_refused() {
    let fixture = Fixture::start().await;
    let (_, as_grace) = fixture.user("Grace", &NEW_USER).await;

    let refused = requests(as_grace)
        .look_up_movie(LookUpMovieRequest {
            movie_tmdb_id: 1,
            ..Default::default()
        })
        .await;

    assert_refused(
        refused,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_UNKNOWN_TITLE,
    );
}

/// Artwork is served for paths a search handed out, and still for no others: Alien's backdrop
/// is one TMDB has, but no search gave it.
#[tokio::test]
async fn a_poster_a_search_found_is_served_and_an_image_none_found_is_not() {
    let fixture = Fixture::start().await;
    let (_, as_grace) = fixture.user("Grace", &NEW_USER).await;
    let found = search_alien(as_grace).await;
    let token = Some(fixture.server.admin_token.as_str());

    let poster = fetch(
        &fixture.server,
        found[0].poster_url.as_deref().unwrap(),
        token,
    )
    .await;
    let backdrop = fetch(
        &fixture.server,
        "/media/artwork/tmdb/%2FAmR3JG1VQVxU8TfAvljUhfSFUOx.jpg",
        token,
    )
    .await;

    assert_eq!(poster.status, 200);
    assert!(poster.body.starts_with(b"\xff\xd8"));
    assert_eq!(backdrop.status, 404);
    let images: Vec<_> = fixture
        .tmdb
        .requests()
        .into_iter()
        .filter(|request| request.starts_with("/t/p/"))
        .collect();
    assert_eq!(images.len(), 1, "only the poster was fetched: {images:?}");
}

#[tokio::test]
async fn requesting_seasons_1_and_2_of_a_show_creates_two_requests() {
    let fixture = Fixture::start().await;
    let (_, as_grace) = fixture.user("Grace", &NEW_USER).await;
    let grace = requests(as_grace);

    let asked = ask_seasons(&grace, DARK, &[2, 1, 2]).await;

    let seasons: Vec<_> = asked.iter().map(season_of).collect();
    assert_eq!(seasons, [Some(("Dark", 1)), Some(("Dark", 2))]);
    assert!(asked.iter().all(|request| request.movie.is_unset()));
    let mut listed = list(&grace).await;
    listed.reverse();
    assert_eq!(listed, asked);
}

#[tokio::test]
async fn two_users_asking_for_the_same_season_share_one_request_with_both_as_requesters() {
    let fixture = Fixture::start().await;
    let (ada, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let (grace, as_grace) = fixture.user("Grace", &NEW_USER).await;

    let first = ask_seasons(&requests(as_ada), DARK, &[1]).await;
    let second = ask_seasons(&requests(as_grace), DARK, &[1, 2]).await;

    assert_eq!(second.len(), 2);
    assert_eq!(second[0].id, first[0].id);
    assert_eq!(requester_ids(&second[0]), [ada, grace]);
    assert_eq!(requester_ids(&second[1]), [grace]);
}

#[tokio::test]
async fn a_user_who_may_auto_approve_shows_opens_approved_season_requests_and_one_who_may_not_pending_ones()
 {
    let fixture = Fixture::start().await;
    let (_, auto) = fixture
        .user("Ada", &[Permission::RequestTv, Permission::AutoApproveTv])
        .await;
    let (_, covered) = fixture
        .user("Linus", &[Permission::Request, Permission::AutoApprove])
        .await;
    let (_, movies_only) = fixture
        .user(
            "Grace",
            &[Permission::RequestTv, Permission::AutoApproveMovie],
        )
        .await;

    let approved = ask_seasons(&requests(auto), DARK, &[1]).await;
    let also_approved = ask_seasons(&requests(covered), DARK, &[2]).await;
    let pending = ask_seasons(&requests(movies_only), THE_OFFICE, &[1]).await;

    assert_eq!(approved[0].state, RequestState::Approved);
    assert_eq!(also_approved[0].state, RequestState::Approved);
    assert_eq!(pending[0].state, RequestState::Pending);
}

#[tokio::test]
async fn a_user_without_request_tv_is_refused_naming_the_permission() {
    let fixture = Fixture::start().await;
    let (_, as_movie_asker) = fixture.user("Ada", &[Permission::RequestMovie]).await;

    let refused = requests(as_movie_asker)
        .create_season_requests(season_request(DARK, &[1]))
        .await;

    let refusal = assert_refused(
        refused,
        ErrorCode::PermissionDenied,
        RefusalReason::REFUSAL_REASON_PERMISSION_REQUIRED,
    );
    let permission = refusal.params.get("permission").map(String::as_str);
    assert_eq!(permission, Some("PERMISSION_REQUEST_TV"));
}

#[tokio::test]
async fn requesting_a_season_whose_aired_episodes_are_all_matched_is_refused_but_one_with_some_matched_is_not()
 {
    let fixture = Fixture::start().await;
    let _media = fixture
        .library_with(LibraryKind::LIBRARY_KIND_SHOWS, &[DARK_S01E01, DARK_S02E01])
        .await;
    let (_, as_grace) = fixture.user("Grace", &NEW_USER).await;
    let grace = requests(as_grace);

    let refused = grace
        .create_season_requests(season_request(DARK, &[1, 2]))
        .await;
    let nothing_asked = list(&grace).await;
    let partly_in_library = ask_seasons(&grace, DARK, &[1]).await;

    let refusal = assert_refused(
        refused,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_ALREADY_IN_LIBRARY,
    );
    assert_eq!(refusal.field, "season_numbers");
    let season = refusal.params.get("seasonNumber").map(String::as_str);
    assert_eq!(season, Some("2"));
    assert_eq!(nothing_asked, [], "a refusal opens no Request");
    assert_eq!(partly_in_library[0].state, RequestState::Pending);
}

#[tokio::test]
async fn requesting_a_season_the_show_doesnt_have_is_refused() {
    let fixture = Fixture::start().await;
    let (_, as_grace) = fixture.user("Grace", &NEW_USER).await;

    let refused = requests(as_grace)
        .create_season_requests(season_request(DARK, &[1, 9]))
        .await;

    let refusal = assert_refused(
        refused,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_UNKNOWN_TITLE,
    );
    assert_eq!(refusal.field, "season_numbers");
    let season = refusal.params.get("seasonNumber").map(String::as_str);
    assert_eq!(season, Some("9"));
}

#[tokio::test]
async fn requesting_a_season_of_a_show_the_metadata_source_doesnt_have_is_refused() {
    let fixture = Fixture::start().await;
    let (_, as_grace) = fixture.user("Grace", &NEW_USER).await;

    let refused = requests(as_grace)
        .create_season_requests(season_request(1, &[1]))
        .await;

    let refusal = assert_refused(
        refused,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_UNKNOWN_TITLE,
    );
    assert_eq!(refusal.field, "show_tmdb_id");
    let title = refusal.params.get("titleId").map(String::as_str);
    assert_eq!(title, Some("tmdb-1"));
}

#[tokio::test]
async fn requesting_no_season_is_refused() {
    let fixture = Fixture::start().await;
    let (_, as_grace) = fixture.user("Grace", &NEW_USER).await;

    let refused = requests(as_grace)
        .create_season_requests(season_request(DARK, &[]))
        .await;

    let refusal = assert_refused(
        refused,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
    );
    assert_eq!(refusal.field, "season_numbers");
}

#[tokio::test]
async fn without_view_requests_the_events_stream_shows_only_the_users_own_season_requests() {
    let fixture = Fixture::start().await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let (_, as_grace) = fixture.user("Grace", &[Permission::RequestTv]).await;
    let mut graces_events = subscribe_to_requests(as_grace.clone()).await;

    ask_seasons(&requests(as_ada), DARK, &[1]).await;
    let graces = ask_seasons(&requests(as_grace), DARK, &[2]).await;
    let event = next_event(&mut graces_events).await;

    let event::Event::RequestCreated(created) = event else {
        panic!("Ada's Request reached Grace first: {event:?}");
    };
    assert_eq!(created.request.as_option(), graces.first());
}
