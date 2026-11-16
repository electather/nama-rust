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
//!   `(imdb=tt0083658)` and the like, a bare `tt0083658`, or `{namespace-value}` in any other
//!   namespace, such as `{anidb-5101}`.
//! - A country tag ending a Show's name, as Sonarr names same-name Shows apart: `The Office (US)`.
//!   It stays in the stored name, so pins keep their key; Matching reads it with
//!   [`country_tag`](crate::library::country_tags::country_tag).
//!
//! Only paths are read here. The scanner never opens `.nfo` files or any other sidecar.

use std::{path::Path, sync::LazyLock};

use nama_plugin_api::metadata::{ExternalId, ExternalIds, IMDB, TMDB, TVDB};
use regex::Regex;

use crate::{library::contents::Item, library::libraries::Kind};

/// The most Episodes one file can hold, so a typo like `S01E01-E900` isn't 900 Episodes.
pub(super) const MAX_EPISODES_PER_FILE: i64 = 20;

/// An ID Hint in any bracket, as Plex (`{tmdb-123}`), Jellyfin (`[tvdbid-123]`), Sonarr and
/// Radarr write them, `{namespace-value}` in any namespace, or a bare IMDb id.
static ID_HINT: LazyLock<Regex> = LazyLock::new(|| {
    regex(concat!(
        r"(?i)[\[{(]\s*(?P<source>tmdb|tvdb|imdb)(?:id)?\s*[-=]\s*(?P<id>tt\d+|\d+)\s*[\]})]",
        r"|\{\s*(?P<namespace>[a-z0-9]+)-(?P<value>[^\s\[\](){}]+)\s*\}",
        r"|(?:\b|_)(?P<bare>tt\d{7,})(?:\b|_)",
    ))
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
pub(super) fn is_extra(relative: &Path) -> bool {
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
pub(super) fn parse(kind: Kind, relative: &Path) -> Option<Vec<Item>> {
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
                hints: file_wins(&file.hints, folder_hints),
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

/// `folder`'s ID Hints, each replaced by `file`'s of the same namespace.
fn file_wins(file: &ExternalIds, folder: ExternalIds) -> ExternalIds {
    let mut hints = folder;
    for id in file.iter() {
        hints.insert(id);
    }
    hints
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
    /// Sonarr writes, or else IMDb's, or else the first of any other namespace.
    fn hint(&self) -> Option<ExternalId> {
        [TMDB, TVDB, IMDB]
            .into_iter()
            .find_map(|namespace| self.hints.get(namespace))
            .or_else(|| self.hints.iter().next())
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
    if title.is_empty() || (year.is_none() && hints.is_empty()) {
        return None;
    }
    Some(Named { title, year, hints })
}

fn id_hints(name: &str) -> ExternalIds {
    let mut hints = ExternalIds::default();
    for captures in ID_HINT.captures_iter(name) {
        // A bare id has no source, and is always IMDb's.
        let (namespace, value) = if let Some(bare) = captures.name("bare") {
            (IMDB.to_owned(), bare.as_str())
        } else if let (Some(source), Some(id)) = (captures.name("source"), captures.name("id")) {
            (source.as_str().to_ascii_lowercase(), id.as_str())
        } else if let (Some(namespace), Some(value)) =
            (captures.name("namespace"), captures.name("value"))
        {
            (namespace.as_str().to_ascii_lowercase(), value.as_str())
        } else {
            continue;
        };
        let hint = id_hint(&namespace, value);
        // The first hint of a namespace in a name wins.
        if let Some(hint) = hint.filter(|hint| hints.get(hint.namespace()).is_none()) {
            hints.insert(hint);
        }
    }
    hints
}

/// The id an ID Hint names, or `None` where it names none. The well-known namespaces keep the
/// forms they had before any namespace was read, `{tmdbid-123}` included.
fn id_hint(namespace: &str, value: &str) -> Option<ExternalId> {
    let digits = |text: &str| !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit());
    match namespace.strip_suffix("id").unwrap_or(namespace) {
        // Leading zeros go, as they did when these were read as numbers.
        well_known @ (TMDB | TVDB) => digits(value)
            .then(|| value.parse::<i64>().ok())
            .flatten()
            .and_then(|id| ExternalId::new(well_known, id.to_string()).ok()),
        IMDB => {
            let value = value.to_ascii_lowercase();
            value
                .strip_prefix("tt")
                .is_some_and(digits)
                .then(|| ExternalId::new(IMDB, value).ok())
                .flatten()
        }
        // Plex's edition tag names a cut, not a title.
        _ if namespace == "edition" => None,
        _ => ExternalId::new(namespace, value).ok(),
    }
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
#[path = "names_tests.rs"]
mod tests;
