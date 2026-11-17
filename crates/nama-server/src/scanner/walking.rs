//! Walking a Library's folders for the video files in them and the sidecar subtitles and images
//! beside those, and planning what a scan changes from what it found and what was stored.

use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    mem,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use tokio_util::sync::CancellationToken;
use walkdir::{DirEntry, WalkDir};

use super::{Error, Planned, Summary, names};
use crate::{
    library::{Change, FoundImages, LibraryKind, LocalImageFile, PARSER_VERSION, Role, Stored},
    playback::{Sidecar, VersionSidecars, is_sidecar},
};

/// Extensions, in lowercase, of the files the scanner treats as video.
const VIDEO_EXTENSIONS: &[&str] = &[
    "3gp", "avi", "divx", "flv", "m2ts", "m4v", "mkv", "mov", "mp4", "mpeg", "mpg", "mts", "ogm",
    "ogv", "ts", "webm", "wmv",
];

/// Extensions, in lowercase, of the images that may stand in for a downloaded one: those a Custom
/// Image may be.
const IMAGE_EXTENSIONS: &[&str] = &["jpeg", "jpg", "png", "webp"];

/// Names, without the extension, of an image standing in for a Poster, and for a Backdrop, as
/// Jellyfin and Kodi name them (`docs/research/jellyfin-and-plex-images.md`, "Overriding").
const POSTER_NAMES: &[&str] = &["poster", "folder", "cover", "default", "movie"];
const BACKDROP_NAMES: &[&str] = &["fanart", "backdrop", "background", "art"];

#[derive(Debug)]
struct Found {
    path: String,
    /// The path relative to the Library folder it was found in, which is what's parsed.
    relative: PathBuf,
    size: i64,
    modified_ns: i64,
}

#[derive(Debug, Default)]
pub(super) struct Walked {
    files: Vec<Found>,
    sidecars: Vec<Found>,
    images: Vec<Found>,
    /// Folders and files that couldn't be read. What's stored under them is kept.
    unreadable: Vec<PathBuf>,
    /// The Library's folders the Server can't write to.
    pub(super) read_only: Vec<String>,
}

/// Blocks, so call it from `spawn_blocking`.
pub(super) fn walk(folders: &[String], cancel: &CancellationToken) -> Result<Walked, Error> {
    let mut walked = Walked::default();
    for folder in folders {
        let root = Path::new(folder);
        if !writable(root) {
            walked.read_only.push(folder.clone());
        }
        let entries = WalkDir::new(root)
            .follow_links(true)
            .into_iter()
            .filter_entry(|entry| entry.depth() == 0 || !is_hidden(entry.file_name()));
        for entry in entries {
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            match entry.and_then(|entry| listed_file(root, &entry)) {
                Ok(Some(file)) if is_video(Path::new(&file.path)) => walked.files.push(file),
                Ok(Some(file)) if is_sidecar(Path::new(&file.path)) => walked.sidecars.push(file),
                Ok(Some(image)) => walked.images.push(image),
                Ok(None) => {}
                Err(err) => {
                    let path = err.path().unwrap_or(root).to_owned();
                    tracing::warn!("could not read {}: {err}", path.display());
                    walked.unreadable.push(path);
                }
            }
        }
    }
    Ok(walked)
}

/// Whether the Server may delete what's in `folder`.
#[cfg(unix)]
fn writable(folder: &Path) -> bool {
    // A read-only mount, such as the README's Docker example, leaves the folder's mode writable.
    rustix::fs::access(folder, rustix::fs::Access::WRITE_OK).is_ok()
}

// ponytail: Windows folders count as writable, so a delete there fails file by file instead of
// being refused; read their ACLs once a Server runs on Windows.
#[cfg(not(unix))]
const fn writable(_folder: &Path) -> bool {
    true
}

/// Whether a file or folder is hidden: dot files, and the thumbnails Synology NASes leave.
fn is_hidden(name: &OsStr) -> bool {
    name.as_encoded_bytes().starts_with(b".") || name == "@eaDir"
}

/// Whether `path`'s extension is a video's.
pub(super) fn is_video(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|ext| VIDEO_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
}

/// The video, sidecar subtitle or image at `entry`, if it is one outside an extras folder.
fn listed_file(root: &Path, entry: &DirEntry) -> Result<Option<Found>, walkdir::Error> {
    let path = entry.path();
    let listed = is_video(path) || is_sidecar(path) || image_name(path).is_some();
    if !entry.file_type().is_file() || !listed {
        return Ok(None);
    }
    let Ok(relative) = path.strip_prefix(root) else {
        return Ok(None);
    };
    if names::is_extra(relative) {
        return Ok(None);
    }
    let Some(utf8) = path.to_str() else {
        tracing::warn!("skipped {}: its path isn't valid UTF-8", path.display());
        return Ok(None);
    };
    let metadata = entry.metadata()?;
    Ok(Some(Found {
        path: utf8.to_owned(),
        relative: relative.to_owned(),
        size: i64::try_from(metadata.len()).unwrap_or(i64::MAX),
        modified_ns: metadata.modified().map_or(0, nanos_since_epoch),
    }))
}

/// Nanoseconds since the Unix epoch, saturating.
fn nanos_since_epoch(time: SystemTime) -> i64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(after) => i64::try_from(after.as_nanos()).unwrap_or(i64::MAX),
        Err(before) => i64::try_from(before.duration().as_nanos()).map_or(i64::MIN, |n| -n),
    }
}

/// What a scan changes, given what was `stored` and what was `found`. A file the name parser
/// makes no Item from is planned with none, and its path kept for Recognisers.
pub(super) fn plan(
    kind: LibraryKind,
    mut stored: HashMap<String, Stored>,
    found: Walked,
) -> (Vec<Planned>, Summary) {
    let mut planned = Vec::new();
    let mut summary = Summary::default();
    // A file under two of the Library's folders, one inside the other, is found twice.
    let mut seen = HashSet::new();
    let parse = |relative: PathBuf| {
        let items = names::parse(kind, &relative).unwrap_or_default();
        let unread = items.is_empty().then_some(relative);
        (items, unread)
    };
    for file in found.files {
        if !seen.insert(file.path.clone()) {
            continue;
        }
        let added = match stored.remove(&file.path) {
            Some(old) if old.size == file.size && old.modified_ns == file.modified_ns => {
                if old.parser_version < PARSER_VERSION {
                    summary.updated += 1;
                    let (items, unread) = parse(file.relative);
                    let change = Change::Reparse {
                        file: old.id,
                        items,
                        path: file.path,
                    };
                    planned.push(Planned {
                        change,
                        unread,
                        added: false,
                    });
                } else {
                    summary.unchanged += 1;
                }
                continue;
            }
            Some(_) => {
                summary.updated += 1;
                false
            }
            None => {
                summary.added += 1;
                true
            }
        };
        let (items, unread) = parse(file.relative);
        let change = Change::Store {
            items,
            path: file.path,
            size: file.size,
            modified_ns: file.modified_ns,
        };
        planned.push(Planned {
            change,
            unread,
            added,
        });
    }
    for (path, old) in stored {
        let path = Path::new(&path);
        if found.unreadable.iter().any(|dir| path.starts_with(dir)) {
            summary.unchanged += 1;
        } else {
            summary.removed += 1;
            let change = Change::Remove(old.id);
            planned.push(Planned {
                change,
                unread: None,
                added: false,
            });
        }
    }
    (planned, summary)
}

/// The images a walk found that may stand in for a Movie's, and the folders and files it couldn't
/// read.
#[derive(Debug)]
pub(super) struct FoundLocalImages {
    images: Vec<Found>,
    unreadable: Vec<PathBuf>,
}

/// How an image's name ties it to a video: named after it goes first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Naming {
    /// Such as `Up (2009)-poster.jpg`.
    AfterVideo,
    /// Such as `poster.jpg`.
    Alone,
}

impl Walked {
    /// Takes the images out, leaving the videos to [`plan`].
    pub(super) fn take_images(&mut self) -> FoundLocalImages {
        FoundLocalImages {
            images: mem::take(&mut self.images),
            unreadable: self.unreadable.clone(),
        }
    }
}

impl FoundLocalImages {
    /// The ids of `versions`, stored files by path, whose images the walk read, and the image found
    /// beside each that stands in for each role. A Version in a folder the walk couldn't read, or
    /// beside something it couldn't, is left out, so its images stay as stored. An image named alone
    /// stands in for every video in its folder, except in one of the Library's own folders, where
    /// it would for every Movie; one named after a video goes first, then the first by path.
    pub(super) fn beside(self, versions: &HashMap<String, Stored>) -> (Vec<i64>, FoundImages) {
        let mut read = Vec::new();
        let mut videos_in: HashMap<&Path, Vec<(&OsStr, i64)>> = HashMap::new();
        for (path, version) in versions {
            let path = Path::new(path);
            let (Some(folder), Some(stem)) = (path.parent(), path.file_stem()) else {
                continue;
            };
            let unread =
                |missed: &PathBuf| folder.starts_with(missed) || missed.parent() == Some(folder);
            if !self.unreadable.iter().any(unread) {
                read.push(version.id);
                videos_in
                    .entry(folder)
                    .or_default()
                    .push((stem, version.id));
            }
        }
        (read, tied_images(self.images, &videos_in))
    }
}

/// Of `images`, the one standing in for each role of each video in `videos_in`, which holds each
/// folder's videos by their names without the extension, with their Version ids.
fn tied_images(
    mut images: Vec<Found>,
    videos_in: &HashMap<&Path, Vec<(&OsStr, i64)>>,
) -> FoundImages {
    images.sort_by(|a, b| a.path.cmp(&b.path));
    let mut best: HashMap<(i64, Role), (Naming, LocalImageFile)> = HashMap::new();
    for image in images {
        let path = Path::new(&image.path);
        let in_library_folder = image.relative.parent() == Some(Path::new(""));
        let videos = path.parent().and_then(|folder| videos_in.get(folder));
        let tied = (videos.into_iter().flatten()).filter_map(|&(stem, version)| {
            Some((version, image_role(path, stem, in_library_folder)?))
        });
        for (version, (naming, role)) in tied {
            if best
                .get(&(version, role))
                .is_none_or(|(kept, _)| naming < *kept)
            {
                let file = LocalImageFile {
                    path: image.path.clone(),
                    size: image.size,
                    modified_ns: image.modified_ns,
                };
                best.insert((version, role), (naming, file));
            }
        }
    }
    (best.into_iter())
        .map(|(key, (_, file))| (key, file))
        .collect()
}

/// How the image at `image` stands in for the video named `video` without its extension, and for
/// which of its images, if it does. `in_library_folder` if the image is in one of the Library's
/// own folders, where only one named after the video does.
fn image_role(image: &Path, video: &OsStr, in_library_folder: bool) -> Option<(Naming, Role)> {
    let name = image_name(image)?;
    let video = video.to_str()?.to_lowercase();
    let after_video = (name.strip_prefix(&video))
        .and_then(|rest| rest.strip_prefix('-'))
        .and_then(role_named);
    match after_video {
        Some(role) => Some((Naming::AfterVideo, role)),
        None if in_library_folder => None,
        None => role_named(&name).map(|role| (Naming::Alone, role)),
    }
}

/// `path`'s name in lowercase without its extension, if it is an image's that may stand in for a
/// downloaded one.
fn image_name(path: &Path) -> Option<String> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    if !IMAGE_EXTENSIONS.contains(&extension.as_str()) {
        return None;
    }
    let name = path.file_stem()?.to_str()?.to_lowercase();
    let named = |role: &str| role_named(role).is_some();
    let tied = named(&name) || name.rsplit_once('-').is_some_and(|(_, role)| named(role));
    tied.then_some(name)
}

/// The role an image named `name`, alone or after its video, stands in for.
fn role_named(name: &str) -> Option<Role> {
    if POSTER_NAMES.contains(&name) {
        Some(Role::Poster)
    } else if BACKDROP_NAMES.contains(&name) {
        Some(Role::Backdrop)
    } else {
        None
    }
}

/// The sidecar subtitles a walk found, and the folders and files it couldn't read.
#[derive(Debug)]
pub(super) struct FoundSidecars {
    sidecars: Vec<Found>,
    unreadable: Vec<PathBuf>,
}

impl Walked {
    /// Takes the sidecars out, leaving the videos to [`plan`].
    pub(super) fn take_sidecars(&mut self) -> FoundSidecars {
        FoundSidecars {
            sidecars: mem::take(&mut self.sidecars),
            unreadable: self.unreadable.clone(),
        }
    }
}

impl FoundSidecars {
    /// The sidecars found beside each of `versions`, stored files by path, whose found set
    /// differs from the one `stored` holds by Version id. A Version in a folder the walk couldn't
    /// read, or beside a sidecar it couldn't, is left out, so its sidecars stay as stored.
    pub(super) fn changed(
        self,
        versions: &HashMap<String, Stored>,
        mut stored: HashMap<i64, Vec<Sidecar>>,
    ) -> Vec<VersionSidecars> {
        let mut found = by_version(self.sidecars, versions);
        let mut changed = Vec::new();
        for (path, version) in versions {
            let path = Path::new(path);
            let (Some(folder), Some(stem)) = (path.parent(), path.file_stem()) else {
                continue;
            };
            let unread = |missed: &PathBuf| {
                folder.starts_with(missed)
                    || (missed.parent() == Some(folder) && is_sidecar(missed))
            };
            if self.unreadable.iter().any(unread) {
                continue;
            }
            let mut sidecars = found.remove(&version.id).unwrap_or_default();
            sidecars.sort();
            // A sidecar under two of the Library's folders, one inside the other, is found twice.
            sidecars.dedup();
            if stored.remove(&version.id).unwrap_or_default() != sidecars {
                changed.push(VersionSidecars {
                    version: version.id,
                    stem: stem.to_string_lossy().into_owned(),
                    sidecars,
                });
            }
        }
        changed
    }
}

/// `sidecars` by the id of the one of `versions`, stored files by path, each belongs to. One that
/// belongs to none is left out.
fn by_version(
    sidecars: Vec<Found>,
    versions: &HashMap<String, Stored>,
) -> HashMap<i64, Vec<Sidecar>> {
    let mut videos_in: HashMap<&Path, Vec<&Path>> = HashMap::new();
    for path in versions.keys().map(Path::new) {
        if let Some(folder) = path.parent() {
            videos_in.entry(folder).or_default().push(path);
        }
    }
    // Sorted, so of two videos with one stem a sidecar picks the same one each scan.
    for videos in videos_in.values_mut() {
        videos.sort();
    }
    let mut found: HashMap<i64, Vec<Sidecar>> = HashMap::new();
    for sidecar in sidecars {
        let path = Path::new(&sidecar.path);
        let videos = path.parent().and_then(|folder| videos_in.get(folder));
        let video = videos.and_then(|videos| video_of(path, videos));
        let Some(version) = video
            .and_then(Path::to_str)
            .and_then(|video| versions.get(video))
        else {
            continue;
        };
        found.entry(version.id).or_default().push(Sidecar {
            path: sidecar.path,
            size: sidecar.size,
            modified_ns: sidecar.modified_ns,
        });
    }
    found
}

/// The one of `videos`, each in `sidecar`'s folder, that `sidecar` belongs to: the one with the
/// longest name without its extension that `sidecar`'s name starts with, followed by `.`.
fn video_of<'a>(sidecar: &Path, videos: &[&'a Path]) -> Option<&'a Path> {
    (videos.iter().copied())
        .filter_map(|video| Some((video, video.file_stem()?)))
        .filter(|(_, stem)| after_stem(sidecar, stem).is_some_and(|rest| rest.starts_with(b".")))
        .max_by_key(|(_, stem)| stem.len())
        .map(|(video, _)| video)
}

/// What follows `stem` in `file`'s name, if its name starts with it.
pub(super) fn after_stem<'a>(file: &'a Path, stem: &OsStr) -> Option<&'a [u8]> {
    (file.file_name())?
        .as_encoded_bytes()
        .strip_prefix(stem.as_encoded_bytes())
}

#[cfg(test)]
#[path = "walking_tests.rs"]
mod tests;
