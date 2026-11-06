//! Reading Movies and Episodes out of file paths, following the naming conventions Plex and
//! Jellyfin share:
//!
//! - Movies: `Movie Name (Year)/Movie Name (Year).mkv`, or a bare `Movie Name (Year).mkv`.
//! - Shows: `Show Name (Year)/Season 01/Show Name (Year) - S01E01.mkv`, with multi-episode files
//!   (`S01E01-E02`, `S01E01E02`) and specials in Season 0 (`S00E01`).
//! - ID hints anywhere in a Movie's or Show's name: `{tmdb-123}` or `[tmdbid-123]`.
//!
//! Only paths are read here. The scanner never opens `.nfo` files or any other sidecar.

use std::{path::Path, sync::LazyLock};

use regex::Regex;

use crate::libraries::Kind;

/// Something a file holds, as far as its path says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Item {
    /// A Movie.
    Movie {
        /// The title, as the folder or file names it.
        title: String,
        /// The release year.
        year: Option<i64>,
        /// The Movie's TMDB id, from an ID hint.
        tmdb_id: Option<i64>,
    },
    /// One Episode of a Show.
    Episode {
        /// The Show's title, as its folder names it.
        show: String,
        /// The year the Show first aired.
        year: Option<i64>,
        /// The Show's TMDB id, from an ID hint on its folder.
        tmdb_id: Option<i64>,
        /// The Season number; 0 holds specials.
        season: i64,
        /// The Episode number within the Season.
        episode: i64,
    },
}

/// The most Episodes one file can hold, so a typo like `S01E01-E900` isn't 900 Episodes.
const MAX_EPISODES_PER_FILE: i64 = 20;

/// An ID hint: `{tmdb-123}` (Plex) or `[tmdbid-123]` (Jellyfin), plus the spellings each also
/// accepts.
static TMDB_HINT: LazyLock<Regex> =
    LazyLock::new(|| regex(r"(?i)[\[{]\s*tmdb(?:id)?\s*[-=]\s*(\d+)\s*[\]}]"));

/// Any `{…}` or `[…]` tag, such as an ID hint, an edition or a quality.
static TAG: LazyLock<Regex> = LazyLock::new(|| regex(r"\{[^}]*\}|\[[^\]]*\]"));

/// A name ending (before any trailing tags or ` - suffix`) in a year in brackets.
static TITLE_YEAR: LazyLock<Regex> =
    LazyLock::new(|| regex(r"^(?P<title>.*?)\s*\((?P<year>\d{4})\)"));

/// `S01E01`, `s1e1`, `S01E01-E02`, `S01E01E02` or `S01E01-S01E02`, not glued to a word before
/// it.
static EPISODE: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r"(?i)(?:^|[^a-z0-9])s(?P<season>\d{1,3})[ ._]?e(?P<first>\d{1,4})(?P<more>(?:[ ._-]*(?:s\d{1,3})?e\d{1,4})*)",
    )
});

/// The episode numbers after the first in [`EPISODE`]'s `more` group.
static MORE_EPISODES: LazyLock<Regex> = LazyLock::new(|| regex(r"(?i)e(\d+)"));

/// `1x01`, with no digits or letters glued on either side.
static EPISODE_X: LazyLock<Regex> = LazyLock::new(|| {
    regex(r"(?i)(?:^|[^a-z0-9])(?P<season>\d{1,2})x(?P<first>\d{2,3})(?:[^a-z0-9]|$)")
});

/// Folders that hold a title's extras rather than the title itself.
const EXTRAS_FOLDERS: &[&str] = &[
    "behind the scenes",
    "deleted scenes",
    "extras",
    "featurettes",
    "interviews",
    "other",
    "sample",
    "samples",
    "scenes",
    "shorts",
    "trailers",
];

/// File name suffixes, after a `-`, that mark an extra.
const EXTRAS_SUFFIXES: &[&str] = &[
    "behindthescenes",
    "deleted",
    "featurette",
    "interview",
    "other",
    "sample",
    "scene",
    "short",
    "trailer",
];

/// Compiles a pattern written in this file.
#[expect(
    clippy::expect_used,
    reason = "the patterns are constants, and the tests compile every one"
)]
fn regex(pattern: &str) -> Regex {
    Regex::new(pattern).expect("a constant pattern compiles")
}

/// Whether the file at `relative` is an extra, such as a trailer or a featurette, which the
/// scanner leaves out.
pub(crate) fn is_extra(relative: &Path) -> bool {
    let mut components: Vec<String> = relative
        .iter()
        .map(|c| c.to_string_lossy().to_lowercase())
        .collect();
    let Some(file) = components.pop() else {
        return false;
    };
    if components
        .iter()
        .any(|folder| EXTRAS_FOLDERS.contains(&folder.as_str()))
    {
        return true;
    }
    let stem = Path::new(&file)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    stem.rsplit_once('-')
        .is_some_and(|(_, suffix)| EXTRAS_SUFFIXES.contains(&suffix.trim()))
}

/// Parses the path of a video file, relative to the Library folder it's in, in a Library of
/// `kind`. Returns `None` if the path doesn't follow the naming conventions, which leaves the
/// file Unmatched.
pub(crate) fn parse(kind: Kind, relative: &Path) -> Option<Vec<Item>> {
    let components: Vec<String> = relative
        .iter()
        .map(|c| c.to_string_lossy().into_owned())
        .collect();
    let (file, folders) = components.split_last()?;
    let stem = Path::new(file).file_stem()?.to_string_lossy().into_owned();
    match kind {
        Kind::Movies => parse_movie(folders, &stem).map(|movie| vec![movie]),
        Kind::Shows => parse_episodes(folders, &stem),
    }
}

/// A Movie, named by the nearest folder that names one, or else by the file itself.
fn parse_movie(folders: &[String], stem: &str) -> Option<Item> {
    let named = folders
        .iter()
        .rev()
        .find_map(|folder| title_and_year(folder))
        .or_else(|| title_and_year(stem))?;
    Some(Item::Movie {
        title: named.title,
        year: named.year,
        tmdb_id: named.tmdb_id.or_else(|| tmdb_hint(stem)),
    })
}

/// The Episodes in a file, in a Show named by the first folder under the Library folder.
fn parse_episodes(folders: &[String], stem: &str) -> Option<Vec<Item>> {
    let show_folder = folders.first()?;
    let show = title_and_year(show_folder).or_else(|| {
        let title = clean_title(&TAG.replace_all(show_folder, ""));
        (!title.is_empty()).then_some(Named {
            title,
            year: None,
            tmdb_id: None,
        })
    })?;
    let (season, episodes) = episode_numbers(stem)?;
    Some(
        episodes
            .into_iter()
            .map(|episode| Item::Episode {
                show: show.title.clone(),
                year: show.year,
                tmdb_id: show.tmdb_id,
                season,
                episode,
            })
            .collect(),
    )
}

/// The Season and Episode numbers a file name gives.
fn episode_numbers(stem: &str) -> Option<(i64, Vec<i64>)> {
    let captures = EPISODE
        .captures(stem)
        .or_else(|| EPISODE_X.captures(stem))?;
    let season: i64 = captures.name("season")?.as_str().parse().ok()?;
    let first: i64 = captures.name("first")?.as_str().parse().ok()?;
    let last = captures
        .name("more")
        .and_then(|more| MORE_EPISODES.captures_iter(more.as_str()).last())
        .and_then(|c| c.get(1)?.as_str().parse::<i64>().ok())
        .filter(|&last| last > first && last - first < MAX_EPISODES_PER_FILE)
        .unwrap_or(first);
    Some((season, (first..=last).collect()))
}

/// A title and what else a folder or file name says about it.
struct Named {
    title: String,
    year: Option<i64>,
    tmdb_id: Option<i64>,
}

/// The title in a name like `Title (Year)`, or any name with an ID hint. `None` for a name with
/// neither, which is too little to go on.
fn title_and_year(name: &str) -> Option<Named> {
    let tmdb_id = tmdb_hint(name);
    let untagged = TAG.replace_all(name, "");
    let (title, year) = match TITLE_YEAR.captures(&untagged) {
        Some(captures) => (
            captures.name("title")?.as_str(),
            captures
                .name("year")?
                .as_str()
                .parse::<i64>()
                .ok()
                .filter(|year| (1870..=2200).contains(year)),
        ),
        None => (&*untagged, None),
    };
    let title = clean_title(title);
    if title.is_empty() || (year.is_none() && tmdb_id.is_none()) {
        return None;
    }
    Some(Named {
        title,
        year,
        tmdb_id,
    })
}

/// The TMDB id in an ID hint in `name`.
fn tmdb_hint(name: &str) -> Option<i64> {
    TMDB_HINT.captures(name)?.get(1)?.as_str().parse().ok()
}

/// `title` without surrounding whitespace or separators, with runs of spaces collapsed.
fn clean_title(title: &str) -> String {
    title
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|c: char| c == '-' || c == '.' || c == '_' || c.is_whitespace())
        .to_owned()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{Item, is_extra, parse};
    use crate::libraries::Kind;

    #[expect(
        clippy::unnecessary_wraps,
        reason = "a table row's expected value is an `Option`, `None` where the path doesn't parse"
    )]
    fn movie(title: &str, year: Option<i64>, tmdb_id: Option<i64>) -> Option<Vec<Item>> {
        Some(vec![Item::Movie {
            title: title.to_owned(),
            year,
            tmdb_id,
        }])
    }

    #[expect(
        clippy::unnecessary_wraps,
        reason = "a table row's expected value is an `Option`, `None` where the path doesn't parse"
    )]
    fn episodes(
        show: &str,
        year: Option<i64>,
        tmdb_id: Option<i64>,
        season: i64,
        numbers: &[i64],
    ) -> Option<Vec<Item>> {
        Some(
            numbers
                .iter()
                .map(|&episode| Item::Episode {
                    show: show.to_owned(),
                    year,
                    tmdb_id,
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
            // ID hints, in Plex's and Jellyfin's spellings, on the folder or the file.
            (
                "Blade Runner (1982) {tmdb-78}/Blade Runner (1982).mkv",
                movie("Blade Runner", Some(1982), Some(78)),
            ),
            (
                "Blade Runner (1982) [tmdbid-78]/Blade Runner (1982).mkv",
                movie("Blade Runner", Some(1982), Some(78)),
            ),
            (
                "Blade Runner (1982)/Blade Runner (1982) {tmdb-78}.mkv",
                movie("Blade Runner", Some(1982), Some(78)),
            ),
            (
                "Blade Runner {TMDB-78}/movie.mkv",
                movie("Blade Runner", None, Some(78)),
            ),
            // Other ID hints are tags like any other.
            (
                "Blade Runner (1982) {imdb-tt0083658}/Blade Runner (1982).mkv",
                movie("Blade Runner", Some(1982), None),
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
            // The `1x01` spelling, which a resolution doesn't look like.
            ("Dark (2017)/Season 01/Dark - 1x04.mkv", dark(1, &[4])),
            ("Dark (2017)/Season 01/Dark 1920x1080.mkv", None),
            // A Show folder without a year.
            (
                "Dark/Season 01/Dark - S01E01.mkv",
                episodes("Dark", None, None, 1, &[1]),
            ),
            // ID hints on the Show's folder.
            (
                "Dark (2017) {tmdb-70523}/Season 01/Dark - S01E01.mkv",
                episodes("Dark", Some(2017), Some(70523), 1, &[1]),
            ),
            (
                "Dark (2017) [tmdbid-70523]/Season 01/Dark - S01E01.mkv",
                episodes("Dark", Some(2017), Some(70523), 1, &[1]),
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
            ("Blade Runner (1982)/Blade Runner (1982) - Sample.mkv", true),
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
}
