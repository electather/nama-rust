use std::sync::{Arc, Mutex, PoisonError};

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    contract::{
        Candidate, EpisodeAnswer, EpisodeAsked, ImageAnswer, ImageAsked, Movie, MovieAnswer,
        SearchQuery, SearchResults, Season, SeasonAnswer, SeasonAsked, Show, ShowAnswer,
        TitleAsked, TitleId,
    },
    metadata::{ExternalId, MetadataError, TitleSource},
    settings::{SettingField, SettingKind, SettingValue, Settings, SettingsSchema},
};

use super::{Source, TitleSources, Unanswered};
use crate::plugins::{Failure, Registry};

/// A title source that answers every question with the error `.0` makes, or with the title unset
/// where it makes none.
struct Failing(fn() -> Option<MetadataError>);

impl Failing {
    fn refuse<T: Default>(&self) -> Result<T, MetadataError> {
        self.0().map_or_else(|| Ok(T::default()), Err)
    }
}

impl TitleSource for Failing {
    fn namespace(&self) -> &'static str {
        "tmdb"
    }

    fn search<'a>(
        &'a self,
        _query: &'a SearchQuery,
    ) -> BoxFuture<'a, Result<SearchResults, MetadataError>> {
        Box::pin(async { self.refuse() })
    }

    fn movie<'a>(
        &'a self,
        _asked: &'a TitleAsked,
    ) -> BoxFuture<'a, Result<MovieAnswer, MetadataError>> {
        Box::pin(async { self.refuse() })
    }

    fn show<'a>(
        &'a self,
        _asked: &'a TitleAsked,
    ) -> BoxFuture<'a, Result<ShowAnswer, MetadataError>> {
        Box::pin(async { self.refuse() })
    }

    fn season<'a>(
        &'a self,
        _asked: &'a SeasonAsked,
    ) -> BoxFuture<'a, Result<SeasonAnswer, MetadataError>> {
        Box::pin(async { self.refuse() })
    }

    fn episode<'a>(
        &'a self,
        _asked: &'a EpisodeAsked,
    ) -> BoxFuture<'a, Result<EpisodeAnswer, MetadataError>> {
        Box::pin(async { self.refuse() })
    }

    fn image<'a>(
        &'a self,
        _asked: &'a ImageAsked,
    ) -> BoxFuture<'a, Result<ImageAnswer, MetadataError>> {
        Box::pin(async { self.refuse() })
    }
}

struct FailingPlugin(fn() -> Option<MetadataError>);

impl Plugin for FailingPlugin {
    fn manifest(&self) -> Manifest {
        Manifest {
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
            ..Manifest::new(PluginId::from_static("failing"))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.provide::<dyn TitleSource>(Priority::DEFAULT, Arc::new(Failing(self.0)));
        Ok(())
    }
}

/// What asking for a Show, then for an image from the Plugin `failing`, fails with when its
/// Provider fails with `failure`, or when there is no Provider.
async fn asking(failure: Option<fn() -> Option<MetadataError>>) -> (Unanswered, Unanswered) {
    let plugins = Registry::new();
    if let Some(failure) = failure {
        plugins.enable(Arc::new(FailingPlugin(failure))).unwrap();
    }
    let source = TitleSources::new(plugins.extension_points());
    let show = source.show_by(&ExternalId::tmdb(1399)).await.unwrap_err();
    let image = source
        .image(&PluginId::from_static("failing"), "/poster.jpg")
        .await
        .unwrap_err();
    (show, image)
}

#[tokio::test]
async fn without_a_provider_the_source_is_unavailable() {
    assert_eq!(
        asking(None).await,
        (Unanswered::Unavailable, Unanswered::Unavailable)
    );
}

#[tokio::test]
async fn a_title_the_provider_does_not_have_is_not_found() {
    let failure = || None;

    assert_eq!(
        asking(Some(failure)).await,
        (Unanswered::NotFound, Unanswered::NotFound)
    );
}

#[tokio::test]
async fn a_method_the_source_does_not_serve_is_not_found() {
    let failure = || Some(MetadataError::Unsupported);

    assert_eq!(
        asking(Some(failure)).await,
        (Unanswered::NotFound, Unanswered::NotFound)
    );
}

#[tokio::test]
async fn a_provider_that_fails_is_unavailable() {
    let failure = || Some(MetadataError::Failed("TMDB is unreachable".into()));

    assert_eq!(
        asking(Some(failure)).await,
        (Unanswered::Unavailable, Unanswered::Unavailable)
    );
}

#[tokio::test]
async fn a_providers_failure_comes_without_its_plugins_secret_settings() {
    let plugins = Registry::new();
    let failure = || {
        Some(MetadataError::Failed(
            "GET /tv/1399?api_key=s3cr3t failed".into(),
        ))
    };
    let api_key = SettingValue::Text("s3cr3t".to_owned());
    let settings = Settings::new([("api_key".to_owned(), api_key)].into());
    plugins
        .enable_with_settings(Arc::new(FailingPlugin(failure)), settings)
        .unwrap();

    let answer = plugins
        .ask_first(|metadata: Arc<dyn TitleSource>| async move {
            metadata.show(&TitleAsked::default()).await
        })
        .await
        .unwrap();

    let blanked = Failure {
        kind: Unanswered::Unavailable,
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
    /// With `unsupported_id` set on a title or Season.
    UnsupportedId,
    Unsupported,
    Fails,
}

/// The namespaces of the sources asked for a title or Season, in the order they were asked.
type Asked = Arc<Mutex<Vec<&'static str>>>;

/// A title source keeping titles under `namespace`, as the Plugin of that id, and looking them up
/// by `looks_up`.
struct Fake {
    namespace: &'static str,
    looks_up: &'static [&'static str],
    answers: Answers,
    asked: Asked,
}

impl Fake {
    /// Its answer, `found` with its ids for what it has.
    fn answer<T: Default>(
        &self,
        found: impl FnOnce(Vec<TitleId>) -> T,
    ) -> Result<T, MetadataError> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(self.namespace);
        match self.answers {
            Answers::Has | Answers::Empty => Ok(found(vec![self.id()])),
            Answers::NotFound | Answers::UnsupportedId => Ok(T::default()),
            Answers::Unsupported => Err(MetadataError::Unsupported),
            Answers::Fails => Err(MetadataError::Failed("unreachable".into())),
        }
    }
}

impl Fake {
    fn id(&self) -> TitleId {
        TitleId {
            namespace: self.namespace.to_owned(),
            value: "1".to_owned(),
            ..Default::default()
        }
    }

    fn unsupported_id(&self) -> bool {
        matches!(self.answers, Answers::UnsupportedId)
    }
}

impl TitleSource for Fake {
    fn namespace(&self) -> &str {
        self.namespace
    }

    fn lookup_namespaces(&self) -> Vec<String> {
        self.looks_up.iter().map(|&n| n.to_owned()).collect()
    }

    fn search<'a>(
        &'a self,
        _query: &'a SearchQuery,
    ) -> BoxFuture<'a, Result<SearchResults, MetadataError>> {
        let empty = matches!(self.answers, Answers::Empty);
        let found = self.answer(|ids| {
            let candidates = if empty {
                Vec::new()
            } else {
                vec![Candidate {
                    id: self.id().into(),
                    ids,
                    title: "Title".to_owned(),
                    ..Default::default()
                }]
            };
            SearchResults {
                candidates,
                ..Default::default()
            }
        });
        Box::pin(async { found })
    }

    fn movie<'a>(
        &'a self,
        _asked: &'a TitleAsked,
    ) -> BoxFuture<'a, Result<MovieAnswer, MetadataError>> {
        let found = self.answer(|ids| MovieAnswer {
            movie: Movie {
                ids,
                title: "Title".to_owned(),
                ..Default::default()
            }
            .into(),
            ..Default::default()
        });
        let found = found.map(|found| MovieAnswer {
            unsupported_id: self.unsupported_id(),
            ..found
        });
        Box::pin(async { found })
    }

    fn show<'a>(
        &'a self,
        _asked: &'a TitleAsked,
    ) -> BoxFuture<'a, Result<ShowAnswer, MetadataError>> {
        let found = self.answer(|ids| ShowAnswer {
            show: Show {
                ids,
                title: "Title".to_owned(),
                seasons: vec![1],
                ..Default::default()
            }
            .into(),
            ..Default::default()
        });
        let found = found.map(|found| ShowAnswer {
            unsupported_id: self.unsupported_id(),
            ..found
        });
        Box::pin(async { found })
    }

    fn season<'a>(
        &'a self,
        asked: &'a SeasonAsked,
    ) -> BoxFuture<'a, Result<SeasonAnswer, MetadataError>> {
        let found = self.answer(|ids| SeasonAnswer {
            season: Season {
                ids,
                number: asked.season,
                ..Default::default()
            }
            .into(),
            ..Default::default()
        });
        let found = found.map(|found| SeasonAnswer {
            unsupported_id: self.unsupported_id(),
            ..found
        });
        Box::pin(async { found })
    }
}

/// The Plugin serving a [`Fake`], registered at `.1`, looking titles up by `.4`.
struct FakePlugin(
    &'static str,
    Priority,
    Answers,
    Asked,
    &'static [&'static str],
);

impl Plugin for FakePlugin {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            extension_points: vec![PointKey::provide::<dyn TitleSource>()],
            ..Manifest::new(PluginId::from_static(self.0))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        let source = Fake {
            namespace: self.0,
            looks_up: self.4,
            answers: self.2,
            asked: Arc::clone(&self.3),
        };
        registrar.provide::<dyn TitleSource>(self.1, Arc::new(source));
        Ok(())
    }
}

/// Title sources under `aa` and `bb`, `aa` first in provider order and also looking titles up
/// by `bb` ids, answering as given.
fn two(aa: Answers, bb: Answers) -> (TitleSources, Asked) {
    let plugins = Registry::new();
    let asked = Asked::default();
    let fake = |namespace, priority, answers, looks_up| {
        let plugin = FakePlugin(namespace, priority, answers, Arc::clone(&asked), looks_up);
        Arc::new(plugin)
    };
    plugins
        .enable(fake("aa", Priority::EARLY, aa, &["bb"]))
        .unwrap();
    plugins
        .enable(fake("bb", Priority::DEFAULT, bb, &[]))
        .unwrap();
    (TitleSources::new(plugins.extension_points()), asked)
}

fn asked(asked: &Asked) -> Vec<&'static str> {
    asked.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

/// The source [`two`] enables under `namespace`.
fn source(namespace: &'static str) -> Source {
    let looks_up = if namespace == "aa" {
        vec!["bb".to_owned()]
    } else {
        vec![]
    };
    Source {
        plugin: PluginId::from_static(namespace),
        namespace: namespace.to_owned(),
        looks_up,
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
async fn a_source_serves_the_namespaces_it_looks_up_by_but_not_malformed_ones() {
    let plugins = Registry::new();
    let looks_up = &["imdb", "TV-DB"];
    let fake = FakePlugin(
        "aa",
        Priority::DEFAULT,
        Answers::Has,
        Asked::default(),
        looks_up,
    );
    plugins.enable(Arc::new(fake)).unwrap();
    let metadata = TitleSources::new(plugins.extension_points());

    assert_eq!(metadata.sources().await[0].looks_up, ["imdb"]);
    assert!(metadata.serves("imdb").await);
    assert!(!metadata.serves("tvdb").await);
}

#[tokio::test]
async fn a_source_whose_namespace_is_not_well_formed_is_left_out() {
    let plugins = Registry::new();
    let asked = Asked::default();
    let malformed = FakePlugin("bad-ns", Priority::DEFAULT, Answers::Has, asked, &[]);
    plugins.enable(Arc::new(malformed)).unwrap();

    let metadata = TitleSources::new(plugins.extension_points());
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
        Unanswered::Unavailable
    );
    assert_eq!(asked(&sources_asked), ["aa"]);
}

#[tokio::test]
async fn a_search_no_source_answers_is_not_found_and_one_with_no_source_unavailable() {
    let (metadata, _) = two(Answers::NotFound, Answers::Empty);

    assert_eq!(
        metadata.search_in_turn(&query()).await.unwrap_err(),
        Unanswered::NotFound
    );
    let none = metadata.in_order(order(&["cc"]));
    assert_eq!(
        none.search_in_turn(&query()).await.unwrap_err(),
        Unanswered::Unavailable
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
async fn a_source_answering_unsupported_id_passes_to_the_next() {
    let (metadata, sources_asked) = two(Answers::Has, Answers::UnsupportedId);
    let id = ExternalId::new("bb", "1").unwrap();

    let (show_from, _) = metadata.show_by(&id).await.unwrap();
    let (movie_from, _) = metadata.movie_by(&id).await.unwrap();
    let season = metadata.season_from(&source("bb"), &id, 1).await;

    assert_eq!((show_from, movie_from), (source("aa"), source("aa")));
    assert_eq!(season.unwrap_err(), Unanswered::NotFound);
    assert_eq!(asked(&sources_asked), ["bb", "aa", "bb", "aa", "bb"]);
}

#[tokio::test]
async fn an_id_is_asked_of_a_source_looking_it_up_after_its_keeper() {
    let (metadata, sources_asked) = two(Answers::Has, Answers::NotFound);
    let id = ExternalId::new("bb", "1").unwrap();

    let (show_from, _) = metadata
        .in_order(order(&["aa", "bb"]))
        .show_by(&id)
        .await
        .unwrap();

    assert_eq!(show_from, source("aa"));
    assert_eq!(asked(&sources_asked), ["bb", "aa"]);
}

#[tokio::test]
async fn a_source_neither_keeping_nor_looking_up_an_id_is_never_asked() {
    let (metadata, sources_asked) = two(Answers::NotFound, Answers::Has);
    let aa = ExternalId::new("aa", "1").unwrap();
    let cc = ExternalId::new("cc", "1").unwrap();

    assert_eq!(
        metadata.show_by(&aa).await.unwrap_err(),
        Unanswered::NotFound
    );
    assert_eq!(
        metadata.movie_by(&cc).await.unwrap_err(),
        Unanswered::Unavailable
    );
    assert_eq!(asked(&sources_asked), ["aa"]);
}

#[tokio::test]
async fn a_title_by_id_stops_at_a_failing_source() {
    let (metadata, sources_asked) = two(Answers::Has, Answers::Fails);
    let id = ExternalId::new("bb", "1").unwrap();

    assert_eq!(
        metadata.show_by(&id).await.unwrap_err(),
        Unanswered::Unavailable
    );
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

    assert_eq!(
        (search, show),
        (Unanswered::NotFound, Unanswered::Unavailable)
    );
    assert_eq!(asked(&sources_asked), ["bb"]);
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
