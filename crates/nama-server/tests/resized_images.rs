//! Posters asked for at a width: resized to JPEG, or to JPEG XL for a browser that asks for it,
//! kept in a cache with a size limit, or sent as stored where they can't or needn't be resized,
//! over a real socket.

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

use browsing::{Fixture, tmdb_id};
use connectrpc::client::HttpClient;
use http::{Reply, fetch, fetch_with};
use image::{DynamicImage, ImageFormat, RgbImage, RgbaImage};
use jxl::{
    api::{Event, JxlDecoder, JxlDecoderOptions, JxlPixelFormat},
    image::JxlOutputBuffer,
};
use nama_proto::nama::v1::{
    Episode, GetServerInfoRequest, LibraryKind, ListEpisodesRequest, ServerServiceClient,
};
use server::{
    TestServer,
    tmdb::{ALIEN, DARK},
};

/// Where the store keeps Alien's Poster, as the fake TMDB names it.
const POSTER: &str = "artwork/tmdb/%2FvfrQk5IPloGg1v9Rzbh2Eg3VGyM.jpg";
/// Where the store keeps the Still of Dark's S01E01, as the fake TMDB names it.
const STILL: &str = "artwork/tmdb/%2FgNnJBvHy3VBkGrsl9ffpkf6ucP3.jpg";

/// Alien's Matched Movie, with `poster` stored as its Poster.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn alien_with_poster(poster: &[u8]) -> (Fixture, String) {
    let fx = Fixture::alien().await;
    replace_once_downloaded(&fx, POSTER, poster).await;
    let movie = fx.get_movie(fx.library, ALIEN).await.unwrap();
    (fx, movie.poster_url.unwrap())
}

/// Writes `image` over the stored image `key` once the Match's background download has stored
/// it, since that download would replace an image written before it.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn replace_once_downloaded(fx: &Fixture, key: &str, image: &[u8]) {
    let stored = fx.server.data_dir().join(key);
    let downloaded = tokio::time::timeout(Duration::from_secs(10), async {
        while !stored.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    assert!(downloaded.is_ok(), "{key} was never downloaded");
    fs::write(&stored, image).unwrap();
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

/// What Chrome sends as `Accept` for an `<img>`.
const ACCEPTS_JXL: &str = "image/jxl,image/avif,image/webp,image/apng,image/*,*/*;q=0.8";

async fn fetch_at(fx: &Fixture, poster: &str, width: u32) -> Reply {
    let url = format!("{poster}?width={width}");
    fetch(&fx.server, &url, Some(&fx.server.admin_token)).await
}

/// [`fetch_at`], sending `accept` as the `Accept` header.
async fn fetch_accepting(fx: &Fixture, poster: &str, width: u32, accept: &str) -> Reply {
    let url = format!("{poster}?width={width}");
    let headers = [("accept", accept)];
    fetch_with(&fx.server, &url, Some(&fx.server.admin_token), &headers).await
}

/// Where the copy `width` pixels wide of Alien's Poster is kept as a JPEG.
fn copy_of_poster(fx: &Fixture, width: u32) -> PathBuf {
    let copies = fx.server.data_dir().join("resized").join(POSTER);
    copies.join(format!("{width}.jpg"))
}

/// The pixels of a JPEG XL, as the `jxl` crate decodes the first `prefix` bytes of it, or `None`
/// if they draw nothing.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn decode_jxl(jxl: &[u8], prefix: usize) -> Option<RgbaImage> {
    let (mut input, _rest) = jxl.split_at(prefix);
    let mut decoder = JxlDecoder::new(JxlDecoderOptions::default());
    while decoder.process(&mut input, None, None).unwrap() != Event::BasicInfo {}
    let info = decoder.basic_info().unwrap();
    let (width, height) = info.size;
    let format = JxlPixelFormat::rgba8(info.extra_channels.len());
    decoder.set_pixel_format(format).unwrap();
    let mut pixels = vec![0; width * height * 4];
    let mut output = [JxlOutputBuffer::new(&mut pixels, height, width * 4)];
    let drawn = loop {
        match decoder
            .process(&mut input, Some(&mut output), None)
            .unwrap()
        {
            Event::Complete => break true,
            Event::NeedMoreInput { .. } => break decoder.flush_pixels(&mut output, None).unwrap(),
            Event::BasicInfo | Event::FrameHeader | Event::FrameComplete { .. } => {}
        }
    };
    let width = u32::try_from(width).unwrap();
    let height = u32::try_from(height).unwrap();
    drawn.then(|| RgbaImage::from_raw(width, height, pixels).unwrap())
}

/// [`decode_jxl`] of the whole of a JPEG XL.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn jxl_image(jxl: &[u8]) -> RgbaImage {
    decode_jxl(jxl, jxl.len()).unwrap()
}

/// A JPEG `width` by `height` pixels of noise, which JPEG XL can't squeeze into a few bytes.
fn noisy_jpeg(width: u32, height: u32) -> Vec<u8> {
    let noise = RgbImage::from_fn(width, height, |x, y| {
        let seed = (x * 7919 + y * 104_729) ^ (x * y);
        let [red, green, blue, _] = seed.wrapping_mul(2_654_435_761).to_le_bytes();
        [red, green, blue].into()
    });
    encoded(noise, ImageFormat::Jpeg)
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
async fn a_browser_that_asks_for_jpeg_xl_gets_one_that_wide_and_others_a_jpeg() {
    let (fx, poster) = alien_with_poster(&jpeg(1000, 1500)).await;

    let jxl = fetch_accepting(&fx, &poster, 640, ACCEPTS_JXL).await;
    let jpeg = fetch_accepting(&fx, &poster, 640, "image/webp,*/*").await;

    assert_eq!(jxl.status, 200);
    assert_eq!(jxl.header("content-type"), "image/jxl");
    assert_eq!(jxl_image(&jxl.body).dimensions(), (640, 960));
    assert_eq!(jxl.header("vary"), "accept");
    assert_eq!(jpeg.header("content-type"), "image/jpeg");
    assert_eq!(dimensions(&jpeg), (640, 960));
}

#[tokio::test]
async fn jpeg_xl_at_quality_zero_is_not_asked_for() {
    let (fx, poster) = alien_with_poster(&jpeg(1000, 1500)).await;

    let reply = fetch_accepting(&fx, &poster, 640, "image/jxl;q=0, image/*").await;

    assert_eq!(reply.header("content-type"), "image/jpeg");
    assert_eq!(dimensions(&reply), (640, 960));
}

#[tokio::test]
async fn a_jpeg_xl_paints_a_full_frame_from_its_first_hundredth() {
    let (fx, poster) = alien_with_poster(&noisy_jpeg(1000, 1500)).await;

    let reply = fetch_accepting(&fx, &poster, 960, ACCEPTS_JXL).await;

    // With progressive DC its first 1% paints the frame; without, nothing is drawn before 5%.
    let early = decode_jxl(&reply.body, reply.body.len() / 100).unwrap();
    assert_eq!(early.dimensions(), (960, 1440));
    let bottom_right = early.get_pixel(959, 1439);
    assert_ne!(
        bottom_right.0,
        [0, 0, 0, 0],
        "the frame isn't painted whole"
    );
}

#[tokio::test]
async fn an_output_over_7_megapixels_is_a_jpeg_or_as_stored_with_transparency() {
    // At 2560 px wide this is 2560x3840, 9.8 megapixels.
    let opaque = jpeg(2600, 3900);
    let transparent = encoded(
        RgbaImage::from_pixel(2600, 3900, [0, 0, 0, 0].into()),
        ImageFormat::Png,
    );
    let (fx, poster) = alien_with_poster(&opaque).await;

    let as_jpeg = fetch_accepting(&fx, &poster, 2560, ACCEPTS_JXL).await;
    assert_eq!(as_jpeg.header("content-type"), "image/jpeg");
    assert_eq!(dimensions(&as_jpeg), (2560, 3840));

    fs::write(fx.server.data_dir().join(POSTER), &transparent).unwrap();
    // A copy is made from the image's key alone, so the opaque one's would be found.
    fs::remove_dir_all(fx.server.data_dir().join("resized")).unwrap();
    let as_stored = fetch_accepting(&fx, &poster, 2560, ACCEPTS_JXL).await;
    assert!(as_stored.body == transparent, "it was changed");
}

#[tokio::test]
async fn a_png_with_transparency_becomes_a_jpeg_xl_that_keeps_it_only_for_a_browser_that_asks() {
    let png = encoded(
        RgbaImage::from_fn(1000, 1500, |x, _| {
            [200, 80, 40, if x < 500 { 0 } else { 255 }].into()
        }),
        ImageFormat::Png,
    );
    let (fx, poster) = alien_with_poster(&png).await;

    let jxl = fetch_accepting(&fx, &poster, 640, ACCEPTS_JXL).await;
    let other = fetch_accepting(&fx, &poster, 640, "image/webp,*/*").await;

    assert_eq!(jxl.header("content-type"), "image/jxl");
    let decoded = jxl_image(&jxl.body);
    assert_eq!(decoded.dimensions(), (640, 960));
    assert_eq!(decoded.get_pixel(10, 480).0[3], 0);
    assert_eq!(decoded.get_pixel(630, 480).0[3], 255);
    assert!(other.body == png, "it was changed");
}

#[tokio::test]
async fn the_cache_keeps_each_format_of_a_width_apart_within_the_one_limit() {
    let (fx, poster) = alien_with_poster(&jpeg(1000, 1500)).await;
    fx.server.set_image_settings(true, Some(1)).await;
    let copies = fx.server.data_dir().join("resized").join(POSTER);
    let old = fx
        .server
        .data_dir()
        .join("resized/artwork/tmdb/%2Fother.jpg/320.jxl");
    fs::create_dir_all(old.parent().unwrap()).unwrap();
    fs::write(&old, vec![0; 1024 * 1024]).unwrap();
    let an_hour_ago = FileTimes::new().set_accessed(SystemTime::now() - Duration::from_hours(1));
    File::open(&old).unwrap().set_times(an_hour_ago).unwrap();

    fetch_accepting(&fx, &poster, 640, ACCEPTS_JXL).await;
    fetch_at(&fx, &poster, 640).await;
    let jxl = fs::read(copies.join("640.jxl")).unwrap();
    let jpeg = fs::read(copies.join("640.jpg")).unwrap();
    // Resizing again would replace this.
    fs::write(copies.join("640.jxl"), b"kept").unwrap();
    let again = fetch_accepting(&fx, &poster, 640, ACCEPTS_JXL).await;

    assert!(!old.exists(), "the limit wasn't kept");
    assert_eq!(jxl_image(&jxl).dimensions(), (640, 960));
    assert_eq!(image::guess_format(&jpeg).unwrap(), ImageFormat::Jpeg);
    assert_eq!(again.body, b"kept");
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

    for (width, accept) in [(640, "*/*"), (960, "*/*"), (640, ACCEPTS_JXL)] {
        let reply = fetch_accepting(&fx, &poster, width, accept).await;

        assert_eq!(reply.status, 200);
        assert_eq!(reply.body, original, "{width} {accept}");
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

    for (width, accept) in [(640, "*/*"), (641, "*/*"), (640, ACCEPTS_JXL)] {
        let reply = fetch_accepting(&fx, &poster, width, accept).await;

        assert_eq!(reply.status, 200);
        assert_eq!(reply.body, original, "{width} {accept}");
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

/// Dark's S01E01, Matched, with `still` stored as its Still.
async fn secrets_with_still(still: &[u8]) -> Fixture {
    let fx = Fixture::start_on(LibraryKind::LIBRARY_KIND_SHOWS, TestServer::builder()).await;
    fx.write("Dark (2017)/Season 01/Dark - S01E01.mkv", b"video");
    fx.scan_and_match().await;
    replace_once_downloaded(&fx, STILL, still).await;
    fx
}

/// Dark's Season 1 Episodes, as `ListEpisodes` sends them.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn dark_season_1(fx: &Fixture) -> Vec<Episode> {
    fx.browse
        .list_episodes(ListEpisodesRequest {
            library_id: fx.library,
            show_id: tmdb_id(DARK).into(),
            season_number: 1,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .episodes
}

#[tokio::test]
async fn a_still_is_resized_like_a_backdrop_and_browse_then_sends_its_thumbhash() {
    let fx = secrets_with_still(&jpeg(1920, 1080)).await;
    let still = dark_season_1(&fx).await[0].still_url.clone().unwrap();

    let reply = fetch_at(&fx, &still, 640).await;
    let after = dark_season_1(&fx).await;

    assert_eq!(reply.status, 200);
    assert_eq!(dimensions(&reply), (640, 360));
    let shape = thumbhash::thumb_hash_to_approximate_aspect_ratio(
        after[0].still_thumbhash.as_deref().unwrap(),
    )
    .unwrap();
    // A thumbhash keeps its image's shape only roughly.
    assert!((shape - 16.0 / 9.0).abs() < 0.2, "{shape}");
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
