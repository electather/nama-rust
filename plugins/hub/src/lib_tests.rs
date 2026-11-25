use std::{future, sync::Arc};

use buffa::Message as _;
use nama_plugin_api::{
    BoxFuture, Registrar, Registration,
    automation::{
        Kernel, KernelFunction, KernelRefused,
        functions::{
            ListHubMostWatchlisted, ListHubRecommended, ListHubSimilar, ListHubTrending,
            ListLibraries, ListTitles,
        },
    },
    contract::{
        FeaturedSuggestions, HomeRow, HomeRowTitle, HomeRows, HubRecommendedAsked, HubTitle,
        HubTitles, Libraries, Library, LibraryKind, SharedRowsAsked, SimilarAsked, SimilarTitle,
        SimilarTitles, Title, TitleId, TitleKind, TitleRef, TitleRefKind, Titles, TitlesAsked,
        UserFeaturedAsked, UserRowsAsked,
    },
    home_row::{HomeRowMethod, HomeRowSource},
    similar_source::SimilarSource,
};

use super::*;

/// What the scripted Kernel answers: the Hub's suggestions, and the Server's Libraries.
#[derive(Clone, Default)]
struct Script {
    similar: Vec<SimilarTitle>,
    /// Recommended to User 7 alone.
    recommended: Vec<HubTitle>,
    trending: Vec<HubTitle>,
    most_watchlisted: Vec<HubTitle>,
    /// The Movies a Movies Library holds, by TMDB id.
    movies: Vec<&'static str>,
    /// The Show an Episode in a Shows Library belongs to, by TMDB id.
    show: Option<&'static str>,
    /// Whether every Hub function is refused, as while the Hub is unreachable.
    hub_unreachable: bool,
}

/// The Movies Library that a [`Script`]'s Kernel has.
const MOVIES_LIBRARY: i64 = 1;

/// The Shows Library that a [`Script`]'s Kernel has.
const SHOWS_LIBRARY: i64 = 2;

impl Script {
    /// A Kernel answering as scripted, refusing any other function.
    fn kernel(self) -> Arc<dyn Kernel> {
        Arc::new(
            move |function: &str,
                  request: Vec<u8>|
                  -> BoxFuture<'static, Result<Vec<u8>, KernelRefused>> {
                Box::pin(future::ready(self.answer(function, &request)))
            },
        )
    }

    fn answer(&self, function: &str, request: &[u8]) -> Result<Vec<u8>, KernelRefused> {
        let hub_titles = |titles: &[HubTitle]| HubTitles {
            titles: titles.to_vec(),
            ..Default::default()
        };
        let is_hub = function.starts_with("list_hub_");
        if is_hub && self.hub_unreachable {
            return Err(KernelRefused("the Hub is unreachable".to_owned()));
        }
        Ok(if function == ListHubSimilar::NAME {
            SimilarTitles {
                titles: self.similar.clone(),
                ..Default::default()
            }
            .encode_to_vec()
        } else if function == ListHubRecommended::NAME {
            let asked = HubRecommendedAsked::decode_from_slice(request).unwrap();
            assert_eq!(asked.namespaces, ["tmdb", "tvdb", "imdb"]);
            let recommended = if asked.user_id == 7 {
                &self.recommended[..]
            } else {
                &[]
            };
            hub_titles(recommended).encode_to_vec()
        } else if function == ListHubTrending::NAME {
            hub_titles(&self.trending).encode_to_vec()
        } else if function == ListHubMostWatchlisted::NAME {
            hub_titles(&self.most_watchlisted).encode_to_vec()
        } else if function == ListLibraries::NAME {
            let library = |id, kind: LibraryKind| Library {
                id,
                kind: kind.into(),
                ..Default::default()
            };
            Libraries {
                libraries: vec![
                    library(MOVIES_LIBRARY, LibraryKind::LIBRARY_KIND_MOVIES),
                    library(SHOWS_LIBRARY, LibraryKind::LIBRARY_KIND_SHOWS),
                ],
                ..Default::default()
            }
            .encode_to_vec()
        } else if function == ListTitles::NAME {
            let asked = TitlesAsked::decode_from_slice(request).unwrap();
            let titles = if asked.library_id == MOVIES_LIBRARY {
                self.movies.iter().map(|&value| movie(value)).collect()
            } else {
                (self.show.iter()).map(|&show| episode_of(show)).collect()
            };
            Titles {
                titles,
                ..Default::default()
            }
            .encode_to_vec()
        } else {
            return Err(KernelRefused(format!("no function {function}")));
        })
    }
}

/// The Movie `tmdb-<value>`, as `list_titles` answers it.
fn movie(value: &str) -> Title {
    Title {
        title_ref: TitleRef {
            library_id: MOVIES_LIBRARY,
            kind: TitleRefKind::TITLE_REF_KIND_MOVIE.into(),
            id: tmdb(value).into(),
            ..Default::default()
        }
        .into(),
        ids: vec![tmdb(value)],
        ..Default::default()
    }
}

/// An Episode of the Show `tmdb-<show>`, as `list_titles` answers it.
fn episode_of(show: &str) -> Title {
    Title {
        ids: vec![tmdb("62085")],
        show_ids: vec![tmdb(show)],
        ..Default::default()
    }
}

fn tmdb(value: &str) -> TitleId {
    TitleId {
        namespace: "tmdb".to_owned(),
        value: value.to_owned(),
        ..Default::default()
    }
}

/// The Hub's title of `kind` `tmdb-<value>`, weighing `weight`.
fn hub_title(kind: TitleKind, value: &str, weight: f32) -> HubTitle {
    HubTitle {
        kind: kind.into(),
        id: tmdb(value).into(),
        weight,
        ..Default::default()
    }
}

const MOVIE: TitleKind = TitleKind::TITLE_KIND_MOVIE;
const SHOW: TitleKind = TitleKind::TITLE_KIND_SHOW;

/// What the Plugin enabled against `script`'s Kernel provides as `T`.
fn provided<T: ?Sized + Send + Sync + 'static>(script: Script) -> Arc<T> {
    let mut registrar = Registrar::new().with_kernel(script.kernel());
    HubSuggestions.register(&mut registrar).unwrap();
    (registrar.into_registrations().iter())
        .find_map(Registration::provider::<T>)
        .unwrap()
}

/// What a Similar source is asked about the Movie `tmdb-603`.
fn the_matrix() -> SimilarAsked {
    SimilarAsked {
        kind: MOVIE.into(),
        ids: vec![tmdb("603")],
        ..Default::default()
    }
}

/// The User 7's rows and Featured suggestions from the Plugin enabled against `script`.
async fn recommended_for_7(script: Script) -> Result<(HomeRows, FeaturedSuggestions), PluginError> {
    let source = provided::<dyn HomeRowSource>(script);
    let rows = (source.user_rows(&UserRowsAsked {
        user_id: 7,
        ..Default::default()
    }))
    .await?;
    let featured = (source.user_featured(&UserFeaturedAsked {
        user_id: 7,
        ..Default::default()
    }))
    .await?;
    Ok((rows, featured))
}

/// Each row as its key, heading, time to live and titles, each title as its kind and External ID,
/// such as `recommended: Recommended for you, 3600 s: movie tmdb-604`.
fn rows_of(rows: &HomeRows) -> Vec<String> {
    (rows.rows.iter())
        .map(|row| {
            let titles: Vec<String> = (row.titles.iter())
                .map(|title: &HomeRowTitle| {
                    let kind = match title.kind.as_known() {
                        Some(SHOW) => "show",
                        _ => "movie",
                    };
                    format!("{kind} {}-{}", title.id.namespace, title.id.value)
                })
                .collect();
            let (key, heading, ttl) = (&row.key, &row.heading, row.ttl_seconds);
            format!("{key}: {heading}, {ttl} s: {}", titles.join(", "))
        })
        .collect()
}

/// Each suggestion's kind, External ID and weight.
fn suggested(featured: &FeaturedSuggestions) -> Vec<(TitleKind, String, f32)> {
    (featured.suggestions.iter())
        .map(|suggestion| {
            let id = &suggestion.id;
            (
                suggestion.kind.as_known().unwrap(),
                format!("{}-{}", id.namespace, id.value),
                suggestion.weight,
            )
        })
        .collect()
}

/// Nama's charts from the Plugin enabled against `script`.
async fn charts(script: Script) -> Result<HomeRows, PluginError> {
    provided::<dyn HomeRowSource>(script)
        .shared_rows(&SharedRowsAsked::default())
        .await
}

/// The row keyed `key` of `rows`.
fn row<'a>(rows: &'a HomeRows, key: &str) -> &'a HomeRow {
    let Some(row) = rows.rows.iter().find(|row| row.key == key) else {
        panic!("a row keyed {key}: {rows:?}");
    };
    row
}

#[test]
fn the_id_stays_the_hub_plugins_so_an_upgrade_keeps_it_enabled() {
    assert_eq!(HubSuggestions::ID.as_str(), "hub");
    assert_eq!(HubSuggestions.manifest().name, "Hub Suggestions");
}

#[test]
fn it_needs_no_settings() {
    assert_eq!(HubSuggestions.manifest().settings.fields.len(), 0);
}

#[tokio::test]
async fn the_similar_titles_are_the_ones_the_hub_finds_alike() {
    let alike = SimilarTitle {
        id: tmdb("604").into(),
        weight: 0.9,
        ..Default::default()
    };
    let script = Script {
        similar: vec![alike.clone()],
        ..Default::default()
    };

    let similar = provided::<dyn SimilarSource>(script)
        .similar(&the_matrix())
        .await
        .unwrap();

    assert_eq!(similar.titles, [alike]);
}

#[tokio::test]
async fn similar_titles_fail_while_the_hub_is_unreachable_so_the_last_answer_stays() {
    let script = Script {
        hub_unreachable: true,
        ..Default::default()
    };

    let similar = provided::<dyn SimilarSource>(script)
        .similar(&the_matrix())
        .await;

    similar.unwrap_err();
}

#[tokio::test]
async fn a_user_gets_the_hubs_recommendations_as_a_row_and_those_on_the_server_featured() {
    let script = Script {
        recommended: vec![hub_title(MOVIE, "604", 0.8), hub_title(SHOW, "1396", 0.4)],
        show: Some("1396"),
        ..Default::default()
    };

    let (rows, featured) = recommended_for_7(script).await.unwrap();

    assert_eq!(
        rows_of(&rows),
        ["recommended: Recommended for you, 3600 s: movie tmdb-604, show tmdb-1396"]
    );
    // Not the Movie, which no Library has, so Featured couldn't Play it (ADR 0072).
    assert_eq!(suggested(&featured), [(SHOW, "tmdb-1396".to_owned(), 0.4)]);
    assert_eq!(featured.ttl_seconds, 3600);
    let source = provided::<dyn HomeRowSource>(Script::default());
    assert!(source.gives(HomeRowMethod::User));
    assert!(source.gives(HomeRowMethod::Featured));
}

#[tokio::test]
async fn a_user_the_hub_recommends_nothing_gets_no_row_or_suggestions() {
    let (rows, featured) = recommended_for_7(Script::default()).await.unwrap();

    assert_eq!(rows_of(&rows), Vec::<String>::new());
    assert_eq!(suggested(&featured), []);
}

#[tokio::test]
async fn recommendations_fail_while_the_hub_is_unreachable_so_the_last_answer_stays() {
    let script = Script {
        hub_unreachable: true,
        ..Default::default()
    };

    recommended_for_7(script).await.unwrap_err();
}

#[tokio::test]
async fn the_trending_row_is_the_movies_the_libraries_have_in_the_hubs_order() {
    let script = Script {
        trending: ["603", "1", "862"]
            .map(|movie| hub_title(MOVIE, movie, 0.5))
            .to_vec(),
        movies: vec!["862", "603"],
        ..Default::default()
    };

    let rows = charts(script).await.unwrap();

    assert_eq!(
        rows_of(&rows)[0],
        "trending: Trending on Nama, 86400 s: movie tmdb-603, movie tmdb-862"
    );
}

#[tokio::test]
async fn trending_movies_no_library_has_leave_the_row_empty() {
    let script = Script {
        trending: vec![hub_title(MOVIE, "1", 0.5)],
        ..Default::default()
    };

    let rows = charts(script).await.unwrap();

    assert_eq!(row(&rows, "trending").titles, []);
}

#[tokio::test]
async fn the_most_watchlisted_row_is_in_the_hubs_order_had_or_not() {
    let script = Script {
        most_watchlisted: vec![
            hub_title(SHOW, "70523", 1.0),
            hub_title(MOVIE, "603", 0.5),
            hub_title(MOVIE, "78", 0.2),
        ],
        movies: vec!["603"],
        ..Default::default()
    };

    let rows = charts(script).await.unwrap();

    assert_eq!(
        rows_of(&rows)[1],
        "most-watchlisted: Most watchlisted on Nama, 86400 s: show tmdb-70523, movie tmdb-603, \
         movie tmdb-78"
    );
}

#[tokio::test]
async fn the_charts_fail_while_the_hub_is_unreachable() {
    let script = Script {
        hub_unreachable: true,
        ..Default::default()
    };

    charts(script).await.unwrap_err();
}

#[tokio::test]
async fn the_charts_are_shared() {
    let rows = provided::<dyn HomeRowSource>(Script::default());

    assert!(rows.gives(HomeRowMethod::Shared));
}
