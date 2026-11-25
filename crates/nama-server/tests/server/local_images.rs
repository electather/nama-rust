//! Images beside the media: a `poster.jpg` or `fanart.jpg` that Jellyfin or Kodi left beside a
//! Movie's file, or in a Show's or Season's folder, stands in for its downloaded Poster or
//! Backdrop, below a Custom Image (#732, #1016), over a real socket with the TMDB Plugin enabled
//! against a fake TMDB.

use crate::support::{browsing, custom_images, http, log, server};

use std::fs;

use browsing::{Fixture, tmdb_id};
use connectrpc::client::HttpClient;
use custom_images::{DARK_S01E01, image};
use http::fetch;
use image::ImageFormat;
use log::Log;
use nama_proto::nama::v1::{
    LibraryKind, ListMoviesRequest, ListShowsRequest, LookUpShowRequest, RequestServiceClient,
};
use server::{
    TestServer, search,
    tmdb::{ALIEN, DARK, THE_OFFICE},
};

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

    /// Alien's Poster as `ListMovies` gives it, as the bytes served at its URL.
    async fn listed_alien_poster(&self) -> Vec<u8> {
        let listed = self.browse.list_movies(ListMoviesRequest {
            library_id: self.library,
            ..Default::default()
        });
        let movie = listed.await.unwrap().into_owned().movies.into_iter().next();
        self.served(&movie.unwrap().poster_url.unwrap()).await
    }

    fn delete_file(&self, relative: &str) {
        fs::remove_file(self.folder.join(relative)).unwrap();
    }

    /// The bytes served at `url`, as the Admin.
    async fn served(&self, url: &str) -> Vec<u8> {
        let reply = fetch(&self.server, url, Some(&self.server.admin_token)).await;
        assert_eq!(reply.status, 200, "{url}");
        reply.body
    }

    /// Dark's Poster and Backdrop as `GetShow` gives them, each as the bytes served at its URL.
    async fn dark_images(&self) -> (Vec<u8>, Vec<u8>) {
        let show = self.get_dark().await.show.into_option().unwrap();
        let poster = self.served(&show.poster_url.unwrap()).await;
        (poster, self.served(&show.backdrop_url.unwrap()).await)
    }

    /// Dark's Season `number`'s Poster as `GetShow` gives it, as the bytes served at its URL, and
    /// whether it is a Custom Image.
    async fn dark_season_poster(&self, number: u32) -> (Vec<u8>, bool) {
        let seasons = self.get_dark().await.seasons;
        let season = seasons.into_iter().find(|season| season.number == number);
        let season = season.unwrap();
        let poster = self.served(&season.poster_url.unwrap()).await;
        (poster, season.custom_poster)
    }

    fn requests(&self) -> RequestServiceClient<HttpClient> {
        RequestServiceClient::new(HttpClient::plaintext(), self.server.client_config())
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

    assert_eq!(fx.alien_images().await, (poster.clone(), fanart));
    assert_eq!(fx.listed_alien_poster().await, poster);
    let movie = fx.get_movie(fx.library, ALIEN).await;
    assert!(!movie.unwrap().custom_poster);
}

#[tokio::test]
async fn deleting_the_image_beside_the_file_brings_the_downloaded_poster_back_at_the_next_scan() {
    let fx = Fixture::alien().await;
    let downloaded = fx.alien_images().await;
    let beside = image(ImageFormat::Jpeg, 2, 3);
    fx.write("Alien (1979)/poster.jpg", &beside);
    fx.scan_and_match().await;
    let (with_file, _) = fx.alien_images().await;

    fx.delete_file("Alien (1979)/poster.jpg");
    fx.scan_and_match().await;

    assert_eq!(with_file, beside);
    assert_eq!(fx.alien_images().await, downloaded);
}

#[tokio::test]
async fn of_two_items_each_with_a_poster_the_first_added_ones_shows_and_a_third_changes_nothing() {
    let fx = Fixture::alien().await;
    let first = image(ImageFormat::Jpeg, 2, 3);
    fx.write("Alien (1979)/poster.jpg", &first);
    fx.scan_and_match().await;
    // Their folders sort before the first's, so neither path nor folder order picks the first.
    fx.write("A second/Alien (1979).mkv", b"video");
    fx.write("A second/poster.jpg", &image(ImageFormat::Png, 2, 3));
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

#[tokio::test]
async fn deleting_the_first_added_items_poster_shows_the_next_items_at_the_next_scan() {
    let fx = Fixture::alien().await;
    fx.write("Alien (1979)/poster.jpg", &image(ImageFormat::Jpeg, 2, 3));
    fx.scan_and_match().await;
    let second = image(ImageFormat::Png, 2, 3);
    fx.write("A second/Alien (1979).mkv", b"video");
    fx.write("A second/poster.jpg", &second);
    fx.scan_and_match().await;

    fx.delete_file("Alien (1979)/poster.jpg");
    fx.scan_and_match().await;

    assert_eq!(fx.alien_images().await.0, second);
}

#[tokio::test]
async fn an_image_deleted_while_its_copy_was_gone_brings_the_downloaded_poster_back() {
    let fx = Fixture::alien().await;
    let downloaded = fx.alien_images().await;
    fx.write("Alien (1979)/poster.jpg", &image(ImageFormat::Jpeg, 2, 3));
    fx.scan_and_match().await;

    fs::remove_dir_all(fx.server.data_dir().join("local-images")).unwrap();
    fx.delete_file("Alien (1979)/poster.jpg");
    fx.scan_and_match().await;

    assert_eq!(fx.alien_images().await, downloaded);
}

#[tokio::test]
async fn a_poster_and_fanart_in_a_shows_folder_are_its_poster_and_backdrop_after_a_scan() {
    let fx = Fixture::dark().await;
    let poster = image(ImageFormat::Jpeg, 2, 3);
    let fanart = image(ImageFormat::Png, 3, 2);
    fx.write("Dark (2017)/poster.jpg", &poster);
    fx.write("Dark (2017)/fanart.png", &fanart);

    fx.scan_and_match().await;

    assert_eq!(fx.dark_images().await, (poster, fanart));
    let show = fx.get_dark().await.show.into_option().unwrap();
    assert!(!show.custom_poster && !show.custom_backdrop);
    let listed = fx.browse.list_shows(ListShowsRequest {
        library_id: fx.library,
        ..Default::default()
    });
    let listed = listed.await.unwrap().into_owned().shows;
    assert_eq!(listed[0].poster_url, show.poster_url);
}

#[tokio::test]
async fn a_poster_in_a_season_folder_is_that_seasons_poster_and_not_the_shows() {
    let fx = Fixture::dark().await;
    let downloaded = fx.dark_images().await;
    let season = image(ImageFormat::Jpeg, 2, 3);
    fx.write("Dark (2017)/Season 01/poster.jpg", &season);

    fx.scan_and_match().await;

    assert_eq!(fx.dark_season_poster(1).await, (season, false));
    assert_eq!(fx.dark_images().await, downloaded);
}

#[tokio::test]
async fn season_posters_in_the_shows_folder_go_by_number_and_the_seasons_own_folder_goes_first() {
    let fx = Fixture::dark().await;
    fx.write("Dark (2017)/Season 02/Dark - S02E01.mkv", b"video");
    fx.write("Dark (2017)/Specials/Dark - S00E01.mkv", b"video");
    let own = image(ImageFormat::Jpeg, 2, 3);
    let second = image(ImageFormat::Png, 2, 3);
    let specials = image(ImageFormat::WebP, 2, 3);
    fx.write("Dark (2017)/Season 01/poster.jpg", &own);
    fx.write(
        "Dark (2017)/season01-poster.png",
        &image(ImageFormat::Png, 4, 6),
    );
    fx.write("Dark (2017)/season02-poster.png", &second);
    fx.write("Dark (2017)/season-specials-poster.webp", &specials);

    fx.scan_and_match().await;

    let posters = [
        fx.dark_season_poster(0).await.0,
        fx.dark_season_poster(1).await.0,
        fx.dark_season_poster(2).await.0,
    ];
    assert_eq!(posters, [specials, own, second]);
}

#[tokio::test]
async fn a_poster_beside_episodes_straight_in_the_shows_folder_is_the_shows() {
    let fx = Fixture::start_on(LibraryKind::LIBRARY_KIND_SHOWS, TestServer::builder()).await;
    fx.write("Dark (2017)/Dark - S01E01.mkv", b"video");
    let poster = image(ImageFormat::Jpeg, 2, 3);
    fx.write("Dark (2017)/poster.jpg", &poster);

    fx.scan_and_match().await;

    assert_eq!(fx.dark_images().await.0, poster);
    assert_ne!(fx.dark_season_poster(1).await.0, poster);
}

#[tokio::test]
async fn deleting_the_poster_in_a_shows_folder_brings_the_downloaded_one_back_at_the_next_scan() {
    let fx = Fixture::dark().await;
    let downloaded = fx.dark_images().await;
    let beside = image(ImageFormat::Jpeg, 2, 3);
    fx.write("Dark (2017)/poster.jpg", &beside);
    fx.scan_and_match().await;
    let (with_file, _) = fx.dark_images().await;

    fx.delete_file("Dark (2017)/poster.jpg");
    fx.scan_and_match().await;

    assert_eq!(with_file, beside);
    assert_eq!(fx.dark_images().await, downloaded);
}

#[tokio::test]
async fn of_a_shows_two_folders_each_with_a_poster_the_first_added_episodes_shows() {
    let fx = Fixture::dark().await;
    let first = image(ImageFormat::Jpeg, 2, 3);
    fx.write("Dark (2017)/poster.jpg", &first);
    fx.scan_and_match().await;
    // Their folders sort before the first's, so neither path nor folder order picks the first.
    fx.write("A second/Dark (2017)/Season 01/Dark - S01E02.mkv", b"video");
    fx.write(
        "A second/Dark (2017)/poster.jpg",
        &image(ImageFormat::Png, 2, 3),
    );
    fx.scan_and_match().await;
    let (with_two, _) = fx.dark_images().await;

    fx.write("A third/Dark (2017)/Season 02/Dark - S02E01.mkv", b"video");
    fx.write(
        "A third/Dark (2017)/poster.jpg",
        &image(ImageFormat::WebP, 2, 3),
    );
    fx.scan_and_match().await;
    let (with_three, _) = fx.dark_images().await;

    assert_eq!((with_two, with_three), (first.clone(), first));
}

#[tokio::test]
async fn an_image_in_a_shows_folder_that_a_custom_image_could_not_be_is_skipped_and_logged() {
    let log = Log::global();
    let fx = Fixture::dark().await;
    let downloaded = fx.dark_images().await;
    fx.write("Dark (2017)/poster.jpg", b"a text file named poster.jpg");

    fx.scan_and_match().await;

    assert_eq!(fx.dark_images().await, downloaded);
    let poster = fx.folder.join("Dark (2017)/poster.jpg");
    let poster = poster.to_string_lossy();
    let not_an_image = ["skipped", &poster, "isn't a JPEG, PNG or WebP image"];
    assert_eq!(log.count(&not_an_image), 1);
}

#[tokio::test]
async fn a_shows_poster_in_its_folder_shows_on_its_search_hit_and_look_up() {
    let fx = Fixture::start_on(LibraryKind::LIBRARY_KIND_SHOWS, TestServer::builder()).await;
    fx.write(DARK_S01E01, b"video");
    fx.write(
        "The Office {tmdb-2316}/Season 01/The Office - S01E01.mkv",
        b"video",
    );
    fx.write(
        "The Office {tmdb-2316}/poster.jpg",
        &image(ImageFormat::Jpeg, 2, 3),
    );
    fx.write("Dark (2017)/poster.jpg", &image(ImageFormat::Png, 2, 3));
    fx.scan_and_match().await;

    let requests = fx.requests();
    let searched = search::shows(&fx.server.client_config(), "The Office").await;
    let looked_up = requests.look_up_show(LookUpShowRequest {
        show_id: tmdb_id(DARK).into(),
        ..Default::default()
    });
    let looked_up = looked_up.await.unwrap().into_owned().show.into_option();

    let office = (searched.iter())
        .find(|hit| *hit.id == tmdb_id(THE_OFFICE))
        .unwrap();
    let office_poster = office.poster_url.clone().unwrap();
    let dark = fx.get_dark().await.show.into_option().unwrap();
    assert!(
        office_poster.starts_with("/media/local-images/"),
        "{office_poster}"
    );
    assert!(
        dark.poster_url
            .as_deref()
            .unwrap()
            .starts_with("/media/local-images/")
    );
    assert_eq!(looked_up.unwrap().poster_url, dark.poster_url);
}
