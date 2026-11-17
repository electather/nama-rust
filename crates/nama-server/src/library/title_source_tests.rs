use std::sync::{Arc, Mutex, PoisonError};

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    contract::{
        Artwork, ArtworkAnswer, ArtworkAsked, ArtworkKind, Candidate, Movie, MovieAnswer,
        SearchQuery, SearchResults, Season, SeasonAnswer, SeasonAsked, Show, ShowAnswer,
        TitleAsked, TitleId,
    },
    metadata::{ExternalId, ExternalIds, MetadataError, TitleMethod, TitleSource},
};

use super::{METHODS, Source, TitleSources, Unanswered};
use crate::plugins::Registry;

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
    /// As [`Answers::Has`], but without serving [`TitleMethod::Artwork`] or
    /// [`TitleMethod::SeasonArtwork`].
    NoArtwork,
    /// As [`Answers::Has`], but without serving [`TitleMethod::SeasonArtwork`], as a Plugin built
    /// before it.
    NoSeasonArtwork,
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
            Answers::Has | Answers::Empty | Answers::NoArtwork | Answers::NoSeasonArtwork => {
                Ok(found(vec![self.id()]))
            }
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

    /// Its artwork list: one poster at `path`.
    fn poster(&self, path: String) -> Result<ArtworkAnswer, MetadataError> {
        let empty = matches!(self.answers, Answers::Empty);
        let found = self.answer(|_| {
            let poster = Artwork {
                kind: ArtworkKind::ARTWORK_KIND_POSTER.into(),
                path,
                ..Default::default()
            };
            ArtworkAnswer {
                artwork: if empty { vec![] } else { vec![poster] },
                ..Default::default()
            }
        });
        found.map(|found| ArtworkAnswer {
            unsupported_id: self.unsupported_id(),
            ..found
        })
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

    /// One poster, whose path names the id it was asked by.
    fn artwork<'a>(
        &'a self,
        asked: &'a ArtworkAsked,
    ) -> BoxFuture<'a, Result<ArtworkAnswer, MetadataError>> {
        let id = asked.id.as_option().cloned().unwrap_or_default();
        let found = self.poster(format!("/{}-{}.jpg", id.namespace, id.value));
        Box::pin(async { found })
    }

    /// One poster, whose path names the Show's id it was asked by and the Season.
    fn season_artwork<'a>(
        &'a self,
        asked: &'a SeasonAsked,
    ) -> BoxFuture<'a, Result<ArtworkAnswer, MetadataError>> {
        let show = asked.show.as_option().cloned().unwrap_or_default();
        let path = format!("/{}-{}-s{}.jpg", show.namespace, show.value, asked.season);
        let found = self.poster(path);
        Box::pin(async { found })
    }

    fn answers(&self, method: TitleMethod) -> bool {
        let unserved: &[_] = match self.answers {
            Answers::NoArtwork => &[TitleMethod::Artwork, TitleMethod::SeasonArtwork],
            Answers::NoSeasonArtwork => &[TitleMethod::SeasonArtwork],
            Answers::Has
            | Answers::Empty
            | Answers::NotFound
            | Answers::UnsupportedId
            | Answers::Unsupported
            | Answers::Fails => &[],
        };
        !unserved.contains(&method)
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
        answers: METHODS.to_vec(),
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
    assert!(restricted.serves("bb", TitleMethod::Movie).await);
    assert!(!restricted.serves("aa", TitleMethod::Movie).await);
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
    assert!(metadata.serves("imdb", TitleMethod::Show).await);
    assert!(!metadata.serves("tvdb", TitleMethod::Show).await);
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

/// A title source like fanart.tv's: it keeps titles under `fanart`, looks them up by `tmdb` and
/// answers only the artwork list, recording any other method asked of it.
struct ArtworkOnly(Asked);

impl TitleSource for ArtworkOnly {
    fn namespace(&self) -> &'static str {
        "fanart"
    }

    fn lookup_namespaces(&self) -> Vec<String> {
        vec!["tmdb".to_owned()]
    }

    fn search<'a>(
        &'a self,
        _query: &'a SearchQuery,
    ) -> BoxFuture<'a, Result<SearchResults, MetadataError>> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push("search");
        Box::pin(async { Err(MetadataError::Unsupported) })
    }

    fn movie<'a>(
        &'a self,
        _asked: &'a TitleAsked,
    ) -> BoxFuture<'a, Result<MovieAnswer, MetadataError>> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push("movie");
        Box::pin(async { Err(MetadataError::Unsupported) })
    }

    fn answers(&self, method: TitleMethod) -> bool {
        method == TitleMethod::Artwork
    }
}

/// The Plugin `fanart` serving an [`ArtworkOnly`].
struct ArtworkOnlyPlugin(Asked);

impl Plugin for ArtworkOnlyPlugin {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            extension_points: vec![PointKey::provide::<dyn TitleSource>()],
            ..Manifest::new(PluginId::from_static("fanart"))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        let source = ArtworkOnly(Arc::clone(&self.0));
        registrar.provide::<dyn TitleSource>(Priority::DEFAULT, Arc::new(source));
        Ok(())
    }
}

#[tokio::test]
async fn a_source_answering_only_artwork_is_never_asked_for_a_title_or_a_search() {
    let plugins = Registry::new();
    let asked = Asked::default();
    plugins
        .enable(Arc::new(ArtworkOnlyPlugin(Arc::clone(&asked))))
        .unwrap();
    let metadata = TitleSources::new(plugins.extension_points());
    let tmdb = ExternalId::tmdb(603);

    let movie = metadata.movie_by(&tmdb).await.unwrap_err();
    let search = metadata.search_in_turn(&query()).await.unwrap_err();

    assert_eq!(movie, Unanswered::Unavailable);
    assert_eq!(search, Unanswered::Unavailable);
    assert!(!metadata.serves("tmdb", TitleMethod::Movie).await);
    assert!(metadata.serves("tmdb", TitleMethod::Artwork).await);
    assert!(metadata.unserved(&tmdb, TitleMethod::Movie).await);
    assert!(
        asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_empty()
    );
}

/// The ids `ids` name a title by.
fn ids(ids: &[(&str, &str)]) -> ExternalIds {
    (ids.iter())
        .map(|&(namespace, value)| ExternalId::new(namespace, value).unwrap())
        .collect()
}

/// Each image in `lists`, as `namespace:path`.
fn listed(lists: Vec<(Source, Vec<super::Artwork>)>) -> Vec<String> {
    (lists.into_iter())
        .flat_map(|(source, list)| {
            list.into_iter()
                .map(move |art| format!("{}:{}", source.namespace, art.path))
        })
        .collect()
}

/// Each image [`TitleSources::artwork_lists`] lists, as `namespace:path`.
async fn artwork_from(metadata: &TitleSources, title: &[(&str, &str)]) -> Vec<String> {
    listed(
        metadata
            .artwork_lists(super::TitleKind::Movie, &ids(title))
            .await,
    )
}

#[tokio::test]
async fn artwork_is_never_asked_of_a_source_that_does_not_answer_it() {
    let (metadata, sources_asked) = two(Answers::NoArtwork, Answers::Has);

    let lists = artwork_from(&metadata, &[("aa", "1"), ("bb", "1")]).await;

    assert_eq!(lists, ["bb:/bb-1.jpg"]);
    assert_eq!(asked(&sources_asked), ["bb"]);
}

#[tokio::test]
async fn artwork_is_asked_by_a_lookup_namespace_id_when_the_title_lacks_the_sources_own() {
    let (metadata, sources_asked) = two(Answers::Has, Answers::Has);

    let by_bb = artwork_from(&metadata, &[("bb", "7")]).await;
    let by_neither = artwork_from(&metadata, &[("cc", "7")]).await;

    assert_eq!(by_bb, ["aa:/bb-7.jpg", "bb:/bb-7.jpg"]);
    assert_eq!(by_neither, Vec::<String>::new());
    assert_eq!(asked(&sources_asked), ["aa", "bb"]);
}

#[tokio::test]
async fn a_source_with_no_artwork_list_gives_no_entry_while_the_others_answer() {
    for nothing in [
        Answers::UnsupportedId,
        Answers::Fails,
        Answers::NotFound,
        Answers::Empty,
        Answers::Unsupported,
    ] {
        let (metadata, sources_asked) = two(nothing, Answers::Has);

        let lists = artwork_from(&metadata, &[("aa", "1"), ("bb", "1")]).await;

        assert_eq!(lists, ["bb:/bb-1.jpg"]);
        assert_eq!(asked(&sources_asked), ["aa", "bb"]);
    }
}

/// Each image [`TitleSources::season_artwork_lists`] lists for Season 2, as `namespace:path`.
async fn season_artwork_from(metadata: &TitleSources, show: &[(&str, &str)]) -> Vec<String> {
    listed(metadata.season_artwork_lists(&ids(show), 2).await)
}

#[tokio::test]
async fn season_artwork_is_never_asked_of_a_source_serving_only_a_titles_artwork() {
    let (metadata, sources_asked) = two(Answers::NoSeasonArtwork, Answers::Has);

    let lists = season_artwork_from(&metadata, &[("aa", "1"), ("bb", "1")]).await;

    assert_eq!(lists, ["bb:/bb-1-s2.jpg"]);
    assert_eq!(asked(&sources_asked), ["bb"]);
}

#[tokio::test]
async fn season_artwork_is_asked_by_a_lookup_namespace_id_when_the_show_lacks_the_sources_own() {
    let (metadata, sources_asked) = two(Answers::Has, Answers::Has);

    let lists = season_artwork_from(&metadata, &[("bb", "7")]).await;

    assert_eq!(lists, ["aa:/bb-7-s2.jpg", "bb:/bb-7-s2.jpg"]);
    assert_eq!(asked(&sources_asked), ["aa", "bb"]);
}

#[tokio::test]
async fn a_failing_source_is_left_out_of_the_season_artwork_lists() {
    let (metadata, sources_asked) = two(Answers::Fails, Answers::Has);

    let lists = season_artwork_from(&metadata, &[("aa", "1"), ("bb", "1")]).await;

    assert_eq!(lists, ["bb:/bb-1-s2.jpg"]);
    assert_eq!(asked(&sources_asked), ["aa", "bb"]);
}
