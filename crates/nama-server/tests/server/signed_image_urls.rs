//! Signed image URLs (ADR 0105): a caller with a bearer token gets image URLs an `<img>` loads
//! with no cookie or header, each for its one image and for a few minutes, while a cookie caller's
//! stay as they are.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/browsing.rs"]
mod browsing;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/custom_images.rs"]
mod custom_images;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/http.rs"]
mod http;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::time::Duration;

use browsing::{ALIEN_1080P, Fixture, tmdb_id};
use connectrpc::client::HttpClient;
use custom_images::image;
use http::fetch;
use image::ImageFormat;
use nama_proto::nama::v1::{
    BrowseServiceClient, GetMovieRequest, LibraryServiceClient, ListArtworkRequest,
    ListPluginsRequest, Movie, PluginServiceClient, TitleKind,
};
use nama_server::plugins::Registry;
use server::{TestServer, tmdb::ALIEN};

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    /// Alien as `GetMovie` gives it to the Admin calling with a bearer token.
    async fn alien_as_bearer(&self) -> Movie {
        BrowseServiceClient::new(HttpClient::plaintext(), self.server.bearer_client_config())
            .get_movie(GetMovieRequest {
                library_id: self.library,
                movie_id: tmdb_id(ALIEN).into(),
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned()
            .movie
            .into_option()
            .unwrap()
    }

    /// The status of a `GET` for `url` with no cookie or header.
    async fn status(&self, url: &str) -> u16 {
        fetch(&self.server, url, None).await.status
    }
}

#[tokio::test]
async fn a_bearer_callers_poster_and_backdrop_load_with_no_session() {
    let fx = Fixture::alien().await;

    let movie = fx.alien_as_bearer().await;

    for url in [movie.poster_url.unwrap(), movie.backdrop_url.unwrap()] {
        assert_eq!(fx.status(&url).await, 200, "{url}");
    }
}

#[tokio::test]
async fn a_bearer_callers_custom_image_and_image_beside_the_media_load_with_no_session() {
    let fx = Fixture::alien().await;
    let slot = format!("/custom-images/movie/tmdb-{ALIEN}/poster");
    let uploaded = fx.upload(&slot, &image(ImageFormat::Png, 2, 3)).await;
    let alien_folder = ALIEN_1080P.split_once('/').unwrap().0;
    fx.write(
        &format!("{alien_folder}/fanart.jpg"),
        &image(ImageFormat::Jpeg, 3, 2),
    );
    fx.scan_and_match().await;

    let movie = fx.alien_as_bearer().await;

    assert_eq!(uploaded.status, 204);
    let (poster, backdrop) = (movie.poster_url.unwrap(), movie.backdrop_url.unwrap());
    assert!(poster.contains("/custom-images/"), "{poster}");
    assert!(backdrop.contains("/local-images/"), "{backdrop}");
    assert_eq!(fx.status(&poster).await, 200);
    assert_eq!(fx.status(&backdrop).await, 200);
}

#[tokio::test]
async fn a_bearer_callers_plugin_logo_loads_with_no_session() {
    let server = TestServer::builder()
        .plugins(Registry::with_builtins())
        .start()
        .await;
    let plugins = PluginServiceClient::new(HttpClient::plaintext(), server.bearer_client_config());

    let listed = plugins.list_plugins(ListPluginsRequest::default()).await;

    let summaries = listed.unwrap().into_owned().summaries;
    let tmdb = summaries.iter().find(|plugin| plugin.id == "tmdb").unwrap();
    assert_eq!(fetch(&server, &tmdb.logo_url, None).await.status, 200);
}

#[tokio::test]
async fn a_signed_url_loads_only_its_own_image() {
    let fx = Fixture::alien().await;
    let movie = fx.alien_as_bearer().await;
    let (poster, backdrop) = (movie.poster_url.unwrap(), movie.backdrop_url.unwrap());
    let (posters_signature, _) = poster.split_once("/artwork/").unwrap();
    let (_, backdrops_image) = backdrop.split_once("/artwork/").unwrap();

    let backdrop_with_posters_signature = format!("{posters_signature}/artwork/{backdrops_image}");

    assert_ne!(backdrop_with_posters_signature, backdrop);
    assert_eq!(fx.status(&backdrop_with_posters_signature).await, 401);
}

#[tokio::test]
async fn a_signed_url_loads_for_a_few_minutes_and_not_once_it_expires() {
    let fx = Fixture::alien_on(TestServer::builder().moves_clock()).await;
    let poster = fx.alien_as_bearer().await.poster_url.unwrap();

    fx.server.advance_clock(Duration::from_mins(4));
    let within = fx.status(&poster).await;
    fx.server.advance_clock(Duration::from_mins(7));
    let after = fx.status(&poster).await;

    assert_eq!((within, after), (200, 401));
}

#[tokio::test]
async fn a_signed_url_passes_on_the_width_it_asks_for() {
    let fx = Fixture::alien().await;
    fx.server.set_image_settings(true, None).await;
    let poster = fx.alien_as_bearer().await.poster_url.unwrap();

    let listed = fx.status(&format!("{poster}?width=320")).await;
    let unlisted = fx.status(&format!("{poster}?width=321")).await;

    // Only a width on the list is served while resizing is on.
    assert_eq!((listed, unlisted), (200, 400));
}

#[tokio::test]
async fn a_bearer_caller_picks_a_poster_by_the_signed_url_the_artwork_list_gave_it() {
    let fx = Fixture::alien().await;
    let libraries =
        LibraryServiceClient::new(HttpClient::plaintext(), fx.server.bearer_client_config());
    let listed = libraries.list_artwork(ListArtworkRequest {
        kind: TitleKind::TITLE_KIND_MOVIE.into(),
        id: tmdb_id(ALIEN).into(),
        ..Default::default()
    });
    let urls = listed.await.unwrap().into_owned().poster_urls;
    // The one besides the downloaded Poster whose bytes the fake TMDB has.
    let second = urls
        .iter()
        .find(|url| url.ends_with("2xcXrPA5kHJu8a7BxRnwV4tz0ws.jpg"))
        .unwrap();
    assert_eq!(fx.status(second).await, 200);

    let picked: String = url::form_urlencoded::byte_serialize(second.as_bytes()).collect();
    let poster = format!("/custom-images/movie/tmdb-{ALIEN}/poster?artwork={picked}");
    let uploaded = fx.upload(&poster, &[]).await;

    assert_eq!(uploaded.status, 204);
    assert!(fx.alien_as_bearer().await.custom_poster);
}

#[tokio::test]
async fn a_cookie_callers_poster_url_is_the_unsigned_one() {
    let fx = Fixture::alien().await;

    let movie = fx.get_movie(fx.library, ALIEN).await.unwrap();

    assert_eq!(
        movie.poster_url.as_deref(),
        Some("/media/artwork/tmdb/%2FvfrQk5IPloGg1v9Rzbh2Eg3VGyM.jpg")
    );
}
