//! Posters and backdrops: fetched from the metadata Provider the first time they're asked for,
//! then served from the cache on disk.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/tmdb.rs"]
mod tmdb;

use nama_plugin_api::PluginId;
use nama_plugin_tmdb::testing::FakeTmdb;
use nama_server::{
    artwork::{Artwork, Error},
    plugins::Registry,
};

const POSTER: &str = "/vfrQk5IPloGg1v9Rzbh2Eg3VGyM.jpg";

/// A fake TMDB, and a registry with the TMDB Plugin enabled against it.
async fn tmdb() -> (FakeTmdb, Registry) {
    let plugins = Registry::new();
    (tmdb::enable(&plugins).await, plugins)
}

#[tokio::test]
async fn an_image_is_downloaded_once_then_served_from_disk() {
    let (fake, plugins) = tmdb().await;
    let data = tempfile::tempdir().unwrap();
    let tmdb = PluginId::from_static("tmdb");

    let first = Artwork::new(data.path(), plugins.clone())
        .get(&tmdb, POSTER)
        .await
        .unwrap();
    // A fresh cache over the same directory, as after a restart.
    let again = Artwork::new(data.path(), plugins)
        .get(&tmdb, POSTER)
        .await
        .unwrap();

    assert_eq!(fake.requests().len(), 1);
    assert_eq!(first, again);
    assert!(first.path.starts_with(data.path()));
    assert_eq!(first.content_type, "image/jpeg");
    assert!(std::fs::read(&first.path).unwrap().starts_with(b"\xff\xd8"));
}

#[tokio::test]
async fn an_image_the_provider_does_not_have_is_not_found() {
    let (_fake, plugins) = tmdb().await;
    let data = tempfile::tempdir().unwrap();

    let missing = Artwork::new(data.path(), plugins)
        .get(&PluginId::from_static("tmdb"), "/missing.jpg")
        .await;

    assert!(matches!(missing, Err(Error::NotFound)), "{missing:?}");
}

#[tokio::test]
async fn an_image_from_a_plugin_that_is_not_enabled_is_unavailable() {
    let (fake, plugins) = tmdb().await;
    let data = tempfile::tempdir().unwrap();

    let result = Artwork::new(data.path(), plugins)
        .get(&PluginId::from_static("other"), POSTER)
        .await;

    assert!(matches!(result, Err(Error::Unavailable(_))), "{result:?}");
    assert_eq!(fake.requests(), Vec::<String>::new());
}
