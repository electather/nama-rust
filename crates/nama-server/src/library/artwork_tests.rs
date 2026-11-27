use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    io::Cursor,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use axum::{Router, body::Body, extract::Request, http::header};
use image::{ImageFormat, RgbImage};
use nama_plugin_api::{
    PluginId,
    settings::{SettingValue, Settings},
};
use nama_plugin_tmdb::{Endpoints, Tmdb, testing::API_KEY};
use tempfile::TempDir;
use tower::ServiceExt;

use super::{Artwork, Error, KEPT_DIR};
use crate::{
    db::Database,
    images::{BACKDROP_WIDTHS, ImageStore, MAX_SVG_BYTES, Resize},
    library::{
        CustomImages, FoundImages, LocalImages, Metadata, Role,
        contents::{Contents, ImageRef, LocalImageFile, LocalRole},
        custom_images,
        fixture::{MatchedMovie, tmdb_movie},
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

    artwork.download(images.collect(), &HashMap::new()).await;
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
        .download(
            vec![("tmdb".to_owned(), "/slow.jpg".to_owned())],
            &HashMap::new(),
        )
        .await;
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
    let copies = Resize::each(&BACKDROP_WIDTHS, &images);
    let image = ("tmdb".to_owned(), "/swept.jpg".to_owned());
    let ahead = HashMap::from([(image.clone(), copies.as_slice())]);
    let sweep_once_stored = async {
        while artwork.store.list(KEPT_DIR).await.unwrap().is_empty() {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        artwork.drop_unused(&Contents::new(db)).await;
    };

    tokio::join!(artwork.download(vec![image], &ahead), sweep_once_stored);

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

#[tokio::test]
async fn a_poster_whose_title_shows_the_one_beside_its_media_in_its_place_gets_no_copy() {
    let data = tempfile::tempdir().unwrap();
    let media = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let store = ImageStore::new(data.path(), Arc::clone(&db));
    let contents = Contents::new(Arc::clone(&db));
    let poster = |path: &str| ImageRef {
        plugin: PluginId::from_static("tmdb"),
        path: path.to_owned(),
    };
    let with_poster = |tmdb, title, path| Metadata {
        poster: Some(poster(path)),
        ..tmdb_movie(tmdb, title)
    };
    let alien = MatchedMovie::new(&db, media.path(), with_poster(1, "Alien", "/hidden.jpg")).await;
    let dune_path = media.path().canonicalize().unwrap().join("Dune.mkv");
    let dune = with_poster(2, "Dune", "/shown.jpg");
    MatchedMovie::store(&contents, alien.library, dune_path.to_str().unwrap(), dune).await;
    let beside = media.path().join("poster.png");
    std::fs::write(&beside, png()).unwrap();
    let file = LocalImageFile {
        path: beside.to_str().unwrap().to_owned(),
        size: 1,
        modified_ns: 0,
    };
    let found = FoundImages::from([((alien.version, LocalRole::Title(Role::Poster)), file)]);
    let local_images = LocalImages::new(Arc::clone(&db), store.clone());
    (local_images
        .keep(alien.library, &[alien.version], &found)
        .await)
        .unwrap();
    let artwork = Artwork::new(store, Registry::new().extension_points());
    let artwork = resizing_ahead(artwork, &db, &data, local_images);

    let (shown, _) = artwork
        .shown_downloaded(&contents, alien.library)
        .await
        .unwrap();

    assert_eq!(
        shown,
        HashSet::from([("tmdb".to_owned(), "/shown.jpg".to_owned())])
    );
}

/// A 2 by 3 pixel PNG.
fn png() -> Vec<u8> {
    let mut png = Cursor::new(Vec::new());
    (RgbImage::new(2, 3).write_to(&mut png, ImageFormat::Png)).unwrap();
    png.into_inner()
}

/// A Library in `media`, recorded in `db`, of Alien, Dune and Heat, each with its downloaded
/// Backdrop `/<title>.jpg` and the titled Backdrop `/titled.jpg`, where Alien shows a Backdrop
/// beside its media in place of its own and Heat a Custom Backdrop. Returns the Library's id and
/// its images beside the media.
async fn alien_and_heat_showing_other_backdrops(
    db: &Arc<Database>,
    data: &TempDir,
    media: &Path,
    store: &ImageStore,
) -> (i64, LocalImages) {
    let image = |path: &str| {
        Some(ImageRef {
            plugin: PluginId::from_static("tmdb"),
            path: path.to_owned(),
        })
    };
    let with_backdrops = |tmdb, title: &str| Metadata {
        backdrop: image(&format!("/{}.jpg", title.to_lowercase())),
        titled_backdrop: image("/titled.jpg"),
        ..tmdb_movie(tmdb, title)
    };
    let alien = MatchedMovie::new(db, media, with_backdrops(1, "Alien")).await;
    let contents = Contents::new(Arc::clone(db));
    for (tmdb, title) in [(2, "Dune"), (3, "Heat")] {
        let path = media.canonicalize().unwrap().join(format!("{title}.mkv"));
        let matched = with_backdrops(tmdb, title);
        MatchedMovie::store(&contents, alien.library, path.to_str().unwrap(), matched).await;
    }
    let beside = media.join("fanart.png");
    std::fs::write(&beside, png()).unwrap();
    let file = LocalImageFile {
        path: beside.to_str().unwrap().to_owned(),
        size: 1,
        modified_ns: 0,
    };
    let found = FoundImages::from([((alien.version, LocalRole::Title(Role::Backdrop)), file)]);
    let local_images = LocalImages::new(Arc::clone(db), store.clone());
    (local_images
        .keep(alien.library, &[alien.version], &found)
        .await)
        .unwrap();
    let custom = CustomImages::new(Arc::clone(db), store.clone());
    let images = ServerSettings::new(Arc::clone(db), data.path().to_owned()).images();
    let artwork = Artwork::new(store.clone(), Registry::new().extension_points());
    let upload = Request::put("/custom-images/movie/tmdb-3/backdrop")
        .body(Body::from(png()))
        .unwrap();
    let uploaded = (custom_images::router(custom, artwork, images))
        .oneshot(upload)
        .await
        .unwrap();
    assert_eq!(uploaded.status(), 204);
    (alien.library, local_images)
}

/// `artwork`, resizing ahead as the default image settings in `db` say.
fn resizing_ahead(
    artwork: Artwork,
    db: &Arc<Database>,
    data: &TempDir,
    local_images: LocalImages,
) -> Artwork {
    let images = ServerSettings::new(Arc::clone(db), data.path().to_owned()).images();
    let custom_images = CustomImages::new(Arc::clone(db), artwork.store.clone());
    artwork.resizing_ahead(images, custom_images, local_images)
}

#[tokio::test]
async fn a_backdrop_whose_title_shows_a_custom_image_or_one_beside_its_media_in_its_place_is_not_shown()
 {
    let data = tempfile::tempdir().unwrap();
    let media = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let store = ImageStore::new(data.path(), Arc::clone(&db));
    let (library, local_images) =
        alien_and_heat_showing_other_backdrops(&db, &data, media.path(), &store).await;
    let artwork = Artwork::new(store, Registry::new().extension_points());
    let artwork = resizing_ahead(artwork, &db, &data, local_images);

    let (_, shown) = (artwork.shown_downloaded(&Contents::new(Arc::clone(&db)), library))
        .await
        .unwrap();

    assert_eq!(
        shown,
        HashSet::from([("tmdb".to_owned(), "/dune.jpg".to_owned())])
    );
}

#[tokio::test]
async fn only_the_backdrops_titles_show_and_every_titled_backdrop_get_copies_made_ahead() {
    let data = tempfile::tempdir().unwrap();
    let media = tempfile::tempdir().unwrap();
    let images = Router::new().fallback(|| async { png() });
    let artwork = artwork_on(&data, images).await;
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let (library, local_images) =
        alien_and_heat_showing_other_backdrops(&db, &data, media.path(), &artwork.store).await;
    let artwork = resizing_ahead(artwork, &db, &data, local_images);

    artwork.download_missing(Contents::new(Arc::clone(&db)), library);
    Artwork::settled().await;

    let listed = |dir: &str| {
        let entries = std::fs::read_dir(data.path().join(dir)).unwrap();
        let mut names: Vec<_> = (entries.map(|entry| entry.unwrap().file_name()))
            .map(|name| name.into_string().unwrap())
            .collect();
        names.sort();
        names
    };
    let kept = [
        "%2Falien.jpg",
        "%2Fdune.jpg",
        "%2Fheat.jpg",
        "%2Ftitled.jpg",
    ];
    assert_eq!(listed("artwork/tmdb"), kept);
    assert_eq!(
        listed("resized/artwork/tmdb"),
        ["%2Fdune.jpg", "%2Ftitled.jpg"]
    );
}

#[tokio::test]
async fn posters_of_every_source_get_copies_ahead_first_until_the_cache_is_nearly_full_and_rescans_make_none()
 {
    let data = tempfile::tempdir().unwrap();
    let media = tempfile::tempdir().unwrap();
    let mut wide = Cursor::new(Vec::new());
    (RgbImage::new(1000, 1500).write_to(&mut wide, ImageFormat::Png)).unwrap();
    let wide = wide.into_inner();
    // A Backdrop beside the media, which isn't waiting on a download, still waits for Posters.
    let beside = media.path().join("fanart.png");
    std::fs::write(&beside, &wide).unwrap();
    let artwork = artwork_on(&data, Router::new().fallback(|| async { wide })).await;
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    // Named so the Library lists its Backdrops before its Posters.
    let image = |path: String| {
        Some(ImageRef {
            plugin: PluginId::from_static("tmdb"),
            path,
        })
    };
    let with_artwork = |tmdb, title: &str| Metadata {
        poster: image(format!("/z-{title}.jpg")),
        backdrop: image(format!("/a-{title}.jpg")),
        ..tmdb_movie(tmdb, title)
    };
    let alien = MatchedMovie::new(&db, media.path(), with_artwork(1, "alien")).await;
    let contents = Contents::new(Arc::clone(&db));
    let dune_path = media.path().canonicalize().unwrap().join("Dune.mkv");
    let dune = with_artwork(2, "dune");
    MatchedMovie::store(&contents, alien.library, dune_path.to_str().unwrap(), dune).await;
    let settings = ServerSettings::new(Arc::clone(&db), data.path().to_owned());
    settings.change_images(true, Some(1)).await.unwrap();
    // One byte under 80% of 1 MiB, so the first copy made is the last made ahead.
    let cache = data.path().join("resized");
    std::fs::create_dir_all(&cache).unwrap();
    let filler = std::fs::File::create(cache.join("filler")).unwrap();
    filler.set_len(1024 * 1024 / 100 * 80 - 1).unwrap();
    let custom_images = CustomImages::new(Arc::clone(&db), artwork.store.clone());
    let local_images = LocalImages::new(Arc::clone(&db), artwork.store.clone());
    let backdrop = LocalImageFile {
        path: beside.to_str().unwrap().to_owned(),
        size: 1,
        modified_ns: 0,
    };
    let found = FoundImages::from([((alien.version, LocalRole::Title(Role::Backdrop)), backdrop)]);
    (local_images
        .keep(alien.library, &[alien.version], &found)
        .await)
        .unwrap();
    let artwork = artwork.resizing_ahead(settings.images(), custom_images, local_images);

    artwork.download_missing(contents.clone(), alien.library);
    Artwork::settled().await;
    let cached = || {
        let files = walkdir::WalkDir::new(&cache)
            .into_iter()
            .map(Result::unwrap);
        (files.filter(|file| file.file_type().is_file()))
            .map(walkdir::DirEntry::into_path)
            .collect::<BTreeSet<_>>()
    };
    let first = cached();
    artwork.download_missing(contents, alien.library);
    Artwork::settled().await;

    let listed = |dir: &Path| -> Vec<_> {
        (std::fs::read_dir(dir).unwrap())
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect()
    };
    let top = listed(&cache);
    assert_eq!(top.len(), 2, "only the filler and artwork: {top:?}");
    let copied = listed(&cache.join("artwork/tmdb"));
    assert!(
        matches!(copied.as_slice(), [poster] if poster.starts_with("%2Fz-")),
        "{copied:?}"
    );
    assert!(first.contains(&cache.join("filler")));
    assert_eq!(cached(), first, "the second scan made and evicted nothing");
}

// The hang needs one call to reopen between another's `close` and `wait`, so it takes threads and
// many rounds to show.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn settled_calls_made_at_once_all_finish() {
    let rounds = async {
        for _ in 0..500 {
            settle_four_at_once().await;
        }
    };
    tokio::time::timeout(Duration::from_secs(60), rounds)
        .await
        .expect("a settled call hung");
}

async fn settle_four_at_once() {
    let working = super::background_tasks().token();
    let calls: Vec<_> = (0..4).map(|_| tokio::spawn(Artwork::settled())).collect();
    drop(working);
    for call in calls {
        call.await.unwrap();
    }
}
