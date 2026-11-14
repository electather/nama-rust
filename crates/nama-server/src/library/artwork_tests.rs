use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use axum::{Router, body::Body, extract::Request, http::header};
use nama_plugin_api::settings::{SettingValue, Settings};
use nama_plugin_tmdb::{Endpoints, Tmdb, testing::API_KEY};
use tempfile::TempDir;

use super::Artwork;
use crate::{db::Database, images::ImageStore, library::contents::Contents, plugins::Registry};

/// How many image requests are running, and the most that ever were.
#[derive(Default)]
struct Running {
    now: AtomicUsize,
    most: AtomicUsize,
}

/// Answers an image request after a while, counted in `running` meanwhile.
async fn slow_image(running: Arc<Running>) -> &'static [u8] {
    let now = running.now.fetch_add(1, Ordering::SeqCst) + 1;
    running.most.fetch_max(now, Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(50)).await;
    running.now.fetch_sub(1, Ordering::SeqCst);
    b"\xff\xd8"
}

/// Artwork in `data`, through a TMDB whose images `images` answers.
async fn artwork_on(data: &TempDir, images: Router) -> Artwork {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(axum::serve(listener, images).into_future());
    let plugins = Registry::new();
    let tmdb = Tmdb::with_endpoints(Endpoints {
        api: url.clone(),
        images: url,
    });
    let key = BTreeMap::from([("api_key".to_owned(), SettingValue::Text(API_KEY.to_owned()))]);
    (plugins.enable_with_settings(Arc::new(tmdb), Settings::new(key))).unwrap();
    let db = Database::open(data.path()).await.unwrap();
    let store = ImageStore::new(data.path(), Arc::new(db));
    Artwork::new(store, plugins.extension_points())
}

#[tokio::test]
async fn no_more_than_two_images_download_at_once() {
    let running = Arc::new(Running::default());
    let counted = Arc::clone(&running);
    let slow_images = Router::new().fallback(move || slow_image(Arc::clone(&counted)));
    let data = tempfile::tempdir().unwrap();
    let artwork = artwork_on(&data, slow_images).await;
    let images = (0..5).map(|n| ("tmdb".to_owned(), format!("/{n}.jpg")));

    artwork.download(images.collect()).await;
    let _all_finished = artwork.downloads.acquire_many(2).await.unwrap();

    assert_eq!(running.most.load(Ordering::SeqCst), 2);
    let kept = artwork.store.list("artwork").await.unwrap();
    assert_eq!(kept.len(), 5);
}

/// The sweep runs at the end of a scan, under its Library's scan lock.
#[tokio::test]
async fn deleting_unused_images_does_not_wait_for_a_download() {
    let asked = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&asked);
    let never_answers = Router::new().fallback(move || {
        counted.fetch_add(1, Ordering::SeqCst);
        std::future::pending::<()>()
    });
    let data = tempfile::tempdir().unwrap();
    let artwork = artwork_on(&data, never_answers).await;
    let contents = Contents::new(Arc::new(Database::open(data.path()).await.unwrap()));
    artwork
        .download(vec![("tmdb".to_owned(), "/slow.jpg".to_owned())])
        .await;
    while asked.load(Ordering::SeqCst) == 0 {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    let swept = tokio::time::timeout(Duration::from_secs(1), artwork.drop_unused(&contents)).await;

    assert!(swept.is_ok(), "the sweep waited for the download");
}

#[tokio::test]
async fn an_svg_is_never_served_as_an_image() {
    let data = tempfile::tempdir().unwrap();
    let key = ["artwork", "tmdb", "/logo.svg"];
    let db = Database::open(data.path()).await.unwrap();
    let store = ImageStore::new(data.path(), Arc::new(db));
    store.write(&key, b"<svg><script/></svg>").await.unwrap();
    let artwork = Artwork::new(store, Registry::new().extension_points());

    let request = Request::get("/").body(Body::empty()).unwrap();
    let response = artwork
        .serve_image(("artwork", "tmdb", "/logo.svg"), None, request)
        .await
        .unwrap();

    let headers = response.headers();
    assert_eq!(headers[header::CONTENT_TYPE], "application/octet-stream");
    assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
}
