//! The S3 Installed Plugin as the image store (ADR 0052), end to end over a real socket, against
//! an in-process fake S3 bucket: switching to it from local disk copies the originals across, it
//! then keeps every new original, and a bucket that refuses writes is never switched to.

#[path = "support/fake_s3.rs"]
mod fake_s3;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/http.rs"]
mod http;
#[path = "support/refused.rs"]
mod refused;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::{collections::BTreeMap, fs, io::Cursor, path::Path, time::Duration};

use connectrpc::{ConnectError, ErrorCode, client::HttpClient};
use fake_s3::{FakeS3, S3};
use http::{Reply, fetch_with, send};
use http_body_util::Full;
use hyper::{Request, body::Bytes};
use image::{ImageFormat, RgbImage};
use nama_proto::nama::v1::{
    BrowseServiceClient, ExternalId, GetImageSettingsRequest, GetImageSettingsResponse,
    GetMovieRequest, LibraryKind, ListPluginsRequest, Movie, PluginServiceClient, RefusalReason,
    ServerServiceClient, SwitchImageStoreRequest, SwitchImageStoreResponse,
};
use refused::assert_refused;
use server::{TestServer, WAIT, tmdb::ALIEN};

/// TMDB Matches it to Alien.
const ALIEN_1080P: &str = "Alien (1979)/Alien (1979) - 1080p.mkv";

/// Where Alien's Custom Poster is uploaded to.
fn alien_custom_poster() -> String {
    format!("/custom-images/movie/tmdb-{ALIEN}/poster")
}

/// The folders of the data directory local disk keeps originals in, as `ORIGINAL_DIRS` in
/// `src/images.rs` names them.
const ORIGINAL_DIRS: [&str; 4] = ["artwork", "artwork-cache", "custom-images", "plugin-logos"];

fn settings_client(server: &TestServer) -> ServerServiceClient<HttpClient> {
    ServerServiceClient::new(HttpClient::plaintext(), server.client_config())
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn image_settings(server: &TestServer) -> GetImageSettingsResponse {
    settings_client(server)
        .get_image_settings(GetImageSettingsRequest::default())
        .await
        .unwrap()
        .into_owned()
}

async fn switch(
    server: &TestServer,
    plugin_id: &str,
) -> Result<SwitchImageStoreResponse, ConnectError> {
    let request = SwitchImageStoreRequest {
        plugin_id: plugin_id.to_owned(),
        ..Default::default()
    };
    let response = settings_client(server).switch_image_store(request).await?;
    Ok(response.into_owned())
}

/// Waits until the switch to `plugin_id` has finished and it keeps the images, failing the test
/// if the switch failed or after [`WAIT`].
async fn until_switched(server: &TestServer, plugin_id: &str) {
    let waited = tokio::time::timeout(WAIT, async {
        loop {
            let settings = image_settings(server).await;
            if let Some(switching) = settings.switching.as_option() {
                assert_eq!(switching.failure, "", "{switching:?}");
            } else if settings.settings.store_plugin_id == plugin_id {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    assert!(waited.is_ok(), "the switch to {plugin_id:?} never finished");
}

/// Waits until `condition` holds, failing the test with `what` after [`WAIT`].
async fn until(what: &str, mut condition: impl FnMut() -> bool) {
    let waited = tokio::time::timeout(WAIT, async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    assert!(waited.is_ok(), "never {what}");
}

/// A Server with the TMDB Plugin on, and the S3 Plugin enabled against a fake bucket but not yet
/// keeping the images.
struct Fixture {
    server: TestServer,
    s3: FakeS3,
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    async fn start() -> Self {
        let server = TestServer::builder().tmdb().start().await;
        let s3 = FakeS3::start().await;
        s3.enable_on(&server).await;
        Self { server, s3 }
    }

    /// Scans a new Library holding Alien until it is Matched, and returns the Library's id and
    /// Alien as `GetMovie` gives it there.
    async fn match_alien(&self) -> (i64, Movie) {
        let library = self
            .server
            .matched_library(LibraryKind::LIBRARY_KIND_MOVIES, &[ALIEN_1080P])
            .await;
        (library.id, self.alien(library.id).await)
    }

    /// Alien as `GetMovie` gives it in the Library `library`.
    async fn alien(&self, library: i64) -> Movie {
        BrowseServiceClient::new(HttpClient::plaintext(), self.server.client_config())
            .get_movie(GetMovieRequest {
                library_id: library,
                movie_id: ExternalId {
                    namespace: "tmdb".to_owned(),
                    value: ALIEN.to_string(),
                    ..Default::default()
                }
                .into(),
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned()
            .movie
            .into_option()
            .unwrap()
    }

    /// The URLs of the Poster, Backdrop and Title Logo `movie` was Matched with.
    fn artwork_urls(movie: &Movie) -> [String; 3] {
        [
            &movie.poster_url,
            &movie.backdrop_url,
            &movie.title_logo_url,
        ]
        .map(|url| url.clone().unwrap())
    }

    /// Uploads `bytes` as Alien's Custom Poster as the Admin, and returns its URL from `GetMovie`.
    async fn upload_custom_poster(&self, library: i64, bytes: &[u8]) -> String {
        let body = Full::new(Bytes::copy_from_slice(bytes));
        let request = Request::put(alien_custom_poster()).body(body).unwrap();
        let uploaded = send(&self.server, request, Some(&self.server.admin_token)).await;
        assert_eq!(uploaded.status, 204);
        let movie = self.alien(library).await;
        assert!(movie.custom_poster);
        movie.poster_url.unwrap()
    }

    /// Where the Admin's Plugin list has the logo of the Plugin `id`, which listing it stores.
    async fn logo_url(&self, id: &str) -> String {
        let listed = PluginServiceClient::new(HttpClient::plaintext(), self.server.client_config())
            .list_plugins(ListPluginsRequest::default())
            .await
            .unwrap()
            .into_owned()
            .summaries;
        let plugin = listed.into_iter().find(|plugin| plugin.id == id);
        plugin.unwrap().logo_url
    }

    /// The image at `url`, fetched as the Admin with `headers`.
    async fn fetch(&self, url: &str, headers: &[(&str, &str)]) -> Reply {
        fetch_with(&self.server, url, Some(&self.server.admin_token), headers).await
    }

    /// The originals local disk keeps in the data directory, by their key in the store.
    fn on_disk(&self) -> BTreeMap<String, Vec<u8>> {
        let data_dir = self.server.data_dir();
        ORIGINAL_DIRS
            .iter()
            .flat_map(|dir| walkdir::WalkDir::new(data_dir.join(dir)))
            // A folder not made yet holds no originals.
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
            // A dotfile is a write still under way (`synced_beside` in `src/images.rs`): no key
            // names one, and its rename can take it away before it is read.
            .filter(|entry| !entry.file_name().as_encoded_bytes().starts_with(b"."))
            .map(|file| {
                let bytes = fs::read(file.path()).unwrap();
                (key_in(data_dir, file.path()), bytes)
            })
            .collect()
    }
}

/// The store's key for the file at `path` in `data_dir`, its parts joined with `/`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn key_in(data_dir: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(data_dir).unwrap();
    let parts: Vec<_> = relative.iter().map(|part| part.to_str().unwrap()).collect();
    parts.join("/")
}

/// The store's key for the image served at `url`: the path after `/media/`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn key_of(url: &str) -> String {
    url.strip_prefix("/media/").unwrap().to_owned()
}

/// A 2 by 3 PNG.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn png() -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    RgbImage::new(2, 3)
        .write_to(&mut bytes, ImageFormat::Png)
        .unwrap();
    bytes.into_inner()
}

#[tokio::test]
async fn switching_from_local_disk_copies_each_original_into_the_bucket_counting_them() {
    let fx = Fixture::start().await;
    let (library, movie) = fx.match_alien().await;
    let artwork = Fixture::artwork_urls(&movie);
    // Once the scan's downloads are done, so none is put in the bucket during the switch.
    until("kept Alien's artwork on local disk", || {
        let on_disk = fx.on_disk();
        artwork.iter().all(|url| on_disk.contains_key(&key_of(url)))
    })
    .await;
    let custom_poster = fx.upload_custom_poster(library, &png()).await;
    let logo = fx.logo_url(S3).await;
    let urls = [artwork[0].clone(), custom_poster, logo];
    let mut served_before = Vec::new();
    for url in &urls {
        served_before.push(fx.fetch(url, &[]).await);
    }
    let originals = fx.on_disk();
    for url in &urls {
        assert!(originals.contains_key(&key_of(url)), "{url}");
    }
    fx.s3.hold_writes();

    let started = switch(&fx.server, S3).await.unwrap();

    let switching = started.switching.into_option().unwrap();
    assert_eq!(switching.to_plugin_id, S3);
    // Each original is held as it is put in the bucket, so every count is seen. A switch that
    // finishes clears its progress, so the last seen is one short of the total, and the bucket
    // then holding every original shows the count reached it.
    for copied in 0..originals.len() {
        until("got the next original", || fx.s3.held() > copied).await;
        let progress = image_settings(&fx.server).await.switching.into_option();
        let progress = progress.unwrap();
        assert_eq!(
            (progress.copied, progress.total),
            (
                u32::try_from(copied).unwrap(),
                u32::try_from(originals.len()).unwrap()
            ),
            "{progress:?}"
        );
        fx.s3.release_one();
    }
    until_switched(&fx.server, S3).await;
    assert_eq!(fx.s3.objects(), originals);
    assert_eq!(
        image_settings(&fx.server).await.settings.store_plugin_id,
        S3
    );
    // So what is served now can only come from the bucket.
    for dir in ORIGINAL_DIRS {
        let _: Result<(), _> = fs::remove_dir_all(fx.server.data_dir().join(dir));
    }
    for (url, before) in urls.iter().zip(served_before) {
        let after = fx.fetch(url, &[]).await;
        assert_eq!(before.status, 200, "{url}");
        assert_eq!(after.status, 200, "{url}");
        assert_eq!(after.body, before.body, "{url}");
        assert_eq!(after.header("etag"), before.header("etag"), "{url}");
    }
}

#[tokio::test]
async fn with_s3_as_the_store_new_artwork_custom_images_and_logos_go_in_the_bucket_alone() {
    let fx = Fixture::start().await;
    switch(&fx.server, S3).await.unwrap();
    until_switched(&fx.server, S3).await;
    // The S3 Plugin's own logo, stored as it was set up, was copied across and stays behind.
    let left_on_disk = fx.on_disk();

    let (library, movie) = fx.match_alien().await;
    let artwork = Fixture::artwork_urls(&movie);
    until("put Alien's artwork in the bucket", || {
        let objects = fx.s3.objects();
        artwork.iter().all(|url| objects.contains_key(&key_of(url)))
    })
    .await;
    let uploaded = png();
    let custom_poster = fx.upload_custom_poster(library, &uploaded).await;
    // A Plugin not set up, whose logo the Plugin list stores first.
    let logo = fx.logo_url("tvdb").await;

    let objects = fx.s3.objects();
    assert_eq!(objects.get(&key_of(&custom_poster)), Some(&uploaded));
    assert!(
        objects.contains_key(&key_of(&logo)),
        "{logo} isn't in {objects:?}"
    );
    assert_eq!(fx.on_disk(), left_on_disk);
    for url in [&artwork[0], &custom_poster, &logo] {
        assert!(!left_on_disk.contains_key(&key_of(url)), "{url}");
        let served = fx.fetch(url, &[]).await;
        let etag = served.header("etag");
        assert_eq!(served.status, 200, "{url}");
        assert_eq!(Some(&served.body), objects.get(&key_of(url)), "{url}");
        assert!(!etag.is_empty(), "{url}");
        let cached = fx.fetch(url, &[("if-none-match", etag)]).await;
        assert_eq!(cached.status, 304, "{url}");
        assert_eq!(cached.header("etag"), etag, "{url}");
    }
}

#[tokio::test]
async fn switching_to_a_bucket_that_refuses_writes_is_refused_with_the_reason_changing_nothing() {
    let fx = Fixture::start().await;
    let (_, movie) = fx.match_alien().await;
    let poster = movie.poster_url.unwrap();
    until("kept Alien's Poster on local disk", || {
        fx.on_disk().contains_key(&key_of(&poster))
    })
    .await;
    let originals = fx.on_disk();
    let before = fx.fetch(&poster, &[]).await;
    fx.s3.refuse_writes();

    let refused = switch(&fx.server, S3).await;

    let refusal = assert_refused(
        refused,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_IMAGE_STORE_FAILED_CHECK,
    );
    assert_eq!(refusal.field, "plugin_id");
    assert_eq!(refusal.params.get("pluginId").map(String::as_str), Some(S3));
    let reason = refusal.params.get("reason").cloned().unwrap_or_default();
    assert!(
        reason.contains("couldn't write") && reason.contains("AccessDenied"),
        "{reason}"
    );
    let settings = image_settings(&fx.server).await;
    assert_eq!(settings.settings.store_plugin_id, "");
    assert!(settings.switching.is_unset(), "{settings:?}");
    assert_eq!(fx.s3.objects(), BTreeMap::new());
    assert_eq!(fx.on_disk(), originals);
    let after = fx.fetch(&poster, &[]).await;
    assert_eq!(after.status, 200);
    assert_eq!(after.body, before.body);
    assert_eq!(after.header("etag"), before.header("etag"));
}
