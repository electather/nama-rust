use std::{
    collections::HashMap,
    future,
    sync::{Arc, Mutex},
};

use buffa::Message as _;
use nama_plugin_api::{
    BoxFuture, Registrar, Registration,
    automation::{
        Kernel, KernelFunction as _, KernelRefused,
        functions::{ListLibraries, ListTitles, StoreDelete, StoreGet, StorePut},
    },
    contract::{
        FeaturedSuggestions, HomeRow, HomeRowTitle, HomeRows, LibraryKind, SharedRowsAsked,
        SimilarAsked, SimilarTitle, SimilarTitles, TitleKind, TitlesAsked, UserFeaturedAsked,
        UserRowsAsked,
    },
    contract::{
        Libraries, Library, StoreEntry, StoreKey, StoreValue, Stored, Title, TitleId, TitleRef,
        TitleRefKind, Titles,
    },
    home_row::{HomeRowMethod, HomeRowSource},
    similar_source::SimilarSource,
};

use super::*;
use crate::fake_hub::FakeHub;

/// A Kernel whose only functions are the Plugin's Store, kept in memory.
pub(crate) fn store() -> Arc<dyn Kernel> {
    let entries: Arc<Mutex<HashMap<String, Vec<u8>>>> = Arc::default();
    Arc::new(
        move |function: &str,
              request: Vec<u8>|
              -> BoxFuture<'static, Result<Vec<u8>, KernelRefused>> {
            let mut entries = entries.lock().unwrap();
            let answer = if function == StoreGet::NAME {
                let key = StoreKey::decode_from_slice(&request).unwrap();
                StoreValue {
                    value: entries.get(&key.key).cloned(),
                    ..Default::default()
                }
                .encode_to_vec()
            } else if function == StorePut::NAME {
                let entry = StoreEntry::decode_from_slice(&request).unwrap();
                entries.insert(entry.key, entry.value);
                Stored::default().encode_to_vec()
            } else if function == StoreDelete::NAME {
                let key = StoreKey::decode_from_slice(&request).unwrap();
                entries.remove(&key.key);
                Stored::default().encode_to_vec()
            } else {
                return Box::pin(future::ready(Err(KernelRefused(function.to_owned()))));
            };
            Box::pin(future::ready(Ok(answer)))
        },
    )
}

/// What the Plugin enabled against `hub`, keeping its Store in `kernel`, provides as `T`.
fn provided<T: ?Sized + Send + Sync + 'static>(hub: &FakeHub, kernel: &Arc<dyn Kernel>) -> Arc<T> {
    provider(&registered(hub, kernel))
}

/// The Plugin enabled against `hub`, keeping its Store in `kernel`, as the Kernel holds it.
fn registered(hub: &FakeHub, kernel: &Arc<dyn Kernel>) -> Vec<Registration> {
    let mut registrar = Registrar::new().with_kernel(Arc::clone(kernel));
    Hub::through(hub.kernel_link())
        .register(&mut registrar)
        .unwrap();
    registrar.into_registrations()
}

/// What `registrations` provide as `T`.
fn provider<T: ?Sized + Send + Sync + 'static>(registrations: &[Registration]) -> Arc<T> {
    (registrations.iter())
        .find_map(Registration::provider::<T>)
        .unwrap()
}

/// What a Similar source is asked about a Movie known by `ids`.
fn movie(ids: &[(&str, &str)]) -> SimilarAsked {
    SimilarAsked {
        kind: TitleKind::TITLE_KIND_MOVIE.into(),
        ids: ids
            .iter()
            .map(|&(namespace, value)| TitleId {
                namespace: namespace.to_owned(),
                value: value.to_owned(),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
}

/// `similar`'s titles, each as its id's namespace and value and its weight.
fn answered(similar: &SimilarTitles) -> Vec<(&str, &str, f32)> {
    (similar.titles.iter())
        .map(|title: &SimilarTitle| {
            (
                title.id.namespace.as_str(),
                title.id.value.as_str(),
                title.weight,
            )
        })
        .collect()
}

#[test]
fn without_the_kernels_link_the_plugin_refuses_to_start() {
    let mut registrar = Registrar::new().with_kernel(store());

    let refused = Hub::default().register(&mut registrar).unwrap_err();

    assert!(refused.to_string().contains("names no Hub"), "{refused}");
}

#[tokio::test]
async fn a_linked_server_gets_the_titles_the_hub_finds_alike() {
    let hub = FakeHub::start().await;
    let kernel = store();
    hub.link_server();
    let similar = provided::<dyn SimilarSource>(&hub, &kernel);

    let the_matrix = similar
        .similar(&movie(&[("imdb", "tt0133093"), ("tmdb", "603")]))
        .await
        .unwrap();

    assert_eq!(
        answered(&the_matrix),
        [("imdb", "tt0234215", 0.9), ("imdb", "tt0078748", 0.35)]
    );
}

/// The User `user_id`'s rows and Featured suggestions from the Plugin enabled against `hub`,
/// keeping its Store in `kernel`.
async fn recommended_for(
    hub: &FakeHub,
    kernel: &Arc<dyn Kernel>,
    user_id: i64,
) -> Result<(HomeRows, FeaturedSuggestions), PluginError> {
    let source = provided::<dyn HomeRowSource>(hub, kernel);
    let rows = (source.user_rows(&UserRowsAsked {
        user_id,
        ..Default::default()
    }))
    .await?;
    let featured = (source.user_featured(&UserFeaturedAsked {
        user_id,
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
                .map(|title| {
                    let kind = match title.kind.as_known() {
                        Some(TitleKind::TITLE_KIND_SHOW) => "show",
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
            let kind = suggestion.kind.as_known().unwrap();
            (
                kind,
                format!("{}-{}", id.namespace, id.value),
                suggestion.weight,
            )
        })
        .collect()
}

/// A Kernel keeping the Store as [`store`] does, whose one Library holds an Episode of the Show
/// `tmdb-1396`, and no Movie.
fn with_a_show() -> Arc<dyn Kernel> {
    let store = store();
    Arc::new(
        move |function: &str,
              request: Vec<u8>|
              -> BoxFuture<'static, Result<Vec<u8>, KernelRefused>> {
            let answer = if function == ListLibraries::NAME {
                let shows = Library {
                    id: 1,
                    ..Default::default()
                };
                Libraries {
                    libraries: vec![shows],
                    ..Default::default()
                }
                .encode_to_vec()
            } else if function == ListTitles::NAME {
                let episode = Title {
                    ids: vec![tmdb("62085")],
                    show_ids: vec![tmdb("1396")],
                    ..Default::default()
                };
                Titles {
                    titles: vec![episode],
                    ..Default::default()
                }
                .encode_to_vec()
            } else {
                let (store, function) = (Arc::clone(&store), function.to_owned());
                return Box::pin(async move { store.call(&function, request).await });
            };
            Box::pin(future::ready(Ok(answer)))
        },
    )
}

fn tmdb(value: &str) -> TitleId {
    TitleId {
        namespace: "tmdb".to_owned(),
        value: value.to_owned(),
        ..Default::default()
    }
}

#[tokio::test]
async fn a_linked_user_gets_the_hubs_recommendations_as_a_row_and_those_on_the_server_featured() {
    let hub = FakeHub::start().await;
    let kernel = with_a_show();
    hub.link_server();
    hub.link_user_on_hub(7);

    let (rows, featured) = recommended_for(&hub, &kernel, 7).await.unwrap();

    assert_eq!(
        rows_of(&rows),
        ["recommended: Recommended for you, 3600 s: movie tmdb-604, show tmdb-1396"]
    );
    // Not the Movie, which no Library has, so Featured couldn't Play it (ADR 0072).
    assert_eq!(
        suggested(&featured),
        [(TitleKind::TITLE_KIND_SHOW, "tmdb-1396".to_owned(), 0.4)]
    );
    assert_eq!(featured.ttl_seconds, 3600);
    let source = provided::<dyn HomeRowSource>(&hub, &kernel);
    assert!(source.gives(HomeRowMethod::User));
    assert!(source.gives(HomeRowMethod::Featured));
}

#[tokio::test]
async fn a_user_the_hub_recommends_nothing_gets_no_row_or_suggestions() {
    let hub = FakeHub::start().await;
    let kernel = store();
    hub.link_server();

    let (rows, featured) = recommended_for(&hub, &kernel, 8).await.unwrap();

    assert_eq!(rows_of(&rows), Vec::<String>::new());
    assert_eq!(suggested(&featured), []);
}

#[tokio::test]
async fn a_server_that_isnt_linked_gets_no_recommendations_and_asks_the_hub_nothing() {
    let hub = FakeHub::start().await;
    hub.stop();

    let (rows, featured) = recommended_for(&hub, &store(), 7).await.unwrap();

    assert_eq!(rows_of(&rows), Vec::<String>::new());
    assert_eq!(suggested(&featured), []);
}

#[tokio::test]
async fn recommendations_fail_while_the_hub_is_unreachable_so_the_last_answer_stays() {
    let hub = FakeHub::start().await;
    let kernel = store();
    hub.link_server();
    hub.link_user_on_hub(7);
    hub.stop();

    recommended_for(&hub, &kernel, 7).await.unwrap_err();
}

#[tokio::test]
async fn a_title_the_hub_doesnt_know_gets_no_similar_titles() {
    let hub = FakeHub::start().await;
    let kernel = store();
    hub.link_server();

    let unknown = provided::<dyn SimilarSource>(&hub, &kernel)
        .similar(&movie(&[("tmdb", "1")]))
        .await;

    assert_eq!(answered(&unknown.unwrap()), []);
}

#[tokio::test]
async fn a_server_that_isnt_linked_gets_no_similar_titles_and_asks_the_hub_nothing() {
    let hub = FakeHub::start().await;
    hub.stop();

    let similar = provided::<dyn SimilarSource>(&hub, &store())
        .similar(&movie(&[("tmdb", "603")]))
        .await;

    assert_eq!(answered(&similar.unwrap()), []);
}

#[tokio::test]
async fn similar_titles_fail_while_the_hub_is_unreachable_so_the_last_answer_stays() {
    let hub = FakeHub::start().await;
    let kernel = store();
    hub.link_server();
    hub.stop();

    let similar = provided::<dyn SimilarSource>(&hub, &kernel)
        .similar(&movie(&[("tmdb", "603")]))
        .await;

    similar.unwrap_err();
}

/// The Plugin enabled against `hub` with `kernel`, its Server and its User 7 Linked.
fn linked_user_7(hub: &FakeHub, kernel: &Arc<dyn Kernel>) -> Vec<Registration> {
    let registrations = registered(hub, kernel);
    hub.link_server();
    hub.link_user_on_hub(7);
    registrations
}

/// [`linked_user_7`], once the Hub forgot the Server, as when the Account it Linked with is
/// deleted.
fn dropped(hub: &FakeHub, kernel: &Arc<dyn Kernel>) -> Vec<Registration> {
    let registrations = linked_user_7(hub, kernel);
    hub.forget_server();
    registrations
}

/// Checks that the Server forgot its Link, so it can Link again at once.
fn links_again(hub: &FakeHub) {
    assert!(!hub.server_keeps_link());
}

#[tokio::test]
async fn similar_titles_for_a_server_the_hub_forgot_are_none() {
    let hub = FakeHub::start().await;
    let points = dropped(&hub, &store());

    let similar = provider::<dyn SimilarSource>(&points)
        .similar(&movie(&[("tmdb", "603")]))
        .await;

    assert_eq!(answered(&similar.unwrap()), []);
    links_again(&hub);
}

#[tokio::test]
async fn the_recommended_row_for_a_server_the_hub_forgot_is_empty() {
    let hub = FakeHub::start().await;
    let points = dropped(&hub, &with_a_show());

    let rows = provider::<dyn HomeRowSource>(&points)
        .user_rows(&UserRowsAsked {
            user_id: 7,
            ..Default::default()
        })
        .await;

    assert_eq!(rows_of(&rows.unwrap()), Vec::<String>::new());
    links_again(&hub);
}

#[tokio::test]
async fn featured_suggestions_for_a_server_the_hub_forgot_are_none() {
    let hub = FakeHub::start().await;
    let points = dropped(&hub, &with_a_show());

    let featured = provider::<dyn HomeRowSource>(&points)
        .user_featured(&UserFeaturedAsked {
            user_id: 7,
            ..Default::default()
        })
        .await;

    assert_eq!(suggested(&featured.unwrap()), []);
    links_again(&hub);
}

#[tokio::test]
async fn nama_charts_for_a_server_the_hub_forgot_are_no_rows() {
    let hub = FakeHub::start().await;
    let points = dropped(&hub, &with_movies(&["603"]));

    let rows = provider::<dyn HomeRowSource>(&points)
        .shared_rows(&SharedRowsAsked::default())
        .await;

    assert_eq!(rows.unwrap().rows, []);
    links_again(&hub);
}

#[tokio::test]
async fn every_call_while_the_hub_is_unreachable_fails_and_keeps_the_link() {
    every_call_fails_and_keeps_the_link(FakeHub::stop).await;
}

#[tokio::test]
async fn every_call_while_the_hub_answers_failures_fails_and_keeps_the_link() {
    every_call_fails_and_keeps_the_link(FakeHub::fail_server_calls).await;
}

/// Makes every Extension Point's Hub call during `outage`, which only `unauthenticated` would
/// count as a dropped Link.
async fn every_call_fails_and_keeps_the_link(outage: impl FnOnce(&FakeHub)) {
    let hub = FakeHub::start().await;
    let points = linked_user_7(&hub, &store());
    let rows = provider::<dyn HomeRowSource>(&points);
    outage(&hub);

    (provider::<dyn SimilarSource>(&points).similar(&movie(&[("tmdb", "603")])))
        .await
        .unwrap_err();
    let user = UserRowsAsked {
        user_id: 7,
        ..Default::default()
    };
    rows.user_rows(&user).await.unwrap_err();
    let featured = UserFeaturedAsked {
        user_id: 7,
        ..Default::default()
    };
    rows.user_featured(&featured).await.unwrap_err();
    (rows.shared_rows(&SharedRowsAsked::default()))
        .await
        .unwrap_err();

    assert!(hub.server_keeps_link());
}

/// The Movies Library that [`with_movies`]'s Kernel has.
const MOVIES_LIBRARY: i64 = 1;

/// A Kernel keeping the Store as [`store`] does, with a Movies Library holding the Movies whose
/// TMDB ids are `tmdb`, and a Shows Library.
fn with_movies(tmdb: &'static [&'static str]) -> Arc<dyn Kernel> {
    let store = store();
    Arc::new(
        move |function: &str,
              request: Vec<u8>|
              -> BoxFuture<'static, Result<Vec<u8>, KernelRefused>> {
            let answer = if function == ListLibraries::NAME {
                let library = |id, kind: LibraryKind| Library {
                    id,
                    kind: kind.into(),
                    ..Default::default()
                };
                Libraries {
                    libraries: vec![
                        library(MOVIES_LIBRARY, LibraryKind::LIBRARY_KIND_MOVIES),
                        library(2, LibraryKind::LIBRARY_KIND_SHOWS),
                    ],
                    ..Default::default()
                }
                .encode_to_vec()
            } else if function == ListTitles::NAME {
                let asked = TitlesAsked::decode_from_slice(&request).unwrap();
                assert_eq!(
                    asked.library_id, MOVIES_LIBRARY,
                    "only Movies are asked for"
                );
                let movie = |value: &str| Title {
                    title_ref: TitleRef {
                        library_id: MOVIES_LIBRARY,
                        kind: TitleRefKind::TITLE_REF_KIND_MOVIE.into(),
                        id: TitleId {
                            namespace: "tmdb".to_owned(),
                            value: value.to_owned(),
                            ..Default::default()
                        }
                        .into(),
                        ..Default::default()
                    }
                    .into(),
                    ..Default::default()
                };
                Titles {
                    titles: tmdb.iter().map(|&value| movie(value)).collect(),
                    ..Default::default()
                }
                .encode_to_vec()
            } else {
                let (store, function) = (Arc::clone(&store), function.to_owned());
                return Box::pin(async move { store.call(&function, request).await });
            };
            Box::pin(future::ready(Ok(answer)))
        },
    )
}

/// The titles of `rows`' row keyed `key`, each as its id's namespace and value.
fn row_titles<'a>(rows: &'a HomeRows, key: &str) -> Vec<(&'a str, &'a str)> {
    let Some(row) = rows.rows.iter().find(|row| row.key == key) else {
        panic!("a row keyed {key}: {rows:?}");
    };
    (row.titles.iter())
        .map(|title: &HomeRowTitle| (title.id.namespace.as_str(), title.id.value.as_str()))
        .collect()
}

#[tokio::test]
async fn a_linked_server_shares_the_trending_movies_its_libraries_have_in_the_hubs_order() {
    let hub = FakeHub::start().await;
    let kernel = with_movies(&["862", "603", "1"]);
    hub.link_server();

    let rows = provided::<dyn HomeRowSource>(&hub, &kernel)
        .shared_rows(&SharedRowsAsked::default())
        .await
        .unwrap();

    assert_eq!(
        row_titles(&rows, "trending"),
        [("tmdb", "603"), ("tmdb", "862")]
    );
    let HomeRow {
        key,
        heading,
        ttl_seconds,
        titles,
        ..
    } = &rows.rows[0];
    assert_eq!(
        (key.as_str(), heading.as_str()),
        ("trending", "Trending on Nama")
    );
    assert_eq!(*ttl_seconds, 24 * 60 * 60);
    assert!(
        (titles.iter()).all(|title| title.kind.as_known() == Some(TitleKind::TITLE_KIND_MOVIE))
    );
}

#[tokio::test]
async fn trending_movies_no_library_has_leave_the_row_empty() {
    let hub = FakeHub::start().await;
    let kernel = with_movies(&["1"]);
    hub.link_server();

    let rows = provided::<dyn HomeRowSource>(&hub, &kernel)
        .shared_rows(&SharedRowsAsked::default())
        .await
        .unwrap();

    assert_eq!(row_titles(&rows, "trending"), []);
}

#[tokio::test]
async fn a_linked_server_shares_the_most_watchlisted_titles_in_the_hubs_order_had_or_not() {
    let hub = FakeHub::start().await;
    let kernel = with_movies(&["603"]);
    hub.link_server();

    let rows = provided::<dyn HomeRowSource>(&hub, &kernel)
        .shared_rows(&SharedRowsAsked::default())
        .await
        .unwrap();

    let Some(row) = (rows.rows.iter()).find(|row| row.key == "most-watchlisted") else {
        panic!("a most watchlisted row: {rows:?}");
    };
    let titles: Vec<_> = (row.titles.iter())
        .map(|title| (title.kind.as_known(), title.id.value.as_str()))
        .collect();
    let (movie, show) = (
        Some(TitleKind::TITLE_KIND_MOVIE),
        Some(TitleKind::TITLE_KIND_SHOW),
    );
    assert_eq!(titles, [(show, "70523"), (movie, "603"), (movie, "78")]);
    assert_eq!(row.heading, "Most watchlisted on Nama");
    assert_eq!(row.ttl_seconds, 24 * 60 * 60);
}

#[tokio::test]
async fn a_server_that_isnt_linked_has_no_trending_row_and_asks_the_hub_nothing() {
    let hub = FakeHub::start().await;
    hub.stop();

    let rows = provided::<dyn HomeRowSource>(&hub, &with_movies(&["603"]))
        .shared_rows(&SharedRowsAsked::default())
        .await
        .unwrap();

    assert_eq!(rows.rows, []);
}

#[tokio::test]
async fn whats_trending_fails_while_the_hub_is_unreachable() {
    let hub = FakeHub::start().await;
    let kernel = with_movies(&["603"]);
    hub.link_server();
    hub.stop();

    let rows = provided::<dyn HomeRowSource>(&hub, &kernel)
        .shared_rows(&SharedRowsAsked::default())
        .await;

    rows.unwrap_err();
}

#[tokio::test]
async fn the_trending_row_is_shared() {
    let hub = FakeHub::start().await;

    let rows = provided::<dyn HomeRowSource>(&hub, &store());

    assert!(rows.gives(HomeRowMethod::Shared));
}
