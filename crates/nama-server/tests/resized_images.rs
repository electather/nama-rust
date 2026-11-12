//! Posters asked for at a width: resized to JPEG, kept in a cache with a size limit, or sent as
//! stored where they can't or needn't be resized, over a real socket.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/browsing.rs"]
mod browsing;
#[path = "support/http.rs"]
mod http;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::{
    fs::{self, File, FileTimes},
    io::Cursor,
    path::PathBuf,
    time::{Duration, SystemTime},
};

use browsing::Fixture;
use connectrpc::client::HttpClient;
use http::{Reply, fetch};
use image::{DynamicImage, ImageFormat, RgbImage, RgbaImage};
use nama_proto::nama::v1::{GetServerInfoRequest, ServerServiceClient};
use server::tmdb::ALIEN;

/// Where the store keeps Alien's Poster, as the fake TMDB names it.
const POSTER: &str = "artwork/tmdb/%2FvfrQk5IPloGg1v9Rzbh2Eg3VGyM.jpg";

/// Alien's Matched Movie, with `poster` stored as its Poster.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn alien_with_poster(poster: &[u8]) -> (Fixture, String) {
    let fx = Fixture::alien().await;
    let stored = fx.server.data_dir().join(POSTER);
    // The Match downloads the Poster in the background, which would replace the one written here.
    let downloaded = tokio::time::timeout(Duration::from_secs(10), async {
        while !stored.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    assert!(downloaded.is_ok(), "Alien's Poster was never downloaded");
    fs::write(&stored, poster).unwrap();
    let movie = fx.get_movie(fx.library, ALIEN).await.unwrap();
    (fx, movie.poster_url.unwrap())
}

/// `image` encoded as `format`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn encoded(image: impl Into<DynamicImage>, format: ImageFormat) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    image.into().write_to(&mut bytes, format).unwrap();
    bytes.into_inner()
}

/// A JPEG `width` by `height` pixels.
fn jpeg(width: u32, height: u32) -> Vec<u8> {
    encoded(
        RgbImage::from_pixel(width, height, [200, 80, 40].into()),
        ImageFormat::Jpeg,
    )
}

async fn fetch_at(fx: &Fixture, poster: &str, width: u32) -> Reply {
    let url = format!("{poster}?width={width}");
    fetch(&fx.server, &url, Some(&fx.server.admin_token)).await
}

/// Where the copy `width` pixels wide of Alien's Poster is kept.
fn copy_of_poster(fx: &Fixture, width: u32) -> PathBuf {
    let copies = fx.server.data_dir().join("resized").join(POSTER);
    copies.join(format!("{width}.jpg"))
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn dimensions(reply: &Reply) -> (u32, u32) {
    let image = image::load_from_memory_with_format(&reply.body, ImageFormat::Jpeg).unwrap();
    (image.width(), image.height())
}

#[tokio::test]
async fn a_poster_at_each_listed_width_below_its_own_is_a_jpeg_that_wide_with_its_shape() {
    let (fx, poster) = alien_with_poster(&jpeg(1000, 1500)).await;
    let client = ServerServiceClient::new(HttpClient::plaintext(), fx.server.client_config());
    let info = client
        .get_server_info(GetServerInfoRequest::default())
        .await
        .unwrap()
        .into_owned();
    let narrower: Vec<u32> = (info.image_widths.into_iter())
        .filter(|&width| width < 1000)
        .collect();
    assert_eq!(narrower, [320, 480, 640, 960]);

    for width in narrower {
        let reply = fetch_at(&fx, &poster, width).await;

        assert_eq!(reply.status, 200, "{width}");
        assert_eq!(reply.header("content-type"), "image/jpeg");
        assert_eq!(dimensions(&reply), (width, width * 3 / 2));
        assert_eq!(reply.header("vary"), "accept");
        assert_eq!(
            reply.header("cache-control"),
            "private, max-age=31536000, immutable"
        );
    }
}

#[tokio::test]
async fn a_width_not_on_the_list_is_refused() {
    let (fx, poster) = alien_with_poster(&jpeg(1000, 1500)).await;

    let reply = fetch_at(&fx, &poster, 641).await;

    assert_eq!(reply.status, 400);
}

#[tokio::test]
async fn a_width_at_or_above_the_originals_returns_the_original() {
    let original = jpeg(640, 960);
    let (fx, poster) = alien_with_poster(&original).await;

    for width in [640, 960] {
        let reply = fetch_at(&fx, &poster, width).await;

        assert_eq!(reply.status, 200);
        assert_eq!(reply.body, original, "{width}");
        assert_eq!(reply.header("vary"), "accept");
    }
}

#[tokio::test]
async fn a_second_request_for_the_same_width_is_served_from_the_cache() {
    let (fx, poster) = alien_with_poster(&jpeg(1000, 1500)).await;
    fetch_at(&fx, &poster, 640).await;
    // Resizing again would replace this.
    fs::write(copy_of_poster(&fx, 640), b"\xff\xd8kept").unwrap();

    let again = fetch_at(&fx, &poster, 640).await;

    assert_eq!(again.body, b"\xff\xd8kept");
}

#[tokio::test]
async fn deleting_the_cache_folder_loses_nothing() {
    let (fx, poster) = alien_with_poster(&jpeg(1000, 1500)).await;
    fetch_at(&fx, &poster, 640).await;

    fs::remove_dir_all(fx.server.data_dir().join("resized")).unwrap();
    let again = fetch_at(&fx, &poster, 640).await;

    assert_eq!(again.status, 200);
    assert_eq!(dimensions(&again), (640, 960));
    assert!(copy_of_poster(&fx, 640).exists());
}

#[tokio::test]
async fn with_resizing_off_any_width_returns_the_stored_image() {
    let original = jpeg(1000, 1500);
    let (fx, poster) = alien_with_poster(&original).await;

    fx.server.set_image_settings(false, None).await;

    for width in [640, 641] {
        let reply = fetch_at(&fx, &poster, width).await;

        assert_eq!(reply.status, 200);
        assert_eq!(reply.body, original, "{width}");
    }
}

#[tokio::test]
async fn an_svg_a_png_with_transparency_and_an_image_over_12_megapixels_are_sent_as_they_are() {
    let svg =
        b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1000\" height=\"1500\"/>".to_vec();
    let transparent = encoded(
        RgbaImage::from_pixel(1000, 1500, [0, 0, 0, 0].into()),
        ImageFormat::Png,
    );
    // 12,004,000 pixels.
    let huge = jpeg(4000, 3001);
    let (fx, poster) = alien_with_poster(&svg).await;
    let stored = fx.server.data_dir().join(POSTER);

    for (name, original) in [("SVG", svg), ("PNG", transparent), ("12 MP", huge)] {
        fs::write(&stored, &original).unwrap();

        let reply = fetch_at(&fx, &poster, 640).await;

        assert_eq!(reply.status, 200, "{name}");
        assert!(reply.body == original, "{name} was changed");
    }
}

#[tokio::test]
async fn past_its_limit_the_cache_deletes_the_least_recently_sent_copies_until_it_fits() {
    let (fx, poster) = alien_with_poster(&jpeg(1000, 1500)).await;
    fx.server.set_image_settings(true, Some(1)).await;
    let other_copies = fx
        .server
        .data_dir()
        .join("resized/artwork/tmdb/%2Fother.jpg");
    fs::create_dir_all(&other_copies).unwrap();
    let (older, newer) = (other_copies.join("320.jpg"), other_copies.join("480.jpg"));
    let an_hour_ago = SystemTime::now() - Duration::from_hours(1);
    for (copy, sent) in [(&older, an_hour_ago), (&newer, SystemTime::now())] {
        fs::write(copy, vec![0; 600 * 1024]).unwrap();
        let times = FileTimes::new().set_accessed(sent);
        File::options()
            .write(true)
            .open(copy)
            .unwrap()
            .set_times(times)
            .unwrap();
    }

    fetch_at(&fx, &poster, 640).await;

    assert!(!older.exists());
    assert!(newer.exists());
    assert!(copy_of_poster(&fx, 640).exists());
}

#[tokio::test]
async fn a_copy_just_made_is_sent_though_it_is_not_the_most_recently_sent() {
    let (fx, poster) = alien_with_poster(&jpeg(1000, 1500)).await;
    fx.server.set_image_settings(true, Some(1)).await;
    let other_copies = fx
        .server
        .data_dir()
        .join("resized/artwork/tmdb/%2Fother.jpg");
    fs::create_dir_all(&other_copies).unwrap();
    let other = other_copies.join("320.jpg");
    fs::write(&other, vec![0; 1024 * 1024]).unwrap();
    // Sent after the copy below is made, as by a request racing it.
    let sent_later = FileTimes::new().set_accessed(SystemTime::now() + Duration::from_hours(1));
    File::options()
        .write(true)
        .open(&other)
        .unwrap()
        .set_times(sent_later)
        .unwrap();

    let reply = fetch_at(&fx, &poster, 640).await;

    assert_eq!(reply.status, 200);
    assert_eq!(dimensions(&reply), (640, 960));
    assert!(!other.exists());
}

#[tokio::test]
async fn once_a_poster_is_resized_browse_sends_its_thumbhash() {
    let (fx, poster) = alien_with_poster(&jpeg(1000, 1500)).await;
    let before = fx.get_movie(fx.library, ALIEN).await.unwrap();

    fetch_at(&fx, &poster, 640).await;
    let after = fx.get_movie(fx.library, ALIEN).await.unwrap();

    assert_eq!(before.poster_thumbhash, None);
    let thumbhash = after.poster_thumbhash.unwrap();
    let shape = thumbhash::thumb_hash_to_approximate_aspect_ratio(&thumbhash).unwrap();
    // A thumbhash keeps its image's shape only roughly.
    assert!((shape - 2.0 / 3.0).abs() < 0.1, "{shape}");
    assert_eq!(after.backdrop_thumbhash, None);
}

#[tokio::test]
async fn with_resizing_off_no_thumbhash_is_made_or_sent() {
    let (fx, poster) = alien_with_poster(&jpeg(1000, 1500)).await;

    fx.server.set_image_settings(false, None).await;
    fetch_at(&fx, &poster, 640).await;
    fx.server.set_image_settings(true, None).await;
    let never_made = fx.get_movie(fx.library, ALIEN).await.unwrap();
    fetch_at(&fx, &poster, 640).await;
    fx.server.set_image_settings(false, None).await;
    let not_sent = fx.get_movie(fx.library, ALIEN).await.unwrap();

    assert_eq!(never_made.poster_thumbhash, None);
    assert_eq!(not_sent.poster_thumbhash, None);
}
