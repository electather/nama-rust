use std::{ffi::OsStr, fs, path::Path};

use super::*;
use crate::scanner::tests::{Fixture, summary};

#[test]
fn a_sidecar_belongs_to_the_video_with_the_longest_stem_its_name_starts_with() {
    let videos = [Path::new("/m/Up.mkv"), Path::new("/m/Up-1080p.mkv")];
    let video_of = |sidecar: &str| video_of(Path::new(sidecar), &videos);

    assert_eq!(video_of("/m/Up.en.srt"), Some(Path::new("/m/Up.mkv")));
    assert_eq!(
        video_of("/m/Up-1080p.en.srt"),
        Some(Path::new("/m/Up-1080p.mkv"))
    );
    assert_eq!(video_of("/m/Other.en.srt"), None);
}

#[test]
fn an_image_named_alone_stands_in_only_outside_the_librarys_folders_and_one_named_after_the_file_anywhere()
 {
    let video = OsStr::new("Up (2009)");
    let role =
        |name: &str, in_library_folder| image_role(Path::new(name), video, in_library_folder);

    assert_eq!(role("poster.jpg", false), Some((Tie::Alone, Role::Poster)));
    assert_eq!(role("Folder.PNG", false), Some((Tie::Alone, Role::Poster)));
    assert_eq!(
        role("fanart.webp", false),
        Some((Tie::Alone, Role::Backdrop))
    );
    assert_eq!(role("poster.jpg", true), None);
    assert_eq!(
        role("Up (2009)-poster.jpeg", true),
        Some((Tie::AfterVideo, Role::Poster))
    );
    assert_eq!(
        role("up (2009)-Fanart.jpg", false),
        Some((Tie::AfterVideo, Role::Backdrop))
    );
    assert_eq!(role("Other-poster.jpg", false), None);
    assert_eq!(role("poster.gif", false), None);
    assert_eq!(role("banner.jpg", false), None);
}

#[test]
fn a_season_poster_in_the_shows_folder_is_named_after_its_number_or_specials() {
    assert_eq!(season_in_name("season02-poster"), Some(2));
    assert_eq!(season_in_name("season00-poster"), Some(0));
    assert_eq!(season_in_name("season100-poster"), Some(100));
    assert_eq!(season_in_name("season-specials-poster"), Some(0));
    assert_eq!(season_in_name("season-poster"), None);
    assert_eq!(season_in_name("season+2-poster"), None);
    assert_eq!(season_in_name("season02-fanart"), None);
    assert!(image_name(Path::new("Season01-Poster.PNG")).is_some());
}

#[test]
fn a_show_jpg_in_the_shows_folder_is_the_shows_poster() {
    let path = Path::new("/l/Dark (2017)/Season 01/Dark - S01E01.mkv");
    let video = Video::at(LibraryKind::Shows, path, 1, &HashMap::from([(1, 1)])).unwrap();

    let ties = video.ties(Path::new("/l/Dark (2017)/Show.jpg"), false);

    assert_eq!(ties, [(Tie::Alone, LocalRole::Title(Role::Poster))]);
}

#[test]
fn in_a_shows_library_images_in_the_shows_and_seasons_folders_stand_in_for_the_show_and_seasons() {
    let show = "/l/Dark (2017)";
    let versions: HashMap<String, Stored> = [
        (format!("{show}/Season 01/Dark - S01E01.mkv"), 1),
        (format!("{show}/Season 02/Dark - S02E01.mkv"), 2),
        (format!("{show}/Dark - S00E01.mkv"), 3),
    ]
    .into_iter()
    .map(|(path, id)| {
        let stored = Stored {
            id,
            size: 1,
            modified_ns: 1,
            parser_version: PARSER_VERSION,
        };
        (path, stored)
    })
    .collect();
    let image = |relative: &str| Found {
        path: format!("/l/{relative}"),
        relative: PathBuf::from(relative),
        size: 1,
        modified_ns: 1,
    };
    let images = FoundLocalImages {
        images: [
            "poster.jpg",
            "Dark (2017)/poster.jpg",
            "Dark (2017)/show.jpg",
            "Dark (2017)/fanart.jpg",
            "Dark (2017)/season01-poster.jpg",
            "Dark (2017)/Season 01/poster.jpg",
            "Dark (2017)/Season 01/fanart.jpg",
            "Dark (2017)/season02-poster.jpg",
            "Dark (2017)/season-specials-poster.jpg",
        ]
        .map(image)
        .into(),
        unreadable: Vec::new(),
    };

    let seasons = HashMap::from([(1, 1), (2, 2), (3, 0)]);

    let (mut read, tied) = images.beside(LibraryKind::Shows, &versions, &seasons);

    read.sort_unstable();
    assert_eq!(read, [1, 2, 3]);
    let tied: HashMap<(i64, LocalRole), &str> = (tied.iter())
        .map(|(&key, file)| (key, file.path.as_str()))
        .collect();
    let (poster, backdrop) = (
        LocalRole::Title(Role::Poster),
        LocalRole::Title(Role::Backdrop),
    );
    let season = LocalRole::SeasonPoster;
    let shown = [
        ((1, season), "/l/Dark (2017)/Season 01/poster.jpg"),
        ((2, season), "/l/Dark (2017)/season02-poster.jpg"),
        ((3, season), "/l/Dark (2017)/season-specials-poster.jpg"),
    ];
    let show_images = [1, 2, 3].into_iter().flat_map(|version| {
        [
            ((version, poster), "/l/Dark (2017)/poster.jpg"),
            ((version, backdrop), "/l/Dark (2017)/fanart.jpg"),
        ]
    });
    assert_eq!(tied, shown.into_iter().chain(show_images).collect());
}

#[test]
fn an_episode_its_name_doesnt_give_takes_its_season_from_the_stored_items_and_a_file_without_one_is_left_out()
 {
    let stored = |id| Stored {
        id,
        size: 1,
        modified_ns: 1,
        parser_version: PARSER_VERSION,
    };
    let versions = HashMap::from([
        ("/l/Dark (2017)/Season 01/Dark 01.mkv".to_owned(), stored(1)),
        ("/l/Dark (2017)/Season 01/Notes.mkv".to_owned(), stored(2)),
    ]);
    let image = |relative: &str| Found {
        path: format!("/l/{relative}"),
        relative: PathBuf::from(relative),
        size: 1,
        modified_ns: 1,
    };
    let images = FoundLocalImages {
        images: ["Dark (2017)/poster.jpg", "Dark (2017)/season01-poster.jpg"]
            .map(image)
            .into(),
        unreadable: Vec::new(),
    };

    let (read, tied) = images.beside(LibraryKind::Shows, &versions, &HashMap::from([(1, 1)]));

    assert_eq!(read, [1]);
    let mut tied: Vec<_> = (tied.iter())
        .map(|(&key, file)| (key, file.path.as_str()))
        .collect();
    tied.sort_by_key(|&(_, path)| path);
    assert_eq!(
        tied,
        [
            (
                (1, LocalRole::Title(Role::Poster)),
                "/l/Dark (2017)/poster.jpg"
            ),
            (
                (1, LocalRole::SeasonPoster),
                "/l/Dark (2017)/season01-poster.jpg"
            ),
        ]
    );
}

/// The paths of the files `media` stores.
async fn stored_paths(media: &Fixture) -> Vec<String> {
    media
        .files()
        .await
        .into_iter()
        .map(|(path, _)| path)
        .collect()
}

#[tokio::test]
async fn a_scan_skips_a_folder_holding_nomedia_or_ignore_and_all_below_it() {
    let media = Fixture::new(LibraryKind::Movies).await;
    media.write("Alien (1979)/Alien (1979).mkv", 10);
    media.write("Samples/Up (2009).mkv", 10);
    media.write("Samples/.nomedia", 0);
    media.write("Staging/Deep/Heat (1995).mkv", 10);
    media.write("Staging/.ignore", 0);

    media.scan().await;

    assert_eq!(
        stored_paths(&media).await,
        ["Alien (1979)/Alien (1979).mkv"]
    );
}

#[tokio::test]
async fn a_marker_added_to_the_library_folder_removes_its_only_items_until_its_deleted() {
    let media = Fixture::new(LibraryKind::Movies).await;
    media.write("Alien (1979)/Alien (1979).mkv", 10);
    media.scan().await;

    media.write(".nomedia", 0);
    assert_eq!(media.scan().await, summary(0, 0, 1, 0));
    assert_eq!(stored_paths(&media).await, Vec::<String>::new());

    fs::remove_file(media.path(".nomedia")).unwrap();
    assert_eq!(media.scan().await, summary(1, 0, 0, 0));
}

#[cfg(unix)]
#[tokio::test]
async fn a_marker_that_cant_be_checked_keeps_whats_stored_under_it() {
    let media = Fixture::new(LibraryKind::Movies).await;
    media.write("Alien (1979)/Alien (1979).mkv", 10);
    media.scan().await;

    // A symlink to itself fails with ELOOP, not NotFound.
    let marker = media.path("Alien (1979)/.nomedia");
    std::os::unix::fs::symlink(&marker, &marker).unwrap();

    assert_eq!(media.scan().await, summary(0, 0, 0, 1));
}
