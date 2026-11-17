//! Custom Images: a Poster or Backdrop an admin uploads for a Movie, Show or Season, which
//! everyone then sees in its place until it is removed (ADR 0036), over a real socket with the
//! TMDB Plugin enabled against a fake TMDB.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/browsing.rs"]
mod browsing;
#[path = "support/custom_images.rs"]
mod custom_images;
#[path = "support/http.rs"]
mod http;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::fs;

use browsing::{ALIEN_1080P, ALIEN_2160P, Fixture, tmdb_id};
use connectrpc::client::HttpClient;
use custom_images::image;
use http::{Reply, fetch};
use image::ImageFormat;
use nama_proto::nama::v1::{
    BrowseServiceClient, ContinueWatchingEntry, LibraryKind, LibraryServiceClient,
    ListArtworkRequest, ListContinueWatchingRequest, ListEpisodesRequest, ListLibrariesRequest,
    LookUpMovieRequest, LookUpShowRequest, ReportProgressRequest, RequestServiceClient,
    SearchLibrariesRequest, SearchMoviesRequest, SearchShowsRequest, SetMatchRequest, Show,
    ShowInLibrary, TitleKind, WatchStateServiceClient, continue_watching_entry::MovieOrEpisode,
};
use server::tmdb::{ALIEN, DARK, DARK_TVDB, DUNE, THE_OFFICE};

/// Its id hint Matches it to the US The Office, which a search alone ties with the UK one.
const THE_OFFICE_PILOT: &str = "The Office {tmdb-2316}/Season 01/The Office - S01E01.mkv";

/// Where Alien's Poster is uploaded to and removed from.
fn alien_poster() -> String {
    format!("/custom-images/movie/tmdb-{ALIEN}/poster")
}

/// Alien's second English Poster in TMDB's artwork list, the one besides the picked Poster whose
/// bytes the fake TMDB has.
const SECOND_POSTER: &str = "2xcXrPA5kHJu8a7BxRnwV4tz0ws.jpg";

#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    /// The image at `url`, fetched as the Admin.
    async fn fetch(&self, url: &str) -> Reply {
        fetch(&self.server, url, Some(&self.server.admin_token)).await
    }

    /// The URLs of Alien's Posters to pick from, as `ListArtwork` gives them to the Admin.
    async fn alien_poster_urls(&self) -> Vec<String> {
        let libraries =
            LibraryServiceClient::new(HttpClient::plaintext(), self.server.client_config());
        let listed = libraries.list_artwork(ListArtworkRequest {
            kind: TitleKind::TITLE_KIND_MOVIE.into(),
            id: tmdb_id(ALIEN).into(),
            ..Default::default()
        });
        listed.await.unwrap().into_owned().poster_urls
    }

    /// Picks the artwork at `url` as Alien's Poster, as the Admin.
    async fn pick_alien_poster(&self, url: &str) -> Reply {
        let url: String = url::form_urlencoded::byte_serialize(url.as_bytes()).collect();
        self.upload(&format!("{}?artwork={url}", alien_poster()), &[])
            .await
    }

    /// Alien's Poster and whether it is a Custom Image, as `GetMovie` gives them in the Library
    /// `library`.
    async fn alien_poster(&self, library: i64) -> (String, bool) {
        let movie = self.get_movie(library, ALIEN).await.unwrap();
        (movie.poster_url.unwrap(), movie.custom_poster)
    }

    /// The Admin's Continue Watching, after starting Dark's first Episode.
    async fn continue_watching_dark(&self) -> Vec<ContinueWatchingEntry> {
        let config = self.server.client_config();
        let episodes = self
            .browse
            .list_episodes(ListEpisodesRequest {
                library_id: self.library,
                show_id: tmdb_id(DARK).into(),
                season_number: 1,
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned()
            .episodes;
        WatchStateServiceClient::new(HttpClient::plaintext(), config.clone())
            .report_progress(ReportProgressRequest {
                version_id: episodes[0].versions[0].id,
                position_ms: 60_000,
                duration_ms: 3_600_000,
                ..Default::default()
            })
            .await
            .unwrap();
        BrowseServiceClient::new(HttpClient::plaintext(), config)
            .list_continue_watching(ListContinueWatchingRequest::default())
            .await
            .unwrap()
            .into_owned()
            .entries
    }

    /// Dark, as a search for it finds it.
    async fn search_dark(&self) -> ShowInLibrary {
        let found = self
            .browse
            .search_libraries(SearchLibrariesRequest {
                query: "dark".to_owned(),
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned();
        found.shows[0].clone()
    }

    fn requests(&self) -> RequestServiceClient<HttpClient> {
        RequestServiceClient::new(HttpClient::plaintext(), self.server.client_config())
    }
}

/// The Show of `entry`, an Episode.
fn show_of(entry: &ContinueWatchingEntry) -> Option<&Show> {
    match &entry.movie_or_episode {
        Some(MovieOrEpisode::Episode(started)) => started.show.as_option(),
        _ => None,
    }
}

#[tokio::test]
async fn a_jpeg_png_or_webp_uploaded_as_a_movies_poster_is_the_one_browse_returns() {
    let fx = Fixture::alien().await;

    for (format, content_type) in [
        (ImageFormat::Jpeg, "image/jpeg"),
        (ImageFormat::Png, "image/png"),
        (ImageFormat::WebP, "image/webp"),
    ] {
        let bytes = image(format, 2, 3);
        let uploaded = fx.upload(&alien_poster(), &bytes).await;
        let movie = fx.get_movie(fx.library, ALIEN).await.unwrap();
        let served = fx.fetch(&movie.poster_url.unwrap()).await;

        assert_eq!(uploaded.status, 204, "{content_type}");
        assert!(movie.custom_poster, "{content_type}");
        assert_eq!(served.status, 200, "{content_type}");
        assert_eq!(served.header("content-type"), content_type);
        assert_eq!(served.body, bytes, "{content_type}");
    }
}

/// Too big, not an image and too many pixels each have a status of their own, which the web app
/// tells apart.
#[tokio::test]
async fn an_upload_too_big_not_an_image_or_over_12_megapixels_is_refused_and_changes_nothing() {
    let fx = Fixture::alien().await;
    let (downloaded, _) = fx.alien_poster(fx.library).await;
    let over_20_mib = vec![0; 20 * 1024 * 1024 + 1];
    // A 2 by 3 GIF's header: `image` here encodes no GIF.
    let gif = b"GIF89a\x02\x00\x03\x00\x00\x00\x00;".to_vec();
    let svg = b"<svg xmlns='http://www.w3.org/2000/svg'/>".to_vec();
    let text = b"a text file named poster.jpg".to_vec();
    let over_12_megapixels = image(ImageFormat::Png, 4000, 3001);

    for (bytes, status) in [
        (over_20_mib, 413),
        (gif, 415),
        (svg, 415),
        (text, 415),
        (over_12_megapixels, 422),
    ] {
        let refused = fx.upload(&alien_poster(), &bytes).await;

        assert_eq!(refused.status, status);
        assert_eq!(
            fx.alien_poster(fx.library).await,
            (downloaded.clone(), false)
        );
    }
}

#[tokio::test]
async fn removing_a_custom_image_brings_the_downloaded_one_back() {
    let fx = Fixture::alien().await;
    let (downloaded, _) = fx.alien_poster(fx.library).await;
    fx.upload(&alien_poster(), &image(ImageFormat::Png, 2, 3))
        .await;
    let (custom, _) = fx.alien_poster(fx.library).await;

    let removed = fx.remove(&alien_poster()).await;

    assert_eq!(removed.status, 204);
    assert_eq!(fx.alien_poster(fx.library).await, (downloaded, false));
    assert_eq!(fx.fetch(&custom).await.status, 404);
}

#[tokio::test]
async fn an_admin_picks_a_poster_from_the_artwork_list_and_it_is_kept_as_a_custom_image() {
    let fx = Fixture::alien().await;
    let (downloaded, _) = fx.alien_poster(fx.library).await;
    let urls = fx.alien_poster_urls().await;
    let second = urls
        .iter()
        .find(|url| url.ends_with(SECOND_POSTER))
        .unwrap();

    let picked = fx.pick_alien_poster(second).await;

    let (custom, is_custom) = fx.alien_poster(fx.library).await;
    let fixture = format!(
        "{}/../../plugins/tmdb/fixtures/images/{SECOND_POSTER}",
        env!("CARGO_MANIFEST_DIR")
    );
    assert_eq!(picked.status, 204);
    assert!(is_custom);
    assert!(custom.starts_with("/media/custom-images/"), "{custom}");
    assert_eq!(fx.fetch(&custom).await.body, fs::read(fixture).unwrap());
    // Best first: the downloaded Poster is the one Artwork picked.
    assert_eq!(urls.first(), Some(&downloaded));

    fx.remove(&alien_poster()).await;
    assert_eq!(fx.alien_poster(fx.library).await, (downloaded, false));
}

#[tokio::test]
async fn picking_artwork_the_server_never_offered_is_refused_and_changes_nothing() {
    let fx = Fixture::alien().await;
    let before = fx.alien_poster(fx.library).await;

    let refused = fx
        .pick_alien_poster(&format!("/media/artwork/tmdb/%2F{SECOND_POSTER}"))
        .await;

    assert_eq!(refused.status, 404);
    assert_eq!(fx.alien_poster(fx.library).await, before);
}

#[tokio::test]
async fn replacing_a_custom_image_changes_its_url() {
    let fx = Fixture::alien().await;
    fx.upload(&alien_poster(), &image(ImageFormat::Png, 2, 3))
        .await;
    let (first, _) = fx.alien_poster(fx.library).await;

    let replacement = image(ImageFormat::Png, 3, 2);
    fx.upload(&alien_poster(), &replacement).await;

    let (second, custom) = fx.alien_poster(fx.library).await;
    assert!(custom);
    assert_ne!(second, first);
    assert_eq!(fx.fetch(&first).await.status, 404);
    assert_eq!(fx.fetch(&second).await.body, replacement);
}

#[tokio::test]
async fn a_custom_image_survives_its_file_being_renamed_and_matched_again_and_a_scan() {
    let fx = Fixture::alien().await;
    fx.upload(&alien_poster(), &image(ImageFormat::Png, 2, 3))
        .await;
    let uploaded = fx.alien_poster(fx.library).await;

    fs::rename(
        fx.folder.join(ALIEN_1080P),
        fx.folder
            .join("Alien (1979)/Alien (1979) - Director's Cut.mkv"),
    )
    .unwrap();
    fx.scan_and_match().await;
    let renamed = fx.alien_poster(fx.library).await;
    fx.scan_and_match().await;

    assert_eq!(renamed, uploaded);
    assert_eq!(fx.alien_poster(fx.library).await, uploaded);
}

#[tokio::test]
async fn correcting_the_match_shows_the_other_titles_images_and_correcting_it_back_the_custom_one()
{
    let fx = Fixture::alien().await;
    fx.upload(&alien_poster(), &image(ImageFormat::Png, 2, 3))
        .await;
    let uploaded = fx.alien_poster(fx.library).await;
    let set_match = |tmdb| SetMatchRequest {
        item_id: fx.item(ALIEN_1080P),
        title_id: tmdb_id(tmdb).into(),
        ..Default::default()
    };

    fx.matches.set_match(set_match(DUNE)).await.unwrap();
    let dune = fx.get_movie(fx.library, DUNE).await.unwrap();
    fx.matches.set_match(set_match(ALIEN)).await.unwrap();

    assert!(!dune.custom_poster);
    assert!(dune.poster_url.unwrap().starts_with("/media/artwork/"));
    assert_eq!(fx.alien_poster(fx.library).await, uploaded);
}

#[tokio::test]
async fn a_custom_image_is_still_there_once_the_titles_last_file_leaves_and_a_new_one_is_matched() {
    let fx = Fixture::alien().await;
    fx.upload(&alien_poster(), &image(ImageFormat::Png, 2, 3))
        .await;
    let uploaded = fx.alien_poster(fx.library).await;

    fx.remove_file(ALIEN_1080P);
    fx.scan_and_match().await;
    let gone = fx.get_movie(fx.library, ALIEN).await;
    fx.write(ALIEN_2160P, b"a better copy");
    fx.scan_and_match().await;

    assert!(gone.is_err(), "Alien should have left the Library");
    assert_eq!(fx.alien_poster(fx.library).await, uploaded);
}

#[tokio::test]
async fn the_same_movie_in_two_libraries_shows_the_one_custom_image_in_both() {
    let fx = Fixture::alien().await;
    let other = fx
        .server
        .create_library(LibraryKind::LIBRARY_KIND_MOVIES, &[ALIEN_1080P])
        .await;
    fx.server.scan_and_match(&other).await;

    fx.upload(&alien_poster(), &image(ImageFormat::Png, 2, 3))
        .await;

    let (url, custom) = fx.alien_poster(fx.library).await;
    assert!(custom);
    assert_eq!(fx.alien_poster(other.id).await, (url, true));
}

#[tokio::test]
async fn a_seasons_custom_poster_shows_on_the_season_and_not_as_its_shows() {
    let fx = Fixture::dark().await;

    let uploaded = fx
        .upload(
            &format!("/custom-images/show/tmdb-{DARK}/season/1/poster"),
            &image(ImageFormat::Png, 2, 3),
        )
        .await;

    assert_eq!(uploaded.status, 204);
    let dark = fx.get_dark().await;
    let season = &dark.seasons[0];
    assert!(season.custom_poster);
    let poster = season.poster_url.as_deref().unwrap();
    assert!(poster.starts_with("/media/custom-images/show/tmdb-70523/season/1/poster/"));
    let show = dark.show.as_option().unwrap();
    let started = fx.continue_watching_dark().await;
    let dark_poster = Some("/media/artwork/tmdb/%2FapbrbWs8M9lyOpJYU5WXrpFbk1Z.jpg");
    assert_eq!(
        (show.poster_url.as_deref(), show.custom_poster),
        (dark_poster, false)
    );
    assert_eq!(
        show_of(&started[0]).unwrap().poster_url.as_deref(),
        dark_poster
    );
}

#[tokio::test]
async fn a_shows_custom_poster_shows_in_continue_watching_and_on_a_request_for_it() {
    let fx = Fixture::dark().await;

    fx.upload(
        &format!("/custom-images/show/tmdb-{DARK}/poster"),
        &image(ImageFormat::Png, 2, 3),
    )
    .await;

    let show = fx.get_dark().await.show.into_option().unwrap();
    assert!(show.custom_poster);
    let poster = show.poster_url;
    let started = fx.continue_watching_dark().await;
    assert_eq!(show_of(&started[0]).unwrap().poster_url, poster);
    let found = fx.search_dark().await;
    assert_eq!(found.poster_url, poster);
    let looked_up = fx
        .requests()
        .look_up_show(LookUpShowRequest {
            show_id: tmdb_id(DARK).into(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .show
        .into_option()
        .unwrap();
    assert_eq!(looked_up.poster_url, poster);
}

#[tokio::test]
async fn a_shows_custom_backdrop_is_the_backdrop_continue_watching_returns_for_its_episodes() {
    let fx = Fixture::dark().await;

    fx.upload(
        &format!("/custom-images/show/tmdb-{DARK}/backdrop"),
        &image(ImageFormat::Png, 3, 2),
    )
    .await;

    let show = fx.get_dark().await.show.into_option().unwrap();
    assert!(show.custom_backdrop);
    let backdrop = show.backdrop_url.unwrap();
    assert!(backdrop.starts_with("/media/custom-images/show/tmdb-70523/backdrop/"));
    let started = fx.continue_watching_dark().await;
    assert_eq!(show_of(&started[0]).unwrap().backdrop_url, Some(backdrop));
}

#[tokio::test]
async fn a_movies_custom_backdrop_shows_on_a_request_for_it() {
    let fx = Fixture::alien().await;

    fx.upload(
        &format!("/custom-images/movie/tmdb-{ALIEN}/backdrop"),
        &image(ImageFormat::Png, 3, 2),
    )
    .await;

    let movie = fx.get_movie(fx.library, ALIEN).await.unwrap();
    assert!(movie.custom_backdrop);
    let looked_up = fx
        .requests()
        .look_up_movie(LookUpMovieRequest {
            movie_id: tmdb_id(ALIEN).into(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .movie
        .into_option()
        .unwrap();
    assert_eq!(looked_up.backdrop_url, movie.backdrop_url);
}

#[tokio::test]
async fn a_path_naming_no_slot_is_not_found() {
    let fx = Fixture::alien().await;

    for path in [
        "/custom-images/movie/tmdb-348/logo",
        "/custom-images/movie/Tmdb-348/poster",
        "/custom-images/show/tmdb-70523/season/1/backdrop",
        "/custom-images/episode/tmdb-1/poster",
    ] {
        let reply = fx.upload(path, &image(ImageFormat::Png, 2, 3)).await;

        assert_eq!(reply.status, 404, "{path}");
    }
}

#[tokio::test]
async fn a_custom_image_gets_a_thumbhash_of_its_own_once_resized_and_a_new_one_replaces_it() {
    let fx = Fixture::alien().await;
    let thumbhash = || async {
        fx.get_movie(fx.library, ALIEN)
            .await
            .unwrap()
            .poster_thumbhash
    };
    let resize = || async {
        let (url, _) = fx.alien_poster(fx.library).await;
        assert_eq!(fx.fetch(&format!("{url}?width=640")).await.status, 200);
    };

    fx.upload(&alien_poster(), &image(ImageFormat::Jpeg, 1000, 1500))
        .await;
    let before_resizing = thumbhash().await;
    resize().await;
    let first = thumbhash().await.unwrap();
    fx.upload(&alien_poster(), &image(ImageFormat::Png, 1500, 1000))
        .await;
    let replaced = thumbhash().await;
    resize().await;
    let second = thumbhash().await.unwrap();

    assert_eq!(before_resizing, None);
    assert_eq!(replaced, None);
    // A thumbhash keeps its image's shape only roughly.
    let shape = |hash: &[u8]| thumbhash::thumb_hash_to_approximate_aspect_ratio(hash).unwrap();
    assert!(
        shape(&first) < 1.0,
        "the first, a portrait, is {}",
        shape(&first)
    );
    assert!(
        shape(&second) > 1.0,
        "the second, a landscape, is {}",
        shape(&second)
    );
}

#[tokio::test]
async fn a_custom_image_removed_and_uploaded_again_gets_its_thumbhash_back_once_resized() {
    let fx = Fixture::alien().await;
    let jpeg = image(ImageFormat::Jpeg, 1000, 1500);
    let resize = || async {
        let (url, _) = fx.alien_poster(fx.library).await;
        assert_eq!(fx.fetch(&format!("{url}?width=640")).await.status, 200);
    };
    fx.upload(&alien_poster(), &jpeg).await;
    resize().await;

    fx.remove(&alien_poster()).await;
    // The same bytes, so the same URL as before.
    fx.upload(&alien_poster(), &jpeg).await;
    resize().await;

    let movie = fx.get_movie(fx.library, ALIEN).await.unwrap();
    assert!(movie.poster_thumbhash.is_some());
}

/// Values are text, not numbers, so a leading zero names another title, which no Library has.
#[tokio::test]
async fn an_upload_for_tmdb_0348_is_not_found_and_leaves_tmdb_348s_poster_alone() {
    let fx = Fixture::alien().await;
    let downloaded = fx.alien_poster(fx.library).await;

    let uploaded = fx
        .upload(
            "/custom-images/movie/tmdb-0348/poster",
            &image(ImageFormat::Png, 2, 3),
        )
        .await;

    assert_eq!(uploaded.status, 404);
    assert_eq!(fx.alien_poster(fx.library).await, downloaded);
}

#[tokio::test]
async fn an_upload_for_a_title_no_library_has_is_not_found_and_kept_nowhere_but_removing_succeeds()
{
    let fx = Fixture::alien().await;
    let mut uploads = vec![
        ("movie/tmdb-1/poster", image(ImageFormat::Png, 2, 3)),
        ("movie/tmdb-1/backdrop", image(ImageFormat::Png, 3, 2)),
        ("show/tmdb-1/poster", image(ImageFormat::Png, 2, 3)),
        ("show/tmdb-1/backdrop", image(ImageFormat::Png, 3, 2)),
        // Alien is a Movie, not a Show.
        ("show/tmdb-348/poster", image(ImageFormat::Png, 2, 3)),
    ];
    // Refused before its body is read, so not as too big. Windows resets a connection closed with
    // a body still unread, so the client there loses the 404 while it's still sending.
    if cfg!(not(windows)) {
        uploads.push(("movie/tmdb-1/poster", vec![0; 20 * 1024 * 1024 + 1]));
    }

    for (path, bytes) in uploads {
        let path = format!("/custom-images/{path}");
        let refused = fx.upload(&path, &bytes).await;

        assert_eq!(refused.status, 404, "{path}");
        assert_eq!(fx.remove(&path).await.status, 204, "{path}");
    }
    assert!(!fx.server.data_dir().join("custom-images").exists());
}

#[tokio::test]
async fn an_upload_for_a_season_or_movie_a_shows_library_lacks_is_not_found() {
    let fx = Fixture::dark().await;

    for path in [
        format!("show/tmdb-{DARK}/season/99/poster"),
        format!("show/tmdb-{DARK}/season/2/poster"),
        // Dark is a Show, not a Movie.
        format!("movie/tmdb-{DARK}/poster"),
    ] {
        let refused = fx
            .upload(
                &format!("/custom-images/{path}"),
                &image(ImageFormat::Png, 2, 3),
            )
            .await;

        assert_eq!(refused.status, 404, "{path}");
    }
    assert!(!fx.server.data_dir().join("custom-images").exists());
}

#[tokio::test]
async fn removing_a_custom_image_once_its_title_has_left_the_server_still_removes_it() {
    let fx = Fixture::alien().await;
    fx.upload(&alien_poster(), &image(ImageFormat::Png, 2, 3))
        .await;
    fx.remove_file(ALIEN_1080P);
    fx.scan_and_match().await;

    let removed = fx.remove(&alien_poster()).await;

    assert_eq!(removed.status, 204);
    let refused = fx
        .upload(&alien_poster(), &image(ImageFormat::Png, 2, 3))
        .await;
    assert_eq!(refused.status, 404);
}

#[tokio::test]
async fn a_movies_custom_images_kept_under_its_imdb_id_show_everywhere_it_does() {
    let fx = Fixture::alien().await;
    let images = "/custom-images/movie/imdb-tt0078748";
    fx.upload(&format!("{images}/poster"), &image(ImageFormat::Png, 2, 3))
        .await;
    fx.upload(
        &format!("{images}/backdrop"),
        &image(ImageFormat::Png, 3, 2),
    )
    .await;

    let movie = fx.get_movie(fx.library, ALIEN).await.unwrap();
    let looked_up = fx
        .requests()
        .look_up_movie(LookUpMovieRequest {
            movie_id: tmdb_id(ALIEN).into(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .movie
        .into_option()
        .unwrap();

    assert!(movie.custom_poster && movie.custom_backdrop);
    let poster = movie.poster_url.unwrap();
    assert!(
        poster.starts_with(&format!("/media{images}/poster/")),
        "{poster}"
    );
    assert_eq!(looked_up.poster_url, Some(poster.clone()));
    assert_eq!(looked_up.backdrop_url, movie.backdrop_url);
    let found = (fx.browse)
        .search_libraries(SearchLibrariesRequest {
            query: "alien".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .movies;
    assert_eq!(found[0].poster_url, Some(poster));
    let libraries = LibraryServiceClient::new(HttpClient::plaintext(), fx.server.client_config())
        .list_libraries(ListLibrariesRequest::default())
        .await
        .unwrap()
        .into_owned()
        .libraries;
    let library = libraries.iter().find(|it| it.id == fx.library).unwrap();
    assert_eq!(library.backdrop_url, movie.backdrop_url);
}

#[tokio::test]
async fn a_shows_and_seasons_custom_images_kept_under_its_tvdb_id_show_in_browse_and_on_a_request()
{
    let fx = Fixture::dark().await;
    let images = format!("/custom-images/show/tvdb-{DARK_TVDB}");
    for (path, width, height) in [
        ("poster", 2, 3),
        ("backdrop", 3, 2),
        ("season/1/poster", 2, 3),
    ] {
        let uploaded = fx
            .upload(
                &format!("{images}/{path}"),
                &image(ImageFormat::Png, width, height),
            )
            .await;
        assert_eq!(uploaded.status, 204, "{path}");
    }

    let dark = fx.get_dark().await;
    let looked_up = fx
        .requests()
        .look_up_show(LookUpShowRequest {
            show_id: tmdb_id(DARK).into(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .show
        .into_option()
        .unwrap();

    let show = dark.show.into_option().unwrap();
    assert!(show.custom_poster && show.custom_backdrop);
    assert!(dark.seasons[0].custom_poster);
    let backdrop = show.backdrop_url.unwrap();
    assert!(
        backdrop.starts_with(&format!("/media{images}/backdrop/")),
        "{backdrop}"
    );
    assert_eq!(looked_up.poster_url, show.poster_url);
    assert_eq!(looked_up.backdrop_url, Some(backdrop));
}

/// The fake TMDB gives a search hit only its tmdb id, so this proves the hit's own id; the path
/// through a hit's other ids is proved in `tests/tvdb.rs`, whose TVDB hits carry several.
#[tokio::test]
async fn a_custom_poster_kept_under_a_search_hits_id_shows_on_the_hit() {
    let fx = Fixture::alien().await;
    let _shows = (fx.server)
        .matched_library(LibraryKind::LIBRARY_KIND_SHOWS, &[THE_OFFICE_PILOT])
        .await;
    for path in [
        format!("/custom-images/movie/tmdb-{ALIEN}/poster"),
        format!("/custom-images/show/tmdb-{THE_OFFICE}/poster"),
    ] {
        let uploaded = fx.upload(&path, &image(ImageFormat::Png, 2, 3)).await;
        assert_eq!(uploaded.status, 204, "{path}");
    }

    let movies = fx
        .requests()
        .search_movies(SearchMoviesRequest {
            title: "Alien".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .movies;
    let shows = fx
        .requests()
        .search_shows(SearchShowsRequest {
            title: "The Office".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .shows;

    let alien = movies.iter().find(|it| *it.id == tmdb_id(ALIEN)).unwrap();
    let office = (shows.iter())
        .find(|it| *it.id == tmdb_id(THE_OFFICE))
        .unwrap();
    let movie = fx.get_movie(fx.library, ALIEN).await.unwrap();
    assert_eq!(alien.poster_url, movie.poster_url);
    let poster = office.poster_url.clone().unwrap();
    assert!(
        poster.starts_with(&format!(
            "/media/custom-images/show/tmdb-{THE_OFFICE}/poster/"
        )),
        "{poster}"
    );
}
