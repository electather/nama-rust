use std::{
    collections::{BTreeMap, HashSet},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use axum::{Router, body::Body, extract::Request, http::header};
use nama_plugin_api::{
    PluginId,
    settings::{SettingValue, Settings},
};
use nama_plugin_tmdb::{Endpoints, Tmdb, testing::API_KEY};
use tempfile::TempDir;

use super::{Artwork, Picked, pick};
use crate::{
    db::Database,
    images::ImageStore,
    library::{
        contents::{Contents, ImageRef},
        title_source::{self, ArtworkKind, Source},
    },
    plugins::Registry,
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

    artwork
        .download(images.collect(), &HashSet::new(), None)
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
    artwork
        .download(
            vec![("tmdb".to_owned(), "/slow.jpg".to_owned())],
            &HashSet::new(),
            None,
        )
        .await;
    while asked.load(Ordering::SeqCst) == 0 {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    let swept = tokio::time::timeout(Duration::from_secs(1), artwork.drop_unused(&contents)).await;

    assert!(swept.is_ok(), "the sweep waited for the download");
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

/// The source of the Plugin `plugin`.
fn source(plugin: &'static str) -> Source {
    Source {
        plugin: PluginId::from_static(plugin),
        namespace: plugin.to_owned(),
        looks_up: Vec::new(),
        answers: Vec::new(),
    }
}

/// A poster at `path`, in `language`, rated `rating` by `votes` people.
fn poster(
    path: &str,
    language: Option<&str>,
    rating: Option<f64>,
    votes: Option<u32>,
) -> title_source::Artwork {
    title_source::Artwork {
        kind: ArtworkKind::Poster,
        path: path.to_owned(),
        language: language.map(str::to_owned),
        rating,
        votes,
    }
}

/// The path of the poster [`pick`] picks from `lists`, in English.
fn picked_poster(lists: &[(Source, Vec<title_source::Artwork>)]) -> Option<String> {
    pick(lists, "en").poster.map(|poster| poster.path)
}

#[test]
fn the_language_asked_for_beats_the_source_order() {
    let lists = [
        (
            source("tmdb"),
            vec![poster("/fr.jpg", Some("fr"), Some(9.0), None)],
        ),
        (
            source("fanart"),
            vec![poster("/en.jpg", Some("en"), None, None)],
        ),
    ];
    let textless = [
        (
            source("tmdb"),
            vec![poster("/fr.jpg", Some("fr"), None, None)],
        ),
        (
            source("fanart"),
            vec![poster("/none.jpg", None, None, None)],
        ),
    ];

    assert_eq!(picked_poster(&lists).as_deref(), Some("/en.jpg"));
    assert_eq!(picked_poster(&textless).as_deref(), Some("/none.jpg"));
}

#[test]
fn the_source_order_beats_the_rating() {
    let lists = [
        (
            source("tmdb"),
            vec![poster("/low.jpg", Some("en"), Some(1.0), None)],
        ),
        (
            source("fanart"),
            vec![poster("/high.jpg", Some("en"), Some(9.0), None)],
        ),
    ];

    assert_eq!(picked_poster(&lists).as_deref(), Some("/low.jpg"));
}

#[test]
fn the_rating_then_the_votes_then_the_sources_own_order_decide() {
    let rated = [(
        source("tmdb"),
        vec![
            poster("/unrated.jpg", Some("en"), None, Some(90)),
            poster("/low.jpg", Some("en"), Some(5.0), Some(90)),
            poster("/high.jpg", Some("en"), Some(8.0), Some(1)),
        ],
    )];
    let voted = [(
        source("tmdb"),
        vec![
            poster("/few.jpg", Some("en"), Some(8.0), Some(1)),
            poster("/many.jpg", Some("en"), Some(8.0), Some(40)),
            poster("/many-too.jpg", Some("en"), Some(8.0), Some(40)),
        ],
    )];

    assert_eq!(picked_poster(&rated).as_deref(), Some("/high.jpg"));
    assert_eq!(picked_poster(&voted).as_deref(), Some("/many.jpg"));
}

#[test]
fn each_kind_is_picked_from_its_own_sources_list_and_names_its_plugin() {
    let logo = title_source::Artwork {
        kind: ArtworkKind::Logo,
        ..poster("/logo.png", Some("en"), None, None)
    };
    let clear_art = title_source::Artwork {
        kind: ArtworkKind::Other,
        ..poster("/clear.png", Some("en"), None, None)
    };
    let lists = [
        (
            source("tmdb"),
            vec![poster("/poster.jpg", None, None, None)],
        ),
        (source("fanart"), vec![logo, clear_art]),
    ];

    let picked = pick(&lists, "en");

    let image = |plugin, path: &str| {
        Some(ImageRef {
            plugin: PluginId::from_static(plugin),
            path: path.to_owned(),
        })
    };
    assert_eq!(
        picked,
        Picked {
            poster: image("tmdb", "/poster.jpg"),
            backdrop: None,
            titled_backdrop: None,
            logo: image("fanart", "/logo.png"),
        }
    );
}

/// A backdrop at `path`, in `language`, rated `rating`.
fn backdrop(path: &str, language: Option<&str>, rating: Option<f64>) -> title_source::Artwork {
    title_source::Artwork {
        kind: ArtworkKind::Backdrop,
        ..poster(path, language, rating, None)
    }
}

/// The path of the backdrop [`pick`] picks from `lists`, in English.
fn picked_backdrop(lists: &[(Source, Vec<title_source::Artwork>)]) -> Option<String> {
    pick(lists, "en").backdrop.map(|backdrop| backdrop.path)
}

#[test]
fn a_backdrop_without_text_beats_one_in_the_language_asked_for() {
    let lists = [(
        source("tmdb"),
        vec![
            backdrop("/en.jpg", Some("en"), Some(9.0)),
            backdrop("/none.jpg", None, None),
        ],
    )];

    assert_eq!(picked_backdrop(&lists).as_deref(), Some("/none.jpg"));
}

#[test]
fn a_poster_in_the_language_asked_for_still_beats_one_without_text() {
    let lists = [(
        source("tmdb"),
        vec![
            poster("/none.jpg", None, Some(9.0), None),
            poster("/en.jpg", Some("en"), None, None),
        ],
    )];

    assert_eq!(picked_poster(&lists).as_deref(), Some("/en.jpg"));
}

#[test]
fn backdrops_in_other_languages_only_are_picked_by_source_then_rating() {
    let lists = [
        (
            source("tmdb"),
            vec![
                backdrop("/fr-low.jpg", Some("fr"), Some(1.0)),
                backdrop("/de-high.jpg", Some("de"), Some(9.0)),
            ],
        ),
        (
            source("fanart"),
            vec![backdrop("/fr-top.jpg", Some("fr"), Some(10.0))],
        ),
    ];
    let english = [
        (
            source("tmdb"),
            vec![backdrop("/fr.jpg", Some("fr"), Some(9.0))],
        ),
        (
            source("fanart"),
            vec![backdrop("/en.jpg", Some("en"), None)],
        ),
    ];

    assert_eq!(picked_backdrop(&lists).as_deref(), Some("/de-high.jpg"));
    assert_eq!(picked_backdrop(&english).as_deref(), Some("/en.jpg"));
}

/// The path of the titled Backdrop [`pick`] picks from `lists`, in English.
fn picked_titled_backdrop(lists: &[(Source, Vec<title_source::Artwork>)]) -> Option<String> {
    pick(lists, "en")
        .titled_backdrop
        .map(|backdrop| backdrop.path)
}

#[test]
fn the_titled_backdrop_is_the_one_in_the_language_asked_for_and_the_backdrop_stays_textless() {
    let lists = [(
        source("tmdb"),
        vec![
            backdrop("/none.jpg", None, None),
            backdrop("/en.jpg", Some("en"), None),
            backdrop("/fr.jpg", Some("fr"), Some(9.0)),
        ],
    )];

    assert_eq!(picked_titled_backdrop(&lists).as_deref(), Some("/en.jpg"));
    assert_eq!(picked_backdrop(&lists).as_deref(), Some("/none.jpg"));
}

#[test]
fn no_backdrop_in_the_language_asked_for_picks_no_titled_backdrop() {
    let lists = [(
        source("tmdb"),
        vec![
            backdrop("/none.jpg", None, None),
            backdrop("/fr.jpg", Some("fr"), Some(9.0)),
            poster("/en.jpg", Some("en"), None, None),
        ],
    )];

    assert_eq!(picked_titled_backdrop(&lists), None);
}

#[test]
fn titled_backdrops_rank_by_source_then_rating_then_votes() {
    let voted = |path, votes| title_source::Artwork {
        votes: Some(votes),
        ..backdrop(path, Some("en"), Some(8.0))
    };
    let by_source = [
        (
            source("tmdb"),
            vec![backdrop("/low.jpg", Some("en"), Some(1.0))],
        ),
        (
            source("fanart"),
            vec![backdrop("/high.jpg", Some("en"), Some(9.0))],
        ),
    ];
    let by_rating = [(
        source("tmdb"),
        vec![
            backdrop("/low.jpg", Some("en"), Some(1.0)),
            backdrop("/high.jpg", Some("en"), Some(9.0)),
        ],
    )];
    let by_votes = [(
        source("tmdb"),
        vec![voted("/few.jpg", 1), voted("/many.jpg", 40)],
    )];

    assert_eq!(
        picked_titled_backdrop(&by_source).as_deref(),
        Some("/low.jpg")
    );
    assert_eq!(
        picked_titled_backdrop(&by_rating).as_deref(),
        Some("/high.jpg")
    );
    assert_eq!(
        picked_titled_backdrop(&by_votes).as_deref(),
        Some("/many.jpg")
    );
}
