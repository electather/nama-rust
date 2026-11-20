use std::sync::Arc;

use nama_plugin_api::{
    PluginId, PointKey,
    contract::ArtworkKind,
    metadata::{ExternalId, ExternalIds, TitleMethod, TitleSource},
};

use super::{
    METHODS, Source, TitleSources, Unanswered,
    scripted::{Refusal, ScriptedSource, SourcesAsked},
};
use crate::plugins::Registry;

/// How a source [`two`] enables answers.
#[derive(Clone, Copy)]
enum Answers {
    /// With a title, Season, search hit or artwork.
    Has,
    /// With a title or Season, but a search that finds nothing and no artwork.
    Empty,
    NotFound,
    /// With `unsupported_id` set.
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

/// A source under `namespace` looking titles up by `looks_up`, answering as `answers` says. What it
/// has is a title known as `aa-1` and `bb-1`, as a Movie and as a Show, the Show's Season 2, a
/// search hit under its own id, and the poster `/<namespace>.jpg`.
fn scripted(
    namespace: &'static str,
    looks_up: &[&str],
    answers: Answers,
    asked: &SourcesAsked,
) -> ScriptedSource {
    let source = ScriptedSource::new(namespace, asked).looking_up(looks_up);
    let ids = ["aa-1", "bb-1"];
    let titles = (source.clone().with_movie(&ids, "Title"))
        .with_show(&ids, "Title", &[2])
        .with_season("bb-1", 2, &[]);
    let (own, poster) = (format!("{namespace}-1"), format!("/{namespace}.jpg"));
    let has = (titles.clone().found(&[&own], "Title")).with_artwork(
        ArtworkKind::ARTWORK_KIND_POSTER,
        &poster,
        "en",
    );
    match answers {
        Answers::Has => has,
        Answers::Empty => titles,
        Answers::NotFound => source,
        Answers::UnsupportedId => source.refusing(&METHODS, Refusal::UnsupportedId),
        Answers::Unsupported => source.refusing(&METHODS, Refusal::Unsupported),
        Answers::Fails => source.refusing(&METHODS, Refusal::Fails),
        Answers::NoArtwork => has.serving(&all_but(&[
            TitleMethod::Artwork,
            TitleMethod::SeasonArtwork,
        ])),
        Answers::NoSeasonArtwork => has.serving(&all_but(&[TitleMethod::SeasonArtwork])),
    }
}

fn all_but(unserved: &[TitleMethod]) -> Vec<TitleMethod> {
    METHODS
        .into_iter()
        .filter(|method| !unserved.contains(method))
        .collect()
}

/// Title sources under `aa` and `bb`, `aa` first in provider order, as its id sorts first, and also looking titles up
/// by `bb` ids, answering as given.
fn two(aa: Answers, bb: Answers) -> (TitleSources, SourcesAsked) {
    let plugins = Registry::new();
    let asked = SourcesAsked::default();
    for source in [
        scripted("aa", &["bb"], aa, &asked),
        scripted("bb", &[], bb, &asked),
    ] {
        plugins.enable(Arc::new(source)).unwrap();
    }
    (TitleSources::new(plugins.extension_points()), asked)
}

/// The namespaces of the sources asked for anything but their namespace, in the order asked.
fn asked(asked: &SourcesAsked) -> Vec<String> {
    (asked.lines().iter())
        .map(|line| line.split(' ').next().unwrap_or_default().to_owned())
        .collect()
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
    let source = ScriptedSource::new("aa", &SourcesAsked::default());
    let looking_up = source.looking_up(&["imdb", "TV-DB"]);
    plugins.enable(Arc::new(looking_up)).unwrap();
    let metadata = TitleSources::new(plugins.extension_points());

    assert_eq!(metadata.sources().await[0].looks_up, ["imdb"]);
    assert!(metadata.serves("imdb", TitleMethod::Show).await);
    assert!(!metadata.serves("tvdb", TitleMethod::Show).await);
}

#[tokio::test]
async fn a_source_whose_namespace_is_not_well_formed_is_left_out() {
    let plugins = Registry::new();
    let malformed = ScriptedSource::new("bad-ns", &SourcesAsked::default());
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
async fn a_source_finding_no_such_title_clears_its_failure_while_an_unavailable_one_records_one() {
    let (metadata, _) = two(Answers::NotFound, Answers::Fails);
    let (aa, bb) = (PluginId::from_static("aa"), PluginId::from_static("bb"));
    let point = PointKey::provide::<dyn TitleSource>();
    metadata.plugins.provider_failed(&aa, point, "timed out");

    let not_found = metadata.show_by(&ExternalId::new("aa", "1").unwrap()).await;
    let unavailable = metadata.show_by(&ExternalId::new("bb", "1").unwrap()).await;

    assert_eq!(not_found.unwrap_err(), Unanswered::NotFound);
    assert_eq!(unavailable.unwrap_err(), Unanswered::Unavailable);
    assert_eq!(metadata.plugins.failing(&aa, point), None);
    let failing = metadata.plugins.failing(&bb, point).unwrap();
    assert_eq!(failing.in_a_row, 1);
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

#[tokio::test]
async fn a_source_answering_only_artwork_is_never_asked_for_a_title_or_a_search() {
    let plugins = Registry::new();
    let asked = SourcesAsked::default();
    // Like fanart.tv's.
    let artwork_only = ScriptedSource::new("fanart", &asked).looking_up(&["tmdb"]);
    let artwork_only = artwork_only.serving(&[TitleMethod::Artwork]);
    plugins.enable(Arc::new(artwork_only)).unwrap();
    let metadata = TitleSources::new(plugins.extension_points());
    let tmdb = ExternalId::tmdb(603);

    let movie = metadata.movie_by(&tmdb).await.unwrap_err();
    let search = metadata.search_in_turn(&query()).await.unwrap_err();

    assert_eq!(movie, Unanswered::Unavailable);
    assert_eq!(search, Unanswered::Unavailable);
    assert!(!metadata.serves("tmdb", TitleMethod::Movie).await);
    assert!(metadata.serves("tmdb", TitleMethod::Artwork).await);
    assert!(metadata.unserved(&tmdb, TitleMethod::Movie).await);
    assert_eq!(asked.lines(), Vec::<String>::new());
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

    assert_eq!(lists, ["bb:/bb.jpg"]);
    assert_eq!(asked(&sources_asked), ["bb"]);
}

#[tokio::test]
async fn artwork_is_asked_by_a_lookup_namespace_id_when_the_title_lacks_the_sources_own() {
    let (metadata, sources_asked) = two(Answers::Has, Answers::Has);

    let by_bb = artwork_from(&metadata, &[("bb", "7")]).await;
    let by_neither = artwork_from(&metadata, &[("cc", "7")]).await;

    assert_eq!(by_bb, ["aa:/aa.jpg", "bb:/bb.jpg"]);
    assert_eq!(by_neither, Vec::<String>::new());
    assert_eq!(
        sources_asked.lines(),
        ["aa artwork bb-7", "bb artwork bb-7"]
    );
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

        assert_eq!(lists, ["bb:/bb.jpg"]);
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

    assert_eq!(lists, ["bb:/bb.jpg"]);
    assert_eq!(asked(&sources_asked), ["bb"]);
}

#[tokio::test]
async fn season_artwork_is_asked_by_a_lookup_namespace_id_when_the_show_lacks_the_sources_own() {
    let (metadata, sources_asked) = two(Answers::Has, Answers::Has);

    let lists = season_artwork_from(&metadata, &[("bb", "7")]).await;

    assert_eq!(lists, ["aa:/aa.jpg", "bb:/bb.jpg"]);
    let lines = sources_asked.lines();
    assert_eq!(
        lines,
        ["aa season_artwork bb-7 2", "bb season_artwork bb-7 2"]
    );
}

#[tokio::test]
async fn a_failing_source_is_left_out_of_the_season_artwork_lists() {
    let (metadata, sources_asked) = two(Answers::Fails, Answers::Has);

    let lists = season_artwork_from(&metadata, &[("aa", "1"), ("bb", "1")]).await;

    assert_eq!(lists, ["bb:/bb.jpg"]);
    assert_eq!(asked(&sources_asked), ["aa", "bb"]);
}
