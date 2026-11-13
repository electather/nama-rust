use nama_plugin_api::metadata::{ExternalId, ExternalIds};

use super::{
    super::{
        Change, Decided, Item,
        fixture::tmdb_movie,
        testing::{ALIEN, Fixture, movie},
    },
    library_with_movie, matched_episodes, show_poster,
};

#[test]
fn a_shows_poster_is_its_first_seasons_after_the_specials_that_has_one() {
    let specials_and_first = [(0, Some("/specials.jpg")), (1, Some("/1.jpg"))];
    let first_without = [(2, Some("/2.jpg")), (1, None)];
    let specials_alone = [(0, Some("/specials.jpg"))];
    let none = [(0, None), (1, None)];

    assert_eq!(show_poster(specials_and_first), Some("/1.jpg"));
    assert_eq!(show_poster(first_without), Some("/2.jpg"));
    assert_eq!(show_poster(specials_alone), Some("/specials.jpg"));
    assert_eq!(show_poster(none), None);
}

fn imdb(value: &str) -> ExternalId {
    ExternalId::new("imdb", value).unwrap()
}

/// Matches the one Item stored at `path` to a Movie with `ids`.
async fn match_movie(fx: &Fixture, path: &str, ids: ExternalIds) {
    let item = fx.item(path).await;
    let metadata = super::super::Metadata {
        ids,
        ..tmdb_movie(0, "Alien")
    };
    fx.contents
        .record(vec![(item, Decided::Matched(Box::new(metadata)))])
        .await
        .unwrap();
}

async fn library_of(fx: &Fixture, id: &ExternalId) -> Option<i64> {
    fx.contents.library_with_movie(id).await.unwrap()
}

async fn id_rows(fx: &Fixture) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM item_match_ids")
        .fetch_one(fx.contents.db.reader())
        .await
        .unwrap()
}

#[tokio::test]
async fn a_movie_is_found_by_any_of_its_ids() {
    let fx = Fixture::new().await;
    fx.store(ALIEN, 10, movie("Alien", 1979)).await;
    match_movie(
        &fx,
        ALIEN,
        ExternalIds::from_iter([ExternalId::tmdb(348), imdb("tt0078748")]),
    )
    .await;

    assert_eq!(
        library_of(&fx, &ExternalId::tmdb(348)).await,
        Some(fx.library)
    );
    assert_eq!(library_of(&fx, &imdb("tt0078748")).await, Some(fx.library));
    assert_eq!(library_of(&fx, &ExternalId::tmdb(679)).await, None);
    assert_eq!(library_of(&fx, &imdb("tt0090605")).await, None);
    // A TVDB id with the same value is another title's.
    assert_eq!(library_of(&fx, &ExternalId::tvdb(348)).await, None);
}

#[tokio::test]
async fn a_seasons_episodes_are_found_by_any_of_their_shows_ids() {
    let fx = Fixture::new().await;
    let path = "/media/Firefly/Season 01/Firefly - S01E02.mkv";
    let episode = Item::Episode {
        show: "Firefly".to_owned(),
        year: None,
        hint: None,
        season: 1,
        episode: 2,
    };
    fx.store(path, 10, episode).await;
    let item = fx.item(path).await;
    let metadata = super::super::Metadata {
        ids: ExternalIds::from_iter([ExternalId::tmdb(1001)]),
        show_ids: ExternalIds::from_iter([
            ExternalId::tmdb(1437),
            imdb("tt0303461"),
            ExternalId::tvdb(78874),
        ]),
        ..tmdb_movie(0, "The Train Job")
    };
    fx.contents
        .record(vec![(item, Decided::Matched(Box::new(metadata)))])
        .await
        .unwrap();

    let mut conn = fx.contents.db.reader().acquire().await.unwrap();
    for show in [
        ExternalId::tmdb(1437),
        imdb("tt0303461"),
        ExternalId::tvdb(78874),
    ] {
        let found = matched_episodes(&mut conn, &show, 1).await.unwrap();
        assert_eq!(found, [2].into(), "{show}");
    }
    let other_season = matched_episodes(&mut conn, &ExternalId::tmdb(1437), 2).await;
    assert!(other_season.unwrap().is_empty());
    // The Episode's own id isn't its Show's, nor is it a Movie.
    let own = matched_episodes(&mut conn, &ExternalId::tmdb(1001), 1).await;
    assert!(own.unwrap().is_empty());
    let as_movie = library_with_movie(&mut conn, &ExternalId::tmdb(1001)).await;
    assert_eq!(as_movie.unwrap(), None);
}

#[tokio::test]
async fn matching_an_item_again_replaces_its_ids() {
    let fx = Fixture::new().await;
    fx.store(ALIEN, 10, movie("Alien", 1979)).await;
    match_movie(
        &fx,
        ALIEN,
        ExternalIds::from_iter([ExternalId::tmdb(348), imdb("tt0078748")]),
    )
    .await;

    match_movie(&fx, ALIEN, ExternalIds::from_iter([ExternalId::tmdb(679)])).await;

    assert_eq!(
        library_of(&fx, &ExternalId::tmdb(679)).await,
        Some(fx.library)
    );
    assert_eq!(library_of(&fx, &ExternalId::tmdb(348)).await, None);
    assert_eq!(library_of(&fx, &imdb("tt0078748")).await, None);
    assert_eq!(id_rows(&fx).await, 1);

    let item = fx.item(ALIEN).await;
    fx.contents
        .record(vec![(item, Decided::Unmatched(None))])
        .await
        .unwrap();
    assert_eq!(id_rows(&fx).await, 0);
}

#[tokio::test]
async fn removing_a_file_removes_its_items_ids() {
    let fx = Fixture::new().await;
    fx.store(ALIEN, 10, movie("Alien", 1979)).await;
    match_movie(
        &fx,
        ALIEN,
        ExternalIds::from_iter([ExternalId::tmdb(348), imdb("tt0078748")]),
    )
    .await;
    assert_eq!(id_rows(&fx).await, 2);

    let file = fx.contents.stored(fx.library).await.unwrap()[ALIEN].id;
    fx.contents
        .apply(fx.library, vec![Change::Remove(file)])
        .await
        .unwrap();

    assert_eq!(id_rows(&fx).await, 0);
}
