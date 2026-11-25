//! Box Sets (`GLOSSARY.md`) over a real socket: on a Movie's page, on their own page and in a
//! Movies Library's list. A title source of this file's own has the Alien films in the Alien
//! Collection, whose name and listed Movies a test may change, and Batman in a Batman Collection
//! whose sequel no Library has; Heat is in none.

use crate::support::{http, refused, server};

use std::{
    collections::HashMap,
    future,
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};

use connectrpc::ErrorCode;
use connectrpc::client::HttpClient;
use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    contract::{
        BoxSet as SourceBoxSet, Image, ImageAnswer, ImageAsked, Movie, MovieAnswer, SearchQuery,
        SearchResults, TitleAsked, TitleId,
    },
    metadata::{MetadataError, SourceManifest, TitleSource},
};
use nama_plugin_upkeep::Upkeep;
use nama_proto::nama::v1::{
    BoxSet, BrowseServiceClient, CreateRequestRequest, ExternalId, GetBoxSetRequest,
    GetMovieRequest, JobServiceClient, LibraryKind, ListBoxSetsRequest, MatchServiceClient,
    Permission, RefusalReason, RequestServiceClient, RequestState, RunJobRequest, SetMatchRequest,
    home_row_title::MovieOrShow,
};
use nama_server::plugins::Registry;
use refused::assert_refused;
use server::{Library, TestServer};

/// The namespace and Plugin id of [`Series`].
const SERIES: &str = "series";

/// Each Movie [`Series`] has: its id, title, release date and Box Set.
const MOVIES: &[(&str, &str, &str, Option<&str>)] = &[
    ("1", "Alien", "1979-05-25", Some(ALIEN_COLLECTION)),
    ("2", "Aliens", "1986-07-18", Some(ALIEN_COLLECTION)),
    ("3", "Alien 3", "1992-05-22", Some(ALIEN_COLLECTION)),
    ("4", "Batman", "1989-06-23", Some(BATMAN_COLLECTION)),
    ("5", "Batman Returns", "1992-06-19", Some(BATMAN_COLLECTION)),
    ("6", "Heat", "1995-12-15", None),
];

const ALIEN_COLLECTION: &str = "100";
const BATMAN_COLLECTION: &str = "200";
const POSTER: &str = "/alien-collection.jpg";

/// What [`Series`] answers of the Alien Collection: its name and the ids it lists.
#[derive(Debug)]
struct AlienCollection {
    name: String,
    listed: Vec<&'static str>,
}

/// A title source with the Movies of [`MOVIES`], fetched by id, each with its Box Set. It finds
/// nothing by searching, which it answers only since a Library's order needs a source that does.
#[derive(Debug, Clone)]
struct Series(Arc<Mutex<AlienCollection>>);

impl Series {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(AlienCollection {
            name: "Alien Collection".to_owned(),
            listed: vec!["1", "2", "3"],
        })))
    }

    /// Answers the Alien Collection as `name`, listing `listed`, from the next fetch on.
    fn answer_alien_collection(&self, name: &str, listed: &[&'static str]) {
        let mut answered = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        name.clone_into(&mut answered.name);
        answered.listed = listed.to_vec();
    }

    fn box_set(&self, id: &str) -> SourceBoxSet {
        let (name, listed) = if id == ALIEN_COLLECTION {
            let answered = self.0.lock().unwrap_or_else(PoisonError::into_inner);
            (answered.name.clone(), answered.listed.clone())
        } else {
            ("Batman Collection".to_owned(), vec!["4", "5"])
        };
        SourceBoxSet {
            id: series_id(id).into(),
            name,
            overview: Some("Ripley meets the Xenomorph.".to_owned()),
            poster: Some(POSTER.to_owned()),
            backdrop: Some("/alien-collection-backdrop.jpg".to_owned()),
            movies: listed.into_iter().map(series_id).collect(),
            ..Default::default()
        }
    }
}

impl TitleSource for Series {
    fn namespace(&self) -> &str {
        SERIES
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
            |&(id, title, released, box_set)| Movie {
                ids: vec![series_id(id)],
                title: title.to_owned(),
                release_date: Some(released.to_owned()),
                box_set: box_set.map(|box_set| self.box_set(box_set)).into(),
                ..Default::default()
            },
        );
        Box::pin(future::ready(Ok(MovieAnswer {
            movie: movie.into(),
            ..Default::default()
        })))
    }

    fn image<'a>(
        &'a self,
        asked: &'a ImageAsked,
    ) -> BoxFuture<'a, Result<ImageAnswer, MetadataError>> {
        let image = (asked.path == POSTER).then(|| Image {
            content_type: "image/jpeg".to_owned(),
            bytes: include_bytes!(
                "../../../../plugins/tmdb/fixtures/images/AmR3JG1VQVxU8TfAvljUhfSFUOx.jpg"
            )
            .to_vec(),
            ..Default::default()
        });
        Box::pin(future::ready(Ok(ImageAnswer {
            image: image.into(),
            ..Default::default()
        })))
    }
}

impl Plugin for Series {
    fn manifest(&self) -> Manifest {
        Manifest {
            extension_points: vec![PointKey::provide::<dyn TitleSource>()],
            title_source: Some(SourceManifest {
                namespace: SERIES.to_owned(),
                looks_up: Vec::new(),
                searches: true,
            }),
            ..Manifest::new(PluginId::from_static(SERIES))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.provide::<dyn TitleSource>(Priority::DEFAULT, Arc::new(self.clone()));
        Ok(())
    }
}

fn series_id(id: &str) -> TitleId {
    TitleId {
        namespace: SERIES.to_owned(),
        value: id.to_owned(),
        ..Default::default()
    }
}

fn external_id(id: &str) -> ExternalId {
    ExternalId {
        namespace: SERIES.to_owned(),
        value: id.to_owned(),
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
    format!("{title} {{{SERIES}-{id}}}/{title}.mkv")
}

/// A Server with [`Series`] and Upkeep, browsed as the Admin unless a test signs in another User.
struct Fixture {
    server: TestServer,
    series: Series,
}

/// A Box Set's Movie as a test reads it: its title, and the Library it is on the Server in, or
/// `None` if it is missing.
type Shown = (String, Option<i64>);

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    async fn start() -> Self {
        let series = Series::new();
        let plugins = Registry::new();
        // Described too, as a Library's order names only sources the Server describes.
        let described = series.clone();
        plugins.describe_builtin(move || Arc::new(described.clone()));
        plugins.enable(Arc::new(series.clone())).unwrap();
        plugins.describe_builtin(|| Arc::new(Upkeep));
        let server = TestServer::builder().plugins(plugins).start().await;
        Self { server, series }
    }

    /// A Movies Library holding `titles`, scanned and Matched.
    async fn library(&self, titles: &[&str]) -> Library {
        self.library_with_items(titles).await.0
    }

    /// A Movies Library holding `titles`, scanned and Matched, with the item of each file by its
    /// path.
    async fn library_with_items(&self, titles: &[&str]) -> (Library, HashMap<String, i64>) {
        let files: Vec<String> = titles.iter().map(|title| file(title)).collect();
        let files: Vec<&str> = files.iter().map(String::as_str).collect();
        let library = (self.server)
            .create_library_searching(LibraryKind::LIBRARY_KIND_MOVIES, &files, &[SERIES])
            .await;
        let items = self.server.scan_and_match(&library).await;
        (library, items)
    }

    /// Fixes the Match of the file of `title` by hand, to the Movie `to`, as an Admin can.
    async fn pin(&self, items: &HashMap<String, i64>, title: &str, to: &str) {
        MatchServiceClient::new(HttpClient::plaintext(), self.server.client_config())
            .set_match(SetMatchRequest {
                item_id: items[&file(title)],
                title_id: external_id(to).into(),
                ..Default::default()
            })
            .await
            .unwrap();
    }

    fn browse(&self) -> BrowseServiceClient<HttpClient> {
        BrowseServiceClient::new(HttpClient::plaintext(), self.server.client_config())
    }

    /// A User with only `permissions`, browsing.
    async fn browse_as(&self, permissions: &[Permission]) -> BrowseServiceClient<HttpClient> {
        let user = self.server.user("viewer", permissions).await;
        BrowseServiceClient::new(HttpClient::plaintext(), user.client_config())
    }

    /// The Box Set on the page of the Movie `id` in `library`, as `browse` sees it.
    async fn movie_box_set(
        browse: &BrowseServiceClient<HttpClient>,
        library: &Library,
        id: &str,
    ) -> Option<BoxSet> {
        let page = browse.get_movie(GetMovieRequest {
            library_id: library.id,
            movie_id: external_id(id).into(),
            ..Default::default()
        });
        page.await.unwrap().into_owned().box_set.into_option()
    }

    /// The Alien Collection's page, as the Admin sees it.
    async fn alien_collection(&self) -> BoxSet {
        let browse = self.browse();
        let read = browse.get_box_set(GetBoxSetRequest {
            box_set_id: external_id(ALIEN_COLLECTION).into(),
            ..Default::default()
        });
        read.await
            .unwrap()
            .into_owned()
            .box_set
            .into_option()
            .unwrap()
    }

    /// The names of `library`'s Box Sets, as `browse` lists them.
    async fn box_sets(browse: &BrowseServiceClient<HttpClient>, library: &Library) -> Vec<String> {
        let listed = browse.list_box_sets(ListBoxSetsRequest {
            library_id: library.id,
            ..Default::default()
        });
        let listed = listed.await.unwrap().into_owned().box_sets;
        listed.into_iter().map(|box_set| box_set.name).collect()
    }

    /// Runs Upkeep's weekly `refresh-metadata` Job now, as an Admin can, and waits until the
    /// Alien Collection is named `name`, since a Job Run answers once it started, not done.
    async fn refresh_until_named(&self, name: &str) -> BoxSet {
        JobServiceClient::new(HttpClient::plaintext(), self.server.client_config())
            .run_job(RunJobRequest {
                plugin_id: "upkeep".to_owned(),
                job_id: "refresh-metadata".to_owned(),
                ..Default::default()
            })
            .await
            .unwrap();
        let refreshed = async {
            let mut box_set = self.alien_collection().await;
            while box_set.name != name {
                tokio::time::sleep(Duration::from_millis(20)).await;
                box_set = self.alien_collection().await;
            }
            box_set
        };
        tokio::time::timeout(Duration::from_secs(30), refreshed)
            .await
            .unwrap()
    }
}

/// Each of `box_set`'s Movies as a test reads it.
fn shown(box_set: &BoxSet) -> Vec<Shown> {
    (box_set.movies.iter())
        .filter_map(|movie| match movie.movie_or_show.as_ref()? {
            MovieOrShow::Movie(movie) => Some((movie.title.clone(), Some(movie.library_id))),
            MovieOrShow::MissingMovie(movie) => Some((movie.title.clone(), None)),
            MovieOrShow::Show(_) | MovieOrShow::MissingShow(_) => None,
        })
        .collect()
}

fn on(title: &str, library: &Library) -> Shown {
    (title.to_owned(), Some(library.id))
}

fn missing(title: &str) -> Shown {
    (title.to_owned(), None)
}

#[tokio::test]
async fn a_movies_page_shows_its_box_set_in_release_order_with_the_movies_the_server_lacks() {
    let fx = Fixture::start().await;
    let movies = fx.library(&["Aliens", "Alien"]).await;

    let box_set = Fixture::movie_box_set(&fx.browse(), &movies, "2").await;

    let box_set = box_set.expect("Aliens's Box Set");
    assert_eq!(box_set.id.value, ALIEN_COLLECTION);
    assert_eq!(box_set.name, "Alien Collection");
    assert_eq!(
        shown(&box_set),
        [
            on("Alien", &movies),
            on("Aliens", &movies),
            missing("Alien 3")
        ]
    );
    let Some(MovieOrShow::MissingMovie(alien_3)) = &box_set.movies[2].movie_or_show else {
        panic!("Alien 3 isn't missing: {:?}", box_set.movies[2]);
    };
    assert_eq!(alien_3.year, Some(1992));
}

#[tokio::test]
async fn a_box_set_has_its_name_overview_and_artwork_and_its_poster_is_served() {
    let fx = Fixture::start().await;
    fx.library(&["Alien", "Aliens"]).await;

    let box_set = fx.alien_collection().await;

    assert_eq!(
        box_set.overview.as_deref(),
        Some("Ripley meets the Xenomorph.")
    );
    assert!(box_set.backdrop_url.is_some());
    let poster = box_set.poster_url.unwrap();
    let served = http::fetch(&fx.server, &poster, Some(&fx.server.admin_token)).await;
    assert_eq!(served.status, 200);
}

#[tokio::test]
async fn a_movies_library_lists_the_box_sets_with_a_movie_in_it_by_name() {
    let fx = Fixture::start().await;
    let movies = fx.library(&["Batman", "Heat", "Aliens", "Alien"]).await;
    let other = fx.library(&["Heat"]).await;

    let listed = Fixture::box_sets(&fx.browse(), &movies).await;

    assert_eq!(listed, ["Alien Collection", "Batman Collection"]);
    assert_eq!(
        Fixture::box_sets(&fx.browse(), &other).await,
        Vec::<String>::new()
    );
}

#[tokio::test]
async fn one_movie_and_the_missing_ones_show_a_box_set_only_to_a_user_who_may_request_movies() {
    let fx = Fixture::start().await;
    let movies = fx.library(&["Alien"]).await;
    let may_request = fx.browse_as(&[Permission::PERMISSION_REQUEST_MOVIE]).await;
    let may_not = BrowseServiceClient::new(
        HttpClient::plaintext(),
        fx.server.user("watcher", &[]).await.client_config(),
    );

    let as_requester = Fixture::movie_box_set(&may_request, &movies, "1").await;
    let as_watcher = Fixture::movie_box_set(&may_not, &movies, "1").await;
    let read = may_not.get_box_set(GetBoxSetRequest {
        box_set_id: external_id(ALIEN_COLLECTION).into(),
        ..Default::default()
    });

    assert_eq!(
        shown(&as_requester.unwrap()),
        [on("Alien", &movies), missing("Aliens"), missing("Alien 3")]
    );
    assert_eq!(as_watcher, None);
    let refused = assert_refused(
        read.await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_BOX_SET_NOT_FOUND,
    );
    assert_eq!(refused.field, "box_set_id");
    assert_eq!(
        Fixture::box_sets(&may_request, &movies).await,
        ["Alien Collection"]
    );
    assert_eq!(
        Fixture::box_sets(&may_not, &movies).await,
        Vec::<String>::new()
    );
}

#[tokio::test]
async fn a_user_who_may_not_request_movies_sees_only_those_on_the_server() {
    let fx = Fixture::start().await;
    let movies = fx.library(&["Alien", "Aliens"]).await;
    let viewer = fx.browse_as(&[]).await;

    let box_set = Fixture::movie_box_set(&viewer, &movies, "1").await;

    let shown = shown(&box_set.unwrap());
    assert_eq!(shown, [on("Alien", &movies), on("Aliens", &movies)]);
}

#[tokio::test]
async fn a_box_set_split_between_libraries_shows_whole_and_is_listed_in_each() {
    let fx = Fixture::start().await;
    let movies = fx.library(&["Alien"]).await;
    let movies_4k = fx.library(&["Aliens"]).await;

    let box_set = fx.alien_collection().await;

    assert_eq!(
        shown(&box_set),
        [
            on("Alien", &movies),
            on("Aliens", &movies_4k),
            missing("Alien 3")
        ]
    );
    assert_eq!(
        Fixture::box_sets(&fx.browse(), &movies).await,
        ["Alien Collection"]
    );
    assert_eq!(
        Fixture::box_sets(&fx.browse(), &movies_4k).await,
        ["Alien Collection"]
    );
}

#[tokio::test]
async fn a_missing_movie_shows_on_the_server_once_it_is_matched() {
    let fx = Fixture::start().await;
    let movies = fx.library(&["Alien", "Aliens"]).await;
    let before = shown(&fx.alien_collection().await);

    movies.add(&[&file("Alien 3")]);
    fx.server.scan_and_match(&movies).await;

    assert_eq!(before[2], missing("Alien 3"));
    assert_eq!(
        shown(&fx.alien_collection().await)[2],
        on("Alien 3", &movies)
    );
}

#[tokio::test]
async fn a_movie_gone_from_the_server_shows_as_missing_without_a_refresh() {
    let fx = Fixture::start().await;
    let movies = fx.library(&["Alien", "Aliens", "Alien 3"]).await;

    std::fs::remove_file(movies.path(&file("Alien 3"))).unwrap();
    fx.server.scan_and_match(&movies).await;

    assert_eq!(
        shown(&fx.alien_collection().await),
        [
            on("Alien", &movies),
            on("Aliens", &movies),
            missing("Alien 3")
        ]
    );
}

#[tokio::test]
async fn a_missing_movie_someone_requested_shows_its_request() {
    let fx = Fixture::start().await;
    fx.library(&["Alien", "Aliens"]).await;
    RequestServiceClient::new(HttpClient::plaintext(), fx.server.client_config())
        .create_request(CreateRequestRequest {
            movie_id: external_id("3").into(),
            ..Default::default()
        })
        .await
        .unwrap();

    let box_set = fx.alien_collection().await;

    let Some(MovieOrShow::MissingMovie(alien_3)) = &box_set.movies[2].movie_or_show else {
        panic!("Alien 3 isn't missing: {:?}", box_set.movies[2]);
    };
    let request = alien_3.request.as_option().expect("its Request");
    assert_ne!(
        request.state.as_known(),
        Some(RequestState::REQUEST_STATE_UNSPECIFIED)
    );
}

#[tokio::test]
async fn refreshing_metadata_takes_the_box_sets_new_name_and_movies() {
    let fx = Fixture::start().await;
    let movies = fx.library(&["Alien", "Aliens"]).await;

    fx.series
        .answer_alien_collection("Alien Anthology", &["1", "2"]);
    let box_set = fx.refresh_until_named("Alien Anthology").await;

    assert_eq!(
        shown(&box_set),
        [on("Alien", &movies), on("Aliens", &movies)]
    );
}

#[tokio::test]
async fn a_box_set_listing_no_movies_groups_those_naming_it_by_release_date() {
    let fx = Fixture::start().await;
    fx.series.answer_alien_collection("Alien Collection", &[]);
    let movies = fx.library(&["Aliens", "Alien"]).await;

    let box_set = fx.alien_collection().await;

    assert_eq!(
        shown(&box_set),
        [on("Alien", &movies), on("Aliens", &movies)]
    );
}

#[tokio::test]
async fn a_pinned_match_lists_its_box_sets_missing_movies() {
    let fx = Fixture::start().await;
    let (movies, items) = fx.library_with_items(&["Heat"]).await;

    fx.pin(&items, "Heat", "1").await;

    assert_eq!(
        shown(&fx.alien_collection().await),
        [on("Alien", &movies), missing("Aliens"), missing("Alien 3")]
    );
}

#[tokio::test]
async fn a_box_set_no_matched_movie_names_any_more_is_not_found() {
    let fx = Fixture::start().await;
    fx.series
        .answer_alien_collection("Alien Collection", &["1", "2", "6"]);
    let (_, items) = fx.library_with_items(&["Alien", "Heat"]).await;

    fx.pin(&items, "Alien", "4").await;

    let read = fx
        .browse()
        .get_box_set(GetBoxSetRequest {
            box_set_id: external_id(ALIEN_COLLECTION).into(),
            ..Default::default()
        })
        .await;
    assert_refused(
        read,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_BOX_SET_NOT_FOUND,
    );
}

#[tokio::test]
async fn a_movie_its_box_set_does_not_list_still_shows_in_release_order() {
    let fx = Fixture::start().await;
    fx.series
        .answer_alien_collection("Alien Collection", &["1", "3"]);
    let movies = fx.library(&["Alien 3", "Aliens", "Alien"]).await;

    let box_set = fx.alien_collection().await;

    assert_eq!(
        shown(&box_set),
        [
            on("Alien", &movies),
            on("Aliens", &movies),
            on("Alien 3", &movies)
        ]
    );
}
