//! Requesting Movies and Seasons through the generated `RequestService` client, over a real
//! socket, with the TMDB Plugin enabled against a fake TMDB.

#[expect(dead_code, reason = "these tests fetch only whole files")]
#[path = "support/http.rs"]
mod http;
#[path = "support/log.rs"]
mod log;
#[path = "support/refused.rs"]
mod refused;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;
#[path = "support/test_plugin.rs"]
mod test_plugin;

use std::{
    collections::BTreeMap,
    fs, future,
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, Instant},
};

use buffa::Message;
use connectrpc::{
    ConnectError, ErrorCode,
    client::{ClientConfig, HttpClient},
};
use figment::{Figment, providers::Serialized};
use http::fetch;
use log::Log;
use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    fulfiller::{Download, DownloadStatus},
    notifier::{Notification, Notifier, Recipient},
    settings::SettingValue,
};
use nama_plugin_local_auth::LocalAuth;
use nama_plugin_test_choices as test_choices;
use nama_proto::nama::v1::{
    ApproveFollowedShowRequest, ApproveRequestRequest, CancelFollowedShowRequest,
    CancelRequestRequest, CountPendingRequestsRequest, CreateFollowedShowRequest,
    CreateRequestRequest, CreateSeasonRequestsRequest, DeclineFollowedShowRequest,
    DeclineRequestRequest, EnablePluginRequest, EventType, ExtensionPoint, FollowedShow,
    FoundMovie, FoundShow, GetRequestRequest, LibraryKind, ListDefaultQualitiesRequest,
    ListMovieQualitiesRequest, ListOfferedQualitiesRequest, ListPluginsRequest,
    ListQualitiesRequest, ListRequestsRequest, ListRequestsResponse, LookUpMovieRequest,
    LookUpShowRequest, Permission, PluginServiceClient, Quality, RefusalReason, Request,
    RequestServiceClient, RequestState, RetryRequestRequest, SearchMoviesRequest,
    SearchShowsRequest, SetDefaultQualityRequest, TitleKind, UpdatePluginSettingsRequest,
    WithdrawFollowedShowRequest, event, request::MovieOrSeason, setting_value,
};
use nama_server::{
    config::{Config, PluginConfig},
    db::Database,
    ffmpeg::Ffmpeg,
    plugin_settings::{self, PluginSettings},
    plugins::Registry,
};
use refused::assert_refused;
use server::{
    Builder, Library, Subscription, TestServer, WAIT,
    fulfiller::{FakeFulfiller, Fulfills, MOVIE_FULFILLER, SHOWS_FULFILLER, quality},
    requests::Requester,
    subscribe,
    tmdb::{ALIEN, DARK, DARK_TVDB, DUNE, HEAT, THE_OFFICE},
};
use tokio::{net::TcpListener, sync::mpsc};

const DARK_S01E01: &str = "Dark {tmdb-70523}/Season 01/Dark - S01E01.mkv";
const DARK_S01E02: &str = "Dark {tmdb-70523}/Season 01/Dark - S01E02.mkv";
const DARK_S01E03: &str = "Dark {tmdb-70523}/Season 01/Dark - S01E03.mkv";
const DARK_S02E01: &str = "Dark {tmdb-70523}/Season 02/Dark - S02E01.mkv";
/// Season 3 of Dark is still airing in the fake TMDB: Episode 1 has aired, Episode 2 hasn't.
const DARK_S03E01: &str = "Dark {tmdb-70523}/Season 03/Dark - S03E01.mkv";
const DARK_S03E02: &str = "Dark {tmdb-70523}/Season 03/Dark - S03E02.mkv";

/// What a new User gets: `REQUEST` and `VIEW_REQUESTS`.
const NEW_USER: [Permission; 2] = [Permission::Request, Permission::ViewRequests];
/// A new User's Permissions and `CHOOSE_QUALITY`.
const CHOOSER: [Permission; 3] = [
    Permission::Request,
    Permission::ViewRequests,
    Permission::ChooseQuality,
];

/// The id of the HD service profile a fake Fulfiller can offer.
const HD_PROFILE: &str = "4";
/// The id of the Ultra-HD service profile a fake Fulfiller can offer.
const ULTRA_HD_PROFILE: &str = "5";

/// A Server with TMDB on, and an Admin.
struct Fixture {
    server: TestServer,
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

    /// A Fixture with a fake Movie Fulfiller offering the service profiles `profiles`.
    async fn with_fulfiller(profiles: &[&str]) -> Self {
        Self::start_from(TestServer::builder().fulfiller(profiles), Registry::new()).await
    }

    /// A Fixture with a fake Fulfiller for Shows offering the service profiles `profiles`.
    async fn with_shows_fulfiller(profiles: &[&str]) -> Self {
        Self::start_from(
            TestServer::builder().shows_fulfiller(profiles),
            Registry::new(),
        )
        .await
    }

    /// A Fixture whose Movie Fulfiller `fulfiller` is a Built-in Plugin stored as disabled.
    async fn with_fulfiller_disabled(fulfiller: &FakeFulfiller) -> Self {
        let disabled = PluginConfig {
            enabled: false,
            settings: BTreeMap::new(),
        };
        let stored = BTreeMap::from([(MOVIE_FULFILLER.to_owned(), disabled)]);
        let config = Figment::from(Serialized::defaults(Config::default()))
            .merge(Serialized::default("plugins", stored))
            .extract()
            .unwrap();
        let plugins = Registry::new();
        let made = fulfiller.clone();
        plugins.describe_builtin(move || Fulfills::Movies.plugin(made.clone()));
        Self::start_with_config(plugins, config).await
    }

    /// A Fixture with the Plugins enabled in `plugins` too.
    async fn start_with(plugins: Registry) -> Self {
        Self::start_with_config(plugins, Config::default()).await
    }

    /// A Fixture with the Plugins enabled in `plugins` too, started with `config`.
    async fn start_with_config(plugins: Registry, config: Config) -> Self {
        Self::start_from(TestServer::builder().config(config), plugins).await
    }

    /// A Fixture started from `builder` with TMDB on and the Plugins enabled in `plugins` too.
    async fn start_from(builder: Builder, plugins: Registry) -> Self {
        let server = builder.plugins(plugins.clone()).tmdb().start().await;
        Self { server, plugins }
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
        let config = Figment::from(Serialized::defaults(Config::default()))
            .merge(Serialized::default("plugins", stored))
            .extract()
            .unwrap();
        let plugins = Registry::with_builtins();
        if let Some(fulfiller) = fulfiller {
            plugins
                .enable(Fulfills::Movies.plugin(fulfiller.clone()))
                .unwrap();
        }
        Self::start_with_config(plugins, config).await
    }

    /// Serves the Fixture's database again with the Plugins `plugins` knows and the Built-in
    /// `local-auth` started, and those it enabled, as a Server restarted with them would.
    async fn serve_again(&self, plugins: Registry) {
        let data_dir = tempfile::tempdir().unwrap();
        let config = Config {
            data_dir: data_dir.path().to_owned(),
            ..Config::default()
        };
        plugins.describe_builtin(|| Arc::new(LocalAuth));
        let plugins = PluginSettings::start(Arc::clone(self.server.db()), plugins, &config)
            .await
            .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let served = nama_server::serve(
            listener,
            &config,
            Arc::clone(self.server.db()),
            plugins,
            None,
            Ffmpeg::default(),
            future::pending(),
        );
        tokio::spawn(async move {
            let _data_dir = data_dir;
            served.await
        });
    }

    /// Enables the Built-in Plugin `test-choices`, as the Admin.
    async fn enable_test_choices(&self) -> Result<(), ConnectError> {
        self.enable(test_choices::ID).await
    }

    /// Enables the Plugin `id`, as the Admin.
    async fn enable(&self, id: &str) -> Result<(), ConnectError> {
        self.plugin_service()
            .enable_plugin(EnablePluginRequest {
                id: id.to_owned(),
                ..Default::default()
            })
            .await
            .map(drop)
    }

    /// A Fixture with the test Plugin, a Movie Fulfiller and a Notifier, Installed and enabled as
    /// an Admin would, with its text `settings`.
    async fn with_installed_plugin(settings: &[(&str, &str)]) -> Self {
        let fixture = Self::start().await;
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
            .fetch_one(self.server.db().reader())
            .await
            .unwrap()
    }

    /// Has the Admin offer [`HD_PROFILE`] and [`ULTRA_HD_PROFILE`] as "HD", the default, and "4K",
    /// for the Plugin `plugin`. Returns them as saved.
    ///
    /// The fake Fulfiller must offer both: the Server refuses a Quality its service lacks.
    async fn offer_hd_and_4k(&self, plugin: &str) -> Vec<Quality> {
        let qualities = vec![
            quality(HD_PROFILE, "HD", true),
            quality(ULTRA_HD_PROFILE, "4K", false),
        ];
        self.server.replace_qualities(plugin, qualities).await
    }

    /// Creates a Movies Library holding Alien, scans it, and waits until Alien is Matched.
    async fn library_with_alien(&self) -> Library {
        self.server
            .matched_library(
                LibraryKind::LIBRARY_KIND_MOVIES,
                &["Alien (1979)/Alien (1979).mkv"],
            )
            .await
    }
}

fn requests(config: ClientConfig) -> RequestServiceClient<HttpClient> {
    RequestServiceClient::new(HttpClient::plaintext(), config)
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
    match &request.movie_or_season {
        Some(MovieOrSeason::Season(season)) => {
            Some((season.show_title.as_str(), season.season_number))
        }
        _ => None,
    }
}

/// [`Requester::list`], once none of the Requests is Approved, as after enabling their Fulfiller
/// hands them off in the background.
#[expect(
    clippy::expect_used,
    reason = "a test helper fails its test by panicking"
)]
async fn list_once_handed_off(client: &RequestServiceClient<HttpClient>) -> Vec<Request> {
    tokio::time::timeout(WAIT, async {
        loop {
            let listed = client.list().await;
            if listed
                .iter()
                .all(|request| request.state != RequestState::Approved)
            {
                return listed;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("a Request stayed Approved")
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
    let ada = fixture.server.user("Ada", &NEW_USER).await;
    let (ada, as_ada) = (ada.id, ada.client_config());
    let grace = fixture.server.user("Grace", &NEW_USER).await;
    let (grace, as_grace) = (grace.id, grace.client_config());

    let first = requests(as_ada).ask(HEAT).await;
    let second = requests(as_grace.clone()).ask(HEAT).await;
    let again = requests(as_grace.clone()).ask(HEAT).await;

    assert_eq!(second.id, first.id);
    assert_eq!(second.state, RequestState::Pending);
    assert_eq!(requester_ids(&second), [ada, grace]);
    assert_eq!(again, second, "asking twice changes nothing");
    let Some(MovieOrSeason::Movie(movie)) = &second.movie_or_season else {
        panic!("not a Movie Request: {second:?}");
    };
    assert_eq!(movie.title, "Heat");
    assert_eq!(second.requesters[1].name, "Grace");
    assert_eq!(requests(as_grace).list().await, [second]);
}

#[tokio::test]
async fn a_user_who_may_auto_approve_movies_opens_an_approved_request_and_one_who_may_not_a_pending_one()
 {
    let fixture = Fixture::start().await;
    let auto = fixture
        .server
        .user(
            "Ada",
            &[Permission::RequestMovie, Permission::AutoApproveMovie],
        )
        .await
        .client_config();
    let covered = fixture
        .server
        .user("Linus", &[Permission::Request, Permission::AutoApprove])
        .await
        .client_config();
    let as_grace = fixture
        .server
        .user("Grace", &NEW_USER)
        .await
        .client_config();

    let approved = requests(auto).ask(HEAT).await;
    let also_approved = requests(covered).ask(ALIEN).await;
    let pending = requests(as_grace).ask(DUNE).await;

    assert_eq!(approved.state, RequestState::Approved);
    assert_eq!(also_approved.state, RequestState::Approved);
    assert_eq!(pending.state, RequestState::Pending);
}

#[tokio::test]
async fn a_user_without_request_movie_is_refused_naming_the_permission() {
    let fixture = Fixture::start().await;
    let as_viewer = fixture
        .server
        .user("Ada", &[Permission::ViewRequests])
        .await
        .client_config();

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
    let _media = fixture.library_with_alien().await;
    let as_grace = fixture
        .server
        .user("Grace", &NEW_USER)
        .await
        .client_config();

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
    let as_grace = fixture
        .server
        .user("Grace", &NEW_USER)
        .await
        .client_config();

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
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let as_grace = fixture
        .server
        .user("Grace", &[Permission::RequestMovie])
        .await
        .client_config();
    let adas = requests(as_ada.clone()).ask(HEAT).await;
    requests(as_ada.clone()).ask(DUNE).await;
    let grace = requests(as_grace);
    let shared = grace.ask(DUNE).await;

    let listed = grace.list().await;
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
        requests(as_ada).list().await,
        [shared, adas],
        "newest first"
    );
}

/// Every state a Request or Followed Show can be listed in.
const STATES: [RequestState; 7] = [
    RequestState::Pending,
    RequestState::Approved,
    RequestState::Declined,
    RequestState::Processing,
    RequestState::PartiallyAvailable,
    RequestState::Available,
    RequestState::Failed,
];

/// The page of at most `size` Requests and Followed Shows in `state` after the one that returned
/// `token`, or the first for an empty one.
async fn page(
    client: &RequestServiceClient<HttpClient>,
    state: RequestState,
    size: u32,
    token: &str,
) -> Result<ListRequestsResponse, ConnectError> {
    let listing = ListRequestsRequest {
        state: state.into(),
        page_size: size,
        page_token: token.to_owned(),
        ..Default::default()
    };
    Ok(client.list_requests(listing).await?.into_owned())
}

#[tokio::test]
async fn pages_list_every_request_and_followed_show_once_in_order_even_with_one_opened_between_them()
 {
    let fixture = Fixture::start().await;
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let ada = requests(as_ada);
    let (followed, _) = ada.follow(DARK).await;
    ada.ask(HEAT).await;
    ada.ask(DUNE).await;
    let (all, _) = ada.list_in(RequestState::Unspecified).await;

    let first = page(&ada, RequestState::Unspecified, 4, "").await.unwrap();
    ada.ask(ALIEN).await;
    let second = page(&ada, RequestState::Unspecified, 4, &first.next_page_token)
        .await
        .unwrap();

    assert_eq!(all.len(), 5, "Dune, Heat and Dark's three Seasons");
    assert_eq!(first.requests, all[..4]);
    assert_eq!(first.followed_shows, []);
    assert_eq!(
        second.requests,
        all[4..],
        "no Alien, which opened after the first page"
    );
    assert_eq!(
        second.followed_shows,
        [followed],
        "after its Season Requests opened in the same second"
    );
    assert_eq!(second.next_page_token, "", "the last page");
}

#[tokio::test]
async fn a_page_token_used_with_another_state_or_made_up_is_refused() {
    let fixture = Fixture::start().await;
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let ada = requests(as_ada);
    ada.ask(HEAT).await;
    ada.ask(DUNE).await;
    let pending = page(&ada, RequestState::Pending, 1, "").await.unwrap();

    let all = page(&ada, RequestState::Unspecified, 1, &pending.next_page_token).await;
    let made_up = page(&ada, RequestState::Pending, 1, "made up").await;

    for refused in [all, made_up] {
        let refusal = assert_refused(
            refused,
            ErrorCode::InvalidArgument,
            RefusalReason::REFUSAL_REASON_PAGE_TOKEN_INVALID,
        );
        assert_eq!(refusal.field, "page_token");
    }
}

#[tokio::test]
async fn listing_by_a_state_gives_only_the_requests_and_followed_shows_in_it_and_no_state_gives_all()
 {
    let fixture = Fixture::with_shows_fulfiller(&[]).await;
    let fulfiller = fixture.server.fulfiller();
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let ada = requests(as_ada.clone());
    let alien = ada.ask(ALIEN).await;
    // With no Movie Fulfiller, an approved Movie waits Approved.
    let approved = fixture.server.approve(ada.ask(DUNE).await.id).await;
    let (followed, dark) = ada.follow(DARK).await;
    fixture.server.approve(dark[0].id).await;
    let processing = fixture.server.approve(dark[1].id).await;
    fulfiller.refuse();
    let failed = fixture.server.approve(dark[2].id).await;
    let declined = fixture.server.decline(ada.ask(HEAT).await.id, "").await;
    let pending = ada.ask_seasons(THE_OFFICE, &[1]).await;
    let mut adas_events = subscribe_to_requests(&as_ada).await;
    let _movies = fixture.library_with_alien().await;
    let available = state_changed(adas_events.next().await);
    let _shows = fixture
        .server
        .matched_library(LibraryKind::LIBRARY_KIND_SHOWS, &[DARK_S01E01])
        .await;
    let partially_available = state_changed(adas_events.next().await);

    let (all_requests, all_followed) = ada.list_in(RequestState::Unspecified).await;
    let mut by_state = Vec::new();
    for state in STATES {
        by_state.push((state, ada.list_in(state).await));
    }

    let newest_first = [
        (pending[0].id, RequestState::Pending),
        (declined.id, RequestState::Declined),
        (failed.id, RequestState::Failed),
        (processing.id, RequestState::Processing),
        (dark[0].id, RequestState::PartiallyAvailable),
        (approved.id, RequestState::Approved),
        (alien.id, RequestState::Available),
    ];
    assert_eq!(available, (alien.id, RequestState::Available));
    assert_eq!(
        partially_available,
        (dark[0].id, RequestState::PartiallyAvailable)
    );
    assert_eq!(states(&all_requests), newest_first, "no state lists all");
    assert_eq!(
        all_followed,
        [followed],
        "no state lists the Pending Followed Show"
    );
    for (state, (requests, followed_shows)) in by_state {
        let want: Vec<_> = all_requests
            .iter()
            .filter(|request| request.state == state)
            .cloned()
            .collect();
        let want_followed: Vec<_> = all_followed
            .iter()
            .filter(|show| show.state == state)
            .cloned()
            .collect();
        assert_eq!(requests, want, "Requests {state:?}");
        assert_eq!(followed_shows, want_followed, "Followed Shows {state:?}");
    }
}

#[tokio::test]
async fn without_view_requests_a_user_lists_only_their_own_requests_in_every_state() {
    let fixture = Fixture::start().await;
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let as_grace = fixture
        .server
        .user("Grace", &[Permission::RequestMovie, Permission::RequestTv])
        .await
        .client_config();
    let ada = requests(as_ada);
    let grace = requests(as_grace);
    ada.ask(HEAT).await;
    ada.follow(DARK).await;
    let graces = grace.ask(DUNE).await;
    let graces_declined = fixture.server.decline(grace.ask(ALIEN).await.id, "").await;

    let mut listed = Vec::new();
    for state in STATES {
        listed.push((state, grace.list_in(state).await));
    }

    for (state, (requests, followed)) in listed {
        let want = if state == RequestState::Pending {
            std::slice::from_ref(&graces)
        } else if state == RequestState::Declined {
            std::slice::from_ref(&graces_declined)
        } else {
            &[]
        };
        assert_eq!(requests, want, "{state:?}");
        assert_eq!(followed, [], "{state:?}");
    }
}

#[tokio::test]
async fn a_failed_season_request_lists_under_failed_without_its_approved_followed_show() {
    let fixture = Fixture::with_shows_fulfiller(&[]).await;
    let fulfiller = fixture.server.fulfiller();
    fulfiller.refuse();
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let ada = requests(as_ada);
    let (followed, asked) = ada.follow(DARK).await;
    let approved = approve_followed(&fixture, followed.id).await.unwrap();

    let (failed, followed_failed) = ada.list_in(RequestState::Failed).await;
    let (_, followed_approved) = ada.list_in(RequestState::Approved).await;

    let newest_first = asked.iter().rev();
    let every_season_failed: Vec<_> = newest_first
        .map(|request| (request.id, RequestState::Failed))
        .collect();
    assert_eq!(states(&failed), every_season_failed);
    assert!(
        failed
            .iter()
            .all(|request| request.followed_show_id == Some(followed.id))
    );
    assert_eq!(followed_failed, []);
    assert_eq!(followed_approved, [approved]);
}

#[tokio::test]
async fn without_view_requests_the_events_stream_shows_only_the_users_own_requests() {
    let fixture = Fixture::start().await;
    let ada = fixture.server.user("Ada", &NEW_USER).await;
    let (ada, as_ada) = (ada.id, ada.client_config());
    let as_grace = fixture
        .server
        .user("Grace", &[Permission::RequestMovie])
        .await
        .client_config();
    let mut graces_events = subscribe_to_requests(&as_grace).await;

    requests(as_ada.clone()).ask(HEAT).await;
    let shared = requests(as_ada).ask(DUNE).await;
    requests(as_grace).ask(DUNE).await;
    let event = graces_events.next().await;

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
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let grace = fixture
        .server
        .user("Grace", &[Permission::RequestMovie])
        .await;
    let (grace, as_grace) = (grace.id, grace.client_config());
    let mut adas_events = subscribe_to_requests(&as_ada).await;

    let graces = requests(as_grace).ask(HEAT).await;
    let event = adas_events.next().await;

    let event::Event::RequestCreated(created) = event else {
        panic!("not Grace's new Request: {event:?}");
    };
    assert_eq!(created.request.as_option(), Some(&graces));
    assert_eq!(requester_ids(&graces), [grace]);
}

#[tokio::test]
async fn an_admin_approves_a_members_pending_request_and_the_member_sees_it_live() {
    let fixture = Fixture::start().await;
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let pending = requests(as_ada.clone()).ask(HEAT).await;
    let mut adas_events = subscribe_to_requests(&as_ada).await;

    let approved = fixture.server.approve(pending.id).await;
    let event = adas_events.next().await;

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
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let ada = requests(as_ada.clone());
    let pending = ada.ask(ALIEN).await;
    fixture.server.approve(pending.id).await;
    let mut adas_events = subscribe_to_requests(&as_ada).await;

    let _media = fixture.library_with_alien().await;
    let event = adas_events.next().await;

    let event::Event::RequestStateChanged(changed) = event else {
        panic!("not the Request becoming Available: {event:?}");
    };
    assert_eq!(changed.request.state, RequestState::Available);
    assert_eq!(ada.list().await, [changed.request.into_option().unwrap()]);
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
    let fixture = Fixture::with_fulfiller(&[]).await;
    let fulfiller = fixture.server.fulfiller();
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let as_grace = fixture
        .server
        .user("Grace", &[Permission::RequestMovie])
        .await
        .client_config();
    let heat = downloading("Heat.1995.1080p", 42.5);
    let dune = downloading("Dune.2021.2160p", 7.0);
    fulfiller.download(HEAT, vec![heat.clone()]);
    fulfiller.download(DUNE, vec![dune.clone()]);
    let adas = requests(as_ada).ask(HEAT).await;
    let graces = requests(as_grace.clone()).ask(DUNE).await;
    let mut graces_events = subscribe(&as_grace, &[EventType::EVENT_TYPE_REQUEST_PROGRESS]).await;

    fixture.server.approve(adas.id).await;
    fixture.server.approve(graces.id).await;
    let event = graces_events.next().await;

    let event::Event::RequestProgress(progress) = event else {
        panic!("not progress: {event:?}");
    };
    assert_eq!(progress.request.id, graces.id);
    assert_eq!(progress.request.downloads, std::slice::from_ref(&dune));
    let listed = requests(as_grace.clone()).list().await;
    assert_eq!(states(&listed), [(graces.id, RequestState::Processing)]);
    assert_eq!(listed[0].downloads, [dune]);
    let got = fixture.server.admin_requests().get(adas.id).await;
    assert_eq!(got.downloads, [heat]);
}

#[tokio::test]
async fn a_request_whose_fulfiller_cant_say_how_it_is_progressing_stays_processing_with_none() {
    let fixture = Fixture::with_fulfiller(&[]).await;
    let fulfiller = fixture.server.fulfiller();
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let ada = requests(as_ada.clone());
    let mut adas_events = subscribe(&as_ada, &[EventType::EVENT_TYPE_REQUEST_PROGRESS]).await;
    fulfiller.download(HEAT, vec![downloading("Heat.1995.1080p", 42.5)]);
    let heat = fixture.server.approve(ada.ask(HEAT).await.id).await;
    let event::Event::RequestProgress(_) = adas_events.next().await else {
        panic!("not Heat's first progress");
    };

    fulfiller.fail_progress();
    // The Server asks again as soon as another Request becomes Processing.
    fixture.server.approve(ada.ask(ALIEN).await.id).await;
    let event = adas_events.next().await;

    let event::Event::RequestProgress(progress) = event else {
        panic!("not progress: {event:?}");
    };
    assert_eq!(progress.request.id, heat.id);
    assert_eq!(progress.request.downloads, []);
    let got = ada.get(heat.id).await;
    assert_eq!(got.state, RequestState::Processing);
    assert_eq!(got.downloads, []);
}

#[tokio::test]
async fn approving_a_pending_request_hands_it_to_the_movie_fulfiller_and_the_member_sees_it_processing_live()
 {
    let fixture = Fixture::with_fulfiller(&[]).await;
    let fulfiller = fixture.server.fulfiller();
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let ada = requests(as_ada.clone());
    let pending = ada.ask(HEAT).await;
    let mut adas_events = subscribe_to_requests(&as_ada).await;

    let approved = fixture.server.approve(pending.id).await;
    let events = [adas_events.next().await, adas_events.next().await];

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
    assert_eq!(ada.list().await, [approved]);
}

#[tokio::test]
async fn a_request_opened_approved_goes_to_the_movie_fulfiller_and_joining_it_does_not_again() {
    let fixture = Fixture::with_fulfiller(&[]).await;
    let fulfiller = fixture.server.fulfiller();
    let may_auto_approve = [Permission::Request, Permission::AutoApproveMovie];
    let as_ada = fixture
        .server
        .user("Ada", &may_auto_approve)
        .await
        .client_config();
    let as_grace = fixture
        .server
        .user("Grace", &may_auto_approve)
        .await
        .client_config();

    let opened = requests(as_ada).ask(HEAT).await;
    let joined = requests(as_grace).ask(HEAT).await;

    assert_eq!(opened.state, RequestState::Processing);
    assert_eq!(joined.state, RequestState::Processing);
    assert_eq!(fulfiller.handed(), [HEAT]);
    assert_eq!(
        fixture.fulfiller_reference(opened.id).await.as_deref(),
        Some("movie-949")
    );
}

#[tokio::test]
async fn a_user_who_may_auto_approve_movies_joining_a_pending_request_approves_it_hands_it_off_and_tells_its_requesters()
 {
    let fixture = Fixture::with_fulfiller(&[]).await;
    let fulfiller = fixture.server.fulfiller();
    let ada = fixture.server.user("Ada", &NEW_USER).await;
    let (ada, as_ada) = (ada.id, ada.client_config());
    let grace = fixture
        .server
        .user(
            "Grace",
            &[Permission::Request, Permission::AutoApproveMovie],
        )
        .await;
    let (grace, as_grace) = (grace.id, grace.client_config());
    let pending = requests(as_ada.clone()).ask(HEAT).await;
    let mut adas_events = subscribe_to_requests(&as_ada).await;

    let joined = requests(as_grace).ask(HEAT).await;
    let events = [
        adas_events.next().await,
        adas_events.next().await,
        adas_events.next().await,
    ];

    assert_eq!(joined.id, pending.id);
    assert_eq!(joined.state, RequestState::Processing);
    assert_eq!(requester_ids(&joined), [ada, grace]);
    assert_eq!(fulfiller.handed(), [HEAT]);
    let [
        event::Event::RequesterAdded(_),
        event::Event::RequestStateChanged(approved),
        event::Event::RequestStateChanged(processing),
    ] = events
    else {
        panic!("not the join, the approval, then the hand-off: {events:?}");
    };
    assert_eq!(approved.request.state, RequestState::Approved);
    assert_eq!(requester_ids(&approved.request), [ada, grace]);
    assert_eq!(processing.request.as_option(), Some(&joined));
}

#[tokio::test]
async fn a_user_who_may_auto_approve_movies_joining_an_approved_request_leaves_it_approved() {
    let fixture = Fixture::start().await;
    let may_auto_approve = [Permission::Request, Permission::AutoApproveMovie];
    let as_ada = fixture
        .server
        .user("Ada", &may_auto_approve)
        .await
        .client_config();
    let as_grace = fixture
        .server
        .user("Grace", &may_auto_approve)
        .await
        .client_config();
    let approved = requests(as_ada.clone()).ask(HEAT).await;
    let mut adas_events = subscribe_to_requests(&as_ada).await;

    let joined = requests(as_grace).ask(HEAT).await;
    let added = adas_events.next().await;

    assert_eq!(approved.state, RequestState::Approved);
    assert_eq!(joined.state, RequestState::Approved);
    let event::Event::RequesterAdded(added) = added else {
        panic!("not the join: {added:?}");
    };
    assert_eq!(added.request.as_option(), Some(&joined));
}

#[tokio::test]
async fn a_user_who_may_auto_approve_movies_joining_a_failed_request_leaves_it_failed_and_does_not_hand_it_off_again()
 {
    let fixture = Fixture::with_fulfiller(&[]).await;
    let fulfiller = fixture.server.fulfiller();
    fulfiller.refuse();
    let may_auto_approve = [Permission::Request, Permission::AutoApproveMovie];
    let as_ada = fixture
        .server
        .user("Ada", &may_auto_approve)
        .await
        .client_config();
    let as_grace = fixture
        .server
        .user("Grace", &may_auto_approve)
        .await
        .client_config();
    let failed = requests(as_ada).ask(HEAT).await;
    fulfiller.recover();

    let joined = requests(as_grace).ask(HEAT).await;

    assert_eq!(failed.state, RequestState::Failed);
    assert_eq!(joined.state, RequestState::Failed);
    assert_eq!(fulfiller.handed(), [HEAT]);
}

#[tokio::test]
async fn a_request_the_movie_fulfiller_refuses_fails_with_its_reason_and_the_member_sees_it_live() {
    let fixture = Fixture::with_fulfiller(&[]).await;
    let fulfiller = fixture.server.fulfiller();
    fulfiller.refuse();
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let ada = requests(as_ada.clone());
    let pending = ada.ask(HEAT).await;
    let mut adas_events = subscribe_to_requests(&as_ada).await;

    let failed = fixture.server.approve(pending.id).await;
    let events = [adas_events.next().await, adas_events.next().await];

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
    assert_eq!(ada.list().await, [failed]);
}

#[tokio::test]
async fn retrying_a_failed_request_once_the_movie_fulfiller_recovers_moves_it_to_processing() {
    let fixture = Fixture::with_fulfiller(&[]).await;
    let fulfiller = fixture.server.fulfiller();
    fulfiller.refuse();
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let ada = requests(as_ada.clone());
    let failed = fixture.server.approve(ada.ask(HEAT).await.id).await;
    let mut adas_events = subscribe_to_requests(&as_ada).await;

    fulfiller.recover();
    let retried = fixture.server.retry(failed.id).await;
    let events = [adas_events.next().await, adas_events.next().await];

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
    assert_eq!(ada.list().await, [retried]);
}

#[tokio::test]
async fn retrying_a_request_that_isnt_failed_is_refused() {
    let fixture = Fixture::with_fulfiller(&[]).await;
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let ada = requests(as_ada);
    let pending = ada.ask(HEAT).await;
    let processing = fixture.server.approve(ada.ask(ALIEN).await.id).await;

    for id in [pending.id, processing.id] {
        let refusal = assert_refused(
            fixture.server.try_retry(id).await,
            ErrorCode::FailedPrecondition,
            RefusalReason::REFUSAL_REASON_REQUEST_NOT_FAILED,
        );
        assert_eq!(refusal.field, "id");
    }
    assert_eq!(
        states(&ada.list().await),
        [
            (processing.id, RequestState::Processing),
            (pending.id, RequestState::Pending)
        ]
    );
}

#[tokio::test]
async fn approving_a_request_hands_it_to_an_installed_movie_fulfiller_which_says_how_it_is_progressing()
 {
    let fixture = Fixture::with_installed_plugin(&[("fulfiller", "take")]).await;
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let pending = requests(as_ada.clone()).ask(HEAT).await;
    let mut adas_events = subscribe(&as_ada, &[EventType::EVENT_TYPE_REQUEST_PROGRESS]).await;

    let approved = fixture.server.approve(pending.id).await;
    let event = adas_events.next().await;

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
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let pending = requests(as_ada).ask(HEAT).await;

    let failed = fixture.server.approve(pending.id).await;
    fixture
        .set_installed_settings(&[("fulfiller", "take")])
        .await;
    let retried = fixture.server.retry(pending.id).await;

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
        let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
        let pending = requests(as_ada).ask(HEAT).await;

        let failed = fixture.server.approve(pending.id).await;
        fixture
            .set_installed_settings(&[("fulfiller", "take")])
            .await;
        let retried = fixture.server.retry(pending.id).await;

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
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let ada = requests(as_ada);
    let mut approved = Vec::new();
    for tmdb_id in [HEAT, ALIEN, DUNE] {
        let pending = ada.ask(tmdb_id).await;
        approved.push(fixture.server.approve(pending.id).await.state);
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
    let fixture = Fixture::with_fulfiller(&[]).await;
    let fulfiller = fixture.server.fulfiller();
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let pending = requests(as_ada).ask_seasons(DARK, &[1]).await;

    let approved = fixture.server.approve(pending[0].id).await;

    assert_eq!(approved.state, RequestState::Approved);
    assert_eq!(fulfiller.handed(), Vec::<i64>::new());
}

#[tokio::test]
async fn approving_a_pending_season_request_hands_it_to_the_shows_fulfiller_and_the_member_sees_it_processing_live()
 {
    let fixture = Fixture::with_shows_fulfiller(&[]).await;
    let fulfiller = fixture.server.fulfiller();
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let ada = requests(as_ada.clone());
    let pending = ada.ask_seasons(DARK, &[2]).await;
    let mut adas_events = subscribe_to_requests(&as_ada).await;

    let approved = fixture.server.approve(pending[0].id).await;
    let events = [adas_events.next().await, adas_events.next().await];

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
    assert_eq!(ada.list().await, [approved]);
}

#[tokio::test]
async fn a_season_request_opened_approved_goes_to_the_shows_fulfiller() {
    let fixture = Fixture::with_shows_fulfiller(&[]).await;
    let fulfiller = fixture.server.fulfiller();
    let as_ada = fixture
        .server
        .user("Ada", &[Permission::RequestTv, Permission::AutoApproveTv])
        .await
        .client_config();

    let opened = requests(as_ada).ask_seasons(DARK, &[1, 3]).await;

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
async fn a_user_who_may_auto_approve_tv_joining_a_pending_season_request_approves_it_and_hands_it_off()
 {
    let fixture = Fixture::with_shows_fulfiller(&[]).await;
    let fulfiller = fixture.server.fulfiller();
    let ada = fixture.server.user("Ada", &NEW_USER).await;
    let (ada, as_ada) = (ada.id, ada.client_config());
    let grace = fixture
        .server
        .user("Grace", &[Permission::RequestTv, Permission::AutoApproveTv])
        .await;
    let (grace, as_grace) = (grace.id, grace.client_config());
    let pending = requests(as_ada).ask_seasons(DARK, &[2]).await;

    let joined = requests(as_grace).ask_seasons(DARK, &[2]).await;

    assert_eq!(states(&joined), [(pending[0].id, RequestState::Processing)]);
    assert_eq!(requester_ids(&joined[0]), [ada, grace]);
    assert_eq!(
        fulfiller.handed_seasons(),
        [(DARK, Some(DARK_TVDB), 2, false)]
    );
}

#[tokio::test]
async fn a_season_request_the_shows_fulfiller_refuses_fails_with_its_reason_and_retrying_once_it_recovers_moves_it_to_processing()
 {
    let fixture = Fixture::with_shows_fulfiller(&[]).await;
    let fulfiller = fixture.server.fulfiller();
    fulfiller.refuse();
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let pending = requests(as_ada).ask_seasons(DARK, &[1]).await;

    let failed = fixture.server.approve(pending[0].id).await;
    fulfiller.recover();
    let retried = fixture.server.retry(failed.id).await;

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
    let fixture = Fixture::with_shows_fulfiller(&[]).await;
    let fulfiller = fixture.server.fulfiller();
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let pending = requests(as_ada).ask_seasons(DARK, &[1]).await;
    forget_show_tvdb_ids(fixture.server.db()).await;

    let approved = fixture.server.approve(pending[0].id).await;

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
    let fixture = Fixture::with_shows_fulfiller(&[]).await;
    let fulfiller = fixture.server.fulfiller();
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let pending = requests(as_ada).ask(HEAT).await;

    let approved = fixture.server.approve(pending.id).await;

    assert_eq!(approved.state, RequestState::Approved);
    assert_eq!(fulfiller.handed_seasons(), []);
}

#[tokio::test]
async fn enabling_a_shows_fulfiller_hands_it_the_season_requests_approved_while_there_was_none() {
    let fixture = Fixture::with_test_choices_disabled(None).await;
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let ada = requests(as_ada);
    let pending = ada.ask_seasons(DARK, &[1, 2]).await;
    let mut approved = Vec::new();
    for request in pending {
        approved.push(fixture.server.approve(request.id).await);
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
        states(&list_once_handed_off(&ada).await),
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
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let ada = requests(as_ada);
    let mut approved = Vec::new();
    for movie in [HEAT, DUNE] {
        let pending = ada.ask(movie).await;
        approved.push(fixture.server.approve(pending.id).await);
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
        states(&list_once_handed_off(&ada).await),
        [
            (approved[1].id, RequestState::Processing),
            (approved[0].id, RequestState::Processing)
        ]
    );
}

#[tokio::test]
async fn enabling_a_movie_fulfiller_answers_without_waiting_for_it_to_take_the_waiting_requests() {
    let fulfiller = FakeFulfiller::default();
    fulfiller.stall();
    let fixture = Fixture::with_fulfiller_disabled(&fulfiller).await;
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let pending = requests(as_ada).ask(HEAT).await;
    fixture.server.approve(pending.id).await;

    tokio::time::timeout(WAIT, fixture.enable(MOVIE_FULFILLER))
        .await
        .expect("enabling waited for the Fulfiller to take the Request")
        .unwrap();

    tokio::time::timeout(WAIT, async {
        while fulfiller.handed().is_empty() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the waiting Request never went to the Fulfiller");
    assert_eq!(fulfiller.handed(), [HEAT]);
}

/// Ada's Request for Heat, approved while there was no Movie Fulfiller, once it has left Approved
/// after the Server is started again with `fulfiller` as the Movie Fulfiller.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn waiting_request_after_starting_with(fulfiller: &FakeFulfiller) -> Request {
    let fixture = Fixture::start().await;
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let ada = requests(as_ada);
    let pending = ada.ask(HEAT).await;
    let approved = fixture.server.approve(pending.id).await;
    assert_eq!(approved.state, RequestState::Approved);
    let restarted = Registry::new();
    let fulfiller = fulfiller.clone();
    restarted.describe_builtin(move || Fulfills::Movies.plugin(fulfiller.clone()));

    fixture.serve_again(restarted).await;

    let [request] = list_once_handed_off(&ada).await.try_into().unwrap();
    request
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
    let fulfiller = FakeFulfiller::default();
    fulfiller.refuse();
    let request = waiting_request_after_starting_with(&fulfiller).await;

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
    let stored = plugin_settings::stored(fixture.server.db())
        .await
        .unwrap()
        .unwrap();
    assert!(!stored[test_choices::ID].enabled);
}

#[tokio::test]
async fn a_processing_request_becomes_available_once_a_scan_matches_its_movie() {
    let fixture = Fixture::with_fulfiller(&[]).await;
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let ada = requests(as_ada);
    let processing = fixture.server.approve(ada.ask(ALIEN).await.id).await;
    assert_eq!(processing.state, RequestState::Processing);

    let _media = fixture.library_with_alien().await;

    assert_eq!(
        states(&ada.list().await),
        [(processing.id, RequestState::Available)]
    );
}

#[tokio::test]
async fn a_failed_request_becomes_available_without_its_reason_once_a_scan_matches_its_movie() {
    let fixture = Fixture::with_fulfiller(&[]).await;
    fixture.server.fulfiller().refuse();
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let ada = requests(as_ada);
    let failed = fixture.server.approve(ada.ask(ALIEN).await.id).await;
    assert_eq!(failed.state, RequestState::Failed);

    let _media = fixture.library_with_alien().await;

    let [available] = ada.list().await.try_into().unwrap();
    assert_eq!(available.state, RequestState::Available);
    assert_eq!(available.failure_reason, None);
}

#[tokio::test]
async fn a_scan_makes_a_pending_request_available_and_leaves_a_declined_one_declined() {
    let fixture = Fixture::start().await;
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let ada = requests(as_ada);
    let declined = fixture.server.decline(ada.ask(ALIEN).await.id, "").await;
    let pending = ada.ask(ALIEN).await;

    let _media = fixture.library_with_alien().await;

    assert_eq!(
        states(&ada.list().await),
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
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let ada = requests(as_ada);
    let first = ada.ask(ALIEN).await;
    let library = fixture.library_with_alien().await;
    let mut removed = subscribe(
        &fixture.server.client_config(),
        &[EventType::EVENT_TYPE_LIBRARY_ITEM_REMOVED],
    )
    .await;

    fs::remove_dir_all(library.folder().join("Alien (1979)")).unwrap();
    fixture.server.scan(library.id).await;
    removed.next().await;
    let again = ada.ask(ALIEN).await;

    assert_eq!(
        states(&ada.list().await),
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
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let ada = requests(as_ada);
    let pending = ada.ask(HEAT).await;

    let declined = fixture.server.decline(pending.id, "Not in 4K yet").await;
    let again = ada.ask(HEAT).await;

    assert_eq!(declined.state, RequestState::Declined);
    assert_eq!(declined.decline_reason.as_deref(), Some("Not in 4K yet"));
    assert_ne!(again.id, pending.id);
    assert_eq!(again.state, RequestState::Pending);
    assert_eq!(again.decline_reason, None);
    assert_eq!(ada.list().await, [again, declined], "newest first");
}

#[tokio::test]
async fn declining_with_a_blank_reason_gives_none() {
    let fixture = Fixture::start().await;
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let pending = requests(as_ada).ask(HEAT).await;

    let declined = fixture.server.decline(pending.id, "  ").await;

    assert_eq!(declined.decline_reason, None);
}

#[tokio::test]
async fn a_decline_reason_is_kept_without_its_surrounding_spaces() {
    let fixture = Fixture::start().await;
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let pending = requests(as_ada).ask(HEAT).await;

    let declined = fixture
        .server
        .decline(pending.id, "  Not in 4K yet\n")
        .await;

    assert_eq!(declined.decline_reason.as_deref(), Some("Not in 4K yet"));
}

#[tokio::test]
async fn cancelling_removes_a_request_and_tells_its_requesters() {
    let fixture = Fixture::start().await;
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let approved = fixture.server.admin_requests().ask(HEAT).await;
    let joined = requests(as_ada.clone()).ask(HEAT).await;
    let mut adas_events = subscribe_to_requests(&as_ada).await;

    fixture
        .server
        .admin_requests()
        .cancel_request(CancelRequestRequest {
            id: approved.id,
            ..Default::default()
        })
        .await
        .unwrap();
    let event = adas_events.next().await;

    let event::Event::RequestCancelled(cancelled) = event else {
        panic!("not the cancelling: {event:?}");
    };
    assert_eq!(cancelled.request.as_option(), Some(&joined));
    assert_eq!(requests(as_ada).list().await, []);
}

#[tokio::test]
async fn approving_a_request_that_isnt_pending_is_refused() {
    let fixture = Fixture::start().await;
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let pending = requests(as_ada).ask(HEAT).await;
    fixture.server.decline(pending.id, "").await;

    let refused = fixture
        .server
        .admin_requests()
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

    let refused = fixture
        .server
        .admin_requests()
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
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let ada = requests(as_ada);
    let id = ada.ask(HEAT).await.id;

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
    assert_eq!(ada.list().await[0].state, RequestState::Pending);
}

#[tokio::test]
async fn one_of_two_requesters_withdraws_and_the_request_stays_with_the_other() {
    let fixture = Fixture::start().await;
    let ada = fixture.server.user("Ada", &NEW_USER).await;
    let (ada, as_ada) = (ada.id, ada.client_config());
    let grace = fixture.server.user("Grace", &NEW_USER).await;
    let (grace, as_grace) = (grace.id, grace.client_config());
    let asked = requests(as_ada.clone()).ask(HEAT).await;
    requests(as_grace.clone()).ask(HEAT).await;
    let mut adas_events = subscribe_to_requests(&as_ada).await;
    let mut graces_events = subscribe_to_requests(&as_grace).await;

    requests(as_ada).withdraw(asked.id).await;

    for event in [adas_events.next().await, graces_events.next().await] {
        let event::Event::RequesterRemoved(removed) = event else {
            panic!("not the withdrawal: {event:?}");
        };
        assert_eq!(removed.user_id, ada);
    }
    let remaining = requests(as_grace).list().await;
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, asked.id);
    assert_eq!(requester_ids(&remaining[0]), [grace]);
}

#[tokio::test]
async fn the_last_requester_withdrawing_removes_the_request_and_the_movie_can_be_requested_again() {
    let fixture = Fixture::start().await;
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let ada = requests(as_ada.clone());
    let asked = ada.ask(HEAT).await;
    let mut adas_events = subscribe_to_requests(&as_ada).await;

    ada.withdraw(asked.id).await;
    let event = adas_events.next().await;
    let gone = ada.list().await;
    let again = ada.ask(HEAT).await;

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
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let ada = requests(as_ada);
    let asked = ada.ask(HEAT).await;
    fixture.server.approve(asked.id).await;

    let refused = ada.try_withdraw(asked.id).await;

    let refusal = assert_refused(
        refused,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_REQUEST_NOT_PENDING,
    );
    assert_eq!(refusal.field, "id");
    assert_eq!(ada.list().await[0].state, RequestState::Approved);
}

#[tokio::test]
async fn withdrawing_from_a_request_the_user_isnt_a_requester_of_is_refused() {
    let fixture = Fixture::start().await;
    let ada = fixture.server.user("Ada", &NEW_USER).await;
    let (ada, as_ada) = (ada.id, ada.client_config());
    let as_grace = fixture
        .server
        .user("Grace", &NEW_USER)
        .await
        .client_config();
    let asked = requests(as_ada.clone()).ask(HEAT).await;

    let refused = requests(as_grace).try_withdraw(asked.id).await;

    let refusal = assert_refused(
        refused,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_NOT_A_REQUESTER,
    );
    assert_eq!(refusal.field, "id");
    assert_eq!(requester_ids(&requests(as_ada).list().await[0]), [ada]);
}

#[tokio::test]
async fn without_view_requests_withdrawing_from_someone_elses_request_is_refused_as_not_found() {
    let fixture = Fixture::start().await;
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let as_grace = fixture
        .server
        .user("Grace", &[Permission::Request])
        .await
        .client_config();
    let asked = requests(as_ada).ask(HEAT).await;

    let refused = requests(as_grace).try_withdraw(asked.id).await;

    assert_refused(
        refused,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_REQUEST_NOT_FOUND,
    );
}

/// Subscribes to Request events, as the User `config` calls as.
async fn subscribe_to_requests(config: &ClientConfig) -> Subscription {
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

#[tokio::test]
async fn a_search_finds_movies_with_their_year_and_poster_and_no_library_or_request() {
    let fixture = Fixture::start().await;
    let as_grace = fixture
        .server
        .user("Grace", &NEW_USER)
        .await
        .client_config();

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
    let _media = fixture.library_with_alien().await;
    let as_grace = fixture
        .server
        .user("Grace", &NEW_USER)
        .await
        .client_config();

    let found = search_alien(as_grace).await;

    assert!(found[0].library_id.is_some(), "{:?}", found[0]);
    assert_eq!(found[1].library_id, None, "Alien: Romulus isn't in it");
}

#[tokio::test]
async fn a_movie_someone_else_requested_shows_their_request_and_asking_joins_it() {
    let fixture = Fixture::start().await;
    let ada = fixture.server.user("Ada", &NEW_USER).await;
    let (ada, as_ada) = (ada.id, ada.client_config());
    let grace = fixture.server.user("Grace", &NEW_USER).await;
    let (grace, as_grace) = (grace.id, grace.client_config());
    let adas = requests(as_ada).ask(ALIEN).await;

    let found = search_alien(as_grace.clone()).await;
    let joined = requests(as_grace.clone()).ask(ALIEN).await;
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
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let as_grace = fixture
        .server
        .user("Grace", &[Permission::RequestMovie])
        .await
        .client_config();
    requests(as_ada).ask(ALIEN).await;

    let found = search_alien(as_grace).await;

    assert!(found[0].request.is_unset(), "{:?}", found[0]);
}

#[tokio::test]
async fn searching_for_a_blank_title_is_refused() {
    let fixture = Fixture::start().await;
    let as_grace = fixture
        .server
        .user("Grace", &NEW_USER)
        .await
        .client_config();

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
    let as_grace = fixture
        .server
        .user("Grace", &NEW_USER)
        .await
        .client_config();

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
    let as_grace = fixture
        .server
        .user("Grace", &NEW_USER)
        .await
        .client_config();
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
        .server
        .tmdb()
        .requests()
        .into_iter()
        .filter(|request| request.starts_with("/t/p/"))
        .collect();
    assert_eq!(images.len(), 1, "only the poster was fetched: {images:?}");
}

#[tokio::test]
async fn requesting_seasons_1_and_2_of_a_show_creates_two_requests() {
    let fixture = Fixture::start().await;
    let as_grace = fixture
        .server
        .user("Grace", &NEW_USER)
        .await
        .client_config();
    let grace = requests(as_grace);

    let asked = grace.ask_seasons(DARK, &[2, 1, 2]).await;

    let seasons: Vec<_> = asked.iter().map(season_of).collect();
    assert_eq!(seasons, [Some(("Dark", 1)), Some(("Dark", 2))]);
    let mut listed = grace.list().await;
    listed.reverse();
    assert_eq!(listed, asked);
}

#[tokio::test]
async fn two_users_asking_for_the_same_season_share_one_request_with_both_as_requesters() {
    let fixture = Fixture::start().await;
    let ada = fixture.server.user("Ada", &NEW_USER).await;
    let (ada, as_ada) = (ada.id, ada.client_config());
    let grace = fixture.server.user("Grace", &NEW_USER).await;
    let (grace, as_grace) = (grace.id, grace.client_config());

    let first = requests(as_ada).ask_seasons(DARK, &[1]).await;
    let second = requests(as_grace).ask_seasons(DARK, &[1, 2]).await;

    assert_eq!(second.len(), 2);
    assert_eq!(second[0].id, first[0].id);
    assert_eq!(requester_ids(&second[0]), [ada, grace]);
    assert_eq!(requester_ids(&second[1]), [grace]);
}

#[tokio::test]
async fn a_user_who_may_auto_approve_shows_opens_approved_season_requests_and_one_who_may_not_pending_ones()
 {
    let fixture = Fixture::start().await;
    let auto = fixture
        .server
        .user("Ada", &[Permission::RequestTv, Permission::AutoApproveTv])
        .await
        .client_config();
    let covered = fixture
        .server
        .user("Linus", &[Permission::Request, Permission::AutoApprove])
        .await
        .client_config();
    let movies_only = fixture
        .server
        .user(
            "Grace",
            &[Permission::RequestTv, Permission::AutoApproveMovie],
        )
        .await
        .client_config();

    let approved = requests(auto).ask_seasons(DARK, &[1]).await;
    let also_approved = requests(covered).ask_seasons(DARK, &[2]).await;
    let pending = requests(movies_only).ask_seasons(THE_OFFICE, &[1]).await;

    assert_eq!(approved[0].state, RequestState::Approved);
    assert_eq!(also_approved[0].state, RequestState::Approved);
    assert_eq!(pending[0].state, RequestState::Pending);
}

#[tokio::test]
async fn a_user_without_request_tv_is_refused_naming_the_permission() {
    let fixture = Fixture::start().await;
    let as_movie_asker = fixture
        .server
        .user("Ada", &[Permission::RequestMovie])
        .await
        .client_config();

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
        .server
        .matched_library(LibraryKind::LIBRARY_KIND_SHOWS, &[DARK_S01E01, DARK_S02E01])
        .await;
    let as_grace = fixture
        .server
        .user("Grace", &NEW_USER)
        .await
        .client_config();
    let grace = requests(as_grace);

    let refused = grace
        .create_season_requests(season_request(DARK, &[1, 2]))
        .await;
    let nothing_asked = grace.list().await;
    let partly_in_library = grace.ask_seasons(DARK, &[1]).await;

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
    let as_grace = fixture
        .server
        .user("Grace", &NEW_USER)
        .await
        .client_config();

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
    let as_grace = fixture
        .server
        .user("Grace", &NEW_USER)
        .await
        .client_config();

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
    let as_grace = fixture
        .server
        .user("Grace", &NEW_USER)
        .await
        .client_config();

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
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let as_grace = fixture
        .server
        .user("Grace", &[Permission::RequestTv])
        .await
        .client_config();
    let mut graces_events = subscribe_to_requests(&as_grace).await;

    requests(as_ada).ask_seasons(DARK, &[1]).await;
    let graces = requests(as_grace).ask_seasons(DARK, &[2]).await;
    let event = graces_events.next().await;

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
async fn matching_some_of_a_pending_seasons_episodes_leaves_it_pending_and_approvable_then_all_makes_it_available_live()
 {
    let fixture = Fixture::start().await;
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let ada = requests(as_ada.clone());
    let asked = ada.ask_seasons(DARK, &[1, 2]).await;
    let (season_1, season_2) = (asked[0].id, asked[1].id);
    let mut adas_events = subscribe_to_requests(&as_ada).await;

    let library = fixture
        .server
        .matched_library(LibraryKind::LIBRARY_KIND_SHOWS, &[DARK_S01E01])
        .await;
    let some_matched = states(&ada.list().await);
    fixture.server.approve(season_1).await;
    let approved = state_changed(adas_events.next().await);
    library.add(&[DARK_S01E02, DARK_S01E03]);
    fixture.server.scan(library.id).await;
    let all_matched = state_changed(adas_events.next().await);

    assert_eq!(
        some_matched,
        [
            (season_2, RequestState::Pending),
            (season_1, RequestState::Pending)
        ]
    );
    assert_eq!(approved, (season_1, RequestState::Approved));
    assert_eq!(all_matched, (season_1, RequestState::Available));
}

#[tokio::test]
async fn a_processing_season_request_becomes_partially_available_then_available_live() {
    let fixture = Fixture::with_shows_fulfiller(&[]).await;
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let ada = requests(as_ada.clone());
    let processing = fixture
        .server
        .approve(ada.ask_seasons(DARK, &[1]).await[0].id)
        .await;
    let mut adas_events = subscribe_to_requests(&as_ada).await;

    let library = fixture
        .server
        .matched_library(LibraryKind::LIBRARY_KIND_SHOWS, &[DARK_S01E01])
        .await;
    let some_matched = state_changed(adas_events.next().await);
    library.add(&[DARK_S01E02, DARK_S01E03]);
    fixture.server.scan(library.id).await;
    let all_matched = state_changed(adas_events.next().await);

    assert_eq!(processing.state, RequestState::Processing);
    assert_eq!(
        some_matched,
        (processing.id, RequestState::PartiallyAvailable)
    );
    assert_eq!(all_matched, (processing.id, RequestState::Available));
}

#[tokio::test]
async fn an_approved_season_request_waiting_for_a_fulfiller_stays_approved_after_a_partial_match_and_is_handed_off_once_one_is_enabled()
 {
    let fixture = Fixture::with_test_choices_disabled(None).await;
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let ada = requests(as_ada);
    let approved = fixture
        .server
        .approve(ada.ask_seasons(DARK, &[1]).await[0].id)
        .await;

    let _media = fixture
        .server
        .matched_library(LibraryKind::LIBRARY_KIND_SHOWS, &[DARK_S01E01])
        .await;
    let some_matched = states(&ada.list().await);
    fixture.enable_test_choices().await.unwrap();

    assert_eq!(some_matched, [(approved.id, RequestState::Approved)]);
    assert_eq!(
        states(&list_once_handed_off(&ada).await),
        [(approved.id, RequestState::Processing)]
    );
    assert_eq!(
        fixture.fulfiller_reference(approved.id).await.as_deref(),
        Some("show-70523-season-1")
    );
}

#[tokio::test]
async fn a_failed_season_request_keeps_its_reason_after_a_partial_match_and_can_be_retried() {
    let fixture = Fixture::with_shows_fulfiller(&[]).await;
    let fulfiller = fixture.server.fulfiller();
    fulfiller.refuse();
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let ada = requests(as_ada);
    let failed = fixture
        .server
        .approve(ada.ask_seasons(DARK, &[1]).await[0].id)
        .await;

    let _media = fixture
        .server
        .matched_library(LibraryKind::LIBRARY_KIND_SHOWS, &[DARK_S01E01])
        .await;
    let some_matched = ada.get(failed.id).await;
    fulfiller.recover();
    let retried = fixture.server.retry(failed.id).await;

    assert_eq!(some_matched.state, RequestState::Failed);
    assert_eq!(
        some_matched.failure_reason.as_deref(),
        Some("Sonarr is unreachable")
    );
    assert_eq!(retried.state, RequestState::Processing);
}

#[tokio::test]
async fn a_season_still_airing_becomes_available_once_its_aired_episodes_are_matched_and_a_later_episode_does_not_reopen_it()
 {
    let fixture = Fixture::start().await;
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let ada = requests(as_ada);
    let asked = ada.ask_seasons(DARK, &[3]).await;

    let library = fixture
        .server
        .matched_library(LibraryKind::LIBRARY_KIND_SHOWS, &[DARK_S03E01])
        .await;
    let aired_matched = states(&ada.list().await);
    let mut matches = subscribe(
        &fixture.server.client_config(),
        &[EventType::EVENT_TYPE_ITEM_MATCHED],
    )
    .await;
    library.add(&[DARK_S03E02]);
    fixture.server.scan(library.id).await;
    let later = matches.next().await;

    assert_eq!(aired_matched, [(asked[0].id, RequestState::Available)]);
    assert!(matches!(later, event::Event::ItemMatched(_)), "{later:?}");
    assert_eq!(
        states(&ada.list().await),
        aired_matched,
        "still Available, with no new Request"
    );
}

#[tokio::test]
async fn a_scan_makes_an_approved_season_request_available_and_leaves_a_declined_one_declined() {
    let fixture = Fixture::start().await;
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let as_grace = fixture
        .server
        .user("Grace", &[Permission::RequestTv, Permission::AutoApproveTv])
        .await
        .client_config();
    let ada = requests(as_ada);
    let declined = fixture
        .server
        .decline(ada.ask_seasons(DARK, &[1]).await[0].id, "")
        .await;
    let approved = requests(as_grace).ask_seasons(DARK, &[1]).await;

    let _media = fixture
        .server
        .matched_library(
            LibraryKind::LIBRARY_KIND_SHOWS,
            &[DARK_S01E01, DARK_S01E02, DARK_S01E03],
        )
        .await;

    assert_eq!(
        states(&ada.list().await),
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
    let _media = fixture
        .server
        .matched_library(LibraryKind::LIBRARY_KIND_SHOWS, &[DARK_S01E01, DARK_S02E01])
        .await;
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let as_grace = fixture
        .server
        .user("Grace", &NEW_USER)
        .await
        .client_config();
    let adas = requests(as_ada).ask_seasons(DARK, &[1]).await;

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
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let as_grace = fixture
        .server
        .user("Grace", &[Permission::RequestTv])
        .await
        .client_config();
    requests(as_ada).ask_seasons(DARK, &[1]).await;

    let dark = look_up_dark(as_grace).await;

    assert!(dark.seasons[0].request.is_unset(), "{:?}", dark.seasons[0]);
}

#[tokio::test]
async fn a_show_search_finds_shows_with_their_year_and_poster() {
    let fixture = Fixture::start().await;
    let as_grace = fixture
        .server
        .user("Grace", &NEW_USER)
        .await
        .client_config();

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
    let as_grace = fixture
        .server
        .user("Grace", &NEW_USER)
        .await
        .client_config();

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
    let as_grace = fixture
        .server
        .user("Grace", &[Permission::RequestMovie])
        .await
        .client_config();

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
    let (followed, mut asked) = client.follow(DARK).await;
    let season_3 = asked.pop().unwrap();
    assert_eq!(season_of(&season_3), Some(("Dark", 3)));
    fixture
        .server
        .admin_requests()
        .cancel_request(CancelRequestRequest {
            id: season_3.id,
            ..Default::default()
        })
        .await
        .unwrap();
    let id = followed.id;
    fixture
        .server
        .db()
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
async fn approve_followed(fixture: &Fixture, id: i64) -> Result<FollowedShow, ConnectError> {
    approve_followed_at(fixture, id, None).await
}

/// [`approve_followed`], changing the Show's Quality to `quality` if given.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn approve_followed_at(
    fixture: &Fixture,
    id: i64,
    quality: Option<i64>,
) -> Result<FollowedShow, ConnectError> {
    fixture
        .server
        .admin_requests()
        .approve_followed_show(ApproveFollowedShowRequest {
            id,
            quality_id: quality,
            ..Default::default()
        })
        .await
        .map(|approved| approved.into_owned().followed_show.into_option().unwrap())
}

/// The Followed Shows the caller of `client` may see.
async fn list_followed(client: &RequestServiceClient<HttpClient>) -> Vec<FollowedShow> {
    client.list_in(RequestState::Unspecified).await.1
}

/// How many Requests and Followed Shows are Pending, as the caller of `client` is told.
async fn count_pending(client: &RequestServiceClient<HttpClient>) -> Result<u32, ConnectError> {
    client
        .count_pending_requests(CountPendingRequestsRequest::default())
        .await
        .map(|counted| counted.into_owned().count)
}

#[tokio::test]
async fn a_pending_followed_show_and_each_of_its_pending_season_requests_count_once_each_and_an_approved_request_not_at_all()
 {
    let fixture = Fixture::start().await;
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let ada_requests = requests(as_ada);
    let heat = ada_requests.ask(HEAT).await;
    let as_admin = fixture.server.admin_requests();
    assert_eq!(count_pending(&as_admin).await.unwrap(), 1);

    let (_, asked) = ada_requests.follow(DARK).await;
    fixture.server.approve(heat.id).await;

    assert_eq!(asked.len(), 3, "{asked:?}");
    assert_eq!(count_pending(&as_admin).await.unwrap(), 4);
}

#[tokio::test]
async fn without_view_requests_a_request_manager_counts_only_the_pending_requests_and_followed_shows_they_are_a_requester_of()
 {
    let fixture = Fixture::start().await;
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let as_grace = fixture
        .server
        .user("Grace", &[Permission::ManageRequests, Permission::Request])
        .await
        .client_config();
    let grace_requests = requests(as_grace);
    let ada_requests = requests(as_ada);
    ada_requests.ask(HEAT).await;
    ada_requests.follow(DARK).await;
    grace_requests.ask(ALIEN).await;

    assert_eq!(count_pending(&grace_requests).await.unwrap(), 1);
    let as_admin = fixture.server.admin_requests();
    assert_eq!(count_pending(&as_admin).await.unwrap(), 6);
}

#[tokio::test]
async fn counting_pending_requests_without_manage_requests_is_refused() {
    let fixture = Fixture::start().await;
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();

    let refused = count_pending(&requests(as_ada)).await;

    let refusal = assert_refused(
        refused,
        ErrorCode::PermissionDenied,
        RefusalReason::REFUSAL_REASON_PERMISSION_REQUIRED,
    );
    let permission = refusal.params.get("permission").map(String::as_str);
    assert_eq!(permission, Some("PERMISSION_MANAGE_REQUESTS"));
}

/// The Seasons of Dark the caller of `client` has Requests for, in Season order.
async fn dark_seasons_requested(client: &RequestServiceClient<HttpClient>) -> Vec<u32> {
    let mut seasons: Vec<u32> = client
        .list()
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
        .server
        .matched_library(LibraryKind::LIBRARY_KIND_SHOWS, &[DARK_S03E01])
        .await;
    let ada = fixture.server.user("Ada", &NEW_USER).await;
    let (ada, as_ada) = (ada.id, ada.client_config());
    let ada_requests = requests(as_ada);

    let (followed, asked) = ada_requests.follow(DARK).await;
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
        states(&ada_requests.list().await),
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
    let ada = fixture.server.user("Ada", &NEW_USER).await;
    let (ada, as_ada) = (ada.id, ada.client_config());
    let grace = fixture.server.user("Grace", &NEW_USER).await;
    let (grace, as_grace) = (grace.id, grace.client_config());
    requests(as_grace).follow(DARK).await;
    let (followed, _) = follow_dark_before_season_3(&fixture, &requests(as_ada.clone())).await;
    approve_followed(&fixture, followed.id).await.unwrap();
    let mut adas_events = subscribe_to_requests(&as_ada).await;

    let _media = fixture
        .server
        .matched_library(LibraryKind::LIBRARY_KIND_SHOWS, &[DARK_S03E01])
        .await;
    let event = adas_events.next().await;

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
    let as_ada = fixture
        .server
        .user("Ada", &[Permission::RequestTv, Permission::AutoApproveTv])
        .await
        .client_config();
    let ada = requests(as_ada);
    ada.ask_seasons(DARK, &[1]).await;

    let _media = fixture
        .server
        .matched_library(LibraryKind::LIBRARY_KIND_SHOWS, &[DARK_S03E01])
        .await;

    assert_eq!(dark_seasons_requested(&ada).await, [1]);
    assert_eq!(list_followed(&ada).await, []);
}

#[tokio::test]
async fn a_declined_followed_show_declines_its_season_requests_and_opens_no_request_for_a_new_season()
 {
    let fixture = Fixture::start().await;
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let ada = requests(as_ada);
    let (followed, asked) = follow_dark_before_season_3(&fixture, &ada).await;

    let declined = fixture
        .server
        .admin_requests()
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
        .server
        .matched_library(LibraryKind::LIBRARY_KIND_SHOWS, &[DARK_S03E01])
        .await;

    assert_eq!(declined.state, RequestState::Declined);
    assert_eq!(declined.decline_reason.as_deref(), Some("Not yet"));
    let listed = ada.list().await;
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
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let ada = requests(as_ada);
    let (followed, asked) = follow_dark_before_season_3(&fixture, &ada).await;
    approve_followed(&fixture, followed.id).await.unwrap();

    fixture
        .server
        .admin_requests()
        .cancel_followed_show(CancelFollowedShowRequest {
            id: followed.id,
            ..Default::default()
        })
        .await
        .unwrap();
    let _media = fixture
        .server
        .matched_library(LibraryKind::LIBRARY_KIND_SHOWS, &[DARK_S03E01])
        .await;

    assert_eq!(list_followed(&ada).await, []);
    let listed = ada.list().await;
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
    let as_movie_asker = fixture
        .server
        .user("Ada", &[Permission::RequestMovie])
        .await
        .client_config();

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
    let ada = fixture.server.user("Ada", &NEW_USER).await;
    let (ada, as_ada) = (ada.id, ada.client_config());
    let grace = fixture
        .server
        .user("Grace", &[Permission::RequestTv, Permission::AutoApproveTv])
        .await;
    let (grace, as_grace) = (grace.id, grace.client_config());

    let (first, _) = requests(as_ada).follow(DARK).await;
    let (joined, asked) = requests(as_grace.clone()).follow(DARK).await;

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
async fn a_user_who_may_auto_approve_tv_following_a_show_approves_someone_elses_pending_season_request_and_hands_it_off()
 {
    let fixture = Fixture::with_shows_fulfiller(&[]).await;
    let fulfiller = fixture.server.fulfiller();
    let ada = fixture.server.user("Ada", &NEW_USER).await;
    let (ada, as_ada) = (ada.id, ada.client_config());
    let grace = fixture
        .server
        .user("Grace", &[Permission::RequestTv, Permission::AutoApproveTv])
        .await;
    let (grace, as_grace) = (grace.id, grace.client_config());
    let season_2 = requests(as_ada).ask_seasons(DARK, &[2]).await;

    let (_, asked) = requests(as_grace).follow(DARK).await;

    let joined = asked.iter().find(|r| r.id == season_2[0].id);
    assert_eq!(
        joined.map(|r| r.state),
        Some(RequestState::Processing.into())
    );
    assert_eq!(joined.map(requester_ids), Some(vec![ada, grace]));
    let handed: Vec<u32> = fulfiller
        .handed_seasons()
        .iter()
        .map(|&(_, _, season, _)| season)
        .collect();
    assert!(handed.contains(&2), "Season 2 not handed off: {handed:?}");
}

#[tokio::test]
async fn following_an_approved_followed_show_without_auto_approve_leaves_someone_elses_pending_season_request_pending()
 {
    let fixture = Fixture::start().await;
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let as_bob = fixture.server.user("Bob", &NEW_USER).await.client_config();
    let as_grace = fixture
        .server
        .user("Grace", &NEW_USER)
        .await
        .client_config();
    let season_2 = requests(as_ada).ask_seasons(DARK, &[2]).await;
    let (followed, _) = requests(as_bob).follow(DARK).await;
    approve_followed(&fixture, followed.id).await.unwrap();

    let (_, asked) = requests(as_grace).follow(DARK).await;

    let joined = asked.iter().find(|r| r.id == season_2[0].id);
    assert_eq!(joined.map(|r| r.state), Some(RequestState::Pending.into()));
}

#[tokio::test]
async fn a_user_who_may_auto_approve_shows_opens_an_approved_followed_show_which_cant_be_approved_again()
 {
    let fixture = Fixture::start().await;
    let as_ada = fixture
        .server
        .user("Ada", &[Permission::RequestTv, Permission::AutoApproveTv])
        .await
        .client_config();

    let (followed, asked) = requests(as_ada).follow(DARK).await;
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
    let fixture = Fixture::with_shows_fulfiller(&[]).await;
    let fulfiller = fixture.server.fulfiller();
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let ada = requests(as_ada);
    let (followed, asked) = ada.follow(DARK).await;

    approve_followed(&fixture, followed.id).await.unwrap();

    assert_eq!(
        fulfiller.handed_seasons(),
        [
            (DARK, Some(DARK_TVDB), 1, true),
            (DARK, Some(DARK_TVDB), 2, true),
            (DARK, Some(DARK_TVDB), 3, true)
        ]
    );
    let mut listed = states(&ada.list().await);
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
    let fixture = Fixture::with_shows_fulfiller(&[]).await;
    let fulfiller = fixture.server.fulfiller();
    let as_ada = fixture
        .server
        .user("Ada", &[Permission::RequestTv, Permission::AutoApproveTv])
        .await
        .client_config();

    let (_, asked) = requests(as_ada).follow(DARK).await;

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
    let fixture = Fixture::with_shows_fulfiller(&[]).await;
    let fulfiller = fixture.server.fulfiller();
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let (_, asked) = requests(as_ada).follow(DARK).await;

    let approved = fixture.server.approve(asked[0].id).await;

    assert_eq!(approved.state, RequestState::Processing);
    assert_eq!(
        fulfiller.handed_seasons(),
        [(DARK, Some(DARK_TVDB), 1, false)]
    );
}

#[tokio::test]
async fn a_season_request_the_shows_fulfiller_cant_follow_for_fails_with_its_reason_and_retrying_once_it_can_moves_it_to_processing()
 {
    let fixture = Fixture::with_shows_fulfiller(&[]).await;
    let fulfiller = fixture.server.fulfiller();
    fulfiller.refuse_to_follow();
    let as_ada = fixture
        .server
        .user("Ada", &[Permission::RequestTv, Permission::AutoApproveTv])
        .await
        .client_config();

    let (_, asked) = requests(as_ada).follow(DARK).await;
    fulfiller.recover();
    let retried = fixture.server.retry(asked[0].id).await;

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
    let fixture = Fixture::with_shows_fulfiller(&[]).await;
    let fulfiller = fixture.server.fulfiller();
    let as_ada = fixture
        .server
        .user("Ada", &[Permission::RequestTv, Permission::AutoApproveTv])
        .await
        .client_config();
    let (followed, _) = requests(as_ada).follow(DARK).await;

    fixture
        .server
        .admin_requests()
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
    let fixture = Fixture::with_shows_fulfiller(&[]).await;
    let fulfiller = fixture.server.fulfiller();
    let as_ada = fixture
        .server
        .user("Ada", &[Permission::RequestTv, Permission::AutoApproveTv])
        .await
        .client_config();
    let ada = requests(as_ada);
    let (followed, _) = ada.follow(DARK).await;

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
    let fixture = Fixture::with_shows_fulfiller(&[]).await;
    let fulfiller = fixture.server.fulfiller();
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let (followed, _) = requests(as_ada).follow(DARK).await;

    fixture
        .server
        .admin_requests()
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
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let grace = fixture.server.user("Grace", &NEW_USER).await;
    let (grace, as_grace) = (grace.id, grace.client_config());
    let (ada, grace_requests) = (requests(as_ada), requests(as_grace));
    let (followed, _) = ada.follow(DARK).await;
    grace_requests.follow(DARK).await;
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

/// Subscribes to Followed Show events, as the User `config` calls as.
async fn subscribe_to_followed_shows(config: &ClientConfig) -> Subscription {
    subscribe(
        config,
        &[
            EventType::EVENT_TYPE_FOLLOWED_SHOW_CREATED,
            EventType::EVENT_TYPE_FOLLOWED_SHOW_REQUESTER_ADDED,
            EventType::EVENT_TYPE_FOLLOWED_SHOW_STATE_CHANGED,
            EventType::EVENT_TYPE_FOLLOWED_SHOW_CANCELLED,
            EventType::EVENT_TYPE_FOLLOWED_SHOW_REQUESTER_REMOVED,
            EventType::EVENT_TYPE_FOLLOWED_SHOW_WITHDRAWN,
        ],
    )
    .await
}

/// A Followed Show event: what happened, the User who joined or withdrew, and the Followed Show.
type FollowedShowEvent = (&'static str, Option<i64>, FollowedShow);

/// The next `count` events of `subscription`, each a Followed Show event.
#[expect(
    clippy::expect_used,
    clippy::panic,
    reason = "a test helper fails its test by panicking"
)]
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "any event but a Followed Show's fails the test"
)]
async fn next_followed_show_events(
    subscription: &mut Subscription,
    count: usize,
) -> Vec<FollowedShowEvent> {
    use event::Event as E;
    let mut sent = Vec::with_capacity(count);
    for _ in 0..count {
        let (happened, user, followed) = match subscription.next().await {
            E::FollowedShowCreated(e) => ("created", None, e.followed_show),
            E::FollowedShowRequesterAdded(e) => {
                ("requester added", Some(e.user_id), e.followed_show)
            }
            E::FollowedShowStateChanged(e) => ("state changed", None, e.followed_show),
            E::FollowedShowCancelled(e) => ("cancelled", None, e.followed_show),
            E::FollowedShowRequesterRemoved(e) => {
                ("requester removed", Some(e.user_id), e.followed_show)
            }
            E::FollowedShowWithdrawn(e) => ("withdrawn", None, e.followed_show),
            other => panic!("not a Followed Show event: {other:?}"),
        };
        let followed = followed
            .into_option()
            .expect("a Followed Show event without one");
        sent.push((happened, user, followed));
    }
    sent
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn withdraw_followed(client: &RequestServiceClient<HttpClient>, id: i64) {
    client
        .withdraw_followed_show(WithdrawFollowedShowRequest {
            id,
            ..Default::default()
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn each_change_to_a_followed_show_sends_it_as_it_is_after_or_as_it_was_before_its_removal() {
    let fixture = Fixture::start().await;
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let grace = fixture.server.user("Grace", &NEW_USER).await;
    let (grace, as_grace) = (grace.id, grace.client_config());
    let (ada_requests, grace_requests) = (requests(as_ada), requests(as_grace));
    let mut events = subscribe_to_followed_shows(&fixture.server.client_config()).await;

    let (opened, _) = ada_requests.follow(DARK).await;
    let (joined, _) = grace_requests.follow(DARK).await;
    withdraw_followed(&grace_requests, opened.id).await;
    let approved = approve_followed(&fixture, opened.id).await.unwrap();
    fixture
        .server
        .admin_requests()
        .cancel_followed_show(CancelFollowedShowRequest {
            id: opened.id,
            ..Default::default()
        })
        .await
        .unwrap();
    let (office, _) = ada_requests.follow(THE_OFFICE).await;
    withdraw_followed(&ada_requests, office.id).await;

    assert_eq!(
        next_followed_show_events(&mut events, 7).await,
        [
            ("created", None, opened),
            ("requester added", Some(grace), joined.clone()),
            ("requester removed", Some(grace), joined),
            ("state changed", None, approved.clone()),
            ("cancelled", None, approved),
            ("created", None, office.clone()),
            ("withdrawn", None, office),
        ]
    );
}

#[tokio::test]
async fn a_followed_show_event_reaches_its_requesters_and_view_requests_subscribers_and_no_other_user()
 {
    let fixture = Fixture::start().await;
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let as_grace = fixture
        .server
        .user("Grace", &[Permission::RequestTv])
        .await
        .client_config();
    let as_hopper = fixture
        .server
        .user("Hopper", &[Permission::RequestTv])
        .await
        .client_config();
    let mut adas_events = subscribe_to_followed_shows(&as_ada).await;
    let mut graces_events = subscribe_to_followed_shows(&as_grace).await;
    let mut hoppers_events = subscribe_to_followed_shows(&as_hopper).await;

    let (graces_requests, hoppers_requests) = (requests(as_grace), requests(as_hopper));

    let (office, _) = hoppers_requests.follow(THE_OFFICE).await;
    let (dark, _) = graces_requests.follow(DARK).await;
    withdraw_followed(&hoppers_requests, office.id).await;
    approve_followed(&fixture, dark.id).await.unwrap();
    fixture
        .server
        .admin_requests()
        .cancel_followed_show(CancelFollowedShowRequest {
            id: dark.id,
            ..Default::default()
        })
        .await
        .unwrap();
    // Both end on this second Followed Show, so an event that wrongly reached either shows up
    // before what they expect next.
    let (office_again, _) = hoppers_requests.follow(THE_OFFICE).await;
    graces_requests.follow(THE_OFFICE).await;
    withdraw_followed(&graces_requests, office_again.id).await;

    let kinds_and_ids = |sent: Vec<FollowedShowEvent>| -> Vec<(&str, i64)> {
        sent.into_iter()
            .map(|(happened, _, followed)| (happened, followed.id))
            .collect()
    };
    assert_eq!(
        kinds_and_ids(next_followed_show_events(&mut adas_events, 8).await),
        [
            ("created", office.id),
            ("created", dark.id),
            ("withdrawn", office.id),
            ("state changed", dark.id),
            ("cancelled", dark.id),
            ("created", office_again.id),
            ("requester added", office_again.id),
            ("requester removed", office_again.id),
        ]
    );
    assert_eq!(
        kinds_and_ids(next_followed_show_events(&mut graces_events, 5).await),
        [
            ("created", dark.id),
            ("state changed", dark.id),
            ("cancelled", dark.id),
            ("requester added", office_again.id),
            ("requester removed", office_again.id),
        ],
        "Hopper's Followed Show reached Grace"
    );
    assert_eq!(
        kinds_and_ids(next_followed_show_events(&mut hoppers_events, 5).await),
        [
            ("created", office.id),
            ("withdrawn", office.id),
            ("created", office_again.id),
            ("requester added", office_again.id),
            ("requester removed", office_again.id),
        ],
        "Grace's Followed Show reached Hopper"
    );
}

#[tokio::test]
async fn opening_a_followed_show_for_a_show_whose_seasons_are_all_in_a_library_sends_its_creation()
{
    let fixture = Fixture::start().await;
    let _media = fixture
        .server
        .matched_library(
            LibraryKind::LIBRARY_KIND_SHOWS,
            &[
                DARK_S01E01,
                DARK_S01E02,
                DARK_S01E03,
                DARK_S02E01,
                DARK_S03E01,
            ],
        )
        .await;
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let mut adas_events = subscribe_to_followed_shows(&as_ada).await;

    let (followed, asked) = requests(as_ada).follow(DARK).await;

    assert_eq!(asked, []);
    assert_eq!(
        next_followed_show_events(&mut adas_events, 1).await,
        [("created", None, followed)]
    );
}

#[tokio::test]
async fn a_scan_matching_more_of_a_season_in_a_library_when_its_show_was_followed_opens_no_request()
{
    let fixture = Fixture::start().await;
    let library = fixture
        .server
        .matched_library(LibraryKind::LIBRARY_KIND_SHOWS, &[DARK_S03E01])
        .await;
    let as_ada = fixture
        .server
        .user("Ada", &[Permission::RequestTv, Permission::AutoApproveTv])
        .await
        .client_config();
    let ada = requests(as_ada);
    ada.follow(DARK).await;
    let mut matches = subscribe(
        &fixture.server.client_config(),
        &[EventType::EVENT_TYPE_ITEM_MATCHED],
    )
    .await;

    library.add(&[DARK_S03E02]);
    fixture.server.scan(library.id).await;
    matches.next().await;

    assert_eq!(dark_seasons_requested(&ada).await, [1, 2]);
}

#[tokio::test]
async fn a_season_request_a_followed_show_opened_isnt_opened_again_once_cancelled() {
    let fixture = Fixture::start().await;
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let ada = requests(as_ada);
    let (followed, _) = follow_dark_before_season_3(&fixture, &ada).await;
    approve_followed(&fixture, followed.id).await.unwrap();
    let library = fixture
        .server
        .matched_library(LibraryKind::LIBRARY_KIND_SHOWS, &[DARK_S03E01])
        .await;
    let opened: Vec<_> = ada
        .list()
        .await
        .into_iter()
        .filter(|request| season_of(request) == Some(("Dark", 3)))
        .collect();
    fixture
        .server
        .admin_requests()
        .cancel_request(CancelRequestRequest {
            id: opened[0].id,
            ..Default::default()
        })
        .await
        .unwrap();
    let mut matches = subscribe(
        &fixture.server.client_config(),
        &[EventType::EVENT_TYPE_ITEM_MATCHED],
    )
    .await;

    library.add(&[DARK_S03E02]);
    fixture.server.scan(library.id).await;
    matches.next().await;

    assert_eq!(opened.len(), 1, "{opened:?}");
    assert_eq!(dark_seasons_requested(&ada).await, [1, 2]);
}

/// Offers one Quality for Shows as the Admin, of the profile `name` and named after it, and so the
/// default, in place of any offered before.
async fn offer_only_show_quality(fixture: &Fixture, name: &str) {
    fixture
        .server
        .replace_qualities(SHOWS_FULFILLER, vec![quality(name, name, true)])
        .await;
}

#[tokio::test]
async fn a_season_that_comes_out_later_opens_its_request_at_the_followed_shows_quality() {
    let fixture = Fixture::with_shows_fulfiller(&["4K", "HD"]).await;
    offer_only_show_quality(&fixture, "4K").await;
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let (followed, _) = follow_dark_before_season_3(&fixture, &requests(as_ada.clone())).await;
    approve_followed(&fixture, followed.id).await.unwrap();
    offer_only_show_quality(&fixture, "HD").await;
    let mut adas_events = subscribe_to_requests(&as_ada).await;

    let _media = fixture
        .server
        .matched_library(LibraryKind::LIBRARY_KIND_SHOWS, &[DARK_S03E01])
        .await;
    let event = adas_events.next().await;

    let event::Event::RequestCreated(created) = event else {
        panic!("not a new Request: {event:?}");
    };
    assert_eq!(season_of(&created.request), Some(("Dark", 3)));
    assert_eq!(followed.quality.name, "4K");
    assert_eq!(created.request.quality.name, "4K");
}

#[tokio::test]
async fn asking_for_a_movie_takes_the_askers_default_quality_and_refuses_one_not_offered() {
    let fixture = Fixture::with_fulfiller(&[HD_PROFILE, ULTRA_HD_PROFILE]).await;
    let offered = fixture.offer_hd_and_4k(MOVIE_FULFILLER).await;
    let as_ada = fixture.server.user("Ada", &CHOOSER).await.client_config();
    let ada = requests(as_ada);
    ada.set_default_quality(SetDefaultQualityRequest {
        plugin_id: MOVIE_FULFILLER.to_owned(),
        quality: Some(offered[1].id),
        ..Default::default()
    })
    .await
    .unwrap();

    let defaulted = ada.ask(ALIEN).await;
    let not_offered = ada.try_ask_at(HEAT, 999).await;

    assert_eq!(defaulted.quality.name, "4K");
    let refusal = assert_refused(
        not_offered,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_QUALITY_NOT_OFFERED,
    );
    assert_eq!(refusal.field, "quality");
}

#[tokio::test]
async fn asking_for_seasons_takes_the_shows_quality_and_refuses_another() {
    let fixture = Fixture::with_shows_fulfiller(&[HD_PROFILE, ULTRA_HD_PROFILE]).await;
    let offered = fixture.offer_hd_and_4k(SHOWS_FULFILLER).await;
    let as_ada = fixture.server.user("Ada", &CHOOSER).await.client_config();
    let ada = requests(as_ada);
    ada.ask_seasons_at(DARK, &[1], offered[1].id).await;

    let later = ada.ask_seasons(DARK, &[2]).await;
    let other = ada.try_ask_seasons_at(DARK, &[3], offered[0].id).await;

    assert_eq!(later[0].quality.name, "4K");
    let refusal = assert_refused(
        other,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_SHOW_HAS_QUALITY,
    );
    assert_eq!(refusal.field, "quality");
}

#[tokio::test]
async fn following_a_show_at_a_quality_opens_its_season_requests_at_it_unless_the_asker_may_not_choose()
 {
    let fixture = Fixture::with_shows_fulfiller(&[HD_PROFILE, ULTRA_HD_PROFILE]).await;
    let offered = fixture.offer_hd_and_4k(SHOWS_FULFILLER).await;
    let as_ada = fixture.server.user("Ada", &CHOOSER).await.client_config();
    let as_bob = fixture.server.user("Bob", &NEW_USER).await.client_config();

    let (followed, asked) = requests(as_ada).follow_at(DARK, offered[1].id).await;
    let not_allowed = requests(as_bob).try_follow_at(DARK, offered[1].id).await;

    assert_eq!(followed.quality.name, "4K");
    let asked_at: Vec<_> = asked
        .iter()
        .map(|request| request.quality.name.as_str())
        .collect();
    assert_eq!(asked_at, ["4K"; 3]);
    let refusal = assert_refused(
        not_allowed,
        ErrorCode::PermissionDenied,
        RefusalReason::REFUSAL_REASON_QUALITY_NOT_ALLOWED,
    );
    assert_eq!(refusal.field, "quality");
}

#[tokio::test]
async fn approving_a_season_at_another_quality_moves_its_show_and_once_handed_off_refuses_another()
{
    let fixture = Fixture::with_shows_fulfiller(&[HD_PROFILE, ULTRA_HD_PROFILE]).await;
    let offered = fixture.offer_hd_and_4k(SHOWS_FULFILLER).await;
    let as_ada = fixture.server.user("Ada", &CHOOSER).await.client_config();
    let (_, asked) = requests(as_ada).follow_at(DARK, offered[1].id).await;

    let approved = fixture.server.approve_at(asked[0].id, offered[0].id).await;
    let fixed = fixture
        .server
        .try_approve_at(asked[1].id, offered[1].id)
        .await;

    assert_eq!(approved.state, RequestState::Processing);
    assert_eq!(approved.quality.name, "HD");
    let change = &approved.quality_change;
    assert_eq!(change.changed_by.as_deref(), Some("Admin"));
    assert_eq!(change.previous_name.as_deref(), Some("4K"));
    let admin = fixture.server.admin_requests();
    let offered_to_managers = admin
        .list_offered_qualities(ListOfferedQualitiesRequest {
            kind: TitleKind::TITLE_KIND_SHOW.into(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .qualities;
    assert_eq!(offered_to_managers, offered);
    assert_eq!(admin.get(asked[1].id).await.quality.name, "HD");
    assert_eq!(list_followed(&admin).await[0].quality.name, "HD");
    let refusal = assert_refused(
        fixed,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_SHOW_HAS_QUALITY,
    );
    assert_eq!(refusal.field, "quality_id");
}

#[tokio::test]
async fn approving_a_followed_show_at_another_quality_hands_off_its_season_requests_at_that_one() {
    let fixture = Fixture::with_shows_fulfiller(&[HD_PROFILE, ULTRA_HD_PROFILE]).await;
    let offered = fixture.offer_hd_and_4k(SHOWS_FULFILLER).await;
    let as_ada = fixture.server.user("Ada", &CHOOSER).await.client_config();
    let (followed, asked) = requests(as_ada).follow_at(DARK, offered[1].id).await;

    let approved = approve_followed_at(&fixture, followed.id, Some(offered[0].id))
        .await
        .unwrap();
    let season_1 = fixture.server.admin_requests().get(asked[0].id).await;

    assert_eq!(approved.quality.name, "HD");
    assert_eq!(season_1.state, RequestState::Processing);
    assert_eq!(season_1.quality.name, "HD");
}

#[tokio::test]
async fn retrying_a_failed_request_at_another_quality_hands_it_off_at_that_one_keeping_only_that_change()
 {
    let fixture = Fixture::with_fulfiller(&[HD_PROFILE, ULTRA_HD_PROFILE]).await;
    let fulfiller = fixture.server.fulfiller();
    fulfiller.refuse();
    let offered = fixture.offer_hd_and_4k(MOVIE_FULFILLER).await;
    let failed = fixture.server.admin_requests().ask(HEAT).await;
    let failed_again = fixture.server.retry_at(failed.id, offered[1].id).await;
    fulfiller.recover();

    let not_offered = fixture.server.try_retry_at(failed.id, 999).await;
    let retried = fixture.server.retry_at(failed.id, offered[0].id).await;

    assert_eq!(failed_again.state, RequestState::Failed);
    assert_eq!(
        failed_again.quality_change.previous_name.as_deref(),
        Some("HD")
    );
    assert_eq!(retried.state, RequestState::Processing);
    assert_eq!(retried.quality.name, "HD");
    assert_eq!(retried.quality_change.previous_name.as_deref(), Some("4K"));
    let refusal = assert_refused(
        not_offered,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_QUALITY_NOT_OFFERED,
    );
    assert_eq!(refusal.field, "quality_id");
}

#[tokio::test]
async fn an_auto_approved_joiners_quality_approves_a_pending_request_at_it_and_another_joiners_does_not()
 {
    let fixture = Fixture::with_fulfiller(&[HD_PROFILE, ULTRA_HD_PROFILE]).await;
    let offered = fixture.offer_hd_and_4k(MOVIE_FULFILLER).await;
    let as_ada = fixture.server.user("Ada", &CHOOSER).await.client_config();
    let as_bob = fixture.server.user("Bob", &CHOOSER).await.client_config();
    let auto_approved = [
        Permission::Request,
        Permission::AutoApproveMovie,
        Permission::ChooseQuality,
    ];
    let as_grace = fixture
        .server
        .user("Grace", &auto_approved)
        .await
        .client_config();
    requests(as_ada).ask_at(HEAT, offered[1].id).await;

    let pending = requests(as_bob).ask_at(HEAT, offered[0].id).await;
    let approved = requests(as_grace).ask_at(HEAT, offered[0].id).await;

    assert_eq!(pending.state, RequestState::Pending);
    assert_eq!(pending.quality.name, "4K");
    assert_eq!(approved.state, RequestState::Processing);
    assert_eq!(approved.quality.name, "HD");
    let change = &approved.quality_change;
    assert_eq!(change.changed_by.as_deref(), Some("Grace"));
    assert_eq!(change.previous_name.as_deref(), Some("4K"));
}

#[tokio::test]
async fn a_hand_off_shows_the_quality_whose_profile_the_fulfiller_reports_it_used() {
    let fixture = Fixture::with_fulfiller(&[HD_PROFILE, ULTRA_HD_PROFILE]).await;
    let fulfiller = fixture.server.fulfiller();
    let offered = fixture.offer_hd_and_4k(MOVIE_FULFILLER).await;
    fulfiller.report_used(ULTRA_HD_PROFILE, "Ultra-HD");

    let handed_off = fixture.server.admin_requests().ask(HEAT).await;

    assert_eq!(handed_off.quality.name, "HD");
    let used = &handed_off.quality_used;
    assert_eq!((used.id, used.name.as_str()), (offered[1].id, "4K"));
}

#[tokio::test]
async fn a_request_keeps_its_quality_and_profile_after_the_admin_removes_it_from_the_list() {
    let fixture = Fixture::with_fulfiller(&[HD_PROFILE, ULTRA_HD_PROFILE]).await;
    let offered = fixture.offer_hd_and_4k(MOVIE_FULFILLER).await;
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
    let pending = requests(as_ada).ask(HEAT).await;
    let only_4k = Quality {
        is_default: true,
        ..offered[1].clone()
    };

    fixture
        .server
        .replace_qualities(MOVIE_FULFILLER, vec![only_4k])
        .await;
    let approved = fixture.server.approve(pending.id).await;

    let kept = &approved.quality;
    assert_eq!(
        (kept.name.as_str(), kept.profile_id.as_str()),
        ("HD", HD_PROFILE)
    );
}

#[tokio::test]
async fn saving_qualities_the_list_rules_refuse_or_while_the_fulfillers_check_fails_is_refused() {
    let fixture = Fixture::with_fulfiller(&[HD_PROFILE]).await;
    let fulfiller = fixture.server.fulfiller();
    let hd = || quality(HD_PROFILE, "HD", true);

    let not_offered = fixture
        .server
        .try_replace_qualities(
            MOVIE_FULFILLER,
            vec![hd(), quality(ULTRA_HD_PROFILE, "4K", false)],
        )
        .await;
    let repeated = fixture
        .server
        .try_replace_qualities(
            MOVIE_FULFILLER,
            vec![hd(), quality(HD_PROFILE, "Full HD", false)],
        )
        .await;
    fulfiller.fail_checks();
    let check_failed = fixture
        .server
        .try_replace_qualities(MOVIE_FULFILLER, vec![hd()])
        .await;

    let refusal = assert_refused(
        not_offered,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_CHOICE_NOT_OFFERED,
    );
    assert_eq!(refusal.field, "qualities[1].profile_id");
    let repeated = assert_refused(
        repeated,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_QUALITY_REPEATED,
    );
    assert_eq!(repeated.field, "qualities[1].profile_id");
    assert_refused(
        check_failed,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_SETTINGS_CHECK_FAILED,
    );
    let listed = fixture
        .plugin_service()
        .list_qualities(ListQualitiesRequest {
            id: MOVIE_FULFILLER.to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(listed.into_owned().qualities, []);
}

#[tokio::test]
async fn a_default_the_admin_removed_is_flagged_in_settings_and_left_out_of_the_request_dialog() {
    let fixture = Fixture::with_fulfiller(&[HD_PROFILE, ULTRA_HD_PROFILE]).await;
    let offered = fixture.offer_hd_and_4k(MOVIE_FULFILLER).await;
    let as_ada = fixture.server.user("Ada", &CHOOSER).await.client_config();
    let ada = requests(as_ada);
    ada.set_default_quality(SetDefaultQualityRequest {
        plugin_id: MOVIE_FULFILLER.to_owned(),
        quality: Some(offered[1].id),
        ..Default::default()
    })
    .await
    .unwrap();

    fixture
        .server
        .replace_qualities(MOVIE_FULFILLER, vec![offered[0].clone()])
        .await;
    let settings = ada
        .list_default_qualities(ListDefaultQualitiesRequest::default())
        .await
        .unwrap()
        .into_owned()
        .defaults;
    let dialog = ada
        .list_movie_qualities(ListMovieQualitiesRequest::default())
        .await
        .unwrap()
        .into_owned();

    assert_eq!(settings.len(), 1);
    assert_eq!(settings[0].plugin_id, MOVIE_FULFILLER);
    assert_eq!(
        settings[0].fulfiller,
        ExtensionPoint::EXTENSION_POINT_MOVIE_FULFILLER
    );
    assert_eq!(settings[0].quality, Some(offered[1].id));
    assert_eq!(dialog.qualities, [offered[0].clone()]);
    assert_eq!(dialog.personal_default, None);
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
            version: "0.0.0".to_owned(),
            extension_points: vec![PointKey::provide::<dyn Notifier>()],
            ..Manifest::new(PluginId::from_static(self.0))
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
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();

    let request = requests(as_ada).ask(HEAT).await;

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
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();

    requests(as_ada).ask(HEAT).await;

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
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();

    let request = requests(as_ada).ask(HEAT).await;

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
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();

    let request = requests(as_ada).ask(HEAT).await;
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
    let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();

    let request = requests(as_ada).ask(HEAT).await;

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
        let as_ada = fixture.server.user("Ada", &NEW_USER).await.client_config();
        let ada = requests(as_ada);
        let failed = [
            "test-plugin",
            "notifier::Notifier",
            "provider failed",
            reason,
        ];
        let logged = log.count(&failed);

        let first = ada.ask(HEAT).await;
        log.wait_for(&failed, logged + 1).await;
        let second = ada.ask(ALIEN).await;

        assert_eq!(first.state, RequestState::Pending, "{notifier}");
        assert_eq!(second.state, RequestState::Pending, "{notifier}");
    }
}
