//! A Movie's and Show's titled Backdrop: the Backdrop with text in the User's language, sent beside
//! the textless one, over a real socket with the TMDB Plugin enabled against a fake TMDB and a
//! title source of this file's own that lists an English Backdrop.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/http.rs"]
mod http;
#[path = "support/kept_artwork.rs"]
mod kept_artwork;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::{future, io::Cursor, sync::Arc};

use connectrpc::client::HttpClient;
use http::fetch;
use image::{ImageFormat, RgbImage};
use kept_artwork::{kept, until_kept};
use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    contract::{Artwork, ArtworkAnswer, ArtworkAsked, ArtworkKind, Image, ImageAnswer, ImageAsked},
    metadata::{MetadataError, SourceManifest, TitleMethod, TitleSource},
};
use nama_proto::nama::v1::{
    BrowseServiceClient, ClearMatchRequest, ExternalId, GetMovieRequest, GetShowRequest,
    LibraryKind, MatchServiceClient, Movie, Show,
};
use nama_server::plugins::Registry;
use server::{
    Library, TestServer,
    tmdb::{ALIEN, DARK},
};

/// TMDB Matches it to Alien.
const ALIEN_FILE: &str = "Alien (1979)/Alien (1979).mkv";
/// TMDB Matches it to Dark.
const DARK_S01E01: &str = "Dark (2017)/Season 01/Dark - S01E01.mkv";
/// The namespace and Plugin id of [`Titled`].
const TITLED: &str = "titled";
/// The English Backdrop [`Titled`] lists for every title.
const TITLED_BACKDROP: &str = "/titled-en.jpg";

/// A title source that only lists artwork, by a title's TMDB id: an English Backdrop for every
/// title, which it serves as a JPEG.
struct Titled;

impl TitleSource for Titled {
    fn namespace(&self) -> &str {
        TITLED
    }

    fn lookup_namespaces(&self) -> Vec<String> {
        vec!["tmdb".to_owned()]
    }

    fn artwork<'a>(
        &'a self,
        _asked: &'a ArtworkAsked,
    ) -> BoxFuture<'a, Result<ArtworkAnswer, MetadataError>> {
        let backdrop = Artwork {
            kind: ArtworkKind::ARTWORK_KIND_BACKDROP.into(),
            path: TITLED_BACKDROP.to_owned(),
            language: Some("en".to_owned()),
            ..Default::default()
        };
        Box::pin(future::ready(Ok(ArtworkAnswer {
            artwork: vec![backdrop],
            ..Default::default()
        })))
    }

    fn image<'a>(
        &'a self,
        _asked: &'a ImageAsked,
    ) -> BoxFuture<'a, Result<ImageAnswer, MetadataError>> {
        let image = Image {
            content_type: "image/jpeg".to_owned(),
            bytes: jpeg(),
            ..Default::default()
        };
        Box::pin(future::ready(Ok(ImageAnswer {
            image: image.into(),
            ..Default::default()
        })))
    }

    fn answers(&self, method: TitleMethod) -> bool {
        matches!(method, TitleMethod::Artwork | TitleMethod::Image)
    }
}

impl Plugin for Titled {
    fn manifest(&self) -> Manifest {
        Manifest {
            extension_points: vec![PointKey::provide::<dyn TitleSource>()],
            title_source: Some(SourceManifest {
                namespace: TITLED.to_owned(),
                looks_up: vec!["tmdb".to_owned()],
                searches: false,
            }),
            ..Manifest::new(PluginId::from_static(TITLED))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.provide::<dyn TitleSource>(Priority::DEFAULT, Arc::new(Self));
        Ok(())
    }
}

/// A 1280 by 720 JPEG, big enough to be resized.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn jpeg() -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    (RgbImage::new(1280, 720).write_to(&mut bytes, ImageFormat::Jpeg)).unwrap();
    bytes.into_inner()
}

/// A Server with TMDB and [`Titled`] enabled.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn server() -> TestServer {
    let plugins = Registry::new();
    // Described too, as a Library's order names only sources the Server describes.
    plugins.describe_builtin(|| Arc::new(Titled));
    plugins.enable(Arc::new(Titled)).unwrap();
    TestServer::builder().plugins(plugins).tmdb().start().await
}

fn tmdb(id: i64) -> ExternalId {
    ExternalId {
        namespace: "tmdb".to_owned(),
        value: id.to_string(),
        ..Default::default()
    }
}

fn browse(server: &TestServer) -> BrowseServiceClient<HttpClient> {
    BrowseServiceClient::new(HttpClient::plaintext(), server.client_config())
}

/// Alien, Matched in a new Movies Library searching `title_sources`, as `GetMovie` gives it.
async fn alien(server: &TestServer, title_sources: &[&str]) -> (Library, Movie) {
    let library = (server.create_library_searching(
        LibraryKind::LIBRARY_KIND_MOVIES,
        &[ALIEN_FILE],
        title_sources,
    ))
    .await;
    server.scan_and_match(&library).await;
    let movie = get_alien(server, &library).await;
    assert_eq!(movie.id.value, ALIEN.to_string());
    (library, movie)
}

/// Alien as `GetMovie` gives it in `library`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn get_alien(server: &TestServer, library: &Library) -> Movie {
    let request = GetMovieRequest {
        library_id: library.id,
        movie_id: tmdb(ALIEN).into(),
        ..Default::default()
    };
    let got = browse(server)
        .get_movie(request)
        .await
        .unwrap()
        .into_owned();
    got.movie.into_option().unwrap()
}

/// Dark, Matched in a new Shows Library searching `title_sources`, as `GetShow` gives it.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn dark(server: &TestServer, title_sources: &[&str]) -> Show {
    let library = (server.create_library_searching(
        LibraryKind::LIBRARY_KIND_SHOWS,
        &[DARK_S01E01],
        title_sources,
    ))
    .await;
    server.scan_and_match(&library).await;
    let request = GetShowRequest {
        library_id: library.id,
        show_id: tmdb(DARK).into(),
        ..Default::default()
    };
    let got = browse(server).get_show(request).await.unwrap().into_owned();
    got.show.into_option().unwrap()
}

#[tokio::test]
async fn a_source_with_an_english_backdrop_gives_movies_and_shows_a_titled_backdrop_it_serves() {
    let server = server().await;
    let token = Some(server.admin_token.as_str());

    let (_, movie) = alien(&server, &["tmdb", TITLED]).await;
    let show = dark(&server, &["tmdb", TITLED]).await;

    for url in [movie.titled_backdrop_url, show.titled_backdrop_url] {
        let url = url.expect("no titled Backdrop");
        assert!(url.starts_with("/media/artwork/titled/"), "{url}");
        let fetched = fetch(&server, &url, token).await;
        assert_eq!((fetched.status, fetched.body), (200, jpeg()));
    }
    let plain = [movie.backdrop_url, show.backdrop_url];
    assert!(
        (plain.iter()).all(|url| url
            .as_ref()
            .is_some_and(|url| url.contains("/artwork/tmdb/"))),
        "{plain:?}"
    );
}

#[tokio::test]
async fn once_resized_a_titled_backdrop_is_sent_with_its_thumbhash() {
    let server = server().await;
    let (library, before) = alien(&server, &["tmdb", TITLED]).await;
    let url = before.titled_backdrop_url.clone().unwrap();

    let resized = format!("{url}?width=640");
    fetch(&server, &resized, Some(&server.admin_token)).await;
    let after = get_alien(&server, &library).await;

    assert_eq!(before.titled_backdrop_thumbhash, None);
    assert!(after.titled_backdrop_thumbhash.is_some());
}

#[tokio::test]
async fn titles_whose_sources_have_no_english_backdrop_send_no_titled_backdrop() {
    let server = server().await;

    let (_, movie) = alien(&server, &["tmdb"]).await;
    let show = dark(&server, &["tmdb"]).await;

    assert!(movie.backdrop_url.is_some());
    assert_eq!(
        (movie.titled_backdrop_url, movie.titled_backdrop_thumbhash),
        (None, None)
    );
    assert_eq!(
        (show.titled_backdrop_url, show.titled_backdrop_thumbhash),
        (None, None)
    );
}

#[tokio::test]
async fn a_titled_backdrop_is_kept_while_a_match_stores_it_and_dropped_once_none_does() {
    let server = server().await;
    let library = (server.create_library_searching(
        LibraryKind::LIBRARY_KIND_MOVIES,
        &[ALIEN_FILE],
        &["tmdb", TITLED],
    ))
    .await;
    let items = server.scan_and_match(&library).await;
    until_kept(&server, TITLED, &[TITLED_BACKDROP]).await;

    // A scan ends by deleting the images no Match stores.
    server.scan_and_match(&library).await;
    let kept_while_matched = kept(&server, TITLED, TITLED_BACKDROP).exists();
    let matches = MatchServiceClient::new(HttpClient::plaintext(), server.client_config());
    let cleared = ClearMatchRequest {
        item_id: items[ALIEN_FILE],
        ..Default::default()
    };
    matches.clear_match(cleared).await.unwrap();

    assert!(kept_while_matched);
    assert!(!kept(&server, TITLED, TITLED_BACKDROP).exists());
}
