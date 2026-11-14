use std::sync::{Arc, Mutex, PoisonError};

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    metadata::{
        Candidate, Episode, ExternalId, ExternalIds, IMDB, Image, MetadataError, Movie,
        SearchQuery, Season, Show, TitleSource,
    },
    settings::{SettingField, SettingKind, SettingValue, Settings, SettingsSchema},
};

use super::{Error, MetadataSource, Source};
use crate::plugins::{Failure, Registry};

/// A title source that answers every question with the error `.0` makes.
struct Failing(fn() -> MetadataError);

impl TitleSource for Failing {
    fn namespace(&self) -> &'static str {
        "tmdb"
    }

    fn search<'a>(
        &'a self,
        _query: &'a SearchQuery,
    ) -> BoxFuture<'a, Result<Vec<Candidate>, MetadataError>> {
        Box::pin(async { Err(self.0()) })
    }

    fn movie<'a>(&'a self, _id: &'a ExternalId) -> BoxFuture<'a, Result<Movie, MetadataError>> {
        Box::pin(async { Err(self.0()) })
    }

    fn show<'a>(&'a self, _id: &'a ExternalId) -> BoxFuture<'a, Result<Show, MetadataError>> {
        Box::pin(async { Err(self.0()) })
    }

    fn season<'a>(
        &'a self,
        _show: &'a ExternalId,
        _season: u32,
    ) -> BoxFuture<'a, Result<Season, MetadataError>> {
        Box::pin(async { Err(self.0()) })
    }

    fn episode<'a>(
        &'a self,
        _show: &'a ExternalId,
        _season: u32,
        _episode: u32,
    ) -> BoxFuture<'a, Result<Episode, MetadataError>> {
        Box::pin(async { Err(self.0()) })
    }

    fn image<'a>(&'a self, _path: &'a str) -> BoxFuture<'a, Result<Image, MetadataError>> {
        Box::pin(async { Err(self.0()) })
    }
}

struct FailingPlugin(fn() -> MetadataError);

impl Plugin for FailingPlugin {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: PluginId::from_static("failing"),
            name: String::new(),
            description: String::new(),
            version: "0.0.0".to_owned(),
            settings: SettingsSchema {
                fields: vec![SettingField {
                    key: "api_key".to_owned(),
                    label: String::new(),
                    description: String::new(),
                    kind: SettingKind::Secret,
                    required: false,
                    default: None,
                }],
            },
            extension_points: vec![PointKey::provide::<dyn TitleSource>()],
            logo_svg: None,
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.provide::<dyn TitleSource>(Priority::DEFAULT, Arc::new(Failing(self.0)));
        Ok(())
    }
}

/// What asking for a Show, then for an image from the Plugin `failing`, fails with when its
/// Provider fails with `failure`, or when there is no Provider.
async fn asking(failure: Option<fn() -> MetadataError>) -> (Error, Error) {
    let plugins = Registry::new();
    if let Some(failure) = failure {
        plugins.enable(Arc::new(FailingPlugin(failure))).unwrap();
    }
    let source = MetadataSource::new(plugins.extension_points());
    let show = source.show(&ExternalId::tmdb(1399)).await.unwrap_err();
    let image = source
        .image(&PluginId::from_static("failing"), "/poster.jpg")
        .await
        .unwrap_err();
    (show, image)
}

#[tokio::test]
async fn without_a_provider_the_source_is_unavailable() {
    assert_eq!(asking(None).await, (Error::Unavailable, Error::Unavailable));
}

#[tokio::test]
async fn a_title_the_provider_does_not_have_is_not_found() {
    let failure = || MetadataError::NotFound;

    assert_eq!(
        asking(Some(failure)).await,
        (Error::NotFound, Error::NotFound)
    );
}

#[tokio::test]
async fn an_id_the_provider_cannot_look_up_is_not_found() {
    let failure = || MetadataError::UnsupportedId(ExternalId::new(IMDB, "tt0944947").unwrap());

    assert_eq!(
        asking(Some(failure)).await,
        (Error::NotFound, Error::NotFound)
    );
}

#[tokio::test]
async fn a_method_the_source_does_not_serve_is_not_found() {
    let failure = || MetadataError::Unsupported;

    assert_eq!(
        asking(Some(failure)).await,
        (Error::NotFound, Error::NotFound)
    );
}

#[tokio::test]
async fn a_provider_that_fails_is_unavailable() {
    let failure = || MetadataError::Failed("TMDB is unreachable".into());

    assert_eq!(
        asking(Some(failure)).await,
        (Error::Unavailable, Error::Unavailable)
    );
}

#[tokio::test]
async fn a_providers_failure_comes_without_its_plugins_secret_settings() {
    let plugins = Registry::new();
    let failure = || MetadataError::Failed("GET /tv/1399?api_key=s3cr3t failed".into());
    let api_key = SettingValue::Text("s3cr3t".to_owned());
    let settings = Settings::new([("api_key".to_owned(), api_key)].into());
    plugins
        .enable_with_settings(Arc::new(FailingPlugin(failure)), settings)
        .unwrap();

    let answer = plugins
        .ask_first(|metadata: Arc<dyn TitleSource>| async move {
            metadata.show(&ExternalId::tmdb(1399)).await
        })
        .await
        .unwrap();

    let blanked = Failure {
        kind: Error::Unavailable,
        message: "GET /tv/1399?api_key=… failed".to_owned(),
    };
    assert_eq!(answer.result.unwrap_err(), blanked);
}

/// How a [`Fake`] title source answers.
#[derive(Clone, Copy)]
enum Answers {
    /// With a title, Season or search hit.
    Has,
    /// With a search that finds nothing.
    Empty,
    NotFound,
    Unsupported,
    Fails,
}

/// The namespaces of the sources asked for a title or Season, in the order they were asked.
type Asked = Arc<Mutex<Vec<&'static str>>>;

/// A title source keeping titles under `namespace`, as the Plugin of that id.
struct Fake {
    namespace: &'static str,
    answers: Answers,
    asked: Asked,
}

impl Fake {
    fn answer<T>(&self, found: impl FnOnce(ExternalIds) -> T) -> Result<T, MetadataError> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(self.namespace);
        let ids = ExternalIds::from_iter([ExternalId::new(self.namespace, "1").unwrap()]);
        match self.answers {
            Answers::Has | Answers::Empty => Ok(found(ids)),
            Answers::NotFound => Err(MetadataError::NotFound),
            Answers::Unsupported => Err(MetadataError::Unsupported),
            Answers::Fails => Err(MetadataError::Failed("unreachable".into())),
        }
    }
}

impl TitleSource for Fake {
    fn namespace(&self) -> &str {
        self.namespace
    }

    fn search<'a>(
        &'a self,
        _query: &'a SearchQuery,
    ) -> BoxFuture<'a, Result<Vec<Candidate>, MetadataError>> {
        let empty = matches!(self.answers, Answers::Empty);
        let found = self.answer(|ids| {
            if empty {
                return Vec::new();
            }
            vec![Candidate {
                id: ExternalId::new(self.namespace, "1").unwrap(),
                ids,
                title: "Title".to_owned(),
                original_title: None,
                year: None,
                poster: None,
                origin_country: Vec::new(),
                vote_count: 0,
            }]
        });
        Box::pin(async { found })
    }

    fn movie<'a>(&'a self, _id: &'a ExternalId) -> BoxFuture<'a, Result<Movie, MetadataError>> {
        let found = self.answer(|ids| Movie {
            ids,
            title: "Title".to_owned(),
            original_title: None,
            overview: None,
            release_date: None,
            runtime_minutes: None,
            poster: None,
            backdrop: None,
        });
        Box::pin(async { found })
    }

    fn show<'a>(&'a self, _id: &'a ExternalId) -> BoxFuture<'a, Result<Show, MetadataError>> {
        let found = self.answer(|ids| Show {
            ids,
            title: "Title".to_owned(),
            original_title: None,
            overview: None,
            first_air_date: None,
            poster: None,
            backdrop: None,
            seasons: vec![1],
        });
        Box::pin(async { found })
    }

    fn season<'a>(
        &'a self,
        _show: &'a ExternalId,
        season: u32,
    ) -> BoxFuture<'a, Result<Season, MetadataError>> {
        let found = self.answer(|ids| Season {
            ids,
            number: season,
            title: String::new(),
            overview: None,
            air_date: None,
            poster: None,
            episodes: Vec::new(),
        });
        Box::pin(async { found })
    }
}

/// The Plugin serving a [`Fake`], registered at `.1`.
struct FakePlugin(&'static str, Priority, Answers, Asked);

impl Plugin for FakePlugin {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: PluginId::from_static(self.0),
            name: String::new(),
            description: String::new(),
            version: "0.0.0".to_owned(),
            settings: SettingsSchema::default(),
            extension_points: vec![PointKey::provide::<dyn TitleSource>()],
            logo_svg: None,
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        let source = Fake {
            namespace: self.0,
            answers: self.2,
            asked: Arc::clone(&self.3),
        };
        registrar.provide::<dyn TitleSource>(self.1, Arc::new(source));
        Ok(())
    }
}

/// Title sources under `aa` and `bb`, `aa` first in provider order, answering as given.
fn two(aa: Answers, bb: Answers) -> (MetadataSource, Asked) {
    let plugins = Registry::new();
    let asked = Asked::default();
    let fake = |namespace, priority, answers| {
        Arc::new(FakePlugin(namespace, priority, answers, Arc::clone(&asked)))
    };
    plugins.enable(fake("aa", Priority::EARLY, aa)).unwrap();
    plugins.enable(fake("bb", Priority::DEFAULT, bb)).unwrap();
    (MetadataSource::new(plugins.extension_points()), asked)
}

fn asked(asked: &Asked) -> Vec<&'static str> {
    asked.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

fn source(namespace: &'static str) -> Source {
    Source {
        plugin: PluginId::from_static(namespace),
        namespace: namespace.to_owned(),
    }
}

fn order(namespaces: &[&str]) -> Vec<String> {
    namespaces.iter().map(|&n| n.to_owned()).collect()
}

fn query() -> super::SearchQuery {
    super::SearchQuery {
        kind: super::TitleKind::Show,
        title: "Title".to_owned(),
        year: None,
    }
}

#[tokio::test]
async fn sources_come_in_provider_order_or_in_the_order_given() {
    let (metadata, _) = two(Answers::Has, Answers::Has);

    assert_eq!(metadata.sources().await, [source("aa"), source("bb")]);
    let reordered = metadata.in_order(order(&["bb", "aa"]));
    assert_eq!(reordered.sources().await, [source("bb"), source("aa")]);
    let restricted = metadata.in_order(order(&["bb", "cc"]));
    assert_eq!(restricted.sources().await, [source("bb")]);
    assert!(restricted.serves("bb").await);
    assert!(!restricted.serves("aa").await);
}

#[tokio::test]
async fn a_source_whose_namespace_is_not_well_formed_is_left_out() {
    let plugins = Registry::new();
    let asked = Asked::default();
    let malformed = FakePlugin("bad-ns", Priority::DEFAULT, Answers::Has, asked);
    plugins.enable(Arc::new(malformed)).unwrap();

    let metadata = MetadataSource::new(plugins.extension_points());
    assert_eq!(metadata.sources().await, []);
}

#[tokio::test]
async fn a_search_answers_from_the_first_source_in_order() {
    let (metadata, sources_asked) = two(Answers::Has, Answers::Has);

    let (found_by, _) = metadata
        .in_order(order(&["bb", "aa"]))
        .search_in_turn(&query())
        .await
        .unwrap();

    assert_eq!(found_by, source("bb"));
    assert_eq!(asked(&sources_asked), ["bb"]);
}

#[tokio::test]
async fn a_search_falls_through_a_source_that_finds_nothing() {
    for nothing in [Answers::NotFound, Answers::Empty, Answers::Unsupported] {
        let (metadata, sources_asked) = two(nothing, Answers::Has);

        let (found_by, candidates) = metadata.search_in_turn(&query()).await.unwrap();

        assert_eq!(found_by, source("bb"));
        assert_eq!(candidates.len(), 1);
        assert_eq!(asked(&sources_asked), ["aa", "bb"]);
    }
}

#[tokio::test]
async fn a_failing_source_stops_the_search() {
    let (metadata, sources_asked) = two(Answers::Fails, Answers::Has);

    assert_eq!(
        metadata.search_in_turn(&query()).await.unwrap_err(),
        Error::Unavailable
    );
    assert_eq!(asked(&sources_asked), ["aa"]);
}

#[tokio::test]
async fn a_search_no_source_answers_is_not_found_and_one_with_no_source_unavailable() {
    let (metadata, _) = two(Answers::NotFound, Answers::Empty);

    assert_eq!(
        metadata.search_in_turn(&query()).await.unwrap_err(),
        Error::NotFound
    );
    let none = metadata.in_order(order(&["cc"]));
    assert_eq!(
        none.search_in_turn(&query()).await.unwrap_err(),
        Error::Unavailable
    );
}

#[tokio::test]
async fn a_title_by_id_is_asked_first_of_its_namespaces_source() {
    let (metadata, sources_asked) = two(Answers::Has, Answers::Has);
    let id = ExternalId::new("bb", "1").unwrap();

    let (show_from, _) = metadata.show_by(&id).await.unwrap();
    let (movie_from, movie) = metadata.movie_by(&id).await.unwrap();

    assert_eq!((show_from, movie_from), (source("bb"), source("bb")));
    assert_eq!(movie.plugin, PluginId::from_static("bb"));
    assert_eq!(asked(&sources_asked), ["bb", "bb"]);
}

#[tokio::test]
async fn a_title_by_id_falls_through_to_the_other_sources_in_order() {
    let (metadata, sources_asked) = two(Answers::Has, Answers::Unsupported);
    let id = ExternalId::new("bb", "1").unwrap();

    let (show_from, _) = metadata.show_by(&id).await.unwrap();

    assert_eq!(show_from, source("aa"));
    assert_eq!(asked(&sources_asked), ["bb", "aa"]);
}

#[tokio::test]
async fn a_title_by_id_stops_at_a_failing_source() {
    let (metadata, sources_asked) = two(Answers::Has, Answers::Fails);
    let id = ExternalId::new("bb", "1").unwrap();

    assert_eq!(metadata.show_by(&id).await.unwrap_err(), Error::Unavailable);
    assert_eq!(asked(&sources_asked), ["bb"]);
}

#[tokio::test]
async fn a_source_not_in_the_order_is_never_asked() {
    let (metadata, sources_asked) = two(Answers::Has, Answers::NotFound);
    let only_bb = metadata.in_order(order(&["bb"]));

    let search = only_bb.search_in_turn(&query()).await.unwrap_err();
    let show = only_bb
        .show_by(&ExternalId::new("aa", "1").unwrap())
        .await
        .unwrap_err();

    assert_eq!((search, show), (Error::NotFound, Error::NotFound));
    assert_eq!(asked(&sources_asked), ["bb", "bb"]);
}

#[tokio::test]
async fn a_season_is_asked_only_of_the_source_given() {
    let (metadata, sources_asked) = two(Answers::Has, Answers::Has);

    let season = metadata
        .season_from(&source("bb"), &ExternalId::new("bb", "1").unwrap(), 2)
        .await
        .unwrap();

    assert_eq!(season.number, 2);
    assert_eq!(asked(&sources_asked), ["bb"]);
}
