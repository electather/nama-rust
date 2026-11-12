//! Posters and backdrops: downloaded when a Match is made, fetched from the metadata Provider if
//! someone asks first, and kept in the Server's image store while a Match stores them.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/browsing.rs"]
mod browsing;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/http.rs"]
mod http;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use browsing::{ALIEN_1080P, Fixture, tmdb_id};
use connectrpc::client::HttpClient;
use http::fetch;
use nama_plugin_api::PluginId;
use nama_plugin_tmdb::testing::FakeTmdb;
use nama_proto::nama::v1::{
    ClearMatchRequest, DeleteLibraryRequest, LibraryKind, LibraryServiceClient,
    RequestServiceClient, SearchMoviesRequest, SetMatchRequest,
};
use nama_server::{
    artwork::{Artwork, Error},
    db::Database,
    images::ImageStore,
    plugins::{ExtensionPoints, Registry},
};
use server::{
    TestServer, WAIT,
    tmdb::{ALIEN, HEAT},
};

const POSTER: &str = "/vfrQk5IPloGg1v9Rzbh2Eg3VGyM.jpg";
const BACKDROP: &str = "/AmR3JG1VQVxU8TfAvljUhfSFUOx.jpg";
const HEAT_POSTER: &str = "/umSVjVdbVwtx5ryCA2QXL44Durm.jpg";
const HEAT_BACKDROP: &str = "/rfEXNlql4CafRmtgp2VLrdyBMCo.jpg";
/// Dark's Season 1 Poster and its Show's Backdrop.
const DARK_SEASON_1_POSTER: &str = "/5LoHuHWA4H8jElFlZDvsmU2n63b.jpg";
const DARK_BACKDROP: &str = "/3lBDg3i6nn5R2NKFCJ6oKyUo2j5.jpg";
/// The Still of Dark's S01E01.
const SECRETS_STILL: &str = "/gNnJBvHy3VBkGrsl9ffpkf6ucP3.jpg";
const DARK_S01E01: &str = "Dark (2017)/Season 01/Dark - S01E01.mkv";

/// Where `server` keeps TMDB's image at `path`, as the store escapes its leading `/`.
fn kept(server: &TestServer, path: &str) -> PathBuf {
    server
        .data_dir()
        .join("artwork/tmdb")
        .join(path.replacen('/', "%2F", 1))
}

/// Waits until `server` keeps each of TMDB's images at `paths`, failing the test after [`WAIT`].
async fn until_kept(server: &TestServer, paths: &[&str]) {
    let all_kept = || paths.iter().all(|path| kept(server, path).exists());
    let waited = tokio::time::timeout(WAIT, async {
        while !all_kept() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    assert!(waited.is_ok(), "never kept {paths:?}");
}

#[tokio::test]
async fn a_scan_downloads_a_matched_movies_poster_and_backdrop_before_anyone_views_them() {
    let fx = Fixture::alien().await;

    until_kept(&fx.server, &[POSTER, BACKDROP]).await;
}

#[tokio::test]
async fn a_scan_downloads_an_episodes_season_poster_show_backdrop_and_still() {
    let fx = Fixture::start_on(LibraryKind::LIBRARY_KIND_SHOWS, TestServer::builder()).await;
    fx.write(DARK_S01E01, b"video");

    fx.scan_and_match().await;

    until_kept(
        &fx.server,
        &[DARK_SEASON_1_POSTER, DARK_BACKDROP, SECRETS_STILL],
    )
    .await;
}

#[tokio::test]
async fn an_episodes_still_is_dropped_once_no_match_stores_it() {
    let mut fx = Fixture::start_on(LibraryKind::LIBRARY_KIND_SHOWS, TestServer::builder()).await;
    fx.write(DARK_S01E01, b"video");
    fx.items = fx.scan_and_match().await;
    until_kept(&fx.server, &[SECRETS_STILL]).await;

    fx.matches
        .clear_match(ClearMatchRequest {
            item_id: fx.item(DARK_S01E01),
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(!kept(&fx.server, SECRETS_STILL).exists());
}

#[tokio::test]
async fn a_pinned_match_downloads_its_titles_images() {
    let fx = Fixture::alien().await;

    pin(&fx, HEAT).await;

    until_kept(&fx.server, &[HEAT_POSTER, HEAT_BACKDROP]).await;
}

/// A download that failed, or a Library Matched before Matches downloaded, is filled in.
#[tokio::test]
async fn a_scan_downloads_an_image_the_store_lacks_and_a_view_meanwhile_fetches_it() {
    let fx = Fixture::alien().await;
    until_kept(&fx.server, &[POSTER]).await;
    fs::remove_file(kept(&fx.server, POSTER)).unwrap();
    let poster = fx
        .get_movie(fx.library, ALIEN)
        .await
        .unwrap()
        .poster_url
        .unwrap();

    let viewed = fetch(&fx.server, &poster, Some(&fx.server.admin_token)).await;
    fs::remove_file(kept(&fx.server, POSTER)).unwrap();
    fx.scan_and_match().await;

    assert_eq!(viewed.status, 200);
    until_kept(&fx.server, &[POSTER]).await;
}

/// A client should try again later, rather than take the image to be gone.
#[tokio::test]
async fn an_image_not_kept_from_a_provider_that_fails_is_unavailable() {
    let mut fx = Fixture::alien().await;
    let poster = fx
        .get_movie(fx.library, ALIEN)
        .await
        .unwrap()
        .poster_url
        .unwrap();
    // Once the scan's downloads are done, so none writes the poster again.
    until_kept(&fx.server, &[POSTER, BACKDROP]).await;
    fx.server.stop_tmdb();
    fs::remove_file(kept(&fx.server, POSTER)).unwrap();

    let reply = fetch(&fx.server, &poster, Some(&fx.server.admin_token)).await;

    assert_eq!(reply.status, 503);
}

#[tokio::test]
async fn a_match_corrected_to_another_title_drops_the_old_titles_images() {
    let fx = Fixture::alien().await;
    until_kept(&fx.server, &[POSTER, BACKDROP]).await;

    pin(&fx, HEAT).await;

    assert!(!kept(&fx.server, POSTER).exists());
    assert!(!kept(&fx.server, BACKDROP).exists());
}

#[tokio::test]
async fn deleting_a_library_drops_the_images_only_its_items_used() {
    let fx = Fixture::alien().await;
    let other = (fx.server)
        .matched_library(LibraryKind::LIBRARY_KIND_MOVIES, &[ALIEN_1080P])
        .await;
    until_kept(&fx.server, &[POSTER, BACKDROP]).await;

    delete_library(&fx.server, fx.library).await;
    let shared_kept = kept(&fx.server, POSTER).exists();
    delete_library(&fx.server, other.id).await;

    assert!(shared_kept);
    assert!(!kept(&fx.server, POSTER).exists());
    assert!(!kept(&fx.server, BACKDROP).exists());
}

/// What a search shows goes to a cache of its own, so deleting it loses nothing a Match stores.
#[tokio::test]
async fn a_search_results_image_is_cached_apart_from_the_kept_ones() {
    let fx = Fixture::start_on(LibraryKind::LIBRARY_KIND_MOVIES, TestServer::builder()).await;
    let found = RequestServiceClient::new(HttpClient::plaintext(), fx.server.client_config())
        .search_movies(SearchMoviesRequest {
            title: "Alien".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .movies;
    let poster = found[0].poster_url.clone().unwrap();

    let viewed = fetch(&fx.server, &poster, Some(&fx.server.admin_token)).await;
    let kept_from_search = kept(&fx.server, POSTER).exists();
    fx.write(ALIEN_1080P, b"video");
    fx.scan_and_match().await;
    until_kept(&fx.server, &[POSTER]).await;
    fs::remove_dir_all(fx.server.data_dir().join("artwork-cache")).unwrap();

    assert_eq!(viewed.status, 200);
    assert!(!kept_from_search);
    assert!(kept(&fx.server, POSTER).exists());
}

/// Pins the Alien file's Match to the Movie with the TMDB id `tmdb`, as the Admin.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn pin(fx: &Fixture, tmdb: i64) {
    fx.matches
        .set_match(SetMatchRequest {
            item_id: fx.item(ALIEN_1080P),
            title_id: tmdb_id(tmdb).into(),
            ..Default::default()
        })
        .await
        .unwrap();
}

/// Deletes the Library `id` as the Admin.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn delete_library(server: &TestServer, id: i64) {
    LibraryServiceClient::new(HttpClient::plaintext(), server.client_config())
        .delete_library(DeleteLibraryRequest {
            id,
            ..Default::default()
        })
        .await
        .unwrap();
}

/// A fake TMDB, and the TMDB Plugin enabled against it.
async fn tmdb() -> (FakeTmdb, ExtensionPoints) {
    let plugins = Registry::new();
    (
        server::tmdb::enable(&plugins).await,
        plugins.extension_points(),
    )
}

/// Artwork kept in `data`, fetched through `plugins`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn artwork_in(data: &Path, plugins: ExtensionPoints) -> Artwork {
    let db = Database::open(data).await.unwrap();
    Artwork::new(ImageStore::new(data, Arc::new(db)), plugins)
}

#[tokio::test]
async fn an_image_is_downloaded_once_then_kept() {
    let (fake, plugins) = tmdb().await;
    let data = tempfile::tempdir().unwrap();
    let tmdb = PluginId::from_static("tmdb");

    artwork_in(data.path(), plugins.clone())
        .await
        .keep(&tmdb, POSTER)
        .await
        .unwrap();
    // A fresh store over the same directory, as after a restart.
    artwork_in(data.path(), plugins)
        .await
        .keep(&tmdb, POSTER)
        .await
        .unwrap();

    assert_eq!(fake.requests().len(), 1);
}

#[tokio::test]
async fn an_image_the_provider_does_not_have_is_not_found() {
    let (_fake, plugins) = tmdb().await;
    let data = tempfile::tempdir().unwrap();

    let missing = artwork_in(data.path(), plugins)
        .await
        .keep(&PluginId::from_static("tmdb"), "/missing.jpg")
        .await;

    assert!(matches!(missing, Err(Error::NotFound)), "{missing:?}");
}

#[tokio::test]
async fn an_image_from_a_plugin_that_is_not_enabled_is_unavailable() {
    let (fake, plugins) = tmdb().await;
    let data = tempfile::tempdir().unwrap();

    let result = artwork_in(data.path(), plugins)
        .await
        .keep(&PluginId::from_static("other"), POSTER)
        .await;

    assert!(matches!(result, Err(Error::Unavailable(_))), "{result:?}");
    assert_eq!(fake.requests(), Vec::<String>::new());
}
