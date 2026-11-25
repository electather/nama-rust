use std::time::Duration;

use nama_plugin_api::metadata::{ExternalId, ExternalIds};

use super::{
    super::{
        Change, Decided, Item, ItemKind, Metadata,
        fixture::tmdb_movie,
        testing::{ALIEN, Fixture, movie},
    },
    Listed,
};
use crate::db;

/// Stores a Movie file at `path` and Matches it to a Movie with `ids`, called `title`.
async fn matched_movie(fx: &Fixture, path: &str, title: &str, ids: ExternalIds) {
    fx.store(path, 10, movie(title, 2000)).await;
    let metadata = Metadata {
        ids,
        ..tmdb_movie(0, title)
    };
    let item = fx.item(path).await;
    fx.contents
        .record(vec![(item, Decided::Matched(Box::new(metadata)))])
        .await
        .unwrap();
}

/// Stores an Episode file at `path`, Season 1 Episode `number` of Dark, and Matches it to the
/// Episode `tmdb-(1000 + number)` called `title`, of a Show with `show_ids`.
async fn matched_episode(
    fx: &Fixture,
    path: &str,
    number: i64,
    title: &str,
    show_ids: ExternalIds,
) {
    let episode = Item::Episode {
        show: "Dark".to_owned(),
        year: None,
        hint: None,
        season: 1,
        episode: number,
    };
    fx.store(path, 10, episode).await;
    let metadata = Metadata {
        show_ids,
        show_title: Some("Dark".to_owned()),
        ..tmdb_movie(1000 + number, title)
    };
    let item = fx.item(path).await;
    fx.contents
        .record(vec![(item, Decided::Matched(Box::new(metadata)))])
        .await
        .unwrap();
}

async fn page_ids(
    fx: &Fixture,
    after: Option<&ExternalId>,
    limit: Option<usize>,
) -> Vec<ExternalId> {
    let page = fx
        .contents
        .titles_page(fx.library, ItemKind::Movie, after, limit);
    (page.await.unwrap().into_iter())
        .map(|listed| listed.id)
        .collect()
}

#[tokio::test]
async fn a_page_lists_each_movie_once_by_id_after_the_one_named() {
    let fx = Fixture::new().await;
    for (tmdb, path) in [
        (10, "/media/10.mkv"),
        (9, "/media/9.mkv"),
        (2, "/media/2.mkv"),
    ] {
        let ids = ExternalIds::from_iter([ExternalId::tmdb(tmdb)]);
        matched_movie(&fx, path, &format!("Movie {tmdb}"), ids).await;
    }
    let cut = ExternalIds::from_iter([ExternalId::tmdb(9)]);
    matched_movie(&fx, "/media/9 - Cut.mkv", "Movie 9", cut).await;

    let all = page_ids(&fx, None, None).await;
    let first = page_ids(&fx, None, Some(2)).await;
    let rest = page_ids(&fx, Some(&ExternalId::tmdb(9)), Some(2)).await;

    let ids =
        |tmdb: &[i64]| -> Vec<ExternalId> { tmdb.iter().map(|&id| ExternalId::tmdb(id)).collect() };
    assert_eq!(all, ids(&[2, 9, 10]));
    assert_eq!(first, ids(&[2, 9]));
    assert_eq!(rest, ids(&[10]));
}

#[tokio::test]
async fn a_page_of_episodes_gives_their_show_numbers_ids_and_added_time() {
    let fx = Fixture::shows().await;
    let dark = ExternalIds::from_iter([ExternalId::tmdb(70_523), ExternalId::tvdb(121)]);
    let before = db::now();
    matched_episode(&fx, "/media/Dark - S01E01.mkv", 1, "Secrets", dark).await;
    let after = db::now();

    let page = fx
        .contents
        .titles_page(fx.library, ItemKind::Episode, None, None)
        .await
        .unwrap();

    let [listed] = page.as_slice() else {
        panic!("{page:?}");
    };
    assert_eq!(
        listed,
        &Listed {
            id: ExternalId::tmdb(1001),
            title: "Secrets".to_owned(),
            show_title: Some("Dark".to_owned()),
            season: Some(1),
            episode: Some(1),
            added_at: listed.added_at,
            genres: Vec::new(),
            genre_ids: Vec::new(),
            content_rating: None,
            show_id: Some(ExternalId::tmdb(70_523)),
            ids: ExternalIds::from_iter([ExternalId::tmdb(1001)]),
            show_ids: ExternalIds::from_iter([ExternalId::tmdb(70_523), ExternalId::tvdb(121)]),
        }
    );
    assert!((before..=after).contains(&listed.added_at), "{listed:?}");
}

/// When the one Movie of `fx`'s Library was added, as a page lists it.
async fn movie_added_at(fx: &Fixture) -> i64 {
    let page = fx
        .contents
        .titles_page(fx.library, ItemKind::Movie, None, None);
    let page = page.await.unwrap();
    let [listed] = page.as_slice() else {
        panic!("{page:?}");
    };
    listed.added_at
}

#[tokio::test]
async fn a_movie_keeps_the_time_it_was_added_through_a_reparse_and_a_new_match() {
    let fx = Fixture::new().await;
    let ids = ExternalIds::from_iter([ExternalId::tmdb(348)]);
    let before = db::now();
    matched_movie(&fx, ALIEN, "Alien", ids.clone()).await;
    let after = db::now();
    let added = movie_added_at(&fx).await;

    db::advance_clock(Duration::from_secs(2));
    let file = fx.contents.stored(fx.library).await.unwrap()[ALIEN].id;
    let reparse = Change::Reparse {
        edition: None,
        file,
        path: ALIEN.to_owned(),
        items: vec![movie("Alien", 2000)],
    };
    fx.contents.apply(fx.library, vec![reparse]).await.unwrap();
    let rematched = Metadata {
        ids,
        ..tmdb_movie(0, "Alien: Director's Cut")
    };
    let item = fx.item(ALIEN).await;
    fx.contents
        .record(vec![(item, Decided::Matched(Box::new(rematched)))])
        .await
        .unwrap();

    assert!((before..=after).contains(&added), "{added}");
    assert_eq!(movie_added_at(&fx).await, added);
}

#[tokio::test]
async fn a_movie_was_added_when_its_newest_version_was() {
    let fx = Fixture::new().await;
    let ids = ExternalIds::from_iter([ExternalId::tmdb(348)]);
    matched_movie(&fx, "/media/Alien.mkv", "Alien", ids.clone()).await;

    db::advance_clock(Duration::from_secs(2));
    let before = db::now();
    matched_movie(&fx, "/media/Alien - Cut.mkv", "Alien", ids).await;
    let after = db::now();

    let added = movie_added_at(&fx).await;
    assert!((before..=after).contains(&added), "{added}");
}

#[tokio::test]
async fn season_of_gives_an_episodes_show_ids_and_season_and_none_for_one_not_stored() {
    let fx = Fixture::new().await;
    let dark = ExternalIds::from_iter([ExternalId::tmdb(70_523), "tvdb-121361".parse().unwrap()]);
    matched_episode(&fx, "/media/Dark - S01E02.mkv", 2, "Lies", dark.clone()).await;

    let lies = fx
        .contents
        .season_of(fx.library, &ExternalId::tmdb(1002))
        .await;
    let unknown = fx
        .contents
        .season_of(fx.library, &ExternalId::tmdb(9))
        .await;

    assert_eq!(lies.unwrap(), Some((dark, 1)));
    assert_eq!(unknown.unwrap(), None);
}
