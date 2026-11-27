use std::{collections::BTreeSet, io::Cursor, path::Path, sync::Arc};

use axum::{body::Body, extract::Request};
use image::{ImageFormat, RgbImage};
use nama_plugin_api::{
    PluginId,
    metadata::{ExternalId, ExternalIds},
};
use tempfile::TempDir;
use tower::ServiceExt;

use super::*;
use crate::{
    library::{
        FoundImages, LocalImages, Metadata,
        contents::{ImageRef, LocalImageFile},
        custom_images::{self, Urls as CustomImageUrls},
        fixture::{MatchedMovie, matched_episodes_file, tmdb_movie},
        scripted::{ScriptedSource, SourcesAsked},
    },
    plugins::Registry,
    server_settings::Settings as ServerSettings,
};

const LIBRARY: i64 = 1;
const CACHE_LIMIT: Option<u64> = Some(1 << 30);
const MOVIE_POSTER: TitleImage = TitleImage::Movie(Role::Poster);
const MOVIE_BACKDROP: TitleImage = TitleImage::Movie(Role::Backdrop);
const POSTER: LocalRole = LocalRole::Title(Role::Poster);

fn tmdb(id: i64) -> ExternalId {
    ExternalId::tmdb(id)
}

/// A Poster or Backdrop the Match of the title `tmdb-<id>` stores, at `path`.
fn stored(id: i64, image: TitleImage, path: &str) -> StoredTitleImage {
    StoredTitleImage {
        plugin: "tmdb".to_owned(),
        path: path.to_owned(),
        ids: ExternalIds::from_iter([tmdb(id)]),
        image,
    }
}

/// The image beside the media of the title `tmdb-<id>`, of an Episode in `season` for a Show's.
fn beside(id: i64, role: LocalRole, season: Option<i64>, stored: &str) -> TitleLocalImage {
    TitleLocalImage {
        ids: ExternalIds::from_iter([tmdb(id)]),
        role,
        season,
        library: LIBRARY,
        stored: stored.to_owned(),
    }
}

/// What the titles of `rows` show in place of their downloaded images: its Custom Images and
/// images beside the media.
fn in_place(rows: &Rows) -> InPlaceOfDownloaded {
    let urls = (rows.custom.iter()).map(|(slot, version)| (slot.clone(), format!("/{version}")));
    InPlaceOfDownloaded::new(
        CustomImageUrls::for_tests(urls),
        LocalImageUrls::filed(&rows.beside_media),
    )
}

fn images(list: &[CopyAhead]) -> Vec<&Image> {
    list.iter().map(|ahead| &ahead.image).collect()
}

fn tmdb_image(path: &str) -> Image {
    downloaded("tmdb", path)
}

fn custom(slot: Slot) -> Image {
    Image::Custom {
        slot,
        version: "v1".to_owned(),
    }
}

fn beside_media(stored: &str) -> Image {
    Image::BesideMedia {
        library: LIBRARY,
        stored: stored.to_owned(),
    }
}

#[test]
fn a_downloaded_poster_a_custom_poster_hides_gets_no_entry() {
    let slot = Slot::movie(&tmdb(1), Role::Poster);
    let rows = Rows {
        library: LIBRARY,
        posters: vec![
            stored(1, MOVIE_POSTER, "/hidden.jpg"),
            stored(2, MOVIE_POSTER, "/shown.jpg"),
        ],
        custom: vec![(slot.clone(), "v1".to_owned())],
        ..Rows::default()
    };

    let list = listed(&rows, &in_place(&rows), CACHE_LIMIT);

    assert_eq!(images(&list), [&tmdb_image("/shown.jpg"), &custom(slot)]);
}

#[test]
fn a_downloaded_poster_an_image_beside_the_media_hides_gets_no_entry() {
    let rows = Rows {
        library: LIBRARY,
        posters: vec![
            stored(1, MOVIE_POSTER, "/hidden.jpg"),
            stored(2, MOVIE_POSTER, "/shown.jpg"),
        ],
        beside_media: vec![beside(1, POSTER, None, "alien-poster")],
        ..Rows::default()
    };

    let list = listed(&rows, &in_place(&rows), CACHE_LIMIT);

    assert_eq!(
        images(&list),
        [&tmdb_image("/shown.jpg"), &beside_media("alien-poster")]
    );
}

#[test]
fn a_season_poster_beside_the_media_a_shows_custom_poster_does_not_hide_keeps_its_entry() {
    let slot = Slot::show(&tmdb(1), Role::Poster);
    let rows = Rows {
        library: LIBRARY,
        beside_media: vec![
            beside(1, POSTER, Some(1), "show-poster"),
            beside(1, LocalRole::SeasonPoster, Some(1), "season-poster"),
        ],
        custom: vec![(slot.clone(), "v1".to_owned())],
        ..Rows::default()
    };

    let list = listed(&rows, &in_place(&rows), CACHE_LIMIT);

    assert_eq!(
        images(&list),
        [&beside_media("season-poster"), &custom(slot)]
    );
}

#[test]
fn a_librarys_custom_backdrop_hides_no_titles_backdrop() {
    let slot = Slot::library(LIBRARY);
    let rows = Rows {
        library: LIBRARY,
        backdrops: vec![stored(1, MOVIE_BACKDROP, "/backdrop.jpg")],
        custom: vec![(slot.clone(), "v1".to_owned())],
        ..Rows::default()
    };

    let list = listed(&rows, &in_place(&rows), CACHE_LIMIT);

    assert_eq!(images(&list), [&tmdb_image("/backdrop.jpg"), &custom(slot)]);
}

#[test]
fn titled_backdrops_title_logos_and_trailer_thumbnails_are_listed_at_their_widths() {
    let image = |path: &str| ("tmdb".to_owned(), path.to_owned());
    let rows = Rows {
        library: LIBRARY,
        titled_backdrops: vec![image("/titled.jpg")],
        title_logos: vec![image("/logo.svg")],
        trailer_thumbnails: vec![image("/trailer.jpg")],
        ..Rows::default()
    };

    let list = listed(&rows, &in_place(&rows), CACHE_LIMIT);

    let backdrop = |path| CopyAhead {
        image: tmdb_image(path),
        kind: Kind::Backdrop,
        copies: Resize::each(&[640, 1280, 1920, 2560], CACHE_LIMIT),
    };
    let logo = CopyAhead {
        image: tmdb_image("/logo.svg"),
        kind: Kind::TitleLogo,
        copies: Resize::each(&[320, 480, 640, 960], CACHE_LIMIT),
    };
    assert_eq!(
        list,
        [backdrop("/titled.jpg"), backdrop("/trailer.jpg"), logo]
    );
}

/// A Library with a Poster and Backdrop of each source, and a Title Logo, listed Backdrops first.
fn of_every_source() -> Rows {
    Rows {
        library: LIBRARY,
        backdrops: vec![stored(1, MOVIE_BACKDROP, "/backdrop.jpg")],
        title_logos: vec![("tmdb".to_owned(), "/logo.svg".to_owned())],
        posters: vec![stored(1, MOVIE_POSTER, "/poster.jpg")],
        beside_media: vec![beside(2, POSTER, None, "dune-poster")],
        custom: vec![(Slot::movie(&tmdb(3), Role::Poster), "v1".to_owned())],
        ..Rows::default()
    }
}

#[test]
fn every_poster_comes_before_any_backdrop_or_title_logo() {
    let rows = of_every_source();

    let list = listed(&rows, &in_place(&rows), CACHE_LIMIT);

    let kinds: Vec<_> = list.iter().map(|ahead| ahead.kind).collect();
    assert_eq!(
        kinds,
        [
            Kind::Poster,
            Kind::Poster,
            Kind::Poster,
            Kind::Backdrop,
            Kind::TitleLogo
        ]
    );
    assert_eq!(list[0].copies, Resize::each(&[320, 480, 640], CACHE_LIMIT));
}

#[test]
fn with_resizing_off_every_image_is_listed_with_no_widths() {
    let rows = of_every_source();

    let list = listed(&rows, &in_place(&rows), None);

    assert_eq!(list.len(), 5);
    assert!(list.iter().all(|ahead| ahead.copies.is_empty()), "{list:?}");
}

#[test]
fn a_removed_custom_posters_slot_lists_the_image_it_brings_back() {
    let rows = Rows {
        library: LIBRARY,
        posters: vec![
            stored(1, MOVIE_POSTER, "/alien.jpg"),
            stored(2, MOVIE_POSTER, "/dune.jpg"),
        ],
        backdrops: vec![stored(1, MOVIE_BACKDROP, "/alien-backdrop.jpg")],
        ..Rows::default()
    };

    let slot = Slot::movie(&tmdb(1), Role::Poster);
    let list = listed_at(&slot, &rows, &in_place(&rows), CACHE_LIMIT);

    assert_eq!(images(&list), [&tmdb_image("/alien.jpg")]);
}

/// A 2 by 3 pixel PNG.
fn png() -> Vec<u8> {
    image_of(2, 3)
}

fn image_of(width: u32, height: u32) -> Vec<u8> {
    let mut png = Cursor::new(Vec::new());
    (RgbImage::new(width, height).write_to(&mut png, ImageFormat::Png)).unwrap();
    png.into_inner()
}

/// Copies ahead of the images in `data`, as `images` says, downloading through a title source
/// `tmdb` that answers each of `paths` with `bytes`.
fn copies_ahead_in(
    data: &TempDir,
    db: &Arc<Database>,
    images: ImageSettings,
    (paths, bytes): (&[String], &[u8]),
) -> CopiesAhead {
    let source = (paths.iter()).fold(ScriptedSource::new("tmdb", &SourcesAsked::default()), {
        |source, path| source.with_image(path, bytes)
    });
    let plugins = Registry::new();
    plugins.enable(Arc::new(source)).unwrap();
    let store = ImageStore::new(data.path(), Arc::clone(db));
    let artwork = Artwork::new(store.clone(), plugins.extension_points());
    CopiesAhead::new(Arc::clone(db), store, artwork, images)
}

/// Keeps `file` in `copies_ahead`'s store as the image beside the media of the Version `version`
/// of the Library `library` that stands in for its `role`.
async fn keep_beside(
    db: &Arc<Database>,
    copies_ahead: &CopiesAhead,
    (library, version): (i64, i64),
    role: LocalRole,
    file: &Path,
) {
    let file = LocalImageFile {
        path: file.to_str().unwrap().to_owned(),
        size: 1,
        modified_ns: 0,
    };
    let found = FoundImages::from([((version, role), file)]);
    let local_images = LocalImages::new(Arc::clone(db), copies_ahead.store.clone());
    (local_images.keep(library, &[version], &found).await).unwrap();
}

/// A Library in `media`, recorded in `db`, of Alien, Dune and Heat, each with its downloaded
/// Backdrop `/<title>.jpg` and the titled Backdrop `/titled.jpg`, where Alien shows a Backdrop
/// beside its media in place of its own and Heat a Custom Backdrop, which `copies_ahead` is told
/// of. Returns the Library's id.
async fn alien_and_heat_showing_other_backdrops(
    db: &Arc<Database>,
    data: &TempDir,
    media: &Path,
    copies_ahead: &CopiesAhead,
) -> i64 {
    let image = |path: &str| {
        Some(ImageRef {
            plugin: PluginId::from_static("tmdb"),
            path: path.to_owned(),
        })
    };
    let with_backdrops = |tmdb, title: &str| Metadata {
        backdrop: image(&format!("/{}.jpg", title.to_lowercase())),
        titled_backdrop: image("/titled.jpg"),
        ..tmdb_movie(tmdb, title)
    };
    let alien = MatchedMovie::new(db, media, with_backdrops(1, "Alien")).await;
    let contents = Contents::new(Arc::clone(db));
    for (tmdb, title) in [(2, "Dune"), (3, "Heat")] {
        let path = media.canonicalize().unwrap().join(format!("{title}.mkv"));
        let matched = with_backdrops(tmdb, title);
        MatchedMovie::store(&contents, alien.library, path.to_str().unwrap(), matched).await;
    }
    let fanart = media.join("fanart.png");
    std::fs::write(&fanart, png()).unwrap();
    let backdrop = LocalRole::Title(Role::Backdrop);
    let at = (alien.library, alien.version);
    keep_beside(db, copies_ahead, at, backdrop, &fanart).await;
    let custom = CustomImages::new(Arc::clone(db), copies_ahead.store.clone());
    let images = ServerSettings::new(Arc::clone(db), data.path().to_owned()).images();
    let artwork = copies_ahead.artwork.clone();
    let upload = Request::put("/custom-images/movie/tmdb-3/backdrop")
        .body(Body::from(png()))
        .unwrap();
    let uploaded = (custom_images::router(custom, artwork, copies_ahead.clone(), images))
        .oneshot(upload)
        .await
        .unwrap();
    assert_eq!(uploaded.status(), 204);
    alien.library
}

#[tokio::test]
async fn only_the_backdrops_titles_show_and_every_titled_backdrop_get_copies_made_ahead() {
    let data = tempfile::tempdir().unwrap();
    let media = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let paths = ["/alien.jpg", "/dune.jpg", "/heat.jpg", "/titled.jpg"].map(str::to_owned);
    let images = ServerSettings::new(Arc::clone(&db), data.path().to_owned()).images();
    let copies_ahead = copies_ahead_in(&data, &db, images, (&paths, &png()));
    let library =
        alien_and_heat_showing_other_backdrops(&db, &data, media.path(), &copies_ahead).await;

    copies_ahead.matches_changed(library);
    Artwork::settled().await;

    let listed = |dir: &str| {
        let entries = std::fs::read_dir(data.path().join(dir)).unwrap();
        let mut names: Vec<_> = (entries.map(|entry| entry.unwrap().file_name()))
            .map(|name| name.into_string().unwrap())
            .collect();
        names.sort();
        names
    };
    let kept = [
        "%2Falien.jpg",
        "%2Fdune.jpg",
        "%2Fheat.jpg",
        "%2Ftitled.jpg",
    ];
    assert_eq!(listed("artwork/tmdb"), kept);
    assert_eq!(
        listed("resized/artwork/tmdb"),
        ["%2Fdune.jpg", "%2Ftitled.jpg"]
    );
}

#[tokio::test]
async fn posters_of_every_source_get_copies_ahead_first_until_the_cache_is_nearly_full_and_rescans_make_none()
 {
    let data = tempfile::tempdir().unwrap();
    let media = tempfile::tempdir().unwrap();
    let wide = image_of(1000, 1500);
    // A Backdrop beside the media, which isn't waiting on a download, still waits for Posters.
    let fanart = media.path().join("fanart.png");
    std::fs::write(&fanart, &wide).unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    // Named so the Library lists its Backdrops before its Posters.
    let image = |path: String| {
        Some(ImageRef {
            plugin: PluginId::from_static("tmdb"),
            path,
        })
    };
    let with_artwork = |tmdb, title: &str| Metadata {
        poster: image(format!("/z-{title}.jpg")),
        backdrop: image(format!("/a-{title}.jpg")),
        ..tmdb_movie(tmdb, title)
    };
    let alien = MatchedMovie::new(&db, media.path(), with_artwork(1, "alien")).await;
    let contents = Contents::new(Arc::clone(&db));
    let dune_path = media.path().canonicalize().unwrap().join("Dune.mkv");
    let dune = with_artwork(2, "dune");
    MatchedMovie::store(&contents, alien.library, dune_path.to_str().unwrap(), dune).await;
    let settings = ServerSettings::new(Arc::clone(&db), data.path().to_owned());
    settings.change_images(true, Some(1)).await.unwrap();
    // One byte under 80% of 1 MiB, so the first copy made is the last made ahead.
    let cache = data.path().join("resized");
    std::fs::create_dir_all(&cache).unwrap();
    let filler = std::fs::File::create(cache.join("filler")).unwrap();
    filler.set_len(1024 * 1024 / 100 * 80 - 1).unwrap();
    let backdrop = LocalRole::Title(Role::Backdrop);
    let paths = ["/z-alien.jpg", "/a-alien.jpg", "/z-dune.jpg", "/a-dune.jpg"].map(str::to_owned);
    let copies_ahead = copies_ahead_in(&data, &db, settings.images(), (&paths, &wide));
    let at = (alien.library, alien.version);
    keep_beside(&db, &copies_ahead, at, backdrop, &fanart).await;

    copies_ahead.matches_changed(alien.library);
    Artwork::settled().await;
    let cached = || {
        let files = walkdir::WalkDir::new(&cache)
            .into_iter()
            .map(Result::unwrap);
        (files.filter(|file| file.file_type().is_file()))
            .map(walkdir::DirEntry::into_path)
            .collect::<BTreeSet<_>>()
    };
    let first = cached();
    copies_ahead.matches_changed(alien.library);
    Artwork::settled().await;

    let listed = |dir: &Path| -> Vec<_> {
        (std::fs::read_dir(dir).unwrap())
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect()
    };
    let top = listed(&cache);
    assert_eq!(top.len(), 2, "only the filler and artwork: {top:?}");
    let copied = listed(&cache.join("artwork/tmdb"));
    assert!(
        matches!(copied.as_slice(), [poster] if poster.starts_with("%2Fz-")),
        "{copied:?}"
    );
    assert!(first.contains(&cache.join("filler")));
    assert_eq!(cached(), first, "the second run made and evicted nothing");
}

#[tokio::test]
async fn a_librarys_custom_images_are_its_own_and_its_titles_and_seasons_under_any_of_their_ids() {
    let data = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let folders = [(); 3].map(|()| tempfile::tempdir().unwrap());
    let imdb = ExternalId::new("imdb", "tt0078748").unwrap();
    let alien = Metadata {
        ids: ExternalIds::from_iter([ExternalId::tmdb(348), imdb.clone()]),
        ..tmdb_movie(348, "Alien")
    };
    let movies = MatchedMovie::new(&db, folders[0].path(), alien)
        .await
        .library;
    let other_movie = tmdb_movie(1, "Other");
    let others = MatchedMovie::new(&db, folders[1].path(), other_movie)
        .await
        .library;
    let dark = ExternalId::tmdb(70523);
    let dark_ids = ExternalIds::from_iter([dark.clone()]);
    matched_episodes_file(&db, folders[2].path(), dark_ids, [10, 11]).await;
    let libraries = Libraries::new(Arc::clone(&db)).list().await.unwrap();
    let shows = libraries.iter().map(|library| library.id).max().unwrap();
    let movies_own = [
        Slot::movie(&imdb, Role::Poster),
        Slot::movie(&ExternalId::tmdb(348), Role::Backdrop),
        Slot::library(movies),
    ];
    let shows_own = [
        Slot::show(&dark, Role::Poster),
        Slot::show(&dark, Role::Backdrop),
        Slot::season(&dark, 1),
    ];
    let neither = [
        Slot::movie(&ExternalId::tmdb(1), Role::Poster),
        Slot::library(others),
        Slot::movie(&ExternalId::tmdb(999), Role::Poster),
        Slot::season(&dark, 2),
    ];
    let store = ImageStore::new(data.path(), Arc::clone(&db));
    let copies_ahead = CopiesAhead::for_tests(&db, store, Registry::new().extension_points());
    let all = [&movies_own[..], &shows_own, &neither].concat();
    custom_images::keep_without_bytes(&db, &all).await.unwrap();

    let of_movies = copies_ahead.custom_of(movies).await.unwrap();
    let of_shows = copies_ahead.custom_of(shows).await.unwrap();

    let slots = |of: Vec<(Slot, String)>| -> HashSet<Slot> {
        of.into_iter().map(|(slot, _)| slot).collect()
    };
    assert_eq!(slots(of_movies), HashSet::from(movies_own));
    assert_eq!(slots(of_shows), HashSet::from(shows_own));
}
