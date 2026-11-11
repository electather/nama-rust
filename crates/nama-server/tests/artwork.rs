//! Posters and backdrops: fetched from the metadata Provider the first time they're asked for,
//! then kept in the Server's image store.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/tmdb.rs"]
mod tmdb;

use nama_plugin_api::PluginId;
use nama_plugin_tmdb::testing::FakeTmdb;
use nama_server::{
    artwork::{Artwork, Error},
    plugins::{ExtensionPoints, Registry},
};

const POSTER: &str = "/vfrQk5IPloGg1v9Rzbh2Eg3VGyM.jpg";

/// A fake TMDB, and the TMDB Plugin enabled against it.
async fn tmdb() -> (FakeTmdb, ExtensionPoints) {
    let plugins = Registry::new();
    (tmdb::enable(&plugins).await, plugins.extension_points())
}

#[tokio::test]
async fn an_image_is_downloaded_once_then_kept() {
    let (fake, plugins) = tmdb().await;
    let data = tempfile::tempdir().unwrap();
    let tmdb = PluginId::from_static("tmdb");

    Artwork::new(data.path(), plugins.clone())
        .keep(&tmdb, POSTER)
        .await
        .unwrap();
    // A fresh store over the same directory, as after a restart.
    Artwork::new(data.path(), plugins)
        .keep(&tmdb, POSTER)
        .await
        .unwrap();

    assert_eq!(fake.requests().len(), 1);
}

#[tokio::test]
async fn an_image_the_provider_does_not_have_is_not_found() {
    let (_fake, plugins) = tmdb().await;
    let data = tempfile::tempdir().unwrap();

    let missing = Artwork::new(data.path(), plugins)
        .keep(&PluginId::from_static("tmdb"), "/missing.jpg")
        .await;

    assert!(matches!(missing, Err(Error::NotFound)), "{missing:?}");
}

#[tokio::test]
async fn an_image_from_a_plugin_that_is_not_enabled_is_unavailable() {
    let (fake, plugins) = tmdb().await;
    let data = tempfile::tempdir().unwrap();

    let result = Artwork::new(data.path(), plugins)
        .keep(&PluginId::from_static("other"), POSTER)
        .await;

    assert!(matches!(result, Err(Error::Unavailable(_))), "{result:?}");
    assert_eq!(fake.requests(), Vec::<String>::new());
}
