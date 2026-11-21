use nama_plugin_api::{
    PluginId,
    metadata::{ExternalId, ExternalIds},
};

use crate::library::TitleKind;

use super::super::{
    Decided, Item, Metadata,
    fixture::tmdb_movie,
    testing::{ALIEN, Fixture, movie},
};

/// Matches the one Item stored at `path` to `metadata`.
async fn record(fx: &Fixture, path: &str, metadata: Metadata) {
    let item = fx.item(path).await;
    fx.contents
        .record(vec![(item, Decided::Matched(Box::new(metadata)))])
        .await
        .unwrap();
}

fn dark_episode(episode: i64) -> Metadata {
    Metadata {
        ids: ExternalIds::from_iter([ExternalId::tmdb(1_000 + episode)]),
        show_ids: ExternalIds::from_iter([ExternalId::tmdb(70_523), ExternalId::tvdb(334_824)]),
        show_title: Some("Dark".to_owned()),
        score: Some(8.4),
        ..tmdb_movie(0, "Episode")
    }
}

#[tokio::test]
async fn a_shows_score_is_stored_once_not_on_each_episode() {
    let fx = Fixture::shows().await;
    for episode in 1..=2 {
        let path = format!("/media/Dark/Season 01/Dark - S01E0{episode}.mkv");
        let item = Item::Episode {
            show: "Dark".to_owned(),
            year: None,
            hint: None,
            season: 1,
            episode,
        };
        fx.store(&path, 10, item).await;
        record(&fx, &path, dark_episode(episode)).await;
    }

    let shows = fx.contents.shows(fx.library, None).await.unwrap();
    let stored: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM title_scores")
        .fetch_one(fx.contents.db.reader())
        .await
        .unwrap();

    assert_eq!((shows[0].score, stored), (Some(8.4), 1));
}

#[tokio::test]
async fn a_movie_has_the_score_and_original_language_its_source_gave() {
    let fx = Fixture::new().await;
    fx.store(ALIEN, 10, movie("Alien", 1979)).await;
    let alien = Metadata {
        score: Some(8.2),
        original_language: Some("en".to_owned()),
        ..tmdb_movie(348, "Alien")
    };
    record(&fx, ALIEN, alien).await;

    let movies = fx.contents.movies(fx.library, None).await.unwrap();

    assert_eq!(
        (movies[0].score, movies[0].original_language.as_deref()),
        (Some(8.2), Some("en"))
    );
}

#[tokio::test]
async fn a_title_matched_again_through_another_source_has_only_that_sources_score() {
    let fx = Fixture::new().await;
    fx.store(ALIEN, 10, movie("Alien", 1979)).await;
    let by_tmdb = Metadata {
        score: Some(8.2),
        ..tmdb_movie(348, "Alien")
    };
    record(&fx, ALIEN, by_tmdb).await;
    let by_another = Metadata {
        plugin: PluginId::from_static("another"),
        ..tmdb_movie(348, "Alien")
    };
    record(&fx, ALIEN, by_another).await;

    let movies = fx.contents.movies(fx.library, None).await.unwrap();

    assert_eq!(movies[0].score, None);
}

#[tokio::test]
async fn a_movies_similar_titles_read_back_in_order_from_the_source_it_is_matched_to_only() {
    let fx = Fixture::new().await;
    fx.store(ALIEN, 10, movie("Alien", 1979)).await;
    let similar = vec![ExternalId::tmdb(679), ExternalId::tmdb(126)];
    let alien = Metadata {
        similar: similar.clone(),
        ..tmdb_movie(348, "Alien")
    };
    record(&fx, ALIEN, alien).await;
    let ids = ExternalIds::from_iter([ExternalId::tmdb(348)]);

    let read = || fx.contents.similar(fx.library, TitleKind::Movie, &ids);

    assert_eq!(read().await.unwrap(), similar);
    let by_another = Metadata {
        plugin: PluginId::from_static("another"),
        ..tmdb_movie(348, "Alien")
    };
    record(&fx, ALIEN, by_another).await;
    assert_eq!(read().await.unwrap(), []);
}
