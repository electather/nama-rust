//! The External IDs in the `.nfo` files Kodi, Jellyfin and Emby leave beside videos, and which
//! Items take them as ID Hints in place of their names' (ADR 0087). Nothing else in a file is
//! read: no title, plot, artwork or cast.
//!
//! A Movie takes `<video name>.nfo`, else its stack's `<name>.nfo` (`Alien.nfo` for
//! `Alien-cd1.mkv`), else `movie.nfo` in a folder holding no other Movie. An Episode takes
//! `tvshow.nfo` in its Show's folder for its Show, and `<video name>.nfo` for itself. Names match
//! in any case, and a file naming no ID counts as none.
//!
//! IDs are read leniently, strongest first, the first of each namespace winning: a typed
//! `<uniqueid>`, the `default="true"` one first, then an untyped one holding an IMDb id, then
//! `<imdbid>`, `<tmdbid>` and `<tvdbid>`, then a legacy `<id>`. Only the first block counts, as
//! in a multi-episode file. A file not starting with `<` is read for IMDb, TMDB and TVDB links or a
//! bare IMDb id. Each is checked as a name's ID Hint is, so placeholders such as `0` go.

use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    ffi::OsStr,
    io::{self, Read as _},
    path::{Path, PathBuf},
    sync::LazyLock,
};

use encoding_rs::{Encoding, UTF_8};
use nama_plugin_api::metadata::{ExternalId, IMDB, TMDB, TVDB};
use regex::{Captures, Regex};

use super::{
    names::{self, regex},
    walking::Found,
};
use crate::library::{Item, NfoFile, NfoHints, NfoItem};

/// The largest `.nfo` file read, so a huge or endless one can't stall a scan.
const MAX_BYTES: u64 = 4 * 1024 * 1024;

/// The XML declaration's charset, as in `<?xml version="1.0" encoding="ISO-8859-1"?>`.
static DECLARED: LazyLock<Regex> =
    LazyLock::new(|| regex(r#"(?i)^\s*<\?xml[^>]*encoding\s*=\s*["']([\w.:-]+)["']"#));

/// The name of the first element, after any declaration, comment or doctype.
static ROOT: LazyLock<Regex> = LazyLock::new(|| {
    regex(r"(?s)^\s*(?:<\?.*?\?>\s*|<!--.*?-->\s*|<!DOCTYPE[^>]*>\s*)*<(?P<root>[A-Za-z_][\w.-]*)")
});

/// Comments, and the cast and collection, whose ids aren't the title's.
static NOT_THE_TITLES: LazyLock<Regex> =
    LazyLock::new(|| regex(r"(?is)<!--.*?-->|<(?:actor|set)\b.*?</(?:actor|set)\s*>"));

static ID_ELEMENT: LazyLock<Regex> = LazyLock::new(|| {
    regex(concat!(
        r"(?is)<(?P<tag>uniqueid|imdbid|imdb_id|tmdbid|tvdbid|id)\b(?P<attributes>[^>]*)>",
        r"(?P<value>[^<]*)</(?:uniqueid|imdbid|imdb_id|tmdbid|tvdbid|id)\s*>",
    ))
});

static TYPE: LazyLock<Regex> = LazyLock::new(|| regex(r#"(?i)\btype\s*=\s*["']([^"']*)["']"#));

static DEFAULT: LazyLock<Regex> = LazyLock::new(|| regex(r#"(?i)\bdefault\s*=\s*["']true["']"#));

static ENTITY: LazyLock<Regex> = LazyLock::new(|| {
    regex(r"&(?:#(?P<decimal>\d+)|#[xX](?P<hex>[0-9a-fA-F]+)|(?P<named>amp|lt|gt|quot|apos));")
});

/// An IMDb, TMDB or TVDB link, or a bare IMDb id, as scrapers write a file of links alone.
static LINK: LazyLock<Regex> = LazyLock::new(|| {
    regex(concat!(
        r"(?i)themoviedb\.org/(?:movie|tv)/(?P<tmdb>\d+)",
        r"|thetvdb\.com/\S*?(?:[?&](?:series)?id=|series/)(?P<tvdb>\d+)",
        r"|(?:\b|_)(?P<imdb>tt\d{7,})(?:\b|_)",
    ))
});

/// `<name>` of a stack's part such as `<name>-cd1`, `<name> - Part 2` or `<name>.disc3`.
static STACK_PART: LazyLock<Regex> =
    LazyLock::new(|| regex(r"(?i)^(?P<name>.+?)[ _.-]*(?:cd|dvd|part|pt|disc|disk)[ _.-]*\d+$"));

/// Whether `path` names an `.nfo` file.
pub(super) fn is_nfo(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("nfo"))
}

/// The `.nfo` files a walk found, and the folders and files it couldn't read.
#[derive(Debug)]
pub(super) struct FoundNfos {
    pub(super) nfos: Vec<Found>,
    pub(super) unreadable: Vec<PathBuf>,
}

/// What a scan stores of the `.nfo` files and the ID Hints Items take from them.
#[derive(Debug, Default)]
pub(super) struct NfoChanges {
    /// Files new or changed since the last scan.
    pub(super) files: Vec<NfoFile>,
    /// Paths of stored files that are gone.
    pub(super) gone: Vec<String>,
    /// Items whose ID Hints changed, with their new ones.
    pub(super) items: Vec<(i64, NfoHints)>,
}

impl FoundNfos {
    /// What changed since the files `stored` were: reads only the found files that are new or
    /// whose size or modified time changed, and works out the ID Hints of each of `items`. An
    /// Item or file in or beside what the walk couldn't read keeps what it had.
    pub(super) async fn changes(self, stored: Vec<NfoFile>, items: Vec<NfoItem>) -> NfoChanges {
        let unread = |folder: Option<&Path>| {
            folder.is_some_and(|folder| {
                (self.unreadable.iter())
                    .any(|missed| folder.starts_with(missed) || missed.parent() == Some(folder))
            })
        };
        let mut stored: HashMap<String, NfoFile> = (stored.into_iter())
            .map(|file| (file.path.clone(), file))
            .collect();
        let mut changes = NfoChanges::default();
        let mut current = Vec::new();
        // A file under two of the Library's folders, one inside the other, is found twice.
        let mut seen = HashSet::new();
        for found in self.nfos {
            if !seen.insert(found.path.clone()) {
                continue;
            }
            let file = match stored.remove(&found.path) {
                Some(old) if (old.size, old.modified_ns) == (found.size, found.modified_ns) => old,
                _ => {
                    let ids = read_ids(Path::new(&found.path)).await;
                    let file = NfoFile {
                        path: found.path,
                        size: found.size,
                        modified_ns: found.modified_ns,
                        ids,
                    };
                    changes.files.push(file.clone());
                    file
                }
            };
            current.push(file);
        }
        for (path, file) in stored {
            if unread(Path::new(&path).parent()) {
                current.push(file);
            } else {
                changes.gone.push(path);
            }
        }
        let lookup = Lookup::new(&current, &items);
        for item in &items {
            let path = Path::new(&item.path);
            let hints = lookup.hints(path, &item.item);
            let read = !unread(path.parent()) && !unread(show_folder(path, &item.item));
            if read && hints != item.hints {
                changes.items.push((item.id, hints));
            }
        }
        changes
    }
}

/// The `.nfo` files naming any ID, by folder and lowercase name, and the Movies in each folder.
struct Lookup<'a> {
    ids: HashMap<(&'a Path, String), &'a [ExternalId]>,
    movies_in: HashMap<&'a Path, HashSet<(&'a str, Option<i64>)>>,
}

impl<'a> Lookup<'a> {
    fn new(files: &'a [NfoFile], items: &'a [NfoItem]) -> Self {
        let named = files.iter().filter(|file| !file.ids.is_empty());
        let ids = (named.filter_map(|file| {
            let path = Path::new(&file.path);
            let name = path.file_name()?.to_str()?.to_lowercase();
            Some(((path.parent()?, name), file.ids.as_slice()))
        }))
        .collect();
        let mut movies_in: HashMap<_, HashSet<_>> = HashMap::new();
        for item in items {
            if let (Item::Movie { title, year, .. }, Some(folder)) =
                (&item.item, Path::new(&item.path).parent())
            {
                movies_in
                    .entry(folder)
                    .or_default()
                    .insert((title.as_str(), *year));
            }
        }
        Self { ids, movies_in }
    }

    /// The ID Hints the video at `path`, holding `item`, takes from the `.nfo` files.
    fn hints(&self, path: &Path, item: &Item) -> NfoHints {
        let (Some(folder), Some(stem)) = (path.parent(), path.file_stem().and_then(OsStr::to_str))
        else {
            return NfoHints::default();
        };
        let own = self.ids_of(folder, stem);
        match item {
            Item::Movie { .. } => {
                let stack = STACK_PART.captures(stem).and_then(|part| part.name("name"));
                let alone = self
                    .movies_in
                    .get(folder)
                    .is_some_and(|movies| movies.len() == 1);
                let title = (own.or_else(|| self.ids_of(folder, stack?.as_str())))
                    .or_else(|| alone.then(|| self.ids_of(folder, "movie")).flatten());
                NfoHints {
                    title: title.unwrap_or_default(),
                    episode: Vec::new(),
                }
            }
            Item::Episode { .. } => NfoHints {
                title: show_folder(path, item)
                    .and_then(|show| self.ids_of(show, "tvshow"))
                    .unwrap_or_default(),
                episode: own.unwrap_or_default(),
            },
        }
    }

    /// The IDs of `<stem>.nfo` in `folder`, if it names any.
    fn ids_of(&self, folder: &Path, stem: &str) -> Option<Vec<ExternalId>> {
        let name = format!("{}.nfo", stem.to_lowercase());
        self.ids.get(&(folder, name)).map(|ids| ids.to_vec())
    }
}

/// The folder of the Show `item`, at `path`, is in, if it's an Episode.
fn show_folder<'p>(path: &'p Path, item: &Item) -> Option<&'p Path> {
    match item {
        Item::Episode { .. } => names::episode_folders(path).map(|(show, _)| show),
        Item::Movie { .. } => None,
    }
}

/// The IDs the `.nfo` file at `path` names, strongest first. None, with a warning naming the file,
/// for one that is a link, isn't a regular file, is over [`MAX_BYTES`], can't be read or names
/// none.
async fn read_ids(path: &Path) -> Vec<ExternalId> {
    let ids = match read(path).await {
        Ok(text) => ids_in(&text),
        Err(err) => {
            tracing::warn!("skipped {}: {err}", path.display());
            return Vec::new();
        }
    };
    if ids.is_empty() {
        tracing::warn!(
            "{} names no ID that could be read, so its video is Matched by its name",
            path.display()
        );
    }
    ids
}

/// The text of the `.nfo` file at `path`, decoded as its byte-order mark or XML declaration says,
/// else as UTF-8.
async fn read(path: &Path) -> io::Result<String> {
    let path = path.to_owned();
    let bytes = tokio::task::spawn_blocking(move || read_bytes(&path))
        .await
        .map_err(io::Error::other)??;
    Ok(decode(&bytes))
}

/// Blocks, so call it from `spawn_blocking`.
fn read_bytes(path: &Path) -> io::Result<Vec<u8>> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.is_symlink() {
        return Err(io::Error::other("it's a link, which isn't followed"));
    }
    if !metadata.is_file() {
        return Err(io::Error::other("it isn't a regular file"));
    }
    if metadata.len() > MAX_BYTES {
        return Err(io::Error::other("it's over 4 MiB"));
    }
    let mut bytes = Vec::new();
    // Taken short of the limit, in case it grew since.
    (std::fs::File::open(path)?.take(MAX_BYTES)).read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn decode(bytes: &[u8]) -> String {
    let (encoding, bom) = Encoding::for_bom(bytes).unwrap_or_else(|| {
        let head = String::from_utf8_lossy(bytes.get(..200).unwrap_or(bytes));
        let declared = DECLARED
            .captures(&head)
            .and_then(|declared| Encoding::for_label(declared.get(1)?.as_str().as_bytes()));
        (declared.unwrap_or(UTF_8), 0)
    });
    let text = bytes.get(bom..).unwrap_or_default();
    encoding.decode_without_bom_handling(text).0.into_owned()
}

/// The IDs `text` names, strongest first, of each namespace the first.
fn ids_in(text: &str) -> Vec<ExternalId> {
    let mut found = if text.trim_start().starts_with('<') {
        xml_ids(text)
    } else {
        link_ids(text)
    };
    found.sort_by_key(|(strength, _, _)| *strength);
    let mut ids: Vec<ExternalId> = Vec::new();
    for (_, namespace, value) in found {
        let id = names::id_hint(&namespace, &value);
        if let Some(id) = id.filter(|id| ids.iter().all(|kept| kept.namespace() != id.namespace()))
        {
            ids.push(id);
        }
    }
    ids
}

/// Each id the first element of the XML `text` holds, as (strength, lowest strongest; namespace;
/// value), in document order.
fn xml_ids(text: &str) -> Vec<(u8, String, String)> {
    let Some(root) = ROOT.captures(text).and_then(|root| root.name("root")) else {
        return Vec::new();
    };
    let root = root.as_str().to_ascii_lowercase();
    // ASCII lowercasing keeps every byte where it was, so the offset fits `text`.
    let end = (text.to_ascii_lowercase()).find(&format!("</{root}"));
    let block = end.and_then(|end| text.get(..end)).unwrap_or(text);
    let block = NOT_THE_TITLES.replace_all(block, "");
    // A legacy numeric `<id>` is TVDB's in a Show's or Episode's file, as Kodi wrote them.
    let numeric_id = match root.as_str() {
        "tvshow" | "episodedetails" => TVDB,
        _ => TMDB,
    };
    let mut ids = Vec::new();
    for element in ID_ELEMENT.captures_iter(&block) {
        let value = unescaped(element.name("value").map_or("", |value| value.as_str()));
        let value = value.trim();
        let attributes = element
            .name("attributes")
            .map_or("", |found| found.as_str());
        let typed = TYPE.captures(attributes).and_then(|typed| typed.get(1));
        let imdb = value.to_ascii_lowercase().starts_with("tt");
        let tag = element
            .name("tag")
            .map_or_default(|tag| tag.as_str().to_ascii_lowercase());
        let (strength, namespace) = match (tag.as_str(), typed) {
            ("uniqueid", Some(typed)) => {
                let strength = u8::from(!DEFAULT.is_match(attributes));
                (strength, namespace_of(typed.as_str()))
            }
            ("uniqueid", None) if imdb => (2, IMDB.to_owned()),
            ("id", _) => (4, if imdb { IMDB } else { numeric_id }.to_owned()),
            ("uniqueid", None) => continue,
            (other, _) => (3, other.trim_end_matches("_id").to_owned()),
        };
        ids.push((strength, namespace, value.to_owned()));
    }
    ids
}

/// The namespace a `<uniqueid>`'s `type` names, in the spellings tools write.
fn namespace_of(typed: &str) -> String {
    match typed.to_ascii_lowercase().as_str() {
        "themoviedb" => TMDB.to_owned(),
        "thetvdb" => TVDB.to_owned(),
        other => other.to_owned(),
    }
}

/// Each id the links in `text` name, all equally strong, in order.
fn link_ids(text: &str) -> Vec<(u8, String, String)> {
    let mut ids = Vec::new();
    for link in LINK.captures_iter(text) {
        let named = [TMDB, TVDB, IMDB]
            .into_iter()
            .find_map(|namespace| Some((namespace, link.name(namespace)?)));
        if let Some((namespace, value)) = named {
            ids.push((0, namespace.to_owned(), value.as_str().to_owned()));
        }
    }
    ids
}

/// `text` with its character and XML entities replaced by what they stand for; an unknown or
/// invalid one stays.
fn unescaped(text: &str) -> Cow<'_, str> {
    ENTITY.replace_all(text, |entity: &Captures<'_>| {
        let code = (entity.name("decimal").and_then(|n| n.as_str().parse().ok())).or_else(|| {
            entity
                .name("hex")
                .and_then(|n| u32::from_str_radix(n.as_str(), 16).ok())
        });
        let named = entity.name("named").map(|named| match named.as_str() {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "quot" => '"',
            _ => '\'',
        });
        named.or_else(|| code.and_then(char::from_u32)).map_or_else(
            || {
                entity
                    .get(0)
                    .map_or_default(|whole| whole.as_str().to_owned())
            },
            String::from,
        )
    })
}

#[cfg(test)]
#[path = "nfo_tests.rs"]
mod tests;
