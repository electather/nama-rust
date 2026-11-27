use std::{
    collections::{BTreeMap, HashMap},
    io::Cursor,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use axum::{Router, body::Body, extract::Request, http::header};
use image::{ImageFormat, RgbImage};
use nama_plugin_api::settings::{SettingValue, Settings};
use nama_plugin_tmdb::{Endpoints, Tmdb, testing::API_KEY};
use tempfile::TempDir;

use super::{Artwork, Error, KEPT_DIR};
use crate::{
    db::Database,
    images::{BACKDROP_WIDTHS, ImageStore, MAX_SVG_BYTES, Resize},
    library::{
        contents::Contents,
        scripted::{ScriptedSource, SourcesAsked},
    },
    plugins::Registry,
    server_settings::Settings as ServerSettings,
};

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
        images: url.clone(),
        youtube_thumbnails: url,
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

    artwork
        .download(images.map(|image| (image, Vec::new())).collect())
        .await;
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
    let slow = ("tmdb".to_owned(), "/slow.jpg".to_owned());
    artwork.download(vec![(slow, Vec::new())]).await;
    while asked.load(Ordering::SeqCst) == 0 {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    let swept = tokio::time::timeout(Duration::from_secs(1), artwork.drop_unused(&contents)).await;

    assert!(swept.is_ok(), "the sweep waited for the download");
}

#[tokio::test]
async fn deleting_unused_images_while_their_copies_are_made_ahead_leaves_none_of_them() {
    let data = tempfile::tempdir().unwrap();
    let mut wide = Cursor::new(Vec::new());
    (RgbImage::new(2000, 1000).write_to(&mut wide, ImageFormat::Png)).unwrap();
    let wide = wide.into_inner();
    let artwork = artwork_on(&data, Router::new().fallback(|| async { wide })).await;
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let images = ServerSettings::new(Arc::clone(&db), data.path().to_owned()).images();
    let copies = Resize::each(&BACKDROP_WIDTHS, images.resizing());
    let image = ("tmdb".to_owned(), "/swept.jpg".to_owned());
    let sweep_once_stored = async {
        while artwork.store.list(KEPT_DIR).await.unwrap().is_empty() {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        artwork.drop_unused(&Contents::new(db)).await;
    };

    tokio::join!(artwork.download(vec![(image, copies)]), sweep_once_stored);

    let cached = walkdir::WalkDir::new(data.path().join("resized")).into_iter();
    let left: Vec<_> = (cached.filter_map(Result::ok))
        .filter(|entry| entry.file_type().is_file())
        .collect();
    assert!(left.is_empty(), "{left:?}");
    assert_eq!(artwork.thumbhashes().await.unwrap(), HashMap::new());
}

#[tokio::test]
async fn an_svg_is_served_as_an_image_whose_script_cannot_run() {
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
    assert_eq!(
        [
            &headers[header::CONTENT_TYPE],
            &headers[header::CONTENT_SECURITY_POLICY],
            &headers[header::CONTENT_DISPOSITION],
            &headers[header::X_CONTENT_TYPE_OPTIONS],
        ],
        [
            "image/svg+xml",
            "default-src 'none'; style-src 'unsafe-inline'; sandbox",
            "attachment",
            "nosniff",
        ]
    );
}

/// Artwork in `data` whose one title source, the Plugin `aa`, answers `path` with `bytes`.
async fn artwork_answering(data: &TempDir, path: &str, bytes: &[u8]) -> Artwork {
    let source = ScriptedSource::new("aa", &SourcesAsked::default()).with_image(path, bytes);
    let plugins = Registry::new();
    plugins.enable(Arc::new(source)).unwrap();
    let db = Database::open(data.path()).await.unwrap();
    Artwork::new(
        ImageStore::new(data.path(), Arc::new(db)),
        plugins.extension_points(),
    )
}

/// The body `artwork` serves for the kept image at `path` from the Plugin `aa`.
async fn served(artwork: &Artwork, path: &str) -> Result<Vec<u8>, Error> {
    let request = Request::get("/").body(Body::empty()).unwrap();
    let response = (artwork.serve_image((KEPT_DIR, "aa", path), None, request)).await?;
    let body = axum::body::to_bytes(response.into_body(), usize::MAX);
    Ok(body.await.unwrap().to_vec())
}

#[tokio::test]
async fn a_youtube_thumbnail_without_a_maxresdefault_is_kept_from_its_hqdefault() {
    let data = tempfile::tempdir().unwrap();
    let hq = "https://i.ytimg.com/vi/LjLamj-b0I8/hqdefault.jpg";
    let artwork = artwork_answering(&data, hq, b"\xff\xd8hq").await;

    let body = served(
        &artwork,
        "https://i.ytimg.com/vi/LjLamj-b0I8/maxresdefault.jpg",
    )
    .await;

    assert_eq!(body.unwrap(), b"\xff\xd8hq");
}

#[tokio::test]
async fn an_svg_from_a_title_source_is_stored_without_its_script() {
    let data = tempfile::tempdir().unwrap();
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" onload="alert(1)"><script>alert(2)</script><rect width="1" height="1"/></svg>"#;
    let artwork = artwork_answering(&data, "/logo.svg", svg).await;

    let body = String::from_utf8(served(&artwork, "/logo.svg").await.unwrap()).unwrap();

    assert!(body.contains("<rect"), "{body}");
    assert!(
        !body.contains("script") && !body.contains("onload"),
        "{body}"
    );
}

#[tokio::test]
async fn an_svg_over_the_size_cap_is_not_stored_but_one_at_it_is() {
    let svg = |len: usize| {
        let (open, close) = (r#"<svg xmlns="http://www.w3.org/2000/svg">"#, "</svg>");
        let padding = " ".repeat(len - open.len() - close.len());
        format!("{open}{padding}{close}")
    };
    let (at, over) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let at_cap = artwork_answering(&at, "/logo.svg", svg(MAX_SVG_BYTES).as_bytes()).await;
    let over_cap = artwork_answering(&over, "/logo.svg", svg(MAX_SVG_BYTES + 1).as_bytes()).await;

    let served_over = served(&over_cap, "/logo.svg").await;

    served(&at_cap, "/logo.svg").await.unwrap();
    assert!(
        matches!(served_over, Err(Error::NotFound)),
        "{served_over:?}"
    );
    assert_eq!(over_cap.store.list(KEPT_DIR).await.unwrap().len(), 0);
}

#[tokio::test]
async fn a_body_at_an_svg_path_that_is_not_svg_is_not_stored() {
    let data = tempfile::tempdir().unwrap();
    let artwork = artwork_answering(&data, "/logo.svg", b"<html><svg").await;

    let served = served(&artwork, "/logo.svg").await;

    assert!(matches!(served, Err(Error::NotFound)), "{served:?}");
    assert_eq!(artwork.store.list(KEPT_DIR).await.unwrap().len(), 0);
}

#[tokio::test]
async fn a_png_is_stored_as_the_title_source_gave_it() {
    let data = tempfile::tempdir().unwrap();
    let png = b"\x89PNG\r\n\x1a\n<svg><script/></svg>";
    let artwork = artwork_answering(&data, "/poster.png", png).await;

    assert_eq!(served(&artwork, "/poster.png").await.unwrap(), png);
}
