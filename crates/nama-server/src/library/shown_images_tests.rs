use std::{iter, sync::Arc};

use nama_plugin_api::{
    PluginId,
    metadata::{ExternalId, ExternalIds},
};
use tempfile::TempDir;

use super::*;
use crate::{
    db::Database,
    images::ImageStore,
    library::{
        contents::{LocalRole, TitleLocalImage},
        libraries::Kind as LibraryKind,
    },
    plugins::Registry,
};

const POSTER: TitleImage = TitleImage::Movie(Role::Poster);
const BACKDROP: TitleImage = TitleImage::Movie(Role::Backdrop);
const SHOW_POSTER: TitleImage = TitleImage::Show(Role::Poster);
const SHOW_BACKDROP: TitleImage = TitleImage::Show(Role::Backdrop);
const SEASON_1: TitleImage = TitleImage::SeasonPoster(1);

fn tmdb(id: i64) -> ExternalId {
    ExternalId::tmdb(id)
}

fn imdb(id: &str) -> ExternalId {
    ExternalId::new("imdb", id.to_owned()).unwrap()
}

/// Every id of the title known as `id` and `others`.
fn ids(id: &ExternalId, others: &[&ExternalId]) -> ExternalIds {
    iter::once(id)
        .chain(others.iter().copied())
        .cloned()
        .collect()
}

/// The Custom Image URL the snapshot keeps at `slot`.
fn custom(slot: &Slot) -> String {
    format!("/custom/{slot:?}")
}

/// The URL of the image beside the media the snapshot keeps as `stored`.
fn local(stored: &str) -> String {
    format!("/media/local-images/1/{stored}")
}

fn stored(path: &str) -> Downloaded<'_> {
    Downloaded::Stored {
        plugin: "tmdb",
        path,
    }
}

/// What a caller gets when `url` shows: it with its thumbhash, here its own bytes.
fn showing(url: &str, custom: bool) -> ShownImage {
    ShownImage {
        url: Some(url.to_owned()),
        thumbhash: Some(url.as_bytes().to_vec()),
        custom,
    }
}

fn library(id: i64, kind: LibraryKind) -> Library {
    Library {
        id,
        name: String::new(),
        kind,
        folders: Vec::new(),
        read_only: Vec::new(),
        empty_folders: Vec::new(),
        title_sources: Vec::new(),
        recognisers: Vec::new(),
        segment_sources: Vec::new(),
        preview_frames: false,
        last_scan: None,
    }
}

fn newest(title: &ExternalId, path: &str) -> NewestBackdrop {
    NewestBackdrop {
        plugin: "tmdb".to_owned(),
        path: path.to_owned(),
        title: title.clone(),
        ids: ids(title, &[]),
    }
}

/// Artwork over an image store in `data`, with no title source enabled.
async fn artwork_in(data: &TempDir) -> Artwork {
    let db = Database::open(data.path()).await.unwrap();
    let store = ImageStore::new(data.path(), Arc::new(db));
    Artwork::new(store, Registry::new().extension_points())
}

/// A response's images, each title with what its name says it has:
///
/// - Movie `tmdb-1` and Show `tmdb-1`: a Custom Poster and Backdrop and images beside the media.
/// - Show `tmdb-1`'s Season 1: a Custom Poster and one beside the media; Season 2: one beside.
/// - Movie and Show `tmdb-2`: images beside the media only.
/// - Movie `tmdb-5`: a Custom Poster kept under its other id, `imdb-tt5`.
/// - Movie `tmdb-6`: an image beside its media filed under its other id, `imdb-tt6`.
/// - Library 1: its own Custom Backdrop.
///
/// With `resized`, every URL has a thumbhash; else none has (ADR 0039).
fn snapshot(artwork: Artwork, resized: bool) -> ShownImages {
    let slots = [
        Slot::movie(&tmdb(1), Role::Poster),
        Slot::movie(&tmdb(1), Role::Backdrop),
        Slot::show(&tmdb(1), Role::Poster),
        Slot::show(&tmdb(1), Role::Backdrop),
        Slot::season(&tmdb(1), 1),
        Slot::movie(&imdb("tt5"), Role::Poster),
        Slot::library(1),
    ];
    let custom_urls = slots.each_ref().map(custom);
    let custom = CustomImageUrls::for_tests(slots.into_iter().zip(custom_urls.clone()));
    let image = |ids: ExternalIds, role, season, stored: &str| TitleLocalImage {
        ids,
        role,
        season,
        library: 1,
        stored: stored.to_owned(),
    };
    let poster = LocalRole::Title(Role::Poster);
    let backdrop = LocalRole::Title(Role::Backdrop);
    let season = LocalRole::SeasonPoster;
    let (one, two) = (ids(&tmdb(1), &[]), ids(&tmdb(2), &[]));
    let local_images = vec![
        image(one.clone(), poster, None, "movie-1-poster"),
        image(one.clone(), backdrop, None, "movie-1-backdrop"),
        image(one.clone(), poster, Some(1), "show-1-poster"),
        image(one.clone(), backdrop, Some(1), "show-1-backdrop"),
        image(one.clone(), season, Some(1), "season-1"),
        image(one, season, Some(2), "season-2"),
        image(two.clone(), poster, None, "movie-2-poster"),
        image(two.clone(), backdrop, None, "movie-2-backdrop"),
        image(two.clone(), poster, Some(1), "show-2-poster"),
        image(two, backdrop, Some(1), "show-2-backdrop"),
        image(ids(&imdb("tt6"), &[]), poster, None, "movie-6-poster"),
    ];
    let local_urls: Vec<_> = local_images
        .iter()
        .map(|title_image| local(&title_image.stored))
        .collect();
    let downloaded = ["/poster.jpg", "/backdrop.jpg", "/logo.png", "/offered.jpg"]
        .map(|path| artwork::url("tmdb", path));
    let thumbhashes = (custom_urls.into_iter())
        .chain(local_urls)
        .chain(downloaded)
        .filter(|_| resized)
        .map(|url| {
            let thumbhash = url.as_bytes().to_vec();
            (url, thumbhash)
        });
    let local = LocalImageUrls::filed(local_images);
    ShownImages {
        custom,
        local,
        thumbhashes: thumbhashes.collect(),
        artwork,
    }
}

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "one row per kind of image; split, the table would be read in pieces"
)]
async fn each_image_shows_its_custom_image_else_the_one_beside_its_media_else_the_downloaded_one() {
    let data = tempfile::tempdir().unwrap();
    let images = snapshot(artwork_in(&data).await, true);
    let unresized = snapshot(artwork_in(&data).await, false);
    let title = |id: i64, image, downloaded| {
        images.of_title(&tmdb(id), &ids(&tmdb(id), &[]), image, downloaded)
    };
    let poster = Some(stored("/poster.jpg"));
    let backdrop = Some(stored("/backdrop.jpg"));
    let poster_url = artwork::url("tmdb", "/poster.jpg");
    let backdrop_url = artwork::url("tmdb", "/backdrop.jpg");
    let (movies, shows) = (
        library(2, LibraryKind::Movies),
        library(2, LibraryKind::Shows),
    );

    let rows = [
        // Movies.
        (
            "a Movie's Custom Poster",
            title(1, POSTER, poster),
            showing(&custom(&Slot::movie(&tmdb(1), Role::Poster)), true),
        ),
        (
            "a Movie's Custom Backdrop",
            title(1, BACKDROP, backdrop),
            showing(&custom(&Slot::movie(&tmdb(1), Role::Backdrop)), true),
        ),
        (
            "a Movie's Poster beside its media",
            title(2, POSTER, poster),
            showing(&local("movie-2-poster"), false),
        ),
        (
            "a Movie's Backdrop beside its media",
            title(2, BACKDROP, backdrop),
            showing(&local("movie-2-backdrop"), false),
        ),
        (
            "a Movie's downloaded Poster",
            title(3, POSTER, poster),
            showing(&poster_url, false),
        ),
        (
            "a Movie's downloaded Backdrop",
            title(3, BACKDROP, backdrop),
            showing(&backdrop_url, false),
        ),
        (
            "a Movie with no Poster",
            title(3, POSTER, None),
            ShownImage::default(),
        ),
        (
            "a Movie's Custom Poster kept under its other id",
            images.of_title(&tmdb(5), &ids(&tmdb(5), &[&imdb("tt5")]), POSTER, poster),
            showing(&custom(&Slot::movie(&imdb("tt5"), Role::Poster)), true),
        ),
        (
            "a Movie's Poster beside its media filed under its other id",
            images.of_title(&tmdb(6), &ids(&tmdb(6), &[&imdb("tt6")]), POSTER, poster),
            showing(&local("movie-6-poster"), false),
        ),
        // Shows, which share ids with the Movies above but not their images.
        (
            "a Show's Custom Poster",
            title(1, SHOW_POSTER, poster),
            showing(&custom(&Slot::show(&tmdb(1), Role::Poster)), true),
        ),
        (
            "a Show's Custom Backdrop",
            title(1, SHOW_BACKDROP, backdrop),
            showing(&custom(&Slot::show(&tmdb(1), Role::Backdrop)), true),
        ),
        (
            "a Show's Poster beside its media",
            title(2, SHOW_POSTER, poster),
            showing(&local("show-2-poster"), false),
        ),
        (
            "a Show's Backdrop beside its media",
            title(2, SHOW_BACKDROP, backdrop),
            showing(&local("show-2-backdrop"), false),
        ),
        (
            "a Show's downloaded Poster",
            title(3, SHOW_POSTER, poster),
            showing(&poster_url, false),
        ),
        (
            "a Show with no Backdrop",
            title(3, SHOW_BACKDROP, None),
            ShownImage::default(),
        ),
        // Seasons, whose Custom Poster is not the Show's.
        (
            "a Season's Custom Poster",
            title(1, SEASON_1, poster),
            showing(&custom(&Slot::season(&tmdb(1), 1)), true),
        ),
        (
            "a Season's Poster beside its media",
            title(1, TitleImage::SeasonPoster(2), poster),
            showing(&local("season-2"), false),
        ),
        (
            "a Season's downloaded Poster",
            title(2, SEASON_1, poster),
            showing(&poster_url, false),
        ),
        (
            "a Season with no Poster",
            title(2, SEASON_1, None),
            ShownImage::default(),
        ),
        // Libraries: only their own Custom Backdrop is custom.
        (
            "a Library's Custom Backdrop",
            images.of_library(&library(1, LibraryKind::Movies), None),
            showing(&custom(&Slot::library(1)), true),
        ),
        (
            "a Library's newest Movie's Custom Backdrop",
            images.of_library(&movies, Some(&newest(&tmdb(1), "/backdrop.jpg"))),
            showing(&custom(&Slot::movie(&tmdb(1), Role::Backdrop)), false),
        ),
        (
            "a Library's newest Show's Backdrop beside its media",
            images.of_library(&shows, Some(&newest(&tmdb(2), "/backdrop.jpg"))),
            showing(&local("show-2-backdrop"), false),
        ),
        (
            "a Library's newest title's downloaded Backdrop",
            images.of_library(&movies, Some(&newest(&tmdb(3), "/backdrop.jpg"))),
            showing(&backdrop_url, false),
        ),
        (
            "a Library with no newest title",
            images.of_library(&movies, None),
            ShownImage::default(),
        ),
        // The titled Backdrop, Title Logo and an Episode's Still have only the downloaded one (#1339).
        (
            "a titled Backdrop, even beside a Custom Backdrop",
            images.downloaded(backdrop),
            showing(&backdrop_url, false),
        ),
        (
            "a Title Logo",
            images.downloaded(Some(stored("/logo.png"))),
            showing(&artwork::url("tmdb", "/logo.png"), false),
        ),
        ("no Still", images.downloaded(None), ShownImage::default()),
        // Thumbhashes.
        (
            "a Custom Poster before any is resized",
            unresized.of_title(&tmdb(1), &ids(&tmdb(1), &[]), POSTER, poster),
            ShownImage {
                thumbhash: None,
                ..showing(&custom(&Slot::movie(&tmdb(1), Role::Poster)), true)
            },
        ),
        (
            "a downloaded Poster before any is resized",
            unresized.of_title(&tmdb(3), &ids(&tmdb(3), &[]), POSTER, poster),
            ShownImage {
                thumbhash: None,
                ..showing(&poster_url, false)
            },
        ),
    ];

    for (case, shown, expected) in rows {
        assert_eq!(shown, expected, "{case}");
    }
}

#[tokio::test]
async fn an_offered_image_is_offered_only_when_it_shows() {
    let data = tempfile::tempdir().unwrap();
    let artwork = artwork_in(&data).await;
    let images = snapshot(artwork.clone(), true);
    let tmdb_plugin = PluginId::from_static("tmdb");
    let offered = |path| {
        Some(Downloaded::Offered {
            plugin: &tmdb_plugin,
            path,
        })
    };

    let hidden = images.of_title(
        &tmdb(1),
        &ids(&tmdb(1), &[]),
        POSTER,
        offered("/hidden.jpg"),
    );
    let shown = images.of_title(
        &tmdb(3),
        &ids(&tmdb(3), &[]),
        POSTER,
        offered("/offered.jpg"),
    );

    let custom_poster = custom(&Slot::movie(&tmdb(1), Role::Poster));
    let was_offered = |path| artwork.was_offered("tmdb", path);
    assert_eq!(
        (hidden, was_offered("/hidden.jpg")),
        (showing(&custom_poster, true), false)
    );
    let offered_url = artwork::url("tmdb", "/offered.jpg");
    assert_eq!(
        (shown, was_offered("/offered.jpg")),
        (showing(&offered_url, false), true)
    );
}
