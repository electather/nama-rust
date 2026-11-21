use nama_plugin_api::metadata::{ExternalId, ExternalIds};

use super::{
    super::{Decided, Item, fixture::tmdb_movie, testing::Fixture},
    library_with_movie, library_with_show, matched_episodes,
};

fn imdb(value: &str) -> ExternalId {
    ExternalId::new("imdb", value).unwrap()
}

#[tokio::test]
async fn a_shows_library_and_seasons_episodes_are_found_by_any_of_its_ids() {
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
    let show = library_with_show(&mut conn, &imdb("tt0303461")).await;
    assert_eq!(show.unwrap(), Some(fx.library));
    let own_as_show = library_with_show(&mut conn, &ExternalId::tmdb(1001)).await;
    assert_eq!(own_as_show.unwrap(), None);
}
