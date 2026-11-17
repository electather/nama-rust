//! Images beside the media: a `poster.jpg` or `fanart.jpg` that Jellyfin or Kodi left beside a
//! Movie's file stands in for its downloaded Poster or Backdrop, below a Custom Image (#732), over
//! a real socket with the TMDB Plugin enabled against a fake TMDB.

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
#[path = "support/log.rs"]
mod log;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::fs;

use browsing::Fixture;
use custom_images::image;
use http::fetch;
use image::ImageFormat;
use log::Log;
use server::tmdb::ALIEN;

/// Where Alien's Custom Poster is uploaded to and removed from.
fn custom_alien_poster() -> String {
    format!("/custom-images/movie/tmdb-{ALIEN}/poster")
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    /// Alien's Poster and Backdrop as `GetMovie` gives them, each as the bytes served at its URL.
    async fn alien_images(&self) -> (Vec<u8>, Vec<u8>) {
        let movie = self.get_movie(self.library, ALIEN).await.unwrap();
        let token = Some(self.server.admin_token.as_str());
        let poster = fetch(&self.server, &movie.poster_url.unwrap(), token).await;
        let backdrop = fetch(&self.server, &movie.backdrop_url.unwrap(), token).await;
        assert_eq!((poster.status, backdrop.status), (200, 200));
        (poster.body, backdrop.body)
    }

    fn delete(&self, relative: &str) {
        fs::remove_file(self.folder.join(relative)).unwrap();
    }
}

#[tokio::test]
async fn a_poster_and_fanart_beside_a_movies_file_are_its_poster_and_backdrop_after_a_scan() {
    let fx = Fixture::alien().await;
    let poster = image(ImageFormat::Jpeg, 2, 3);
    let fanart = image(ImageFormat::Png, 3, 2);
    fx.write("Alien (1979)/poster.jpg", &poster);
    fx.write("Alien (1979)/fanart.png", &fanart);

    fx.scan_and_match().await;

    assert_eq!(fx.alien_images().await, (poster, fanart));
    let movie = fx.get_movie(fx.library, ALIEN).await;
    assert!(!movie.unwrap().custom_poster);
}

#[tokio::test]
async fn a_custom_image_beats_the_one_beside_the_file_and_removing_it_brings_that_one_back() {
    let fx = Fixture::alien().await;
    let beside = image(ImageFormat::Jpeg, 2, 3);
    fx.write("Alien (1979)/poster.jpg", &beside);
    fx.scan_and_match().await;
    let custom = image(ImageFormat::Png, 4, 6);

    fx.upload(&custom_alien_poster(), &custom).await;
    let (shown_over_it, _) = fx.alien_images().await;
    fx.remove(&custom_alien_poster()).await;
    let (shown_after, _) = fx.alien_images().await;

    assert_eq!((shown_over_it, shown_after), (custom, beside));
}

#[tokio::test]
async fn deleting_the_image_beside_the_file_brings_the_downloaded_poster_back_at_the_next_scan() {
    let fx = Fixture::alien().await;
    let downloaded = fx.alien_images().await;
    let beside = image(ImageFormat::Jpeg, 2, 3);
    fx.write("Alien (1979)/poster.jpg", &beside);
    fx.scan_and_match().await;
    let (with_file, _) = fx.alien_images().await;

    fx.delete("Alien (1979)/poster.jpg");
    fx.scan_and_match().await;

    assert_eq!(with_file, beside);
    assert_eq!(fx.alien_images().await, downloaded);
}

#[tokio::test]
async fn of_two_copies_each_with_a_poster_the_first_added_ones_shows_and_a_third_changes_nothing() {
    let fx = Fixture::alien().await;
    let first = image(ImageFormat::Jpeg, 2, 3);
    fx.write("Alien (1979)/poster.jpg", &first);
    fx.scan_and_match().await;
    // Their folders sort before the first's, so neither path nor folder order picks the first.
    fx.write("A copy/Alien (1979).mkv", b"video");
    fx.write("A copy/poster.jpg", &image(ImageFormat::Png, 2, 3));
    fx.scan_and_match().await;
    let (with_two, _) = fx.alien_images().await;

    fx.write("A third/Alien (1979).mkv", b"video");
    fx.write("A third/poster.jpg", &image(ImageFormat::WebP, 2, 3));
    fx.scan_and_match().await;
    let (with_three, _) = fx.alien_images().await;

    assert_eq!((with_two, with_three), (first.clone(), first));
}

#[tokio::test]
async fn a_poster_named_after_the_file_counts_but_a_bare_one_in_the_librarys_own_folder_does_not() {
    let fx = Fixture::alien().await;
    let downloaded = fx.alien_images().await;
    fx.write("Alien (1979).mkv", b"video");
    fx.write("poster.jpg", &image(ImageFormat::Jpeg, 2, 3));
    fx.scan_and_match().await;
    let with_bare_one = fx.alien_images().await;
    let named = image(ImageFormat::Png, 2, 3);

    fx.write("Alien (1979)-poster.png", &named);
    fx.scan_and_match().await;

    assert_eq!(with_bare_one, downloaded);
    assert_eq!(fx.alien_images().await.0, named);
}

#[tokio::test]
async fn an_image_beside_the_file_that_a_custom_image_could_not_be_is_skipped_and_logged() {
    let log = Log::global();
    let fx = Fixture::alien().await;
    let downloaded = fx.alien_images().await;
    fx.write("Alien (1979)/poster.jpg", b"a text file named poster.jpg");
    fx.write(
        "Alien (1979)/fanart.png",
        &image(ImageFormat::Png, 4000, 3001),
    );

    fx.scan_and_match().await;

    assert_eq!(fx.alien_images().await, downloaded);
    let poster = fx.folder.join("Alien (1979)/poster.jpg");
    let fanart = fx.folder.join("Alien (1979)/fanart.png");
    let (poster, fanart) = (poster.to_string_lossy(), fanart.to_string_lossy());
    let not_an_image = ["skipped", &poster, "isn't a JPEG, PNG or WebP image"];
    let too_many_pixels = ["skipped", &fanart, "over 12 megapixels"];
    assert_eq!(log.count(&not_an_image), 1);
    assert_eq!(log.count(&too_many_pixels), 1);
}

#[tokio::test]
async fn an_image_beside_the_file_gone_from_the_data_directory_is_copied_again_at_the_next_scan() {
    let fx = Fixture::alien().await;
    let beside = image(ImageFormat::Jpeg, 2, 3);
    fx.write("Alien (1979)/poster.jpg", &beside);
    fx.scan_and_match().await;

    fs::remove_dir_all(fx.server.data_dir().join("local-images")).unwrap();
    fx.scan_and_match().await;

    assert_eq!(fx.alien_images().await.0, beside);
}
