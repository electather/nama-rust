use std::{sync::Arc, time::Duration};

use axum::{
    Router,
    body::{Body, to_bytes},
    extract::Request,
    http::StatusCode,
};
use nama_plugin_api::PluginId;
use tempfile::TempDir;
use tower::ServiceExt;

use super::{Artwork, KEPT_DIR, key, router, url};
use crate::{
    db::Database,
    images::ImageStore,
    library::{
        Metadata, Trailer,
        contents::Contents,
        fixture::{MatchedMovie, tmdb_movie},
        scripted::{ScriptedSource, SourcesAsked},
    },
    plugins::Registry,
    server_settings::Settings as ServerSettings,
};

const THUMBNAIL: &str = "https://i.ytimg.com/vi/LjLamj-b0I8/maxresdefault.jpg";

/// A Library whose one Movie, Matched through the scripted Plugin `aa`, has a Trailer with the
/// thumbnail [`THUMBNAIL`], and the artwork route over it.
struct Fixture {
    artwork: Artwork,
    contents: Contents,
    library: i64,
    route: Router,
    asked: SourcesAsked,
    _dirs: (TempDir, TempDir),
}

impl Fixture {
    /// One whose title source answers each path of `answered` with its bytes.
    async fn new(answered: &[(&str, &[u8])]) -> Self {
        let (data, media) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let asked = SourcesAsked::default();
        let source = (answered.iter()).fold(ScriptedSource::new("aa", &asked), |source, image| {
            source.with_image(image.0, image.1)
        });
        let plugins = Registry::new();
        plugins.enable(Arc::new(source)).unwrap();
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let alien = Metadata {
            plugin: PluginId::from_static("aa"),
            trailers: vec![Trailer {
                name: "Official Trailer".to_owned(),
                url: "https://www.youtube.com/watch?v=LjLamj-b0I8".to_owned(),
                thumbnail: Some(THUMBNAIL.to_owned()),
            }],
            ..tmdb_movie(348, "Alien")
        };
        let library = MatchedMovie::new(&db, media.path(), alien).await.library;
        let store = ImageStore::new(data.path(), Arc::clone(&db));
        let artwork = Artwork::new(store, plugins.extension_points());
        let contents = Contents::new(Arc::clone(&db));
        let images = ServerSettings::new(db, data.path().to_owned()).images();
        let route = router(artwork.clone(), contents.clone(), images);
        Self {
            artwork,
            contents,
            library,
            route,
            asked,
            _dirs: (data, media),
        }
    }

    /// Starts what the Library's Match refresh downloads, and waits until `landed` says it has.
    async fn download_missing(&self, landed: impl AsyncFn(&Self) -> bool) {
        (self.artwork).download_missing(self.contents.clone(), self.library);
        // Not `Artwork::settled`: every lib test shares its tracker, and one test reopening it
        // between another's close and wait leaves that one waiting for good.
        let polled = async {
            while !landed(self).await {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        };
        let landed_in_time = tokio::time::timeout(Duration::from_secs(10), polled).await;
        landed_in_time.expect("the downloads never landed");
    }

    /// Whether the store keeps [`THUMBNAIL`].
    async fn kept(&self) -> bool {
        let key = key(KEPT_DIR, "aa", THUMBNAIL);
        self.artwork.store.contains(&key).await.unwrap()
    }

    /// The status and body a title page's request for [`THUMBNAIL`] gets.
    async fn view(&self) -> (StatusCode, Vec<u8>) {
        let request = Request::get(url("aa", THUMBNAIL)).body(Body::empty());
        let response = (self.route.clone())
            .oneshot(request.unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, body.to_vec())
    }

    /// How many images the title source was asked for.
    fn image_calls(&self) -> usize {
        let lines = self.asked.lines().into_iter();
        lines.filter(|line| line.starts_with("aa image ")).count()
    }
}

#[tokio::test]
async fn a_trailer_thumbnail_not_kept_yet_is_not_found_and_not_fetched_on_view() {
    let fx = Fixture::new(&[(THUMBNAIL, b"\xff\xd8maxres")]).await;

    let (status, _) = fx.view().await;

    assert_eq!((status, fx.image_calls()), (StatusCode::NOT_FOUND, 0));
}

#[tokio::test]
async fn a_trailer_thumbnail_the_match_refresh_kept_is_served_without_fetching_again() {
    let fx = Fixture::new(&[(THUMBNAIL, b"\xff\xd8maxres")]).await;
    fx.download_missing(Fixture::kept).await;
    let fetched = fx.image_calls();

    let viewed = fx.view().await;

    assert_eq!(viewed, (StatusCode::OK, b"\xff\xd8maxres".to_vec()));
    assert_eq!((fetched, fx.image_calls()), (1, 1));
}

#[tokio::test]
async fn a_trailer_thumbnail_its_host_has_in_neither_size_is_never_fetched_on_view() {
    let fx = Fixture::new(&[]).await;
    fx.download_missing(async |fixture: &Fixture| fixture.image_calls() == 2)
        .await;
    let fetched = fx.image_calls();

    let viewed = [fx.view().await.0, fx.view().await.0];

    assert_eq!(viewed, [StatusCode::NOT_FOUND; 2]);
    // `maxresdefault.jpg`, then `hqdefault.jpg`, both by the refresh alone.
    assert_eq!((fetched, fx.image_calls()), (2, 2));
}
