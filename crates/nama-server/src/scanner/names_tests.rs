use std::path::Path;

use nama_plugin_api::metadata::{ExternalId, IMDB};

use super::{is_extra, parse};
use crate::{library::contents::Item, library::libraries::Kind};

fn tmdb(id: i64) -> ExternalId {
    ExternalId::tmdb(id)
}

fn tvdb(id: i64) -> ExternalId {
    ExternalId::tvdb(id)
}

fn imdb(id: &str) -> ExternalId {
    ExternalId::new(IMDB, id).unwrap()
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "a table row's expected value is an `Option`, `None` where the path doesn't parse"
)]
fn movie(title: &str, year: Option<i64>, hint: impl Into<Option<ExternalId>>) -> Option<Vec<Item>> {
    Some(vec![Item::Movie {
        title: title.to_owned(),
        year,
        hint: hint.into(),
    }])
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "a table row's expected value is an `Option`, `None` where the path doesn't parse"
)]
fn episodes(
    show: &str,
    year: Option<i64>,
    hint: impl Into<Option<ExternalId>>,
    season: i64,
    numbers: &[i64],
) -> Option<Vec<Item>> {
    let hint = hint.into();
    Some(
        numbers
            .iter()
            .map(|&episode| Item::Episode {
                show: show.to_owned(),
                year,
                hint: hint.clone(),
                season,
                episode,
            })
            .collect(),
    )
}

#[test]
fn movies() {
    let cases = [
        // The Plex and Jellyfin layout: a folder per Movie.
        (
            "Blade Runner (1982)/Blade Runner (1982).mkv",
            movie("Blade Runner", Some(1982), None),
        ),
        // The folder names the Movie, whatever the file is called.
        (
            "Blade Runner (1982)/br-final.cut.mp4",
            movie("Blade Runner", Some(1982), None),
        ),
        // A bare file in the Library folder.
        (
            "Blade Runner (1982).mkv",
            movie("Blade Runner", Some(1982), None),
        ),
        // Bare files grouped in a folder that doesn't name a Movie.
        (
            "Sci-Fi/Blade Runner (1982).mkv",
            movie("Blade Runner", Some(1982), None),
        ),
        // Folders nested under a grouping folder.
        (
            "B/Blade Runner (1982)/Blade Runner (1982).mkv",
            movie("Blade Runner", Some(1982), None),
        ),
        // Editions, versions and qualities after the year are ignored.
        (
            "Blade Runner (1982)/Blade Runner (1982) {edition-Final Cut} - 2160p.mkv",
            movie("Blade Runner", Some(1982), None),
        ),
        (
            "Blade Runner (1982) [1080p]/Blade Runner (1982) - 1080p.mkv",
            movie("Blade Runner", Some(1982), None),
        ),
        // Multi-part Movies are each the Movie.
        (
            "Blade Runner (1982)/Blade Runner (1982) - pt2.mkv",
            movie("Blade Runner", Some(1982), None),
        ),
        // Brackets in the title and digits that aren't a year.
        (
            "2001 - A Space Odyssey (1968)/2001 - A Space Odyssey (1968).mkv",
            movie("2001 - A Space Odyssey", Some(1968), None),
        ),
        (
            "Alien (Director's Cut) (1979)/Alien.mkv",
            movie("Alien (Director's Cut)", Some(1979), None),
        ),
    ];
    for (path, expected) in cases {
        assert_eq!(parse(Kind::Movies, Path::new(path)), expected, "{path}");
    }
}

#[test]
fn movies_with_id_hints_or_too_little_to_go_on() {
    let cases = [
        // ID hints, in Plex's and Jellyfin's spellings, on the folder or the file.
        (
            "Blade Runner (1982) {tmdb-78}/Blade Runner (1982).mkv",
            movie("Blade Runner", Some(1982), tmdb(78)),
        ),
        (
            "Blade Runner (1982) [tmdbid-78]/Blade Runner (1982).mkv",
            movie("Blade Runner", Some(1982), tmdb(78)),
        ),
        (
            "Blade Runner (1982)/Blade Runner (1982) {tmdb-78}.mkv",
            movie("Blade Runner", Some(1982), tmdb(78)),
        ),
        (
            "Blade Runner {TMDB-78}/movie.mkv",
            movie("Blade Runner", None, tmdb(78)),
        ),
        // TVDB and IMDb ID Hints in every bracket, with `-` or `=`, and a bare IMDb id.
        (
            "Blade Runner (1982) {imdb-tt0083658}/Blade Runner (1982).mkv",
            movie("Blade Runner", Some(1982), imdb("tt0083658")),
        ),
        (
            "Blade Runner (imdbid=tt0083658)/movie.mkv",
            movie("Blade Runner", None, imdb("tt0083658")),
        ),
        (
            "Blade Runner (1982) tt0083658/movie.mkv",
            movie("Blade Runner", Some(1982), imdb("tt0083658")),
        ),
        (
            "Blade Runner (1982)/Blade Runner (1982)_tt0083658.mkv",
            movie("Blade Runner", Some(1982), imdb("tt0083658")),
        ),
        (
            "Blade Runner [tvdbid-1234]/movie.mkv",
            movie("Blade Runner", None, tvdb(1234)),
        ),
        (
            "Blade Runner {TVDB=1234}/movie.mkv",
            movie("Blade Runner", None, tvdb(1234)),
        ),
        // TMDB wins, then TVDB, then IMDb, wherever each is.
        (
            "Blade Runner (1982) {imdb-tt0083658} {tvdb-1234}/Blade Runner (1982) {tmdb-78}.mkv",
            movie("Blade Runner", Some(1982), tmdb(78)),
        ),
        (
            "Blade Runner (1982) {tvdb-1234}/Blade Runner (1982) tt0083658.mkv",
            movie("Blade Runner", Some(1982), tvdb(1234)),
        ),
        // Within a kind, the file's hint wins over the folder's.
        (
            "Blade Runner (1982) {tmdb-1}/Blade Runner (1982) {tmdb-78}.mkv",
            movie("Blade Runner", Some(1982), tmdb(78)),
        ),
        // Letters glued to an IMDb id make it a word, and a TMDB hint holds no `tt`.
        (
            "Blade Runner (1982)/Blade Runner (1982) xtt0083658.mkv",
            movie("Blade Runner", Some(1982), None),
        ),
        ("Blade Runner {tmdb-tt0083658}/movie.mkv", None),
        // A file that names a Movie wins over a grouping folder that looks like one, and
        // takes a folder's ID hint only if the folder names the same Movie.
        (
            "Releases (2020)/Heat (1995).mkv",
            movie("Heat", Some(1995), None),
        ),
        (
            "Collection {tmdb-10}/Alien (1979).mkv",
            movie("Alien", Some(1979), None),
        ),
        (
            "Alien (1979) {tmdb-348}/alien (1979) - 1080p.mkv",
            movie("alien", Some(1979), tmdb(348)),
        ),
        // Too little to go on: no year and no ID hint, or only a year.
        ("Blade Runner/Blade Runner.mkv", None),
        ("movie.mkv", None),
        ("(1982)/movie.mkv", None),
        // Four digits that can't be a year.
        ("Movie (0042)/movie.mkv", None),
    ];
    for (path, expected) in cases {
        assert_eq!(parse(Kind::Movies, Path::new(path)), expected, "{path}");
    }
}

#[test]
fn movies_with_id_hints_in_any_namespace() {
    let cases = [
        // Any other namespace in curly braces, case-folded, but not Plex's edition tag.
        (
            "Perfect Blue {AniDB-5101}/movie.mkv",
            movie(
                "Perfect Blue",
                None,
                ExternalId::new("anidb", "5101").unwrap(),
            ),
        ),
        (
            "Perfect Blue (1997) {tmdb-10494} {anidb-5101}/movie.mkv",
            movie("Perfect Blue", Some(1997), tmdb(10494)),
        ),
        ("Blade Runner {edition-Director's Cut}/movie.mkv", None),
        ("Blade Runner {edition-Final}/movie.mkv", None),
        ("Blade Runner {tmdbid-abc}/movie.mkv", None),
    ];
    for (path, expected) in cases {
        assert_eq!(parse(Kind::Movies, Path::new(path)), expected, "{path}");
    }
}

#[test]
fn shows() {
    let dark = |season, numbers: &[i64]| episodes("Dark", Some(2017), None, season, numbers);
    let cases = [
        // The Plex and Jellyfin layout: Show, Season, Episode.
        (
            "Dark (2017)/Season 01/Dark (2017) - S01E01.mkv",
            dark(1, &[1]),
        ),
        (
            "Dark (2017)/Season 01/Dark (2017) - S01E01 - Secrets.mkv",
            dark(1, &[1]),
        ),
        ("Dark (2017)/Season 3/dark.s03e08.1080p.mkv", dark(3, &[8])),
        ("Dark (2017)/Season 01/s1e2.mkv", dark(1, &[2])),
        ("Dark (2017)/Season 01/Dark_S01E03.mkv", dark(1, &[3])),
        // Episodes straight in the Show's folder.
        ("Dark (2017)/Dark - S02E05.mkv", dark(2, &[5])),
        // The file's Season wins over the folder's.
        ("Dark (2017)/Season 01/Dark - S02E05.mkv", dark(2, &[5])),
    ];
    for (path, expected) in cases {
        assert_eq!(parse(Kind::Shows, Path::new(path)), expected, "{path}");
    }
}

#[test]
fn multi_episode_files_and_specials() {
    let dark = |season, numbers: &[i64]| episodes("Dark", Some(2017), None, season, numbers);
    let cases = [
        // Multi-episode files.
        (
            "Dark (2017)/Season 01/Dark (2017) - S01E01-E02.mkv",
            dark(1, &[1, 2]),
        ),
        (
            "Dark (2017)/Season 01/Dark - S01E01E02.mkv",
            dark(1, &[1, 2]),
        ),
        (
            "Dark (2017)/Season 01/Dark - S01E01-E03 - Pilot.mkv",
            dark(1, &[1, 2, 3]),
        ),
        (
            "Dark (2017)/Season 01/Dark - S01E01-S01E02.mkv",
            dark(1, &[1, 2]),
        ),
        (
            "Dark (2017)/Season 01/Dark - S01E01-02.mkv",
            dark(1, &[1, 2]),
        ),
        (
            "Dark (2017)/Season 01/Dark - S01E01-02 - Pilot.mkv",
            dark(1, &[1, 2]),
        ),
        // A later `E` number that isn't glued on is part of the title, and so is a quality.
        (
            "Dark (2017)/Season 01/Dark - S01E01 - E3 Special.mkv",
            dark(1, &[1]),
        ),
        (
            "Dark (2017)/Season 01/Dark - S01E01-720p.mkv",
            dark(1, &[1]),
        ),
        // An ending before the start, or absurdly far after it, is only the first Episode.
        ("Dark (2017)/Season 01/Dark - S01E05-E02.mkv", dark(1, &[5])),
        (
            "Dark (2017)/Season 01/Dark - S01E01-E900.mkv",
            dark(1, &[1]),
        ),
        // Specials are Season 0, in `Season 00` or `Specials`.
        (
            "Dark (2017)/Season 00/Dark (2017) - S00E01.mkv",
            dark(0, &[1]),
        ),
        ("Dark (2017)/Specials/Dark - S00E02.mkv", dark(0, &[2])),
    ];
    for (path, expected) in cases {
        assert_eq!(parse(Kind::Shows, Path::new(path)), expected, "{path}");
    }
}

#[test]
fn other_show_layouts_and_names() {
    let dark = |season, numbers: &[i64]| episodes("Dark", Some(2017), None, season, numbers);
    let cases = [
        // The `1x01` spelling, which a resolution doesn't look like.
        ("Dark (2017)/Season 01/Dark - 1x04.mkv", dark(1, &[4])),
        ("Dark (2017)/Season 01/Dark 1920x1080.mkv", None),
        // Show folders grouped in a folder.
        (
            "Anime/Dark (2017)/Season 01/Dark - S01E01.mkv",
            dark(1, &[1]),
        ),
        ("Anime/Dark (2017)/Dark - S01E01.mkv", dark(1, &[1])),
        // A Library folder that is a Show's folder: there's no Show folder to name it.
        ("Season 01/Dark - S01E01.mkv", None),
        ("Specials/Dark - S00E01.mkv", None),
        // A Show folder without a year.
        (
            "Dark/Season 01/Dark - S01E01.mkv",
            episodes("Dark", None, None, 1, &[1]),
        ),
        // ID hints on the Show's folder.
        (
            "Dark (2017) {tmdb-70523}/Season 01/Dark - S01E01.mkv",
            episodes("Dark", Some(2017), tmdb(70523), 1, &[1]),
        ),
        (
            "Dark (2017) [tmdbid-70523]/Season 01/Dark - S01E01.mkv",
            episodes("Dark", Some(2017), tmdb(70523), 1, &[1]),
        ),
        // Sonarr's folders: no year, and a TVDB id. An Episode's IMDb id isn't its Show's.
        (
            "Dark [tvdbid-334824]/Season 01/Dark - S01E01 tt5753856.mkv",
            episodes("Dark", None, tvdb(334_824), 1, &[1]),
        ),
        (
            "Dark (tvdb=334824)/Season 01/Dark - S01E01.mkv",
            episodes("Dark", None, tvdb(334_824), 1, &[1]),
        ),
        // No Episode number, a word that only looks like one, or no Show folder.
        ("Dark (2017)/Season 01/Dark - Pilot.mkv", None),
        ("Dark (2017)/Season 01/Dark - Lifes01e01.mkv", None),
        ("Dark - S01E01.mkv", None),
    ];
    for (path, expected) in cases {
        assert_eq!(parse(Kind::Shows, Path::new(path)), expected, "{path}");
    }
}

#[test]
fn extras_are_recognised_by_folder_or_suffix() {
    let cases = [
        ("Blade Runner (1982)/Blade Runner (1982).mkv", false),
        ("Blade Runner (1982)/Featurettes/Making Of.mkv", true),
        ("Blade Runner (1982)/Behind The Scenes/Making Of.mkv", true),
        ("Blade Runner (1982)/extras/Making Of.mkv", true),
        ("Blade Runner (1982)/Blade Runner (1982)-trailer.mkv", true),
        ("Blade Runner (1982)/Blade Runner (1982)-sample.mkv", true),
        // Episode titles that end like a suffix aren't extras.
        ("Lost (2004)/Season 01/Lost - S01E05 - Other.mkv", false),
        // Folders named like extras folders only hold extras inside a title's folder.
        ("Shorts/Paperman (2012).mkv", false),
        ("Other/Heat (1995)/Heat (1995).mkv", false),
        ("Collections/Shorts/Paperman (2012).mkv", false),
        ("Movies/Other/Heat (1995).mkv", false),
        ("Blade Runner {tmdb-78}/Trailers/Teaser.mkv", true),
        ("Extras/Season 01/Extras - S01E01.mkv", false),
        (
            "Dark (2017)/Season 01/Dark - S01E01 - Behind the Scenes.mkv",
            false,
        ),
        ("Dark (2017)/Season 01/Trailers/Teaser.mkv", true),
        // A title that merely contains a word used for extras.
        ("The Other Guys (2010)/The Other Guys (2010).mkv", false),
    ];
    for (path, expected) in cases {
        assert_eq!(is_extra(Path::new(path)), expected, "{path}");
    }
}
