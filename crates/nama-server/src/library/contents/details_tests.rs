use nama_plugin_api::{
    PluginId,
    metadata::{ExternalId, ExternalIds},
};

use crate::library::title_source::Trailer;

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

fn trailer() -> Trailer {
    Trailer {
        name: "Official Trailer".to_owned(),
        url: "https://www.youtube.com/watch?v=ESEUoa-mz2c".to_owned(),
    }
}

fn dark_episode(episode: i64) -> Metadata {
    Metadata {
        ids: ExternalIds::from_iter([ExternalId::tmdb(1_000 + episode)]),
        show_ids: ExternalIds::from_iter([ExternalId::tmdb(70_523), ExternalId::tvdb(334_824)]),
        show_title: Some("Dark".to_owned()),
        score: Some(8.4),
        trailers: vec![trailer()],
        ..tmdb_movie(0, "Episode")
    }
}

#[tokio::test]
async fn a_shows_score_and_trailers_are_stored_once_not_on_each_episode() {
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
    let stored: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM title_details")
        .fetch_one(fx.contents.db.reader())
        .await
        .unwrap();

    assert_eq!(
        (shows[0].score, shows[0].trailers.as_slice(), stored),
        (Some(8.4), [trailer()].as_slice(), 1)
    );
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
