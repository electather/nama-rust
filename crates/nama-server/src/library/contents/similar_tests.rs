use nama_plugin_api::{
    PluginId,
    metadata::{ExternalId, ExternalIds},
};

use crate::library::TitleKind;

use super::super::{
    Decided, Metadata,
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
