//! Library access per User: a User sees every Library, those added later included, or only those
//! an Admin chose, everywhere the Server hands out titles, over a real socket with the fake TMDB,
//! whose Similar source answers Alien's as Aliens and Heat.

use crate::support::{http, refused, server};

use std::time::{Duration, Instant};

use connectrpc::{
    ConnectError, ErrorCode,
    client::{ClientConfig, HttpClient},
};
use http::fetch;
use nama_proto::nama::v1::{
    BrowseServiceClient, CreateRequestRequest, EventType, ExternalId, GetMovieRequest,
    HomeRowTitle, LibraryAccess, LibraryKind, LibraryServiceClient, ListHomeRowTitlesRequest,
    ListHomeRowsRequest, ListLibrariesRequest, ListMoviesRequest, ListUnmatchedRequest,
    ListUsersRequest, LookUpMovieRequest, Permission, PlanPlaybackRequest, PlaybackServiceClient,
    RefusalReason, RequestServiceClient, SearchSubtitlesRequest, SetLibraryAccessRequest,
    UserServiceClient, event, home_row::KernelOrPlugin, home_row_title::MovieOrShow,
};
use refused::{assert_refused, refusal_of};
use server::{
    Library, SignedIn, TestServer, WAIT, search, subscribe,
    tmdb::{ALIEN, DARK, DUNE, HEAT},
};

const ALIEN_FILE: &str = "Alien (1979)/Alien (1979).mkv";
const HEAT_FILE: &str = "Heat (1995) {tmdb-949}/Heat (1995).mkv";

/// A Server with Alien and Heat each in a Movies Library of its own, Bishop seeing only Alien's,
/// Vasquez only Heat's, and Hicks every Library.
struct Fixture {
    server: TestServer,
    alien: Library,
    heat: Library,
    bishop: SignedIn,
    vasquez: SignedIn,
    hicks: SignedIn,
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    async fn start() -> Self {
        let server = TestServer::builder().tmdb().start().await;
        let movies = LibraryKind::LIBRARY_KIND_MOVIES;
        let alien = server.matched_library(movies, &[ALIEN_FILE]).await;
        let heat = server.matched_library(movies, &[HEAT_FILE]).await;
        let bishop = server.user_with_default_permissions("Bishop").await;
        let vasquez = server.user_with_default_permissions("Vasquez").await;
        let hicks = server.user_with_default_permissions("Hicks").await;
        let admin = server.client_config();
        set_access(&admin, bishop.id, Some(&[alien.id]))
            .await
            .unwrap();
        set_access(&admin, vasquez.id, Some(&[heat.id]))
            .await
            .unwrap();
        Self {
            server,
            alien,
            heat,
            bishop,
            vasquez,
            hicks,
        }
    }

    /// The id of the one Version of the Movie with the TMDB id `tmdb` in the Library `library`.
    async fn version(&self, library: i64, tmdb: i64) -> i64 {
        let movie = get_movie(&self.server.client_config(), library, tmdb).await;
        movie.unwrap().versions.first().unwrap().id
    }
}

/// Has `by` give the User `user` the Libraries `chosen`, or every one if it's `None`.
async fn set_access(
    by: &ClientConfig,
    user: i64,
    chosen: Option<&[i64]>,
) -> Result<(), ConnectError> {
    let users = UserServiceClient::new(HttpClient::plaintext(), by.clone());
    let access = chosen.map(|ids| LibraryAccess {
        library_ids: ids.to_vec(),
        ..Default::default()
    });
    let request = SetLibraryAccessRequest {
        user_id: user,
        library_access: access.into(),
        ..Default::default()
    };
    users.set_library_access(request).await.map(drop)
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

/// The ids of the Libraries `who` lists.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn libraries(who: &ClientConfig) -> Vec<i64> {
    let client = LibraryServiceClient::new(HttpClient::plaintext(), who.clone());
    let listed = client.list_libraries(ListLibrariesRequest::default()).await;
    let listed = listed.unwrap().into_owned().libraries;
    listed.iter().map(|library| library.id).collect()
}

/// The Movie with the TMDB id `tmdb` of the Library `library`, as `who` gets it.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn get_movie(
    who: &ClientConfig,
    library: i64,
    tmdb: i64,
) -> Result<nama_proto::nama::v1::Movie, ConnectError> {
    let client = browse(who);
    let got = client.get_movie(GetMovieRequest {
        library_id: library,
        movie_id: tmdb_id(tmdb).into(),
        ..Default::default()
    });
    Ok(got.await?.into_owned().movie.into_option().unwrap())
}

/// The TMDB ids of the Similar titles on the page of the Movie `tmdb` of `library`, as `who` sees
/// it.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn similar(who: &ClientConfig, library: i64, tmdb: i64) -> Vec<String> {
    let client = browse(who);
    let page = client.get_movie(GetMovieRequest {
        library_id: library,
        movie_id: tmdb_id(tmdb).into(),
        ..Default::default()
    });
    let similar = page.await.unwrap().into_owned().similar;
    similar
        .into_iter()
        .map(|hit| hit.id.value.clone())
        .collect()
}

/// The Libraries whose Recently Added row `who`'s home lists.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn recently_added(who: &ClientConfig) -> Vec<i64> {
    let client = browse(who);
    let rows = client.list_home_rows(ListHomeRowsRequest::default());
    (rows.await.unwrap().into_owned().rows.into_iter())
        .filter_map(|row| match row.kernel_or_plugin {
            Some(KernelOrPlugin::RecentlyAdded(row)) => Some(row.library_id),
            _ => None,
        })
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
async fn a_user_with_a_chosen_list_lists_and_browses_only_those_libraries() {
    let fx = Fixture::start().await;
    let bishop = fx.bishop.client_config();

    let client = browse(&bishop);
    let hidden = client.list_movies(ListMoviesRequest {
        library_id: fx.heat.id,
        ..Default::default()
    });
    let hidden_movie = get_movie(&bishop, fx.heat.id, HEAT).await;

    assert_eq!(libraries(&bishop).await, [fx.alien.id]);
    get_movie(&bishop, fx.alien.id, ALIEN).await.unwrap();
    let not_found = ErrorCode::NotFound;
    assert_refused(
        hidden.await,
        not_found,
        RefusalReason::REFUSAL_REASON_LIBRARY_NOT_FOUND,
    );
    assert_refused(
        hidden_movie,
        not_found,
        RefusalReason::REFUSAL_REASON_LIBRARY_NOT_FOUND,
    );
    let every = [fx.alien.id, fx.heat.id];
    assert_eq!(libraries(&fx.hicks.client_config()).await, every);
    (get_movie(&fx.hicks.client_config(), fx.heat.id, HEAT).await).unwrap();
}

#[tokio::test]
async fn a_library_added_later_is_seen_by_users_with_every_library_alone() {
    let fx = Fixture::start().await;

    let later = (fx.server)
        .matched_library(LibraryKind::LIBRARY_KIND_MOVIES, &[])
        .await;

    let every = [fx.alien.id, fx.heat.id, later.id];
    assert_eq!(libraries(&fx.hicks.client_config()).await, every);
    assert_eq!(libraries(&fx.bishop.client_config()).await, [fx.alien.id]);
}

#[tokio::test]
async fn search_and_request_look_ups_leave_out_the_titles_of_hidden_libraries() {
    let fx = Fixture::start().await;
    let vasquez = fx.vasquez.client_config();

    let found = search::movies(&vasquez, "Alien").await;
    let looked_up = RequestServiceClient::new(HttpClient::plaintext(), vasquez.clone())
        .look_up_movie(LookUpMovieRequest {
            movie_id: tmdb_id(ALIEN).into(),
            ..Default::default()
        })
        .await;

    let alien = ALIEN.to_string();
    assert!(found.iter().all(|hit| hit.id.value != alien), "{found:?}");
    assert_refused(
        looked_up,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_TITLE_NOT_FOUND,
    );
    let hicks_found = search::movies(&fx.hicks.client_config(), "Alien").await;
    let in_library = hicks_found.iter().find(|hit| hit.id.value == alien);
    assert_eq!(in_library.unwrap().library_id, Some(fx.alien.id));
}

#[tokio::test]
async fn similar_titles_and_recently_added_rows_leave_out_hidden_libraries() {
    let fx = Fixture::start().await;

    let bishops = similar(&fx.bishop.client_config(), fx.alien.id, ALIEN).await;
    let hicks = similar(&fx.hicks.client_config(), fx.alien.id, ALIEN).await;

    assert_eq!(bishops, Vec::<String>::new());
    assert_eq!(hicks, [HEAT.to_string()]);
    assert_eq!(
        recently_added(&fx.bishop.client_config()).await,
        [fx.alien.id]
    );
    let every = [fx.alien.id, fx.heat.id];
    assert_eq!(recently_added(&fx.hicks.client_config()).await, every);
}

#[tokio::test]
async fn a_plugin_row_leaves_out_a_hidden_librarys_title_rather_than_offer_it_as_missing() {
    let fx = Fixture::start().await;
    fx.server.enable_test_plugin("home-rows").await;
    let admin = fx.server.client_config();
    let started = Instant::now();
    while picks(&admin).await.len() < 4 {
        assert!(started.elapsed() < WAIT, "the row never refreshed");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    let bishops = picks(&fx.bishop.client_config()).await;
    let vasquezs = picks(&fx.vasquez.client_config()).await;

    let (dune, dark) = (
        format!("missing Movie {DUNE}"),
        format!("missing Show {DARK}"),
    );
    assert_eq!(
        bishops,
        [dune.clone(), format!("Movie {ALIEN}"), dark.clone()]
    );
    assert_eq!(vasquezs, [format!("Movie {HEAT}"), dune, dark]);
}

#[tokio::test]
async fn a_hidden_librarys_version_neither_plans_nor_direct_plays() {
    let fx = Fixture::start().await;
    let alien = fx.version(fx.alien.id, ALIEN).await;
    let plan = async |who: &SignedIn| {
        let playback = PlaybackServiceClient::new(HttpClient::plaintext(), who.client_config());
        let request = PlanPlaybackRequest {
            version_id: alien,
            ..Default::default()
        };
        playback.plan_playback(request).await.map(drop)
    };

    let refused = plan(&fx.vasquez).await;
    let planned = plan(&fx.bishop).await;
    let path = format!("/media/versions/{alien}");
    let hidden_file = fetch(&fx.server, &path, Some(&fx.vasquez.token)).await;
    let seen_file = fetch(&fx.server, &path, Some(&fx.bishop.token)).await;

    assert_refused(
        refused,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_VERSION_NOT_FOUND,
    );
    if let Err(err) = planned {
        let reason = refusal_of(&err).reason;
        assert_ne!(reason, RefusalReason::REFUSAL_REASON_VERSION_NOT_FOUND);
    }
    assert_eq!(hidden_file.status, 404);
    assert_eq!(seen_file.status, 200);
}

#[tokio::test]
async fn only_manage_users_changes_library_access_and_not_on_themself() {
    let fx = Fixture::start().await;
    let hudson = (fx.server)
        .user("Hudson", &[Permission::PERMISSION_MANAGE_USERS])
        .await;
    let as_hudson = hudson.client_config();

    let own = set_access(&fx.bishop.client_config(), fx.bishop.id, None).await;
    let hudsons_own = set_access(&as_hudson, hudson.id, None).await;
    let hudsons_for_bishop = set_access(&as_hudson, fx.bishop.id, Some(&[fx.heat.id])).await;

    let required = RefusalReason::REFUSAL_REASON_PERMISSION_REQUIRED;
    assert_refused(own, ErrorCode::PermissionDenied, required);
    assert_refused(hudsons_own, ErrorCode::PermissionDenied, required);
    hudsons_for_bishop.unwrap();
    assert_eq!(libraries(&fx.bishop.client_config()).await, [fx.heat.id]);
}

#[tokio::test]
async fn the_users_list_reads_each_chosen_list_back_less_a_deleted_library() {
    let fx = Fixture::start().await;
    let admin = fx.server.client_config();
    let both = [fx.alien.id, fx.heat.id];
    set_access(&admin, fx.bishop.id, Some(&both)).await.unwrap();
    let libraries_client = LibraryServiceClient::new(HttpClient::plaintext(), admin.clone());
    (libraries_client.delete_library(nama_proto::nama::v1::DeleteLibraryRequest {
        id: fx.heat.id,
        ..Default::default()
    }))
    .await
    .unwrap();

    let users = UserServiceClient::new(HttpClient::plaintext(), admin);
    let listed = users.list_users(ListUsersRequest::default()).await;
    let listed = listed.unwrap().into_owned().users;

    let access_of = |id: i64| {
        let user = listed.iter().find(|user| user.id == id).unwrap();
        (user.library_access.as_option()).map(|access| access.library_ids.clone())
    };
    assert_eq!(access_of(fx.bishop.id), Some(vec![fx.alien.id]));
    assert_eq!(access_of(fx.vasquez.id), Some(vec![]));
    assert_eq!(access_of(fx.hicks.id), None);
}

#[tokio::test]
async fn a_hidden_librarys_unmatched_items_are_not_listed() {
    let fx = Fixture::start().await;
    let list = async |who: &SignedIn| {
        let request = ListUnmatchedRequest {
            library_id: fx.heat.id,
            ..Default::default()
        };
        browse(&who.client_config()).list_unmatched(request).await
    };

    let hidden = list(&fx.bishop).await;
    let seen = list(&fx.hicks).await;

    assert_refused(
        hidden,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_LIBRARY_NOT_FOUND,
    );
    seen.unwrap();
}

#[tokio::test]
async fn creating_a_request_for_a_hidden_librarys_movie_is_refused_as_an_unknown_title() {
    let fx = Fixture::start().await;
    let create = async |who: &SignedIn| {
        let requests = RequestServiceClient::new(HttpClient::plaintext(), who.client_config());
        let request = CreateRequestRequest {
            movie_id: tmdb_id(ALIEN).into(),
            ..Default::default()
        };
        requests.create_request(request).await.map(drop)
    };

    let hidden = create(&fx.vasquez).await;
    let seen = create(&fx.hicks).await;

    assert_refused(
        hidden,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_UNKNOWN_TITLE,
    );
    assert_refused(
        seen,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_ALREADY_IN_LIBRARY,
    );
}

#[tokio::test]
async fn a_hidden_librarys_version_has_no_subtitles_to_search_for() {
    let fx = Fixture::start().await;
    let alien = fx.version(fx.alien.id, ALIEN).await;
    let gorman = (fx.server)
        .user("Gorman", &[Permission::PERMISSION_DOWNLOAD_SUBTITLES])
        .await;
    let admin = fx.server.client_config();
    let search = async || {
        let playback = PlaybackServiceClient::new(HttpClient::plaintext(), gorman.client_config());
        let request = SearchSubtitlesRequest {
            version_id: alien,
            languages: vec!["en".to_owned()],
            ..Default::default()
        };
        playback.search_subtitles(request).await.map(drop)
    };

    let seen = search().await;
    set_access(&admin, gorman.id, Some(&[fx.heat.id]))
        .await
        .unwrap();
    let hidden = search().await;

    // With no subtitle source enabled, a Version they see gets as far as asking for one.
    assert_refused(
        seen,
        ErrorCode::Unavailable,
        RefusalReason::REFUSAL_REASON_NO_SUBTITLE_SOURCE,
    );
    assert_refused(
        hidden,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_VERSION_NOT_FOUND,
    );
}

#[tokio::test]
async fn events_about_a_hidden_library_never_reach_its_subscriber() {
    let fx = Fixture::start().await;
    let item_added = [EventType::EVENT_TYPE_LIBRARY_ITEM_ADDED];
    let mut vasquezs = subscribe(&fx.vasquez.client_config(), &item_added).await;
    let mut hicks = subscribe(&fx.hicks.client_config(), &item_added).await;

    let library_of = |event| {
        let event::Event::LibraryItemAdded(added) = event else {
            panic!("not an Item added: {event:?}");
        };
        added.library_id
    };
    // One scan at a time, as two in flight publish their Items in either order.
    fx.alien.add(&["Aliens (1986)/Aliens (1986).mkv"]);
    fx.server.scan(fx.alien.id).await;
    assert_eq!(library_of(hicks.next().await), fx.alien.id);
    fx.heat.add(&["Dune (2021)/Dune (2021).mkv"]);
    fx.server.scan(fx.heat.id).await;

    // Alien's Item was published first, so Vasquez hearing Heat's first means they never heard it.
    assert_eq!(library_of(vasquezs.next().await), fx.heat.id);
    assert_eq!(library_of(hicks.next().await), fx.heat.id);
}

#[tokio::test]
async fn changing_a_users_library_access_ends_their_open_subscriptions() {
    let fx = Fixture::start().await;
    let mut hicks = subscribe(&fx.hicks.client_config(), &[]).await;

    set_access(&fx.server.client_config(), fx.hicks.id, Some(&[]))
        .await
        .unwrap();

    assert!(hicks.message().await.unwrap().is_none());
}
