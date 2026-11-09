//! Reading Movies and Episodes out of file paths, following the naming conventions Plex and
//! Jellyfin share:
//!
//! - Movies: `Movie Name (Year)/Movie Name (Year).mkv`, or a bare `Movie Name (Year).mkv`,
//!   optionally inside grouping folders.
//! - Shows: `Show Name (Year)/Season 01/Show Name (Year) - S01E01.mkv`, optionally inside
//!   grouping folders, with multi-episode files (`S01E01-E02`, `S01E01E02`, `S01E01-02`) and
//!   specials in Season 0 (`S00E01`).
//! - Extras (`Featurettes/`, `-trailer` and the like) are recognised so they can be left out.
//! - ID Hints anywhere in a Movie's or Show's name: `{tmdb-123}`, `[tvdbid-123]`,
//!   `(imdb=tt0083658)` and the like, or a bare `tt0083658`.
//!
//! Only paths are read here. The scanner never opens `.nfo` files or any other sidecar.

use std::{path::Path, sync::LazyLock};

use nama_plugin_api::metadata::{ExternalId, ExternalIds};
use regex::Regex;

use crate::{contents::Item, libraries::Kind};

/// The version of this parser, stored with each file it reads. See the crate's `AGENTS.md` for
/// when to bump it.
pub(crate) const PARSER_VERSION: i64 = 3;

/// The most Episodes one file can hold, so a typo like `S01E01-E900` isn't 900 Episodes.
const MAX_EPISODES_PER_FILE: i64 = 20;

/// An ID Hint in any bracket, as Plex (`{tmdb-123}`), Jellyfin (`[tvdbid-123]`), Sonarr and
/// Radarr write them, or a bare IMDb id.
static ID_HINT: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r"(?i)[\[{(]\s*(?P<source>tmdb|tvdb|imdb)(?:id)?\s*[-=]\s*(?P<id>tt\d+|\d+)\s*[\]})]|(?:\b|_)(?P<bare>tt\d{7,})(?:\b|_)",
    )
});

/// Any `{…}` or `[…]` tag, such as an ID Hint, an edition or a quality.
static TAG: LazyLock<Regex> = LazyLock::new(|| regex(r"\{[^}]*\}|\[[^\]]*\]"));

/// A name ending (before any trailing tags or ` - suffix`) in a year in brackets.
static TITLE_YEAR: LazyLock<Regex> =
    LazyLock::new(|| regex(r"^(?P<title>.*?)\s*\((?P<year>\d{4})\)"));

/// `S01E01`, `s1e1`, or a multi-episode `S01E01-E02`, `S01E01E02`, `S01E01-S01E02` or
/// `S01E01-02`, not glued to a word before it.
static EPISODE: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r"(?i)(?:^|[^a-z0-9])s(?P<season>\d{1,3})[ ._]?e(?P<first>\d{1,4})(?P<more>(?:-?(?:s\d{1,3})?e\d{1,4}|-\d{1,3}(?:[^0-9a-z]|$))*)",
    )
});

/// The episode numbers after the first in [`EPISODE`]'s `more` group.
static MORE_EPISODES: LazyLock<Regex> = LazyLock::new(|| regex(r"(?i)[e-](\d+)"));

/// A Season's folder inside a Show's folder: `Season 01`, `Season 1`, `S01` or `Specials`.
static SEASON_FOLDER: LazyLock<Regex> =
    LazyLock::new(|| regex(r"(?i)^(?:season[ ._-]*\d{1,4}|s\d{1,4}|specials)$"));

/// `1x01`, with no digits or letters glued on either side.
static EPISODE_X: LazyLock<Regex> = LazyLock::new(|| {
    regex(r"(?i)(?:^|[^a-z0-9])(?P<season>\d{1,2})x(?P<first>\d{2,3})(?:[^a-z0-9]|$)")
});

/// Folders inside a title's folder that hold its extras rather than the title itself.
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

/// File name suffixes, straight after a `-` with no spaces, that mark an extra, as in
/// `Alien (1979)-trailer.mkv`.
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

#[expect(
    clippy::expect_used,
    reason = "the patterns are constants, and the tests compile every one"
)]
fn regex(pattern: &str) -> Regex {
    Regex::new(pattern).expect("a constant pattern compiles")
}

struct Split {
    folders: Vec<String>,
    stem: String,
}

impl Split {
    fn new(relative: &Path) -> Option<Self> {
        let mut folders: Vec<String> = relative
            .iter()
            .map(|c| c.to_string_lossy().into_owned())
            .collect();
        let file = folders.pop()?;
        let stem = Path::new(&file).file_stem()?.to_string_lossy().into_owned();
        Some(Self { folders, stem })
    }
}

/// Whether the file is an extra (a trailer, a featurette), which the scanner leaves out.
pub(crate) fn is_extra(relative: &Path) -> bool {
    let Some(Split { folders, stem }) = Split::new(relative) else {
        return false;
    };
    // An extras folder only holds extras straight inside a title's or a Season's folder: a
    // grouping folder may be called `Shorts`, and so may a title.
    let in_extras_folder = folders.windows(2).any(|pair| {
        let [holder, folder] = pair else {
            return false;
        };
        EXTRAS_FOLDERS.contains(&folder.to_lowercase().as_str())
            && (SEASON_FOLDER.is_match(holder) || title_and_year(holder).is_some())
    });
    in_extras_folder
        || stem
            .rsplit_once('-')
            .is_some_and(|(_, suffix)| EXTRAS_SUFFIXES.contains(&suffix.to_lowercase().as_str()))
}

/// Parses the path of a video file, relative to the Library folder it's in, in a Library of
/// `kind`. Returns `None` if the path doesn't follow the naming conventions, which leaves the
/// file Unmatched.
pub(crate) fn parse(kind: Kind, relative: &Path) -> Option<Vec<Item>> {
    let Split { folders, stem } = Split::new(relative)?;
    match kind {
        Kind::Movies => parse_movie(&folders, &stem).map(|movie| vec![movie]),
        Kind::Shows => parse_episodes(&folders, &stem),
    }
}

/// A Movie, named by the file if its name gives a year or an ID Hint, or else by the nearest
/// folder that does.
///
/// A file that names its Movie takes the ID Hints of the folder it's in only if the folder names
/// the same Movie, so a hint on a grouping folder isn't pinned on everything in it. Of each kind
/// of hint, the file's wins.
fn parse_movie(folders: &[String], stem: &str) -> Option<Item> {
    let named = match title_and_year(stem) {
        Some(file) => {
            let folder_hints = folders
                .last()
                .and_then(|folder| title_and_year(folder))
                .filter(|folder| {
                    folder.year == file.year
                        && folder.title.to_lowercase() == file.title.to_lowercase()
                })
                .map(|folder| folder.hints)
                .unwrap_or_default();
            Named {
                hints: ExternalIds {
                    tmdb: file.hints.tmdb.or(folder_hints.tmdb),
                    imdb: file.hints.imdb.clone().or(folder_hints.imdb),
                    tvdb: file.hints.tvdb.or(folder_hints.tvdb),
                },
                ..file
            }
        }
        None => folders
            .iter()
            .rev()
            .find_map(|folder| title_and_year(folder))?,
    };
    Some(Item::Movie {
        hint: named.hint(),
        title: named.title,
        year: named.year,
    })
}

/// The Episodes in a file, in the Show named by the folder holding the file's Season folder, or
/// by the folder holding the file if it isn't in a Season folder. An ID Hint on an Episode's file
/// names the Episode, not the Show, so it isn't used.
fn parse_episodes(folders: &[String], stem: &str) -> Option<Vec<Item>> {
    let (parent, above) = folders.split_last()?;
    let show_folder = if SEASON_FOLDER.is_match(parent) {
        above.last()?
    } else {
        parent
    };
    if SEASON_FOLDER.is_match(show_folder) {
        return None;
    }
    let show = title_and_year(show_folder).or_else(|| {
        let title = clean_title(&TAG.replace_all(show_folder, ""));
        (!title.is_empty()).then_some(Named {
            title,
            year: None,
            hints: ExternalIds::default(),
        })
    })?;
    let hint = show.hint();
    let (season, episodes) = episode_numbers(stem)?;
    Some(
        episodes
            .into_iter()
            .map(|episode| Item::Episode {
                show: show.title.clone(),
                year: show.year,
                hint: hint.clone(),
                season,
                episode,
            })
            .collect(),
    )
}

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

struct Named {
    title: String,
    year: Option<i64>,
    /// Every ID Hint in the name, of each kind the first.
    hints: ExternalIds,
}

impl Named {
    /// The ID Hint Matching goes by: TMDB's, which the metadata source is, or else TVDB's, which
    /// Sonarr writes, or else IMDb's.
    fn hint(&self) -> Option<ExternalId> {
        let hints = &self.hints;
        hints
            .tmdb
            .map(ExternalId::Tmdb)
            .or_else(|| hints.tvdb.map(ExternalId::Tvdb))
            .or_else(|| hints.imdb.clone().map(ExternalId::Imdb))
    }
}

/// The title in a name like `Title (Year)`, or any name with an ID Hint. `None` for a name with
/// neither, which is too little to go on.
fn title_and_year(name: &str) -> Option<Named> {
    let hints = id_hints(name);
    let unhinted = ID_HINT.replace_all(name, "");
    let untagged = TAG.replace_all(&unhinted, "");
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
    if title.is_empty() || (year.is_none() && hints == ExternalIds::default()) {
        return None;
    }
    Some(Named { title, year, hints })
}

fn id_hints(name: &str) -> ExternalIds {
    let mut hints = ExternalIds::default();
    for captures in ID_HINT.captures_iter(name) {
        // A bare id has no source, and is always IMDb's.
        let source = captures
            .name("source")
            .map(|source| source.as_str().to_ascii_lowercase());
        let Some(id) = captures.name("id").or_else(|| captures.name("bare")) else {
            continue;
        };
        let id = id.as_str().to_ascii_lowercase();
        match source.as_deref() {
            Some("tmdb") => hints.tmdb = hints.tmdb.or_else(|| id.parse().ok()),
            Some("tvdb") => hints.tvdb = hints.tvdb.or_else(|| id.parse().ok()),
            Some("imdb") | None if id.starts_with("tt") => {
                hints.imdb.get_or_insert(id);
            }
            _ => {}
        }
    }
    hints
}

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

    use nama_plugin_api::metadata::ExternalId;

    use super::{is_extra, parse};
    use crate::{contents::Item, libraries::Kind};

    const fn tmdb(id: i64) -> ExternalId {
        ExternalId::Tmdb(id)
    }

    const fn tvdb(id: i64) -> ExternalId {
        ExternalId::Tvdb(id)
    }

    fn imdb(id: &str) -> ExternalId {
        ExternalId::Imdb(id.to_owned())
    }

    #[expect(
        clippy::unnecessary_wraps,
        reason = "a table row's expected value is an `Option`, `None` where the path doesn't parse"
    )]
    fn movie(
        title: &str,
        year: Option<i64>,
        hint: impl Into<Option<ExternalId>>,
    ) -> Option<Vec<Item>> {
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
}
