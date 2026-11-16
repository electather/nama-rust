//! Walking a Library's folders for the video files in them, and planning what a scan changes
//! from what it found and what was stored.

use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use tokio_util::sync::CancellationToken;
use walkdir::{DirEntry, WalkDir};

use super::{Error, Planned, Summary, names};
use crate::{
    library::contents::{Change, Stored},
    library::libraries::Kind,
    library::versions::PARSER_VERSION,
};

/// Extensions, in lowercase, of the files the scanner treats as video.
const VIDEO_EXTENSIONS: &[&str] = &[
    "3gp", "avi", "divx", "flv", "m2ts", "m4v", "mkv", "mov", "mp4", "mpeg", "mpg", "mts", "ogm",
    "ogv", "ts", "webm", "wmv",
];

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
            match entry.and_then(|entry| video_file(root, &entry)) {
                Ok(Some(file)) => walked.files.push(file),
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

fn video_file(root: &Path, entry: &DirEntry) -> Result<Option<Found>, walkdir::Error> {
    let path = entry.path();
    if !entry.file_type().is_file() || !is_video(path) {
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
    kind: Kind,
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
