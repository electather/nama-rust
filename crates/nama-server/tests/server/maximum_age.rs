//! A User's Maximum Age: a title whose Content Rating calls for an older age, or one without a
//! Content Rating unless unrated titles are allowed, reaches them nowhere the Server hands out
//! titles, over a real socket with the fake TMDB, which rates Alien R and Dark TV-MA and gives
//! Heat and Dune no US rating.

use crate::support::{http, refused, server};

use std::time::{Duration, Instant};

use connectrpc::{
    ConnectError, ErrorCode,
    client::{ClientConfig, HttpClient},
};
use http::fetch;
use nama_proto::nama::v1::{
    BrowseServiceClient, CreateRequestRequest, DeleteLibraryRequest, ExternalId, GetMovieRequest,
    GetShowRequest, GetUnmatchedVersionRequest, HomeRowTitle, LibraryKind, LibraryServiceClient,
    ListContinueWatchingRequest, ListEpisodesRequest, ListHomeRowTitlesRequest, ListMoviesRequest,
    ListShowsRequest, ListUnmatchedRequest, ListUsersRequest, LookUpMovieRequest, MaximumAge,
    Permission, PlanPlaybackRequest, PlaybackServiceClient, RefusalReason, ReportProgressRequest,
    RequestServiceClient, SetMaximumAgeRequest, UserServiceClient, WatchStateServiceClient,
    continue_watching_entry::MovieOrEpisode, home_row_title::MovieOrShow,
};
use refused::{assert_refused, refusal_of};
use server::{
    Library, SignedIn, TestServer, WAIT, search,
    tmdb::{ALIEN, DARK, DUNE, HEAT},
};

const ALIEN_FILE: &str = "Alien (1979)/Alien (1979).mkv";
const HEAT_FILE: &str = "Heat (1995) {tmdb-949}/Heat (1995).mkv";
const DARK_FILE: &str = "Dark (2017)/Season 01/Dark - S01E03.mkv";
/// A file the fake TMDB finds nothing for, so it stays Unmatched.
const HOME_MOVIES_FILE: &str = "Home Movies (2019)/Home Movies (2019).mkv";

/// A Server with Alien and Heat in a Movies Library and Dark in a Shows Library, Newt with
/// whatever Maximum Age a test gives her and Hicks with none.
struct Fixture {
    server: TestServer,
    movies: Library,
    shows: Library,
    newt: SignedIn,
    hicks: SignedIn,
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    async fn start() -> Self {
        let server = TestServer::builder().tmdb().start().await;
        let movies = (server)
            .matched_library(LibraryKind::LIBRARY_KIND_MOVIES, &[ALIEN_FILE, HEAT_FILE])
            .await;
        let shows = (server)
            .matched_library(LibraryKind::LIBRARY_KIND_SHOWS, &[DARK_FILE])
            .await;
        let newt = server.user_with_default_permissions("Newt").await;
        let hicks = server.user_with_default_permissions("Hicks").await;
        Self {
            server,
            movies,
            shows,
            newt,
            hicks,
        }
    }

    /// Gives Newt the Maximum Age `age`, letting unrated titles through if `allow_unrated`.
    async fn cap_newt(&self, age: u32, allow_unrated: bool) {
        let max = Some((age, allow_unrated));
        (set_maximum_age(&self.server.client_config(), self.newt.id, max).await).unwrap();
    }

    /// The TMDB ids of the Movies Library's Movies, as `who` lists them.
    async fn movies(&self, who: &SignedIn) -> Vec<String> {
        let client = browse(&who.client_config());
        let listed = client.list_movies(ListMoviesRequest {
            library_id: self.movies.id,
            ..Default::default()
        });
        let listed = listed.await.unwrap().into_owned().movies;
        listed.iter().map(|movie| movie.id.value.clone()).collect()
    }

    /// How many Shows the Shows Library lists to `who`.
    async fn show_count(&self, who: &SignedIn) -> usize {
        let client = browse(&who.client_config());
        let listed = client.list_shows(ListShowsRequest {
            library_id: self.shows.id,
            ..Default::default()
        });
        listed.await.unwrap().into_owned().shows.len()
    }

    /// The Movie with the TMDB id `tmdb`, as `who` gets it.
    async fn movie(
        &self,
        who: &ClientConfig,
        tmdb: i64,
    ) -> Result<nama_proto::nama::v1::GetMovieResponse, ConnectError> {
        let client = browse(who);
        let got = client.get_movie(GetMovieRequest {
            library_id: self.movies.id,
            movie_id: tmdb_id(tmdb).into(),
            ..Default::default()
        });
        Ok(got.await?.into_owned())
    }

    /// The id of the one Version of the Movie with the TMDB id `tmdb`.
    async fn version(&self, tmdb: i64) -> i64 {
        let admin = self.server.client_config();
        let movie = self.movie(&admin, tmdb).await.unwrap().movie.into_option();
        movie.unwrap().versions.first().unwrap().id
    }
}

/// Has `by` give the User `user` the Maximum Age `max`, an age and whether unrated titles reach
/// them, or none if it's `None`.
async fn set_maximum_age(
    by: &ClientConfig,
    user: i64,
    max: Option<(u32, bool)>,
) -> Result<(), ConnectError> {
    let users = UserServiceClient::new(HttpClient::plaintext(), by.clone());
    let maximum_age = max.map(|(age, allow_unrated)| MaximumAge {
        age,
        allow_unrated,
        ..Default::default()
    });
    let request = SetMaximumAgeRequest {
        user_id: user,
        maximum_age: maximum_age.into(),
        ..Default::default()
    };
    users.set_maximum_age(request).await.map(drop)
}

fn browse(who: &ClientConfig) -> BrowseServiceClient<HttpClient> {
    BrowseServiceClient::new(HttpClient::plaintext(), who.clone())
}

fn tmdb_id(tmdb: i64) -> ExternalId {
    ExternalId {
        namespace: "tmdb".to_owned(),
        value: tmdb.to_string(),
        ..Default::default()
    }
}

/// The TMDB ids of the Similar titles on Alien's page, as `who` sees it.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn similar_to_alien(fx: &Fixture, who: &SignedIn) -> Vec<String> {
    let page = fx.movie(&who.client_config(), ALIEN).await.unwrap();
    (page.similar.iter())
        .map(|hit| hit.id.value.clone())
        .collect()
}

/// The titles of the test Plugin's "picks" row as `who` sees it, as kind and TMDB id.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn picks(who: &ClientConfig) -> Vec<String> {
    let client = browse(who);
    let titles = client.list_home_row_titles(ListHomeRowTitlesRequest {
        plugin_id: "test-plugin".to_owned(),
        key: "picks".to_owned(),
        ..Default::default()
    });
    let titles = titles.await.unwrap().into_owned().titles;
    titles.iter().map(shown).collect()
}

fn shown(title: &HomeRowTitle) -> String {
    match &title.movie_or_show {
        Some(MovieOrShow::Movie(movie)) => format!("Movie {}", movie.id.value),
        Some(MovieOrShow::Show(show)) => format!("Show {}", show.id.value),
        Some(MovieOrShow::MissingMovie(movie)) => format!("missing Movie {}", movie.id.value),
        Some(MovieOrShow::MissingShow(show)) => format!("missing Show {}", show.id.value),
        None => "nothing".to_owned(),
    }
}

#[tokio::test]
async fn the_fake_tmdb_rates_the_titles_as_these_tests_assume() {
    let fx = Fixture::start().await;
    let admin = fx.server.client_config();
    let rating = async |tmdb| {
        let movie = fx.movie(&admin, tmdb).await.unwrap().movie.into_option();
        movie.unwrap().content_rating
    };

    assert_eq!(rating(ALIEN).await.as_deref(), Some("R"));
    assert_eq!(rating(HEAT).await, None);
}

#[tokio::test]
async fn a_capped_user_lists_and_opens_only_titles_rated_for_their_age() {
    let fx = Fixture::start().await;
    fx.cap_newt(17, false).await;

    let listed = fx.movies(&fx.newt).await;
    let alien = fx.movie(&fx.newt.client_config(), ALIEN).await;
    let heat = fx.movie(&fx.newt.client_config(), HEAT).await;

    assert_eq!(listed, [ALIEN.to_string()]);
    alien.unwrap();
    assert_refused(
        heat,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_NOTHING_MATCHED,
    );
    let every = [ALIEN.to_string(), HEAT.to_string()];
    assert_eq!(fx.movies(&fx.hicks).await, every);
}

#[tokio::test]
async fn a_show_and_its_episodes_go_by_the_shows_rating() {
    let fx = Fixture::start().await;
    fx.cap_newt(16, true).await;
    let newt = fx.newt.client_config();
    let dark = || tmdb_id(DARK);

    let client = browse(&newt);
    let show = client.get_show(GetShowRequest {
        library_id: fx.shows.id,
        show_id: dark().into(),
        ..Default::default()
    });
    let episodes = client.list_episodes(ListEpisodesRequest {
        library_id: fx.shows.id,
        show_id: dark().into(),
        ..Default::default()
    });

    assert_eq!(fx.show_count(&fx.newt).await, 0);
    let not_found = RefusalReason::REFUSAL_REASON_NOTHING_MATCHED;
    assert_refused(show.await, ErrorCode::NotFound, not_found);
    assert_refused(episodes.await, ErrorCode::NotFound, not_found);
    assert_eq!(fx.show_count(&fx.hicks).await, 1);
}

#[tokio::test]
async fn allowing_unrated_titles_lets_those_without_a_known_rating_through() {
    let fx = Fixture::start().await;
    fx.cap_newt(13, true).await;
    let newt = fx.newt.client_config();

    let listed = fx.movies(&fx.newt).await;
    let heat = search::movies(&newt, "Heat").await;
    let alien = search::movies(&newt, "Alien").await;

    assert_eq!(listed, [HEAT.to_string()]);
    let in_library = |hits: &[nama_proto::nama::v1::MovieHit]| {
        (hits.iter())
            .filter(|hit| hit.library_id.is_some())
            .map(|hit| hit.id.value.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(in_library(&heat), [HEAT.to_string()]);
    assert_eq!(in_library(&alien), Vec::<String>::new());
}

#[tokio::test]
async fn search_finds_nothing_over_the_age_nor_unrated_from_the_title_source() {
    let fx = Fixture::start().await;
    fx.cap_newt(13, false).await;

    let newts = search::movies(&fx.newt.client_config(), "Alien").await;
    let hicks = search::movies(&fx.hicks.client_config(), "Alien").await;

    assert!(newts.is_empty(), "{newts:?}");
    let alien = ALIEN.to_string();
    let in_library = hicks.iter().find(|hit| hit.id.value == alien);
    assert_eq!(in_library.unwrap().library_id, Some(fx.movies.id));
}

#[tokio::test]
async fn similar_titles_leave_out_unrated_ones_for_a_capped_user() {
    let fx = Fixture::start().await;
    fx.cap_newt(17, false).await;

    let newts = similar_to_alien(&fx, &fx.newt).await;
    let hicks = similar_to_alien(&fx, &fx.hicks).await;

    assert_eq!(newts, Vec::<String>::new());
    assert_eq!(hicks, [HEAT.to_string()]);
}

#[tokio::test]
async fn a_plugin_row_judges_titles_on_the_server_and_missing_alike_by_their_rating() {
    let fx = Fixture::start().await;
    // Without Dark on the Server, the row offers it as missing, rated as the title source rates it.
    let libraries = LibraryServiceClient::new(HttpClient::plaintext(), fx.server.client_config());
    let deleted = libraries.delete_library(DeleteLibraryRequest {
        id: fx.shows.id,
        ..Default::default()
    });
    deleted.await.unwrap();
    fx.server.enable_test_plugin("home-rows").await;
    let admin = fx.server.client_config();
    let started = Instant::now();
    while picks(&admin).await.len() < 4 {
        assert!(started.elapsed() < WAIT, "the row never refreshed");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    fx.cap_newt(17, false).await;
    let seventeen = picks(&fx.newt.client_config()).await;
    fx.cap_newt(13, true).await;
    let thirteen_or_unrated = picks(&fx.newt.client_config()).await;

    assert_eq!(
        seventeen,
        [format!("Movie {ALIEN}"), format!("missing Show {DARK}")]
    );
    assert_eq!(
        thirteen_or_unrated,
        [format!("Movie {HEAT}"), format!("missing Movie {DUNE}")]
    );
}

#[tokio::test]
async fn continue_watching_leaves_out_a_started_title_over_the_age() {
    let fx = Fixture::start().await;
    let newt = fx.newt.client_config();
    let watch_states = WatchStateServiceClient::new(HttpClient::plaintext(), newt.clone());
    for movie in [ALIEN, HEAT] {
        let report = ReportProgressRequest {
            version_id: fx.version(movie).await,
            position_ms: 60_000,
            duration_ms: 7_200_000,
            ..Default::default()
        };
        watch_states.report_progress(report).await.unwrap();
    }
    let started = async || {
        let client = browse(&newt);
        let listed = client.list_continue_watching(ListContinueWatchingRequest::default());
        let entries = listed.await.unwrap().into_owned().entries;
        (entries.iter())
            .filter_map(|entry| match &entry.movie_or_episode {
                Some(MovieOrEpisode::Movie(movie)) => Some(movie.id.value.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(started().await.len(), 2);

    fx.cap_newt(13, true).await;

    assert_eq!(started().await, [HEAT.to_string()]);
}

#[tokio::test]
async fn unmatched_items_count_as_unrated() {
    let fx = Fixture::start().await;
    let library = (fx.server)
        .matched_library(LibraryKind::LIBRARY_KIND_MOVIES, &[HOME_MOVIES_FILE])
        .await;
    let listed = async |who: &ClientConfig| {
        let request = ListUnmatchedRequest {
            library_id: library.id,
            ..Default::default()
        };
        let listed = browse(who).list_unmatched(request).await.unwrap();
        listed.into_owned().items
    };
    let version_id = listed(&fx.server.client_config()).await[0].version_id;
    let got = async || {
        let request = GetUnmatchedVersionRequest {
            library_id: library.id,
            version_id,
            ..Default::default()
        };
        browse(&fx.newt.client_config())
            .get_unmatched_version(request)
            .await
    };

    fx.cap_newt(18, false).await;
    let kept_out = listed(&fx.newt.client_config()).await;
    let refused = got().await;
    fx.cap_newt(18, true).await;
    let let_through = listed(&fx.newt.client_config()).await;

    assert!(kept_out.is_empty(), "{kept_out:?}");
    assert_refused(
        refused,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_VERSION_NOT_FOUND,
    );
    assert_eq!(let_through.len(), 1);
    got().await.unwrap();
}

#[tokio::test]
async fn a_version_of_a_title_over_the_age_neither_plans_nor_direct_plays() {
    let fx = Fixture::start().await;
    fx.cap_newt(13, true).await;
    let alien = fx.version(ALIEN).await;
    let plan = async |who: &SignedIn| {
        let playback = PlaybackServiceClient::new(HttpClient::plaintext(), who.client_config());
        let request = PlanPlaybackRequest {
            version_id: alien,
            ..Default::default()
        };
        playback.plan_playback(request).await.map(drop)
    };

    let refused = plan(&fx.newt).await;
    let planned = plan(&fx.hicks).await;
    let path = format!("/media/versions/{alien}");
    let capped_file = fetch(&fx.server, &path, Some(&fx.newt.token)).await;
    let file = fetch(&fx.server, &path, Some(&fx.hicks.token)).await;

    assert_refused(
        refused,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_VERSION_NOT_FOUND,
    );
    if let Err(err) = planned {
        let reason = refusal_of(&err).reason;
        assert_ne!(reason, RefusalReason::REFUSAL_REASON_VERSION_NOT_FOUND);
    }
    assert_eq!(capped_file.status, 404);
    assert_eq!(file.status, 200);
}

#[tokio::test]
async fn a_title_over_the_age_or_unrated_is_neither_looked_up_nor_requested() {
    let fx = Fixture::start().await;
    fx.cap_newt(13, false).await;
    let requests =
        |who: &SignedIn| RequestServiceClient::new(HttpClient::plaintext(), who.client_config());
    let create_dune = async |who: &SignedIn| {
        let request = CreateRequestRequest {
            movie_id: tmdb_id(DUNE).into(),
            ..Default::default()
        };
        requests(who).create_request(request).await.map(drop)
    };

    let newts_requests = requests(&fx.newt);
    let alien = newts_requests.look_up_movie(LookUpMovieRequest {
        movie_id: tmdb_id(ALIEN).into(),
        ..Default::default()
    });
    let newts_dune = create_dune(&fx.newt).await;
    let hicks_dune = create_dune(&fx.hicks).await;

    assert_refused(
        alien.await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_TITLE_NOT_FOUND,
    );
    assert_refused(
        newts_dune,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_UNKNOWN_TITLE,
    );
    hicks_dune.unwrap();
}

#[tokio::test]
async fn only_manage_users_sets_a_maximum_age_and_not_on_themself() {
    let fx = Fixture::start().await;
    let hudson = (fx.server)
        .user("Hudson", &[Permission::PERMISSION_MANAGE_USERS])
        .await;
    let as_hudson = hudson.client_config();
    let twelve = Some((12, false));

    let own = set_maximum_age(&fx.newt.client_config(), fx.newt.id, None).await;
    let hudsons_own = set_maximum_age(&as_hudson, hudson.id, twelve).await;
    let hudsons_for_newt = set_maximum_age(&as_hudson, fx.newt.id, twelve).await;

    let required = RefusalReason::REFUSAL_REASON_PERMISSION_REQUIRED;
    assert_refused(own, ErrorCode::PermissionDenied, required);
    assert_refused(hudsons_own, ErrorCode::PermissionDenied, required);
    hudsons_for_newt.unwrap();
    assert_eq!(fx.movies(&fx.newt).await, Vec::<String>::new());
}

#[tokio::test]
async fn the_users_list_reads_each_maximum_age_back() {
    let fx = Fixture::start().await;
    fx.cap_newt(12, true).await;
    let admin = fx.server.client_config();

    let users = UserServiceClient::new(HttpClient::plaintext(), admin);
    let listed = users.list_users(ListUsersRequest::default()).await;
    let listed = listed.unwrap().into_owned().users;

    let max_of = |id: i64| {
        let user = listed.iter().find(|user| user.id == id).unwrap();
        (user.maximum_age.as_option()).map(|max| (max.age, max.allow_unrated))
    };
    assert_eq!(max_of(fx.newt.id), Some((12, true)));
    assert_eq!(max_of(fx.hicks.id), None);
}
