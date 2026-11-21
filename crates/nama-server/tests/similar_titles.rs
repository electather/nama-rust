//! A title's Similar titles in the Libraries (ADR 0081, ADR 0084), on its page and its request
//! page alike, over a real socket with the TMDB Plugin enabled against a fake TMDB, whose Similar
//! source answers Alien's as Aliens and Heat, and Dark's as Stranger Things and the US Office, and
//! with the test Plugin as a second Similar source. A title source of this file's own has a Movie
//! for every id, for a Library with more Movies than a row holds.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/browsing.rs"]
mod browsing;
#[expect(dead_code, reason = "this binary uses only some of the helpers")]
#[path = "support/home_row_plugin.rs"]
mod home_row_plugin;
#[path = "support/installed_test_plugin.rs"]
mod installed_test_plugin;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;
#[path = "support/test_plugin.rs"]
mod test_plugin;

use std::{collections::HashMap, future, sync::Arc};

use browsing::{ALIEN_1080P, tmdb_id};
use connectrpc::client::HttpClient;
use home_row_plugin::{PLUGIN, disable, enable, text};
use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    contract::{Movie, MovieAnswer, SearchQuery, SearchResults, TitleAsked, TitleId},
    metadata::{MetadataError, SourceManifest, TitleMethod, TitleSource},
};
use nama_proto::nama::v1::{
    BrowseServiceClient, GetMovieRequest, GetShowRequest, LibraryKind, LookUpMovieRequest,
    LookUpShowRequest, MatchServiceClient, MovieHit, RequestServiceClient, SetMatchRequest,
    ShowHit, UpdatePluginSettingsRequest,
};
use nama_server::plugins::Registry;
use server::{
    TestServer,
    tmdb::{ALIEN, DARK, HEAT, THE_OFFICE},
};

const HEAT_FILE: &str = "Heat (1995) {tmdb-949}/Heat (1995).mkv";
const DARK_S01E01: &str = "Dark (2017)/Season 01/Dark - S01E01.mkv";
/// The US Office, by its ID hint, since a search ties it with the UK one.
const THE_OFFICE_S01E01: &str = "The Office {tmdb-2316}/Season 01/The Office - S01E01.mkv";

/// A Server with Alien and Heat in a Movies Library and Dark and the US Office in a Shows Library,
/// browsed and looked up as the Admin.
struct Fixture {
    movies: i64,
    shows: i64,
    browse: BrowseServiceClient<HttpClient>,
    requests: RequestServiceClient<HttpClient>,
    _server: TestServer,
}

impl Fixture {
    async fn start() -> Self {
        let server = TestServer::builder().tmdb().start().await;
        let movies = (server
            .create_library(LibraryKind::LIBRARY_KIND_MOVIES, &[ALIEN_1080P, HEAT_FILE]))
        .await;
        server.scan_and_match(&movies).await;
        let files = [DARK_S01E01, THE_OFFICE_S01E01];
        let shows = server
            .create_library(LibraryKind::LIBRARY_KIND_SHOWS, &files)
            .await;
        server.scan_and_match(&shows).await;
        let config = server.client_config();
        Self {
            movies: movies.id,
            shows: shows.id,
            browse: BrowseServiceClient::new(HttpClient::plaintext(), config.clone()),
            requests: RequestServiceClient::new(HttpClient::plaintext(), config),
            _server: server,
        }
    }
}

/// The Library and TMDB id each of `hits` opens.
fn movies_opened(hits: &[MovieHit]) -> Vec<(Option<i64>, String)> {
    let opened = |hit: &MovieHit| (hit.library_id, hit.id.value.clone());
    hits.iter().map(opened).collect()
}

/// As [`movies_opened`], for Shows.
fn shows_opened(hits: &[ShowHit]) -> Vec<(Option<i64>, String)> {
    let opened = |hit: &ShowHit| (hit.library_id, hit.id.value.clone());
    hits.iter().map(opened).collect()
}

#[tokio::test]
async fn a_movies_page_and_request_page_show_its_similar_titles_in_a_library_alone() {
    let fx = Fixture::start().await;

    let page = fx.browse.get_movie(GetMovieRequest {
        library_id: fx.movies,
        movie_id: tmdb_id(ALIEN).into(),
        ..Default::default()
    });
    let page = page.await.unwrap().into_owned().similar;
    let looked_up = fx.requests.look_up_movie(LookUpMovieRequest {
        movie_id: tmdb_id(ALIEN).into(),
        ..Default::default()
    });
    let looked_up = looked_up.await.unwrap().into_owned().movie.into_option();

    // Aliens is in no Library, so only Heat shows.
    let heat = vec![(Some(fx.movies), HEAT.to_string())];
    assert_eq!(movies_opened(&page), heat);
    assert_eq!(page[0].title, "Heat");
    assert_eq!(movies_opened(&looked_up.unwrap().similar), heat);
}

#[tokio::test]
async fn a_shows_page_and_request_page_show_its_similar_titles_in_a_library_alone() {
    let fx = Fixture::start().await;

    let page = fx.browse.get_show(GetShowRequest {
        library_id: fx.shows,
        show_id: tmdb_id(DARK).into(),
        ..Default::default()
    });
    let page = page.await.unwrap().into_owned().similar;
    let looked_up = fx.requests.look_up_show(LookUpShowRequest {
        show_id: tmdb_id(DARK).into(),
        ..Default::default()
    });
    let looked_up = looked_up.await.unwrap().into_owned().show.into_option();

    // Stranger Things is in no Library, so only the Office shows.
    let the_office = vec![(Some(fx.shows), THE_OFFICE.to_string())];
    assert_eq!(shows_opened(&page), the_office);
    assert_eq!(shows_opened(&looked_up.unwrap().similar), the_office);
}

const DUNE_FILE: &str = "Dune (2021) {tmdb-438631}/Dune (2021).mkv";

/// Alien's and Heat's IMDb ids, as their Matches have them from TMDB.
const ALIEN_IMDB: &str = "imdb-tt0078748";
const HEAT_IMDB: &str = "imdb-tt0113277";

/// A Server with Alien, Heat and Dune in a Movies Library, Matched while the test Plugin, a
/// Similar source, answered as [`answer_with`] set it, beside TMDB's.
struct WithSource {
    server: TestServer,
    alien: i64,
    movies: i64,
    browse: BrowseServiceClient<HttpClient>,
    requests: RequestServiceClient<HttpClient>,
    matches: MatchServiceClient<HttpClient>,
}

#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a test helper fails its test by panicking"
)]
impl WithSource {
    async fn start(answer: &str) -> Self {
        let server = TestServer::builder().tmdb().start().await;
        server.enable_test_plugin("similar-source").await;
        answer_with(&server, answer).await;
        let files = [ALIEN_1080P, HEAT_FILE, DUNE_FILE];
        let movies = (server)
            .create_library(LibraryKind::LIBRARY_KIND_MOVIES, &files)
            .await;
        let items: HashMap<String, i64> = server.scan_and_match(&movies).await;
        let config = server.client_config();
        Self {
            alien: items[ALIEN_1080P],
            movies: movies.id,
            browse: BrowseServiceClient::new(HttpClient::plaintext(), config.clone()),
            requests: RequestServiceClient::new(HttpClient::plaintext(), config.clone()),
            matches: MatchServiceClient::new(HttpClient::plaintext(), config),
            server,
        }
    }

    /// The titles of the Similar titles on Alien's page.
    async fn alien_page(&self) -> Vec<String> {
        let page = self.browse.get_movie(GetMovieRequest {
            library_id: self.movies,
            movie_id: tmdb_id(ALIEN).into(),
            ..Default::default()
        });
        titles(&page.await.unwrap().into_owned().similar)
    }

    /// The titles of the Similar titles on Alien's request page.
    async fn alien_looked_up(&self) -> Vec<String> {
        let looked_up = self.requests.look_up_movie(LookUpMovieRequest {
            movie_id: tmdb_id(ALIEN).into(),
            ..Default::default()
        });
        let found = looked_up.await.unwrap().into_owned().movie.into_option();
        titles(&found.unwrap().similar)
    }

    /// Fixes Alien's Match by hand to Alien, which fetches it again and asks the Similar sources.
    async fn refresh_alien(&self) {
        (self.matches)
            .set_match(SetMatchRequest {
                item_id: self.alien,
                title_id: tmdb_id(ALIEN).into(),
                ..Default::default()
            })
            .await
            .unwrap();
    }
}

/// Saves `answer` as the test Plugin's `similar` setting: the titles it answers about every title,
/// as `<namespace>-<value>@<weight>`, comma-separated, or `fail` or `loop`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn answer_with(server: &TestServer, answer: &str) {
    (server.plugin_service())
        .update_plugin_settings(UpdatePluginSettingsRequest {
            id: PLUGIN.to_owned(),
            settings: std::iter::once(("similar".to_owned(), text(answer))).collect(),
            ..Default::default()
        })
        .await
        .unwrap();
}

fn titles(hits: &[MovieHit]) -> Vec<String> {
    hits.iter().map(|hit| hit.title.clone()).collect()
}

#[tokio::test]
async fn two_sources_merge_by_weight_each_title_once_at_its_heaviest_ties_to_the_lower_plugin_id() {
    // TMDB answers Aliens, in no Library, at 1.0 and Heat at 0.5. The test Plugin answers Dune at
    // 0.5, Heat by its IMDb id at 0.2, Alien itself by its IMDb id at 0.3, and a Movie in no
    // Library at 1.0. Heat keeping TMDB's 0.5 puts it ahead of Alien.
    let answer = format!(
        "tmdb-{DUNE}@0.5,{HEAT_IMDB}@0.2,{ALIEN_IMDB}@0.3,tmdb-{UNLISTED}@1.0",
        DUNE = server::tmdb::DUNE,
        UNLISTED = server::tmdb::UNLISTED,
    );
    let fx = WithSource::start(&answer).await;

    let page = fx.alien_page().await;
    let looked_up = fx.alien_looked_up().await;

    // `test-plugin` sorts before `tmdb`, so Dune wins its tie with Heat.
    assert_eq!(page, ["Dune", "Heat", "Alien"]);
    assert_eq!(looked_up, page);
}

#[tokio::test]
async fn a_failed_refresh_keeps_a_sources_last_answer_and_an_answer_replaces_it() {
    let fx = WithSource::start(&format!("tmdb-{}@1.0", server::tmdb::DUNE)).await;

    answer_with(&fx.server, "fail").await;
    fx.refresh_alien().await;
    let after_failing = fx.alien_page().await;
    answer_with(&fx.server, "").await;
    fx.refresh_alien().await;

    assert_eq!(after_failing, ["Dune", "Heat"]);
    assert_eq!(fx.alien_page().await, ["Heat"]);
}

#[tokio::test]
async fn disabling_a_source_drops_its_titles_and_enabling_it_waits_for_the_next_refresh() {
    let fx = WithSource::start(&format!("tmdb-{}@1.0", server::tmdb::DUNE)).await;

    disable(&fx.server).await;
    let disabled = fx.alien_page().await;
    enable(&fx.server).await;
    let enabled = fx.alien_page().await;
    fx.refresh_alien().await;

    assert_eq!(disabled, ["Heat"]);
    assert_eq!(enabled, ["Heat"], "enabling asks about no title");
    assert_eq!(fx.alien_page().await, ["Dune", "Heat"]);
}

#[tokio::test]
async fn a_request_page_asks_the_sources_live_and_leaves_out_one_running_past_its_limit() {
    let fx = WithSource::start("").await;

    answer_with(&fx.server, &format!("tmdb-{}@1.0", server::tmdb::DUNE)).await;
    let answered = fx.alien_looked_up().await;
    answer_with(&fx.server, "loop").await;
    let looping = fx.alien_looked_up().await;

    assert_eq!(answered, ["Dune", "Heat"]);
    assert_eq!(looping, ["Heat"]);
    assert_eq!(
        fx.alien_page().await,
        ["Heat"],
        "a title page reads what's stored"
    );
}

/// The namespace and Plugin id of [`Numbered`].
const NUMBERED: &str = "numbered";

/// A title source with a Movie for every id in its namespace, titled `Movie <id>`. It finds
/// nothing by searching, which it answers only since a Library's order needs a source that does.
struct Numbered;

impl TitleSource for Numbered {
    fn namespace(&self) -> &str {
        NUMBERED
    }

    fn search<'a>(
        &'a self,
        _query: &'a SearchQuery,
    ) -> BoxFuture<'a, Result<SearchResults, MetadataError>> {
        Box::pin(future::ready(Ok(SearchResults::default())))
    }

    fn movie<'a>(
        &'a self,
        asked: &'a TitleAsked,
    ) -> BoxFuture<'a, Result<MovieAnswer, MetadataError>> {
        let value = (asked.id.as_option()).map_or_else(String::new, |id| id.value.clone());
        let movie = Movie {
            title: format!("Movie {value}"),
            ids: vec![TitleId {
                namespace: NUMBERED.to_owned(),
                value,
                ..Default::default()
            }],
            ..Default::default()
        };
        Box::pin(future::ready(Ok(MovieAnswer {
            movie: movie.into(),
            ..Default::default()
        })))
    }

    fn answers(&self, method: TitleMethod) -> bool {
        matches!(method, TitleMethod::Search | TitleMethod::Movie)
    }
}

impl Plugin for Numbered {
    fn manifest(&self) -> Manifest {
        Manifest {
            extension_points: vec![PointKey::provide::<dyn TitleSource>()],
            title_source: Some(SourceManifest {
                namespace: NUMBERED.to_owned(),
                looks_up: Vec::new(),
                searches: true,
            }),
            ..Manifest::new(PluginId::from_static(NUMBERED))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.provide::<dyn TitleSource>(Priority::DEFAULT, Arc::new(Self));
        Ok(())
    }
}

#[tokio::test]
async fn a_row_holds_the_20_heaviest_titles_in_a_library() {
    let plugins = Registry::new();
    // Described too, as a Library's order names only sources the Server describes.
    plugins.describe_builtin(|| Arc::new(Numbered));
    plugins.enable(Arc::new(Numbered)).unwrap();
    let server = TestServer::builder().plugins(plugins).start().await;
    server.enable_test_plugin("similar-source").await;
    // Movie n weighs n / 100, so the heaviest 20 are Movies 25 down to 6.
    let answer: Vec<String> = (1..=25)
        .map(|n| format!("{NUMBERED}-{n}@{}", f64::from(n) / 100.0))
        .collect();
    answer_with(&server, &answer.join(",")).await;
    let files: Vec<String> = (1..=25)
        .map(|n| format!("Movie {n} (2000) {{{NUMBERED}-{n}}}/Movie {n} (2000).mkv"))
        .collect();
    let files: Vec<&str> = files.iter().map(String::as_str).collect();
    let movies = (server)
        .create_library_searching(LibraryKind::LIBRARY_KIND_MOVIES, &files, &[NUMBERED])
        .await;
    server.scan_and_match(&movies).await;
    let browse = BrowseServiceClient::new(HttpClient::plaintext(), server.client_config());

    let page = browse.get_movie(GetMovieRequest {
        library_id: movies.id,
        movie_id: nama_proto::nama::v1::ExternalId {
            namespace: NUMBERED.to_owned(),
            value: "1".to_owned(),
            ..Default::default()
        }
        .into(),
        ..Default::default()
    });
    let page = titles(&page.await.unwrap().into_owned().similar);

    let heaviest: Vec<String> = (6..=25).rev().map(|n| format!("Movie {n}")).collect();
    assert_eq!(page, heaviest);
}
