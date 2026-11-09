//! Requesting Movies and Seasons through the generated `RequestService` client, over a real
//! socket, with the TMDB Plugin enabled against a fake TMDB.

#[expect(dead_code, reason = "these tests fetch only whole files")]
#[path = "support/http.rs"]
mod http;
#[path = "support/log.rs"]
mod log;
#[path = "support/refused.rs"]
mod refused;
#[expect(dead_code, reason = "the harness's plain `start` has no use here")]
#[path = "support/server.rs"]
mod server;
#[path = "support/test_plugin.rs"]
mod test_plugin;

use std::{
    collections::{BTreeMap, HashMap},
    fs, future,
    path::Path,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use buffa::Message;
use connectrpc::{
    ConnectError, ErrorCode,
    client::{ClientConfig, HttpClient, ServerStream},
};
use http::fetch;
use log::Log;
use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, Priority, Registrar,
    fulfiller::{Download, DownloadStatus, Fulfiller, Movies, Shows},
    notifier::{Notification, Notifier, Recipient},
    settings::{SettingValue, Settings, SettingsSchema},
};
use nama_plugin_test_choices as test_choices;
use nama_plugin_tmdb::{
    Tmdb,
    testing::{API_KEY, FakeTmdb},
};
use nama_proto::nama::v1::{
    __buffa::view::SubscribeResponseView, ApproveFollowedShowRequest, ApproveRequestRequest,
    BrowseServiceClient, CancelFollowedShowRequest, CancelRequestRequest,
    CreateFollowedShowRequest, CreateLibraryRequest, CreateRequestRequest,
    CreateSeasonRequestsRequest, CreateUserRequest, DeclineFollowedShowRequest,
    DeclineRequestRequest, EnablePluginRequest, EventServiceClient, EventType, FollowedShow,
    FoundMovie, FoundShow, GetRequestRequest, LibraryKind, LibraryServiceClient, ListMoviesRequest,
    ListPluginsRequest, ListRequestsRequest, ListShowsRequest, LookUpMovieRequest,
    LookUpShowRequest, Permission, PluginServiceClient, RefusalReason, Request,
    RequestServiceClient, RequestState, RetryRequestRequest, ScanLibraryRequest,
    SearchMoviesRequest, SearchShowsRequest, SessionDelivery, SetPermissionsRequest, SignInRequest,
    SignInServiceClient, SubscribeRequest, SubscribeResponse, UpdatePluginSettingsRequest,
    UserServiceClient, WithdrawFollowedShowRequest, WithdrawRequestRequest, event, setting_value,
    subscribe_response,
};
use nama_server::{
    config::{Config, PluginConfig},
    db::Database,
    plugin_settings,
    plugins::Registry,
};
use refused::assert_refused;
use server::{PASSWORD, TestServer};
use tokio::{net::TcpListener, sync::mpsc};

/// Movies the fake TMDB knows.
const ALIEN: i64 = 348;
const HEAT: i64 = 949;
const DUNE: i64 = 438_631;

/// A Show the fake TMDB knows, whose Season 1 has three aired Episodes and Season 2 one.
const DARK: i64 = 70523;
/// Dark's TVDB id, as the fake TMDB gives it.
const DARK_TVDB: i64 = 334_824;
const DARK_S01E01: &str = "Dark {tmdb-70523}/Season 01/Dark - S01E01.mkv";
const DARK_S01E02: &str = "Dark {tmdb-70523}/Season 01/Dark - S01E02.mkv";
const DARK_S01E03: &str = "Dark {tmdb-70523}/Season 01/Dark - S01E03.mkv";
const DARK_S02E01: &str = "Dark {tmdb-70523}/Season 02/Dark - S02E01.mkv";
/// Season 3 of Dark is still airing in the fake TMDB: Episode 1 has aired, Episode 2 hasn't.
const DARK_S03E01: &str = "Dark {tmdb-70523}/Season 03/Dark - S03E01.mkv";
const DARK_S03E02: &str = "Dark {tmdb-70523}/Season 03/Dark - S03E02.mkv";
const THE_OFFICE: i64 = 2316;

const WAIT: Duration = Duration::from_secs(10);

/// What a new User gets: `REQUEST` and `VIEW_REQUESTS`.
const NEW_USER: [Permission; 2] = [Permission::Request, Permission::ViewRequests];

/// A Server with the TMDB Plugin enabled against a fake TMDB, and an Admin.
struct Fixture {
    server: TestServer,
    db: Arc<Database>,
    tmdb: FakeTmdb,
    plugins: Registry,
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

    /// A Fixture whose Fulfiller for Shows is `fulfiller`.
    async fn with_shows_fulfiller(fulfiller: &FakeFulfiller) -> Self {
        let plugins = Registry::new();
        plugins
            .enable(Arc::new(ShowsFulfillerPlugin(fulfiller.clone())))
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
        let (server, db) = TestServer::start_with_db(Config::default(), plugins.clone()).await;
        Self {
            server,
            db,
            tmdb,
            plugins,
        }
    }

    /// A Fixture with the Built-in Plugins, where the Built-in Movie Fulfiller `test-choices` is
    /// disabled with the settings it needs stored, and with `fulfiller` as the Movie Fulfiller if
    /// given.
    async fn with_test_choices_disabled(fulfiller: Option<&FakeFulfiller>) -> Self {
        let test_choices = PluginConfig {
            enabled: false,
            settings: BTreeMap::from([
                (
                    "api_key".to_owned(),
                    SettingValue::Text(test_choices::API_KEY.to_owned()),
                ),
                ("profile".to_owned(), SettingValue::Text("1".to_owned())),
            ]),
        };
        let stored = BTreeMap::from([(test_choices::ID.to_owned(), test_choices)]);
        let plugins = Registry::with_builtins(&stored);
        if let Some(fulfiller) = fulfiller {
            plugins
                .enable(Arc::new(FulfillerPlugin(fulfiller.clone())))
                .unwrap();
        }
        let fixture = Self::start_with(plugins).await;
        plugin_settings::import(&fixture.db, &stored).await.unwrap();
        fixture
    }

    /// Serves the Fixture's database again with `plugins`, as a Server restarted with them would.
    async fn serve_again(&self, plugins: Registry) {
        let data_dir = tempfile::tempdir().unwrap();
        let config = Config {
            data_dir: data_dir.path().to_owned(),
            ..Config::default()
        };
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let served = nama_server::serve(
            listener,
            &config,
            Arc::clone(&self.db),
            plugins,
            None,
            future::pending(),
        );
        tokio::spawn(async move {
            let _data_dir = data_dir;
            served.await
        });
    }

    /// Enables the Built-in Plugin `test-choices`, as the Admin.
    async fn enable_test_choices(&self) -> Result<(), ConnectError> {
        PluginServiceClient::new(HttpClient::plaintext(), self.server.client_config())
            .enable_plugin(EnablePluginRequest {
                id: test_choices::ID.to_owned(),
                ..Default::default()
            })
            .await
            .map(drop)
    }

    /// A Fixture with the test Plugin, a Movie Fulfiller and a Notifier, Installed and enabled as
    /// an Admin would, with its text `settings`.
    async fn with_installed_plugin(settings: &[(&str, &str)]) -> Self {
        let fixture = Self::start().await;
        // As on a Server's first start.
        plugin_settings::import(&fixture.db, &BTreeMap::new())
            .await
            .unwrap();
        let dir = fixture.server.data_dir().join("plugins");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("test.wasm"), test_plugin::build("")).unwrap();
        // Listing is what reads `plugins/`.
        let plugins = fixture.plugin_service();
        plugins
            .list_plugins(ListPluginsRequest::default())
            .await
            .unwrap();
        fixture.set_installed_settings(settings).await;
        plugins
            .enable_plugin(EnablePluginRequest {
                id: "test-plugin".to_owned(),
                ..Default::default()
            })
            .await
            .unwrap();
        fixture
    }

    /// Sets the Installed test Plugin's text `settings`, as an Admin would.
    async fn set_installed_settings(&self, settings: &[(&str, &str)]) {
        let text_setting = |(key, text): &(&str, &str)| {
            let value = nama_proto::nama::v1::SettingValue {
                value: Some(setting_value::Value::TextValue((*text).to_owned())),
                ..Default::default()
            };
            ((*key).to_owned(), value)
        };
        self.plugin_service()
            .update_plugin_settings(UpdatePluginSettingsRequest {
                id: "test-plugin".to_owned(),
                settings: settings.iter().map(text_setting).collect(),
                ..Default::default()
            })
            .await
            .unwrap();
    }

    fn plugin_service(&self) -> PluginServiceClient<HttpClient> {
        PluginServiceClient::new(HttpClient::plaintext(), self.server.client_config())
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
        write_files(&folder, files);
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

/// Writes `files` under `folder`, as stand-in videos.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn write_files(folder: &Path, files: &[&str]) {
    for file in files {
        let file = folder.join(file);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, b"video").unwrap();
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

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn get(client: &RequestServiceClient<HttpClient>, id: i64) -> Request {
    client
        .get_request(GetRequestRequest {
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

/// A Season handed to a [`FakeFulfiller`]: its Show's TMDB and TVDB ids, its number, and whether
/// the hand-off asked to follow the Show.
type HandedSeason = (i64, Option<i64>, u32, bool);

/// A Fulfiller that takes every Request it is handed, giving `movie-<TMDB id>` or
/// `show-<TMDB id>-season-<number>` as its reference, unless it is refusing them all, and says it
/// is downloading what the test set.
#[derive(Clone, Default)]
struct FakeFulfiller {
    /// The TMDB id of each Movie it was handed, in order.
    handed: Arc<Mutex<Vec<i64>>>,
    /// Each Season it was handed, in order.
    handed_seasons: Arc<Mutex<Vec<HandedSeason>>>,
    refuses: Arc<AtomicBool>,
    /// Whether it refuses the hand-offs that ask it to follow a Show.
    refuses_following: Arc<AtomicBool>,
    /// The TMDB id of each Show it was told to stop following, in order.
    unfollowed: Arc<Mutex<Vec<i64>>>,
    /// What it says it is downloading, by reference.
    downloading: Arc<Mutex<HashMap<String, Vec<Download>>>>,
    /// Whether it fails to say what it is downloading.
    progress_fails: Arc<AtomicBool>,
}

impl FakeFulfiller {
    fn refusing() -> Self {
        let fulfiller = Self::default();
        fulfiller.refuses.store(true, Ordering::Relaxed);
        fulfiller
    }

    fn refusing_to_follow() -> Self {
        let fulfiller = Self::default();
        fulfiller.refuses_following.store(true, Ordering::Relaxed);
        fulfiller
    }

    /// Takes every Request it is handed from now on.
    fn recover(&self) {
        self.refuses.store(false, Ordering::Relaxed);
        self.refuses_following.store(false, Ordering::Relaxed);
    }

    /// Says it is downloading `downloads` for the Movie `tmdb_id` from now on.
    fn download(&self, tmdb_id: i64, downloads: Vec<Download>) {
        self.downloading
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(format!("movie-{tmdb_id}"), downloads);
    }

    fn handed(&self) -> Vec<i64> {
        self.handed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn handed_seasons(&self) -> Vec<HandedSeason> {
        self.handed_seasons
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn unfollowed(&self) -> Vec<i64> {
        self.unfollowed
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
        let fails = self.progress_fails.load(Ordering::Relaxed);
        let downloading = self
            .downloading
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        Box::pin(async move {
            if fails {
                return Err("Radarr is unreachable".into());
            }
            Ok(downloading)
        })
    }
}

impl Fulfiller<Shows> for FakeFulfiller {
    fn fulfill<'a>(&'a self, request: &'a Request) -> BoxFuture<'a, Result<String, PluginError>> {
        let season = &request.season;
        let (show, season_number) = (season.show_tmdb_id, season.season_number);
        let follow = request.followed_show_id.is_some();
        self.handed_seasons
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((show, season.show_tvdb_id, season_number, follow));
        let refuses = self.refuses.load(Ordering::Relaxed);
        let refuses_to_follow = follow && self.refuses_following.load(Ordering::Relaxed);
        Box::pin(async move {
            if refuses {
                return Err("Sonarr is unreachable".into());
            }
            if refuses_to_follow {
                return Err("Sonarr can't follow this Show".into());
            }
            Ok(format!("show-{show}-season-{season_number}"))
        })
    }

    fn unfollow<'a>(&'a self, request: &'a Request) -> BoxFuture<'a, Result<(), PluginError>> {
        self.unfollowed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request.season.show_tmdb_id);
        Box::pin(async { Ok(()) })
    }

    fn progress<'a>(
        &'a self,
        _references: &'a [String],
    ) -> BoxFuture<'a, Result<HashMap<String, Vec<Download>>, PluginError>> {
        Box::pin(async { Ok(HashMap::new()) })
    }
}

/// A test Plugin providing a [`FakeFulfiller`] for Shows.
struct ShowsFulfillerPlugin(FakeFulfiller);

impl Plugin for ShowsFulfillerPlugin {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: PluginId::from_static("fake-shows-fulfiller"),
            version: "0.0.0".to_owned(),
            settings: SettingsSchema::default(),
            logo_svg: None,
        }
    }

    fn register(&self, registrar: &mut Registrar) {
        registrar.provide::<dyn Fulfiller<Shows>>(Priority::DEFAULT, Arc::new(self.0.clone()));
    }
}

/// A test Plugin providing a [`FakeFulfiller`] for Movies.
struct FulfillerPlugin(FakeFulfiller);

impl Plugin for FulfillerPlugin {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: PluginId::from_static("fake-fulfiller"),
            version: "0.0.0".to_owned(),
            settings: SettingsSchema::default(),
            logo_svg: None,
        }
    }

    fn register(&self, registrar: &mut Registrar) {
        registrar.provide::<dyn Fulfiller<Movies>>(Priority::DEFAULT, Arc::new(self.0.clone()));
    }
}

/// One download, `percent` done, as a Fulfiller reports it.
fn downloading(title: &str, percent: f32) -> Download {
    Download {
        title: title.to_owned(),
        percent,
        size_bytes: Some(4_000_000_000),
        eta_seconds: Some(600),
        status: DownloadStatus::DOWNLOAD_STATUS_DOWNLOADING.into(),
        ..Default::default()
    }
}

#[tokio::test]
async fn without_view_requests_a_user_sees_the_progress_of_only_their_own_processing_requests() {
    let fulfiller = FakeFulfiller::default();
    let fixture = Fixture::with_fulfiller(&fulfiller).await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let (_, as_grace) = fixture.user("Grace", &[Permission::RequestMovie]).await;
    let heat = downloading("Heat.1995.1080p", 42.5);
    let dune = downloading("Dune.2021.2160p", 7.0);
    fulfiller.download(HEAT, vec![heat.clone()]);
    fulfiller.download(DUNE, vec![dune.clone()]);
    let adas = ask(&requests(as_ada), HEAT).await;
    let graces = ask(&requests(as_grace.clone()), DUNE).await;
    let mut graces_events =
        subscribe(as_grace.clone(), &[EventType::EVENT_TYPE_REQUEST_PROGRESS]).await;

    fixture.approve(adas.id).await;
    fixture.approve(graces.id).await;
    let event = next_event(&mut graces_events).await;

    let event::Event::RequestProgress(progress) = event else {
        panic!("not progress: {event:?}");
    };
    assert_eq!(progress.request.id, graces.id);
    assert_eq!(progress.request.downloads, std::slice::from_ref(&dune));
    let listed = list(&requests(as_grace.clone())).await;
    assert_eq!(states(&listed), [(graces.id, RequestState::Processing)]);
    assert_eq!(listed[0].downloads, [dune]);
    let got = get(&requests(fixture.server.client_config()), adas.id).await;
    assert_eq!(got.downloads, [heat]);
}

#[tokio::test]
async fn a_request_whose_fulfiller_cant_say_how_it_is_progressing_stays_processing_with_none() {
    let fulfiller = FakeFulfiller::default();
    let fixture = Fixture::with_fulfiller(&fulfiller).await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let ada = requests(as_ada.clone());
    let mut adas_events = subscribe(as_ada, &[EventType::EVENT_TYPE_REQUEST_PROGRESS]).await;
    fulfiller.download(HEAT, vec![downloading("Heat.1995.1080p", 42.5)]);
    let heat = fixture.approve(ask(&ada, HEAT).await.id).await;
    let event::Event::RequestProgress(_) = next_event(&mut adas_events).await else {
        panic!("not Heat's first progress");
    };

    fulfiller.progress_fails.store(true, Ordering::Relaxed);
    // The Server asks again as soon as another Request becomes Processing.
    fixture.approve(ask(&ada, ALIEN).await.id).await;
    let event = next_event(&mut adas_events).await;

    let event::Event::RequestProgress(progress) = event else {
        panic!("not progress: {event:?}");
    };
    assert_eq!(progress.request.id, heat.id);
    assert_eq!(progress.request.downloads, []);
    let got = get(&ada, heat.id).await;
    assert_eq!(got.state, RequestState::Processing);
    assert_eq!(got.downloads, []);
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
    let ada = requests(as_ada.clone());
    let failed = fixture.approve(ask(&ada, HEAT).await.id).await;
    let mut adas_events = subscribe_to_requests(as_ada).await;

    fulfiller.recover();
    let retried = retry(&fixture, failed.id).await.unwrap();
    let events = [
        next_event(&mut adas_events).await,
        next_event(&mut adas_events).await,
    ];

    let [
        event::Event::RequestStateChanged(approved),
        event::Event::RequestStateChanged(processing),
    ] = events
    else {
        panic!("not Approved, then Processing: {events:?}");
    };
    assert_eq!(
        approved.request.as_option().map(|request| request.state),
        Some(RequestState::Approved.into())
    );
    assert_eq!(processing.request.as_option(), Some(&retried));
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
async fn approving_a_request_hands_it_to_an_installed_movie_fulfiller_which_says_how_it_is_progressing()
 {
    let fixture = Fixture::with_installed_plugin(&[("fulfiller", "take")]).await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let pending = ask(&requests(as_ada.clone()), HEAT).await;
    let mut adas_events = subscribe(as_ada, &[EventType::EVENT_TYPE_REQUEST_PROGRESS]).await;

    let approved = fixture.approve(pending.id).await;
    let event = next_event(&mut adas_events).await;

    assert_eq!(approved.state, RequestState::Processing);
    assert_eq!(
        fixture.fulfiller_reference(pending.id).await.as_deref(),
        Some("test-949")
    );
    let event::Event::RequestProgress(progress) = event else {
        panic!("not progress: {event:?}");
    };
    let half_done = Download {
        title: "test-949".to_owned(),
        percent: 50.0,
        ..Default::default()
    };
    assert_eq!(progress.request.downloads, [half_done]);
}

#[tokio::test]
async fn a_request_an_installed_fulfiller_refuses_fails_with_its_reason_and_retrying_once_it_takes_it_moves_it_to_processing()
 {
    let fixture = Fixture::with_installed_plugin(&[("fulfiller", "error")]).await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let pending = ask(&requests(as_ada), HEAT).await;

    let failed = fixture.approve(pending.id).await;
    fixture
        .set_installed_settings(&[("fulfiller", "take")])
        .await;
    let retried = retry(&fixture, pending.id).await.unwrap();

    assert_eq!(failed.state, RequestState::Failed);
    assert_eq!(
        failed.failure_reason.as_deref(),
        Some("the test Plugin's Fulfiller was set to error")
    );
    assert_eq!(retried.state, RequestState::Processing);
    assert_eq!(
        fixture.fulfiller_reference(pending.id).await.as_deref(),
        Some("test-949")
    );
}

#[tokio::test]
async fn a_request_an_installed_fulfiller_loops_forever_or_panics_on_fails_and_the_server_carries_on()
 {
    let crashes = [
        ("loop", "it didn't answer within 5s"),
        ("panic", "the test Plugin's Fulfiller was set to panic"),
    ];
    for (fulfiller, reason) in crashes {
        let fixture = Fixture::with_installed_plugin(&[("fulfiller", fulfiller)]).await;
        let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
        let pending = ask(&requests(as_ada), HEAT).await;

        let failed = fixture.approve(pending.id).await;
        fixture
            .set_installed_settings(&[("fulfiller", "take")])
            .await;
        let retried = retry(&fixture, pending.id).await.unwrap();

        assert_eq!(failed.state, RequestState::Failed, "{fulfiller}");
        let failure = failed.failure_reason.unwrap_or_default();
        assert!(failure.contains(reason), "{fulfiller}: {failure}");
        assert_eq!(retried.state, RequestState::Processing, "{fulfiller}");
    }
}

#[tokio::test]
async fn an_installed_fulfiller_that_crashes_three_times_in_a_row_is_disabled_and_the_third_request_waits_approved()
 {
    let fixture = Fixture::with_installed_plugin(&[("fulfiller", "panic")]).await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let ada = requests(as_ada);
    let mut approved = Vec::new();
    for tmdb_id in [HEAT, ALIEN, DUNE] {
        let pending = ask(&ada, tmdb_id).await;
        approved.push(fixture.approve(pending.id).await.state);
    }

    assert_eq!(
        approved,
        [
            RequestState::Failed,
            RequestState::Failed,
            RequestState::Approved
        ]
    );
    assert!(
        !fixture
            .plugins
            .enabled()
            .contains(&PluginId::from_static("test-plugin"))
    );
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
async fn approving_a_pending_season_request_hands_it_to_the_shows_fulfiller_and_the_member_sees_it_processing_live()
 {
    let fulfiller = FakeFulfiller::default();
    let fixture = Fixture::with_shows_fulfiller(&fulfiller).await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let ada = requests(as_ada.clone());
    let pending = ask_seasons(&ada, DARK, &[2]).await;
    let mut adas_events = subscribe_to_requests(as_ada).await;

    let approved = fixture.approve(pending[0].id).await;
    let events = [
        next_event(&mut adas_events).await,
        next_event(&mut adas_events).await,
    ];

    assert_eq!(approved.state, RequestState::Processing);
    assert_eq!(
        fulfiller.handed_seasons(),
        [(DARK, Some(DARK_TVDB), 2, false)]
    );
    assert_eq!(
        fixture.fulfiller_reference(approved.id).await.as_deref(),
        Some("show-70523-season-2")
    );
    let [_, event::Event::RequestStateChanged(processing)] = events else {
        panic!("not the approval, then the hand-off: {events:?}");
    };
    assert_eq!(processing.request.as_option(), Some(&approved));
    assert_eq!(list(&ada).await, [approved]);
}

#[tokio::test]
async fn a_season_request_opened_approved_goes_to_the_shows_fulfiller() {
    let fulfiller = FakeFulfiller::default();
    let fixture = Fixture::with_shows_fulfiller(&fulfiller).await;
    let (_, as_ada) = fixture
        .user("Ada", &[Permission::RequestTv, Permission::AutoApproveTv])
        .await;

    let opened = ask_seasons(&requests(as_ada), DARK, &[1, 3]).await;

    assert_eq!(
        states(&opened),
        [
            (opened[0].id, RequestState::Processing),
            (opened[1].id, RequestState::Processing)
        ]
    );
    assert_eq!(
        fulfiller.handed_seasons(),
        [
            (DARK, Some(DARK_TVDB), 1, false),
            (DARK, Some(DARK_TVDB), 3, false)
        ]
    );
    assert_eq!(
        fixture.fulfiller_reference(opened[1].id).await.as_deref(),
        Some("show-70523-season-3")
    );
}

#[tokio::test]
async fn a_season_request_the_shows_fulfiller_refuses_fails_with_its_reason_and_retrying_once_it_recovers_moves_it_to_processing()
 {
    let fulfiller = FakeFulfiller::refusing();
    let fixture = Fixture::with_shows_fulfiller(&fulfiller).await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let pending = ask_seasons(&requests(as_ada), DARK, &[1]).await;

    let failed = fixture.approve(pending[0].id).await;
    fulfiller.recover();
    let retried = retry(&fixture, failed.id).await.unwrap();

    assert_eq!(failed.state, RequestState::Failed);
    assert_eq!(
        failed.failure_reason.as_deref(),
        Some("Sonarr is unreachable")
    );
    assert_eq!(retried.state, RequestState::Processing);
    assert_eq!(
        fulfiller.handed_seasons(),
        [
            (DARK, Some(DARK_TVDB), 1, false),
            (DARK, Some(DARK_TVDB), 1, false)
        ]
    );
    assert_eq!(
        fixture.fulfiller_reference(failed.id).await.as_deref(),
        Some("show-70523-season-1")
    );
}

#[tokio::test]
async fn a_season_request_whose_show_has_no_tvdb_id_is_still_handed_to_the_shows_fulfiller() {
    let fulfiller = FakeFulfiller::default();
    let fixture = Fixture::with_shows_fulfiller(&fulfiller).await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let pending = ask_seasons(&requests(as_ada), DARK, &[1]).await;
    forget_show_tvdb_ids(&fixture.db).await;

    let approved = fixture.approve(pending[0].id).await;

    assert_eq!(approved.state, RequestState::Processing);
    assert_eq!(fulfiller.handed_seasons(), [(DARK, None, 1, false)]);
}

/// Forgets every Season Request's Show TVDB id, as for a Show the metadata source has none for.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn forget_show_tvdb_ids(db: &Database) {
    db.write(|tx| {
        Box::pin(async move {
            sqlx::query("UPDATE requests SET show_tvdb_id = NULL")
                .execute(&mut **tx)
                .await
        })
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn an_approved_movie_request_is_not_handed_to_the_shows_fulfiller() {
    let fulfiller = FakeFulfiller::default();
    let fixture = Fixture::with_shows_fulfiller(&fulfiller).await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let pending = ask(&requests(as_ada), HEAT).await;

    let approved = fixture.approve(pending.id).await;

    assert_eq!(approved.state, RequestState::Approved);
    assert_eq!(fulfiller.handed_seasons(), []);
}

#[tokio::test]
async fn enabling_a_shows_fulfiller_hands_it_the_season_requests_approved_while_there_was_none() {
    let fixture = Fixture::with_test_choices_disabled(None).await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let ada = requests(as_ada);
    let pending = ask_seasons(&ada, DARK, &[1, 2]).await;
    let mut approved = Vec::new();
    for request in pending {
        approved.push(fixture.approve(request.id).await);
    }
    assert_eq!(
        states(&approved),
        [
            (approved[0].id, RequestState::Approved),
            (approved[1].id, RequestState::Approved)
        ]
    );

    fixture.enable_test_choices().await.unwrap();

    assert_eq!(
        states(&list(&ada).await),
        [
            (approved[1].id, RequestState::Processing),
            (approved[0].id, RequestState::Processing)
        ]
    );
    assert_eq!(
        fixture.fulfiller_reference(approved[0].id).await.as_deref(),
        Some("show-70523-season-1")
    );
}

#[tokio::test]
async fn enabling_a_movie_fulfiller_hands_it_the_requests_approved_while_there_was_none() {
    let fixture = Fixture::with_test_choices_disabled(None).await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let ada = requests(as_ada);
    let mut approved = Vec::new();
    for movie in [HEAT, DUNE] {
        let pending = ask(&ada, movie).await;
        approved.push(fixture.approve(pending.id).await);
    }
    assert_eq!(
        states(&approved),
        [
            (approved[0].id, RequestState::Approved),
            (approved[1].id, RequestState::Approved)
        ]
    );

    fixture.enable_test_choices().await.unwrap();

    assert_eq!(
        states(&list(&ada).await),
        [
            (approved[1].id, RequestState::Processing),
            (approved[0].id, RequestState::Processing)
        ]
    );
}

/// Ada's Request for Heat, approved while there was no Movie Fulfiller, once it has left Approved
/// after the Server is started again with `fulfiller` as the Movie Fulfiller.
#[expect(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn waiting_request_after_starting_with(fulfiller: &FakeFulfiller) -> Request {
    let fixture = Fixture::start().await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let ada = requests(as_ada);
    let pending = ask(&ada, HEAT).await;
    let approved = fixture.approve(pending.id).await;
    assert_eq!(approved.state, RequestState::Approved);
    let restarted = Registry::new();
    restarted
        .enable(Arc::new(FulfillerPlugin(fulfiller.clone())))
        .unwrap();

    fixture.serve_again(restarted).await;

    tokio::time::timeout(WAIT, async {
        loop {
            let [request] = list(&ada).await.try_into().unwrap();
            if request.state != RequestState::Approved {
                return request;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the waiting Request never went to the Movie Fulfiller")
}

#[tokio::test]
async fn requests_approved_while_there_was_no_movie_fulfiller_go_to_it_once_a_server_with_one_starts()
 {
    let fulfiller = FakeFulfiller::default();

    let request = waiting_request_after_starting_with(&fulfiller).await;

    assert_eq!(request.state, RequestState::Processing);
    assert_eq!(fulfiller.handed(), [HEAT]);
}

#[tokio::test]
async fn a_waiting_request_the_movie_fulfiller_refuses_at_startup_fails() {
    let request = waiting_request_after_starting_with(&FakeFulfiller::refusing()).await;

    assert_eq!(request.state, RequestState::Failed);
}

#[tokio::test]
async fn enabling_a_second_movie_fulfiller_is_refused_naming_the_first_and_it_stays_disabled() {
    let fixture = Fixture::with_test_choices_disabled(Some(&FakeFulfiller::default())).await;

    let refusal = assert_refused(
        fixture.enable_test_choices().await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_FULFILLER_TAKEN,
    );

    assert_eq!(refusal.field, "id");
    assert_eq!(refusal.params["pluginId"], test_choices::ID);
    assert_eq!(refusal.params["fulfillerPluginId"], "fake-fulfiller");
    assert_eq!(refusal.params["titleKind"], "Movie");
    assert!(
        !fixture
            .plugins
            .enabled()
            .contains(&PluginId::from_static(test_choices::ID))
    );
    let stored = plugin_settings::stored(&fixture.db).await.unwrap().unwrap();
    assert!(!stored[test_choices::ID].enabled);
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
async fn a_failed_request_becomes_available_without_its_reason_once_a_scan_matches_its_movie() {
    let fixture = Fixture::with_fulfiller(&FakeFulfiller::refusing()).await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let ada = requests(as_ada);
    let failed = fixture.approve(ask(&ada, ALIEN).await.id).await;
    assert_eq!(failed.state, RequestState::Failed);

    let _media = fixture.library_with_alien().await;

    let [available] = list(&ada).await.try_into().unwrap();
    assert_eq!(available.state, RequestState::Available);
    assert_eq!(available.failure_reason, None);
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
async fn subscribe_to_requests(config: ClientConfig) -> Subscription {
    subscribe(
        config,
        &[
            EventType::EVENT_TYPE_REQUEST_CREATED,
            EventType::EVENT_TYPE_REQUESTER_ADDED,
            EventType::EVENT_TYPE_REQUEST_STATE_CHANGED,
            EventType::EVENT_TYPE_REQUEST_CANCELLED,
            EventType::EVENT_TYPE_REQUESTER_REMOVED,
            EventType::EVENT_TYPE_REQUEST_WITHDRAWN,
        ],
    )
    .await
}

/// Subscribes to events of `types`, as the User `config` calls as.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn subscribe(config: ClientConfig, types: &[EventType]) -> Subscription {
    let only = SubscribeRequest {
        types: types.iter().map(|&kind| kind.into()).collect(),
        ..Default::default()
    };
    EventServiceClient::new(HttpClient::plaintext(), config)
        .subscribe(only)
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

/// The Request `event` says changed state, and its state now.
#[expect(clippy::panic, reason = "a test helper fails its test by panicking")]
fn state_changed(event: event::Event) -> (i64, RequestState) {
    let event::Event::RequestStateChanged(changed) = event else {
        panic!("not a change of state: {event:?}");
    };
    let Some(request) = changed.request.into_option() else {
        panic!("a change of state without its Request");
    };
    (request.id, request.state.as_known().unwrap_or_default())
}

#[tokio::test]
async fn matching_season_1s_aired_episodes_makes_its_request_partially_available_then_available_live()
 {
    let fixture = Fixture::start().await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let ada = requests(as_ada.clone());
    let asked = ask_seasons(&ada, DARK, &[1, 2]).await;
    let (season_1, season_2) = (asked[0].id, asked[1].id);
    let mut adas_events = subscribe_to_requests(as_ada).await;

    let (media, library) = fixture
        .library_with(LibraryKind::LIBRARY_KIND_SHOWS, &[DARK_S01E01])
        .await;
    let some_matched = state_changed(next_event(&mut adas_events).await);
    write_files(media.path(), &[DARK_S01E02, DARK_S01E03]);
    fixture
        .scan_until(library, LibraryKind::LIBRARY_KIND_SHOWS, true)
        .await;
    let all_matched = state_changed(next_event(&mut adas_events).await);

    assert_eq!(some_matched, (season_1, RequestState::PartiallyAvailable));
    assert_eq!(all_matched, (season_1, RequestState::Available));
    assert_eq!(
        states(&list(&ada).await),
        [
            (season_2, RequestState::Pending),
            (season_1, RequestState::Available)
        ]
    );
}

#[tokio::test]
async fn a_season_still_airing_becomes_available_once_its_aired_episodes_are_matched_and_a_later_episode_does_not_reopen_it()
 {
    let fixture = Fixture::start().await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let ada = requests(as_ada);
    let asked = ask_seasons(&ada, DARK, &[3]).await;

    let (media, library) = fixture
        .library_with(LibraryKind::LIBRARY_KIND_SHOWS, &[DARK_S03E01])
        .await;
    let aired_matched = states(&list(&ada).await);
    let mut matches = subscribe(
        fixture.server.client_config(),
        &[EventType::EVENT_TYPE_ITEM_MATCHED],
    )
    .await;
    write_files(media.path(), &[DARK_S03E02]);
    fixture
        .scan_until(library, LibraryKind::LIBRARY_KIND_SHOWS, true)
        .await;
    let later = next_event(&mut matches).await;

    assert_eq!(aired_matched, [(asked[0].id, RequestState::Available)]);
    assert!(matches!(later, event::Event::ItemMatched(_)), "{later:?}");
    assert_eq!(
        states(&list(&ada).await),
        aired_matched,
        "still Available, with no new Request"
    );
}

#[tokio::test]
async fn a_scan_makes_an_approved_season_request_available_and_leaves_a_declined_one_declined() {
    let fixture = Fixture::start().await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let (_, as_grace) = fixture
        .user("Grace", &[Permission::RequestTv, Permission::AutoApproveTv])
        .await;
    let ada = requests(as_ada);
    let declined = decline(&fixture, ask_seasons(&ada, DARK, &[1]).await[0].id, "").await;
    let approved = ask_seasons(&requests(as_grace), DARK, &[1]).await;

    let _media = fixture
        .library_with(
            LibraryKind::LIBRARY_KIND_SHOWS,
            &[DARK_S01E01, DARK_S01E02, DARK_S01E03],
        )
        .await;

    assert_eq!(
        states(&list(&ada).await),
        [
            (approved[0].id, RequestState::Available),
            (declined.id, RequestState::Declined)
        ]
    );
}

/// Dark as `LookUpShow` gives it to the User `config` calls as.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn look_up_dark(config: ClientConfig) -> FoundShow {
    requests(config)
        .look_up_show(LookUpShowRequest {
            show_tmdb_id: DARK,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .show
        .into_option()
        .unwrap()
}

#[tokio::test]
async fn a_shows_seasons_say_whether_they_are_in_a_library_requested_or_neither() {
    let fixture = Fixture::start().await;
    let (_media, _) = fixture
        .library_with(LibraryKind::LIBRARY_KIND_SHOWS, &[DARK_S01E01, DARK_S02E01])
        .await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let (_, as_grace) = fixture.user("Grace", &NEW_USER).await;
    let adas = ask_seasons(&requests(as_ada), DARK, &[1]).await;

    let dark = look_up_dark(as_grace).await;

    assert_eq!(dark.title, "Dark");
    assert_eq!(dark.year, Some(2017));
    assert!(dark.overview.is_some());
    assert!(dark.poster_url.is_some());
    assert!(dark.backdrop_url.is_some());
    let seasons: Vec<_> = dark
        .seasons
        .iter()
        .map(|season| {
            let request = season.request.as_option();
            (season.season_number, season.in_library, request)
        })
        .collect();
    assert_eq!(
        seasons,
        [(1, false, adas.first()), (2, true, None), (3, false, None)],
        "only one of Season 1's three aired Episodes is Matched"
    );
    assert_eq!(dark.seasons[0].air_date.as_deref(), Some("2017-12-01"));
}

#[tokio::test]
async fn without_view_requests_a_shows_seasons_hide_other_peoples_requests() {
    let fixture = Fixture::start().await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let (_, as_grace) = fixture.user("Grace", &[Permission::RequestTv]).await;
    ask_seasons(&requests(as_ada), DARK, &[1]).await;

    let dark = look_up_dark(as_grace).await;

    assert!(dark.seasons[0].request.is_unset(), "{:?}", dark.seasons[0]);
}

#[tokio::test]
async fn a_show_search_finds_shows_with_their_year_and_poster() {
    let fixture = Fixture::start().await;
    let (_, as_grace) = fixture.user("Grace", &NEW_USER).await;

    let found = requests(as_grace)
        .search_shows(SearchShowsRequest {
            title: "The Office".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .shows;

    assert_eq!(
        found[0].poster_url.as_deref(),
        Some("/media/artwork/tmdb/%2F7DJKHzAi83BmQrWLrYYOqcoKfhR.jpg")
    );
    let found: Vec<_> = found
        .iter()
        .map(|show| (show.tmdb_id, show.title.as_str(), show.year))
        .collect();
    assert_eq!(
        found,
        [
            (THE_OFFICE, "The Office", Some(2005)),
            (2996, "The Office", Some(2001))
        ]
    );
}

#[tokio::test]
async fn looking_up_a_show_the_metadata_source_doesnt_have_is_refused() {
    let fixture = Fixture::start().await;
    let (_, as_grace) = fixture.user("Grace", &NEW_USER).await;

    let refused = requests(as_grace)
        .look_up_show(LookUpShowRequest {
            show_tmdb_id: 1,
            ..Default::default()
        })
        .await;

    let refusal = assert_refused(
        refused,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_UNKNOWN_TITLE,
    );
    assert_eq!(refusal.field, "show_tmdb_id");
}

#[tokio::test]
async fn a_user_without_request_tv_is_refused_looking_up_a_show() {
    let fixture = Fixture::start().await;
    let (_, as_grace) = fixture.user("Grace", &[Permission::RequestMovie]).await;

    let refused = requests(as_grace)
        .look_up_show(LookUpShowRequest {
            show_tmdb_id: DARK,
            ..Default::default()
        })
        .await;

    let refusal = assert_refused(
        refused,
        ErrorCode::PermissionDenied,
        RefusalReason::REFUSAL_REASON_PERMISSION_REQUIRED,
    );
    let permission = refusal.params.get("permission").map(String::as_str);
    assert_eq!(permission, Some("PERMISSION_REQUEST_TV"));
}

/// Asks for the whole Show `show` as the caller of `client`, and returns the Followed Show and the
/// Season Requests.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn follow(
    client: &RequestServiceClient<HttpClient>,
    show: i64,
) -> (FollowedShow, Vec<Request>) {
    let followed = client
        .create_followed_show(CreateFollowedShowRequest {
            show_tmdb_id: show,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned();
    (
        followed.followed_show.into_option().unwrap(),
        followed.requests,
    )
}

/// Follows Dark as the caller of `client` as if it had two Seasons then: the fake TMDB has no
/// Season yet to come out, so Season 3 stands in for one. Its Request is cancelled as the Admin,
/// and the Followed Show is told Season 2 was the latest. Returns the Followed Show and the Season
/// 1 and 2 Requests.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn follow_dark_before_season_3(
    fixture: &Fixture,
    client: &RequestServiceClient<HttpClient>,
) -> (FollowedShow, Vec<Request>) {
    let (followed, mut asked) = follow(client, DARK).await;
    let season_3 = asked.pop().unwrap();
    assert_eq!(season_of(&season_3), Some(("Dark", 3)));
    requests(fixture.server.client_config())
        .cancel_request(CancelRequestRequest {
            id: season_3.id,
            ..Default::default()
        })
        .await
        .unwrap();
    let id = followed.id;
    fixture
        .db
        .write(move |tx| {
            Box::pin(
                sqlx::query("UPDATE followed_shows SET latest_season = 2 WHERE id = ?")
                    .bind(id)
                    .execute(&mut **tx),
            )
        })
        .await
        .unwrap();
    (followed, asked)
}

/// Approves the Followed Show `id` as the Admin, and returns it as the approval left it.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn approve_followed(fixture: &Fixture, id: i64) -> Result<FollowedShow, ConnectError> {
    requests(fixture.server.client_config())
        .approve_followed_show(ApproveFollowedShowRequest {
            id,
            ..Default::default()
        })
        .await
        .map(|approved| approved.into_owned().followed_show.into_option().unwrap())
}

/// The Followed Shows the caller of `client` may see.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn list_followed(client: &RequestServiceClient<HttpClient>) -> Vec<FollowedShow> {
    client
        .list_requests(ListRequestsRequest::default())
        .await
        .unwrap()
        .into_owned()
        .followed_shows
}

/// The Seasons of Dark the caller of `client` has Requests for, in Season order.
async fn dark_seasons_requested(client: &RequestServiceClient<HttpClient>) -> Vec<u32> {
    let mut seasons: Vec<u32> = list(client)
        .await
        .iter()
        .filter_map(|request| season_of(request).map(|(_, number)| number))
        .collect();
    seasons.sort_unstable();
    seasons
}

#[tokio::test]
async fn asking_for_a_whole_show_opens_a_followed_show_and_a_request_per_season_not_in_a_library_and_one_approval_approves_them()
 {
    let fixture = Fixture::start().await;
    // Season 3's one aired Episode, so Season 3 is in a Library.
    let _media = fixture
        .library_with(LibraryKind::LIBRARY_KIND_SHOWS, &[DARK_S03E01])
        .await;
    let (ada, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let ada_requests = requests(as_ada);

    let (followed, asked) = follow(&ada_requests, DARK).await;
    let approved = approve_followed(&fixture, followed.id).await.unwrap();

    let seasons: Vec<_> = asked.iter().map(season_of).collect();
    assert_eq!(seasons, [Some(("Dark", 1)), Some(("Dark", 2))]);
    assert!(
        asked
            .iter()
            .all(|request| request.state == RequestState::Pending
                && request.followed_show_id == Some(followed.id)),
        "{asked:?}"
    );
    assert_eq!(followed.state, RequestState::Pending);
    assert_eq!(followed.requesters[0].user_id, ada);
    assert_eq!(approved.state, RequestState::Approved);
    assert_eq!(list_followed(&ada_requests).await, [approved]);
    assert_eq!(
        states(&list(&ada_requests).await),
        [
            (asked[1].id, RequestState::Approved),
            (asked[0].id, RequestState::Approved)
        ]
    );
}

#[tokio::test]
async fn once_a_followed_show_is_approved_a_scan_matching_a_new_seasons_episodes_opens_its_request_for_its_requesters_live()
 {
    let fixture = Fixture::start().await;
    let (ada, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let (grace, as_grace) = fixture.user("Grace", &NEW_USER).await;
    follow(&requests(as_grace), DARK).await;
    let (followed, _) = follow_dark_before_season_3(&fixture, &requests(as_ada.clone())).await;
    approve_followed(&fixture, followed.id).await.unwrap();
    let mut adas_events = subscribe_to_requests(as_ada).await;

    let _media = fixture
        .library_with(LibraryKind::LIBRARY_KIND_SHOWS, &[DARK_S03E01])
        .await;
    let event = next_event(&mut adas_events).await;

    let event::Event::RequestCreated(created) = event else {
        panic!("not a new Request: {event:?}");
    };
    let request = created.request.into_option().unwrap();
    assert_eq!(season_of(&request), Some(("Dark", 3)));
    assert_eq!(request.state, RequestState::Available);
    assert_eq!(requester_ids(&request), [grace, ada]);
    assert_eq!(request.followed_show_id, Some(followed.id));
}

#[tokio::test]
async fn asking_for_season_1_alone_then_scanning_season_3_opens_no_request() {
    let fixture = Fixture::start().await;
    let (_, as_ada) = fixture
        .user("Ada", &[Permission::RequestTv, Permission::AutoApproveTv])
        .await;
    let ada = requests(as_ada);
    ask_seasons(&ada, DARK, &[1]).await;

    let _media = fixture
        .library_with(LibraryKind::LIBRARY_KIND_SHOWS, &[DARK_S03E01])
        .await;

    assert_eq!(dark_seasons_requested(&ada).await, [1]);
    assert_eq!(list_followed(&ada).await, []);
}

#[tokio::test]
async fn a_declined_followed_show_declines_its_season_requests_and_opens_no_request_for_a_new_season()
 {
    let fixture = Fixture::start().await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let ada = requests(as_ada);
    let (followed, asked) = follow_dark_before_season_3(&fixture, &ada).await;

    let declined = requests(fixture.server.client_config())
        .decline_followed_show(DeclineFollowedShowRequest {
            id: followed.id,
            reason: " Not yet ".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .followed_show
        .into_option()
        .unwrap();
    let _media = fixture
        .library_with(LibraryKind::LIBRARY_KIND_SHOWS, &[DARK_S03E01])
        .await;

    assert_eq!(declined.state, RequestState::Declined);
    assert_eq!(declined.decline_reason.as_deref(), Some("Not yet"));
    let listed = list(&ada).await;
    assert_eq!(
        states(&listed),
        [
            (asked[1].id, RequestState::Declined),
            (asked[0].id, RequestState::Declined)
        ]
    );
    assert_eq!(listed[0].decline_reason.as_deref(), Some("Not yet"));
}

#[tokio::test]
async fn a_cancelled_followed_show_leaves_its_season_requests_and_opens_no_request_for_a_new_season()
 {
    let fixture = Fixture::start().await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let ada = requests(as_ada);
    let (followed, asked) = follow_dark_before_season_3(&fixture, &ada).await;
    approve_followed(&fixture, followed.id).await.unwrap();

    requests(fixture.server.client_config())
        .cancel_followed_show(CancelFollowedShowRequest {
            id: followed.id,
            ..Default::default()
        })
        .await
        .unwrap();
    let _media = fixture
        .library_with(LibraryKind::LIBRARY_KIND_SHOWS, &[DARK_S03E01])
        .await;

    assert_eq!(list_followed(&ada).await, []);
    let listed = list(&ada).await;
    assert_eq!(
        states(&listed),
        [
            (asked[1].id, RequestState::Approved),
            (asked[0].id, RequestState::Approved)
        ]
    );
    assert!(
        listed
            .iter()
            .all(|request| request.followed_show_id.is_none())
    );
}

#[tokio::test]
async fn a_user_without_request_tv_is_refused_a_whole_show_naming_the_permission() {
    let fixture = Fixture::start().await;
    let (_, as_movie_asker) = fixture.user("Ada", &[Permission::RequestMovie]).await;

    let refused = requests(as_movie_asker)
        .create_followed_show(CreateFollowedShowRequest {
            show_tmdb_id: DARK,
            ..Default::default()
        })
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
async fn a_second_user_asking_for_a_whole_show_joins_its_followed_show_and_shows_it_in_the_lookup()
{
    let fixture = Fixture::start().await;
    let (ada, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let (grace, as_grace) = fixture
        .user("Grace", &[Permission::RequestTv, Permission::AutoApproveTv])
        .await;

    let (first, _) = follow(&requests(as_ada), DARK).await;
    let (joined, asked) = follow(&requests(as_grace.clone()), DARK).await;

    assert_eq!(joined.id, first.id);
    assert_eq!(
        joined.state,
        RequestState::Pending,
        "joining approves nothing"
    );
    let followers: Vec<_> = joined.requesters.iter().map(|r| r.user_id).collect();
    assert_eq!(followers, [ada, grace]);
    assert!(asked.iter().all(|r| requester_ids(r) == [ada, grace]));
    assert_eq!(
        look_up_dark(as_grace).await.followed_show.as_option(),
        Some(&joined)
    );
}

#[tokio::test]
async fn a_user_who_may_auto_approve_shows_opens_an_approved_followed_show_which_cant_be_approved_again()
 {
    let fixture = Fixture::start().await;
    let (_, as_ada) = fixture
        .user("Ada", &[Permission::RequestTv, Permission::AutoApproveTv])
        .await;

    let (followed, asked) = follow(&requests(as_ada), DARK).await;
    let refused = approve_followed(&fixture, followed.id).await;

    assert_eq!(followed.state, RequestState::Approved);
    assert!(asked.iter().all(|r| r.state == RequestState::Approved));
    let refusal = assert_refused(
        refused,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_FOLLOWED_SHOW_NOT_PENDING,
    );
    let id = refusal.params.get("followedShowId").map(String::as_str);
    assert_eq!(id, Some(followed.id.to_string().as_str()));
}

#[tokio::test]
async fn approving_a_followed_show_hands_its_season_requests_to_the_shows_fulfiller_to_follow_the_show()
 {
    let fulfiller = FakeFulfiller::default();
    let fixture = Fixture::with_shows_fulfiller(&fulfiller).await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let ada = requests(as_ada);
    let (followed, asked) = follow(&ada, DARK).await;

    approve_followed(&fixture, followed.id).await.unwrap();

    assert_eq!(
        fulfiller.handed_seasons(),
        [
            (DARK, Some(DARK_TVDB), 1, true),
            (DARK, Some(DARK_TVDB), 2, true),
            (DARK, Some(DARK_TVDB), 3, true)
        ]
    );
    let mut listed = states(&list(&ada).await);
    listed.reverse();
    let processing: Vec<_> = asked
        .iter()
        .map(|request| (request.id, RequestState::Processing))
        .collect();
    assert_eq!(listed, processing);
}

#[tokio::test]
async fn a_whole_show_asked_for_approved_hands_its_season_requests_to_the_shows_fulfiller_to_follow_the_show()
 {
    let fulfiller = FakeFulfiller::default();
    let fixture = Fixture::with_shows_fulfiller(&fulfiller).await;
    let (_, as_ada) = fixture
        .user("Ada", &[Permission::RequestTv, Permission::AutoApproveTv])
        .await;

    let (_, asked) = follow(&requests(as_ada), DARK).await;

    assert!(
        asked
            .iter()
            .all(|request| request.state == RequestState::Processing),
        "{asked:?}"
    );
    assert_eq!(
        fulfiller.handed_seasons(),
        [
            (DARK, Some(DARK_TVDB), 1, true),
            (DARK, Some(DARK_TVDB), 2, true),
            (DARK, Some(DARK_TVDB), 3, true)
        ]
    );
}

#[tokio::test]
async fn a_season_request_approved_alone_while_its_followed_show_is_pending_is_handed_off_without_following()
 {
    let fulfiller = FakeFulfiller::default();
    let fixture = Fixture::with_shows_fulfiller(&fulfiller).await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let (_, asked) = follow(&requests(as_ada), DARK).await;

    let approved = fixture.approve(asked[0].id).await;

    assert_eq!(approved.state, RequestState::Processing);
    assert_eq!(
        fulfiller.handed_seasons(),
        [(DARK, Some(DARK_TVDB), 1, false)]
    );
}

#[tokio::test]
async fn a_season_request_the_shows_fulfiller_cant_follow_for_fails_with_its_reason_and_retrying_once_it_can_moves_it_to_processing()
 {
    let fulfiller = FakeFulfiller::refusing_to_follow();
    let fixture = Fixture::with_shows_fulfiller(&fulfiller).await;
    let (_, as_ada) = fixture
        .user("Ada", &[Permission::RequestTv, Permission::AutoApproveTv])
        .await;

    let (_, asked) = follow(&requests(as_ada), DARK).await;
    fulfiller.recover();
    let retried = retry(&fixture, asked[0].id).await.unwrap();

    assert_eq!(asked[0].state, RequestState::Failed);
    assert_eq!(
        asked[0].failure_reason.as_deref(),
        Some("Sonarr can't follow this Show")
    );
    assert_eq!(retried.state, RequestState::Processing);
    assert_eq!(
        fulfiller.handed_seasons().last(),
        Some(&(DARK, Some(DARK_TVDB), 1, true))
    );
}

#[tokio::test]
async fn cancelling_an_approved_followed_show_tells_the_shows_fulfiller_to_stop_following_it() {
    let fulfiller = FakeFulfiller::default();
    let fixture = Fixture::with_shows_fulfiller(&fulfiller).await;
    let (_, as_ada) = fixture
        .user("Ada", &[Permission::RequestTv, Permission::AutoApproveTv])
        .await;
    let (followed, _) = follow(&requests(as_ada), DARK).await;

    requests(fixture.server.client_config())
        .cancel_followed_show(CancelFollowedShowRequest {
            id: followed.id,
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(fulfiller.unfollowed(), [DARK]);
}

#[tokio::test]
async fn the_last_requester_withdrawing_from_an_approved_followed_show_tells_the_shows_fulfiller_to_stop_following_it()
 {
    let fulfiller = FakeFulfiller::default();
    let fixture = Fixture::with_shows_fulfiller(&fulfiller).await;
    let (_, as_ada) = fixture
        .user("Ada", &[Permission::RequestTv, Permission::AutoApproveTv])
        .await;
    let ada = requests(as_ada);
    let (followed, _) = follow(&ada, DARK).await;

    ada.withdraw_followed_show(WithdrawFollowedShowRequest {
        id: followed.id,
        ..Default::default()
    })
    .await
    .unwrap();

    assert_eq!(fulfiller.unfollowed(), [DARK]);
}

#[tokio::test]
async fn cancelling_a_pending_followed_show_tells_the_shows_fulfiller_nothing() {
    let fulfiller = FakeFulfiller::default();
    let fixture = Fixture::with_shows_fulfiller(&fulfiller).await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let (followed, _) = follow(&requests(as_ada), DARK).await;

    requests(fixture.server.client_config())
        .cancel_followed_show(CancelFollowedShowRequest {
            id: followed.id,
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(fulfiller.unfollowed(), Vec::<i64>::new());
}

#[tokio::test]
async fn the_last_requester_withdrawing_ends_a_followed_show_and_leaves_its_season_requests() {
    let fixture = Fixture::start().await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let (grace, as_grace) = fixture.user("Grace", &NEW_USER).await;
    let (ada, grace_requests) = (requests(as_ada), requests(as_grace));
    let (followed, _) = follow(&ada, DARK).await;
    follow(&grace_requests, DARK).await;
    let withdraw = WithdrawFollowedShowRequest {
        id: followed.id,
        ..Default::default()
    };

    ada.withdraw_followed_show(withdraw.clone()).await.unwrap();
    let left_to_grace = list_followed(&grace_requests).await;
    grace_requests
        .withdraw_followed_show(withdraw)
        .await
        .unwrap();

    let followers: Vec<_> = left_to_grace[0]
        .requesters
        .iter()
        .map(|r| r.user_id)
        .collect();
    assert_eq!(followers, [grace]);
    assert_eq!(list_followed(&grace_requests).await, []);
    assert_eq!(dark_seasons_requested(&grace_requests).await, [1, 2, 3]);
}

#[tokio::test]
async fn a_scan_matching_more_of_a_season_in_a_library_when_its_show_was_followed_opens_no_request()
{
    let fixture = Fixture::start().await;
    let (media, library) = fixture
        .library_with(LibraryKind::LIBRARY_KIND_SHOWS, &[DARK_S03E01])
        .await;
    let (_, as_ada) = fixture
        .user("Ada", &[Permission::RequestTv, Permission::AutoApproveTv])
        .await;
    let ada = requests(as_ada);
    follow(&ada, DARK).await;
    let mut matches = subscribe(
        fixture.server.client_config(),
        &[EventType::EVENT_TYPE_ITEM_MATCHED],
    )
    .await;

    write_files(media.path(), &[DARK_S03E02]);
    fixture
        .scan_until(library, LibraryKind::LIBRARY_KIND_SHOWS, true)
        .await;
    next_event(&mut matches).await;

    assert_eq!(dark_seasons_requested(&ada).await, [1, 2]);
}

#[tokio::test]
async fn a_season_request_a_followed_show_opened_isnt_opened_again_once_cancelled() {
    let fixture = Fixture::start().await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
    let ada = requests(as_ada);
    let (followed, _) = follow_dark_before_season_3(&fixture, &ada).await;
    approve_followed(&fixture, followed.id).await.unwrap();
    let (media, library) = fixture
        .library_with(LibraryKind::LIBRARY_KIND_SHOWS, &[DARK_S03E01])
        .await;
    let opened: Vec<_> = list(&ada)
        .await
        .into_iter()
        .filter(|request| season_of(request) == Some(("Dark", 3)))
        .collect();
    requests(fixture.server.client_config())
        .cancel_request(CancelRequestRequest {
            id: opened[0].id,
            ..Default::default()
        })
        .await
        .unwrap();
    let mut matches = subscribe(
        fixture.server.client_config(),
        &[EventType::EVENT_TYPE_ITEM_MATCHED],
    )
    .await;

    write_files(media.path(), &[DARK_S03E02]);
    fixture
        .scan_until(library, LibraryKind::LIBRARY_KIND_SHOWS, true)
        .await;
    next_event(&mut matches).await;

    assert_eq!(opened.len(), 1, "{opened:?}");
    assert_eq!(dark_seasons_requested(&ada).await, [1, 2]);
}

/// What a [`FakeNotifier`] does with a notification.
#[derive(Clone, Copy)]
enum Delivery {
    Keeps,
    Fails,
    NeverFinishes,
}

/// A Notifier that keeps each notification it is handed with its recipient, unless it fails or
/// never finishes delivering.
#[derive(Clone)]
struct FakeNotifier {
    delivery: Delivery,
    delivered: Arc<Mutex<Vec<(Notification, Recipient)>>>,
}

impl FakeNotifier {
    fn new(delivery: Delivery) -> Self {
        Self {
            delivery,
            delivered: Arc::default(),
        }
    }

    fn delivered(&self) -> Vec<(Notification, Recipient)> {
        self.delivered
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// What it was handed, once it has been handed something.
    async fn delivered_soon(&self) -> Vec<(Notification, Recipient)> {
        let started = Instant::now();
        while self.delivered().is_empty() && started.elapsed() < WAIT {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        self.delivered()
    }
}

impl Notifier for FakeNotifier {
    fn deliver<'a>(
        &'a self,
        notification: &'a Notification,
        recipient: &'a Recipient,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        Box::pin(async move {
            match self.delivery {
                Delivery::Keeps => {}
                Delivery::Fails => return Err("the service is down".into()),
                Delivery::NeverFinishes => future::pending().await,
            }
            self.delivered
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push((notification.clone(), recipient.clone()));
            Ok(())
        })
    }
}

/// A test Plugin providing a [`FakeNotifier`].
struct NotifierPlugin(&'static str, FakeNotifier);

impl Plugin for NotifierPlugin {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: PluginId::from_static(self.0),
            version: "0.0.0".to_owned(),
            settings: SettingsSchema::default(),
            logo_svg: None,
        }
    }

    fn register(&self, registrar: &mut Registrar) {
        registrar.provide::<dyn Notifier>(Priority::DEFAULT, Arc::new(self.1.clone()));
    }
}

/// A Fixture with each of `notifiers` enabled under its Plugin id.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn with_notifiers(notifiers: &[(&'static str, &FakeNotifier)]) -> Fixture {
    let plugins = Registry::new();
    for (id, notifier) in notifiers {
        plugins
            .enable(Arc::new(NotifierPlugin(id, (*notifier).clone())))
            .unwrap();
    }
    Fixture::start_with(plugins).await
}

#[tokio::test]
async fn creating_a_movie_request_delivers_it_to_each_enabled_notifiers_channel() {
    let telegram = FakeNotifier::new(Delivery::Keeps);
    let email = FakeNotifier::new(Delivery::Keeps);
    let fixture = with_notifiers(&[("telegram", &telegram), ("email", &email)]).await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;

    let request = ask(&requests(as_ada), HEAT).await;

    for notifier in [telegram, email] {
        let delivered = notifier.delivered_soon().await;
        assert_eq!(delivered.len(), 1, "{delivered:?}");
        let (notification, recipient) = &delivered[0];
        assert_eq!(*recipient, Recipient::Channel);
        assert_eq!(notification.request.as_option(), Some(&request));
        let Some(event::Event::RequestCreated(created)) = &notification.event.event else {
            panic!("not the new Request: {notification:?}");
        };
        assert_eq!(created.request.as_option(), Some(&request));
    }
}

#[tokio::test]
async fn a_disabled_notifier_is_handed_nothing() {
    let enabled = FakeNotifier::new(Delivery::Keeps);
    let disabled = FakeNotifier::new(Delivery::Keeps);
    let fixture = with_notifiers(&[("enabled", &enabled), ("disabled", &disabled)]).await;
    fixture.plugins.disable(&PluginId::from_static("disabled"));
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;

    ask(&requests(as_ada), HEAT).await;

    assert_eq!(enabled.delivered_soon().await.len(), 1);
    assert_eq!(disabled.delivered(), []);
}

#[tokio::test]
async fn a_notifier_that_fails_is_logged_and_neither_it_nor_a_stuck_one_holds_up_the_others_or_the_request()
 {
    let log = Log::global();
    let working = FakeNotifier::new(Delivery::Keeps);
    let failing = FakeNotifier::new(Delivery::Fails);
    let stuck = FakeNotifier::new(Delivery::NeverFinishes);
    let fixture = with_notifiers(&[
        ("working", &working),
        ("failing", &failing),
        ("stuck", &stuck),
    ])
    .await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;

    let request = ask(&requests(as_ada), HEAT).await;

    assert_eq!(request.state, RequestState::Pending);
    assert_eq!(working.delivered_soon().await.len(), 1);
    log.wait_for(&["failing", "the service is down"], 1).await;
}

/// Starts an HTTP server on 127.0.0.1 that answers every request `204 No Content`, and returns its
/// URL and each request's body as it comes in.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn notification_server() -> (String, mpsc::UnboundedReceiver<axum::body::Bytes>) {
    let (posted, bodies) = mpsc::unbounded_channel();
    let app = axum::Router::new().fallback(move |body: axum::body::Bytes| {
        let _: Result<(), _> = posted.send(body);
        future::ready(axum::http::StatusCode::NO_CONTENT)
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    tokio::spawn(async move {
        let _: Result<(), _> = axum::serve(listener, app).await;
    });
    (url, bodies)
}

#[tokio::test]
async fn creating_a_movie_request_has_an_installed_notifier_post_it_to_the_url_in_its_settings() {
    let (url, mut posted) = notification_server().await;
    let fixture = Fixture::with_installed_plugin(&[("url", &url)]).await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;

    let request = ask(&requests(as_ada), HEAT).await;
    let body = tokio::time::timeout(WAIT, posted.recv()).await.unwrap();

    let notification = Notification::decode_from_slice(&body.unwrap()).unwrap();
    assert_eq!(notification.request.as_option(), Some(&request));
    let Some(event::Event::RequestCreated(created)) = &notification.event.event else {
        panic!("not the new Request: {notification:?}");
    };
    assert_eq!(created.request.as_option(), Some(&request));
}

#[tokio::test]
async fn an_installed_notifiers_error_is_logged_with_its_reason_and_the_request_is_still_made() {
    let log = Log::global();
    let fixture = Fixture::with_installed_plugin(&[("notifier", "error")]).await;
    let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;

    let request = ask(&requests(as_ada), HEAT).await;

    assert_eq!(request.state, RequestState::Pending);
    log.wait_for(
        &[
            "test-plugin",
            "notifier::Notifier",
            "provider failed",
            "the test Plugin's Notifier was set to error",
        ],
        1,
    )
    .await;
}

#[tokio::test]
async fn an_installed_notifier_that_loops_forever_or_panics_is_logged_and_the_server_carries_on() {
    let log = Log::global();
    let crashes = [
        ("loop", "it didn't answer within 5s"),
        ("panic", "the test Plugin's Notifier was set to panic"),
    ];
    for (notifier, reason) in crashes {
        let fixture = Fixture::with_installed_plugin(&[("notifier", notifier)]).await;
        let (_, as_ada) = fixture.user("Ada", &NEW_USER).await;
        let ada = requests(as_ada);
        let failed = [
            "test-plugin",
            "notifier::Notifier",
            "provider failed",
            reason,
        ];
        let logged = log.count(&failed);

        let first = ask(&ada, HEAT).await;
        log.wait_for(&failed, logged + 1).await;
        let second = ask(&ada, ALIEN).await;

        assert_eq!(first.state, RequestState::Pending, "{notifier}");
        assert_eq!(second.state, RequestState::Pending, "{notifier}");
    }
}
