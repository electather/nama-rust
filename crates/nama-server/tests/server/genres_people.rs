//! The bundled Genres and People Plugin's Similar titles (#1587), over a real socket. A title
//! source of this file's own gives Movies known genres and Roles: Ridley Scott directed Alien,
//! Blade Runner and Prometheus, which no Library has; Event Horizon shares Alien's genres alone,
//! and Heat shares nothing with them.

use crate::support::server;

use std::{future, sync::Arc, time::Duration};

use connectrpc::client::HttpClient;
use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    contract::{Movie, MovieAnswer, Role, SearchQuery, SearchResults, TitleAsked, TitleId},
    metadata::{MetadataError, SourceManifest, TitleMethod, TitleSource},
};
use nama_plugin_upkeep::Upkeep;
use nama_proto::nama::v1::{
    BrowseServiceClient, EnablePluginRequest, ExternalId, GetMovieRequest, JobServiceClient,
    LibraryKind, LookUpMovieRequest, MovieHit, PluginPermission, RequestServiceClient,
    RunJobRequest,
};
use nama_server::plugins::Registry;
use server::{Library, TestServer};

/// The namespace and Plugin id of [`Scripted`].
const SCRIPTED: &str = "scripted";

/// Each Movie [`Scripted`] has: its id, title, genres and whether Ridley Scott directed it.
const MOVIES: &[(&str, &str, &[&str], bool)] = &[
    ("1", "Alien", &["Horror", "Science Fiction"], true),
    ("2", "Blade Runner", &["Science Fiction", "Thriller"], true),
    ("3", "Event Horizon", &["Horror", "Science Fiction"], false),
    ("4", "Heat", &["Crime"], false),
    ("9", "Prometheus", &["Horror", "Science Fiction"], true),
];

/// A title source with the Movies of [`MOVIES`], fetched by id. It finds nothing by searching,
/// which it answers only since a Library's order needs a source that does.
struct Scripted;

impl TitleSource for Scripted {
    fn namespace(&self) -> &str {
        SCRIPTED
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
        let asked = asked.id.as_option().map(|id| id.value.as_str());
        let movie = (MOVIES.iter().find(|movie| Some(movie.0) == asked)).map(
            |&(id, title, genres, by_scott)| Movie {
                ids: vec![scripted_id(id)],
                title: title.to_owned(),
                genres: genres.iter().map(|&genre| genre.to_owned()).collect(),
                crew: by_scott.then(ridley_scott).into_iter().collect(),
                ..Default::default()
            },
        );
        Box::pin(future::ready(Ok(MovieAnswer {
            movie: movie.into(),
            ..Default::default()
        })))
    }

    fn answers(&self, method: TitleMethod) -> bool {
        matches!(method, TitleMethod::Search | TitleMethod::Movie)
    }
}

impl Plugin for Scripted {
    fn manifest(&self) -> Manifest {
        Manifest {
            extension_points: vec![PointKey::provide::<dyn TitleSource>()],
            title_source: Some(SourceManifest {
                namespace: SCRIPTED.to_owned(),
                looks_up: Vec::new(),
                searches: true,
            }),
            ..Manifest::new(PluginId::from_static(SCRIPTED))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.provide::<dyn TitleSource>(Priority::DEFAULT, Arc::new(Self));
        Ok(())
    }
}

fn scripted_id(id: &str) -> TitleId {
    TitleId {
        namespace: SCRIPTED.to_owned(),
        value: id.to_owned(),
        ..Default::default()
    }
}

fn ridley_scott() -> Role {
    Role {
        person: TitleId {
            namespace: "tmdb".to_owned(),
            value: "578".to_owned(),
            ..Default::default()
        }
        .into(),
        name: "Ridley Scott".to_owned(),
        part: "Director".to_owned(),
        ..Default::default()
    }
}

/// The file of the Movie `title` in [`MOVIES`], named with its id as a hint.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn file(title: &str) -> String {
    let (id, ..) = MOVIES.iter().find(|movie| movie.1 == title).unwrap();
    format!("{title} {{{SCRIPTED}-{id}}}/{title}.mkv")
}

/// A Server with [`Scripted`] and Upkeep, and the Genres and People Plugin enabled granted
/// `granted`.
struct Fixture {
    server: TestServer,
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    async fn start(granted: &[PluginPermission]) -> Self {
        let plugins = Registry::new();
        // Described too, as a Library's order names only sources the Server describes.
        plugins.describe_builtin(|| Arc::new(Scripted));
        plugins.enable(Arc::new(Scripted)).unwrap();
        plugins.describe_builtin(|| Arc::new(Upkeep));
        let server = TestServer::builder().plugins(plugins).start().await;
        (server.plugin_service())
            .enable_plugin(EnablePluginRequest {
                id: "genres-people".to_owned(),
                granted_permissions: granted.iter().map(|&it| it.into()).collect(),
                ..Default::default()
            })
            .await
            .unwrap();
        Self { server }
    }

    /// A Movies Library holding `titles`, scanned and Matched.
    async fn library(&self, titles: &[&str]) -> Library {
        let files: Vec<String> = titles.iter().map(|title| file(title)).collect();
        let files: Vec<&str> = files.iter().map(String::as_str).collect();
        let library = (self.server)
            .create_library_searching(LibraryKind::LIBRARY_KIND_MOVIES, &files, &[SCRIPTED])
            .await;
        self.server.scan_and_match(&library).await;
        library
    }

    /// The titles of the Similar titles on the page of the Movie `id` in `library`.
    async fn page(&self, library: &Library, id: &str) -> Vec<String> {
        let browse = BrowseServiceClient::new(HttpClient::plaintext(), self.server.client_config());
        let page = browse.get_movie(GetMovieRequest {
            library_id: library.id,
            movie_id: external_id(id).into(),
            ..Default::default()
        });
        titles(&page.await.unwrap().into_owned().similar)
    }

    /// The titles of the Similar titles on the request page of the Movie `id`.
    async fn looked_up(&self, id: &str) -> Vec<String> {
        let requests =
            RequestServiceClient::new(HttpClient::plaintext(), self.server.client_config());
        let found = requests.look_up_movie(LookUpMovieRequest {
            movie_id: external_id(id).into(),
            ..Default::default()
        });
        let found = found.await.unwrap().into_owned().movie.into_option();
        titles(&found.unwrap().similar)
    }

    /// [`Self::page`] once it lists a title, since a Job Run answers once it started, not done.
    async fn page_once_listing(&self, library: &Library, id: &str) -> Vec<String> {
        let listing = async {
            let mut page = self.page(library, id).await;
            while page.is_empty() {
                tokio::time::sleep(Duration::from_millis(20)).await;
                page = self.page(library, id).await;
            }
            page
        };
        tokio::time::timeout(Duration::from_secs(30), listing)
            .await
            .unwrap()
    }

    /// Runs Upkeep's weekly `refresh-metadata` Job now, as an Admin can.
    async fn refresh_metadata(&self) {
        JobServiceClient::new(HttpClient::plaintext(), self.server.client_config())
            .run_job(RunJobRequest {
                plugin_id: "upkeep".to_owned(),
                job_id: "refresh-metadata".to_owned(),
                ..Default::default()
            })
            .await
            .unwrap();
    }
}

fn external_id(id: &str) -> ExternalId {
    ExternalId {
        namespace: SCRIPTED.to_owned(),
        value: id.to_owned(),
        ..Default::default()
    }
}

fn titles(hits: &[MovieHit]) -> Vec<String> {
    hits.iter().map(|hit| hit.title.clone()).collect()
}

#[tokio::test]
async fn a_shared_director_ranks_above_shared_genres_alone_and_a_title_sharing_nothing_is_absent() {
    let fx = Fixture::start(&[PluginPermission::PLUGIN_PERMISSION_READ_LIBRARIES]).await;
    fx.library(&["Alien", "Blade Runner", "Event Horizon", "Heat"])
        .await;

    // Prometheus is in no Library, so its request page asks the Plugin live, with its Roles.
    let prometheus = fx.looked_up("9").await;

    assert_eq!(prometheus, ["Alien", "Blade Runner", "Event Horizon"]);
}

#[tokio::test]
async fn after_refresh_metadata_a_title_matched_before_another_lists_it() {
    let fx = Fixture::start(&[PluginPermission::PLUGIN_PERMISSION_READ_LIBRARIES]).await;
    let first = fx.library(&["Alien"]).await;
    let later = fx.library(&["Blade Runner"]).await;
    let before = fx.page(&first, "1").await;
    let blade_runner = fx.page(&later, "2").await;

    fx.refresh_metadata().await;

    assert_eq!(before, Vec::<String>::new());
    assert_eq!(blade_runner, ["Alien"], "asked at its Match, Alien stored");
    assert_eq!(fx.page_once_listing(&first, "1").await, ["Blade Runner"]);
}

#[tokio::test]
async fn without_read_libraries_it_suggests_nothing() {
    let fx = Fixture::start(&[]).await;
    fx.library(&["Alien", "Blade Runner"]).await;

    assert_eq!(fx.looked_up("9").await, Vec::<String>::new());
}
