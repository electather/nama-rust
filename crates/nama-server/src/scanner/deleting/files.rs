//! Deleting media from disk: each Version's file, its sidecars, and the folders that leaves empty,
//! never a Library's own folder or one above it (ADR 0038). Blocks, so call it from
//! `spawn_blocking`.

use std::{
    collections::BTreeSet,
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
};

use super::{Deleted, Doomed};
use crate::{
    library::Library,
    scanner::{after_stem, is_video},
};

/// The first folder of `library` holding one of `paths` that the Library's last scan found the
/// Server can't write to.
pub(crate) fn read_only_folder<'a, 'p>(
    library: &'a Library,
    paths: impl IntoIterator<Item = &'p str>,
) -> Option<&'a str> {
    (paths.into_iter())
        .filter_map(|path| root_of(library, Path::new(path))?.to_str())
        .find(|root| library.read_only.iter().any(|folder| folder == root))
}

/// A file on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct File {
    pub(crate) path: PathBuf,
    /// In bytes.
    pub(crate) size: u64,
}

/// What [`delete`] would delete, deleting nothing.
pub(super) fn preview(doomed: &Doomed) -> Deleted {
    each_file(doomed, |_| Ok(()))
}

/// Deletes each Version's file, then its sidecars, then the folders that leaves empty, stopping
/// below any of `kept`, every Library's folders. A Version whose file can't be deleted keeps its
/// sidecars; the rest still go, and nothing is put back.
pub(super) fn delete(doomed: &Doomed, kept: &[&Path]) -> Deleted {
    let deleted = each_file(doomed, |path| fs::remove_file(path));
    for version in &doomed.versions {
        let video = Path::new(&version.path);
        if let (Some(folder), Some(root)) = (video.parent(), root_of(&doomed.library, video)) {
            remove_empty_folders(folder, root, kept);
        }
    }
    deleted
}

/// Removes each Version's file through `remove`, then the sidecars of those that went, each once.
fn each_file(doomed: &Doomed, remove: impl Fn(&Path) -> io::Result<()>) -> Deleted {
    let mut deleted = Deleted::default();
    let mut gone: Vec<&Path> = Vec::new();
    for version in &doomed.versions {
        let video = Path::new(&version.path);
        if root_of(&doomed.library, video).is_none() {
            let outside = io::Error::other("it isn't in any of its Library's folders");
            deleted.failed.push((video.to_owned(), outside));
            continue;
        }
        match remove_file(video, &remove) {
            Ok(file) => {
                deleted.files.push(file);
                deleted.versions.push(version.file.id);
                gone.push(video);
            }
            Err(err) => deleted.failed.push((video.to_owned(), err)),
        }
    }
    // Two Versions in one folder can share a sidecar (`Up-1080p.srt` is named after both `Up.mkv`
    // and `Up-1080p.mkv`), so it's removed and counted once.
    let mut sidecars = BTreeSet::new();
    for &video in &gone {
        match sidecars_of(video, &gone) {
            Ok(found) => sidecars.extend(found),
            Err(err) => {
                let folder = video.parent().unwrap_or(video).to_owned();
                deleted.failed.push((folder, err));
            }
        }
    }
    for sidecar in sidecars {
        match remove_file(&sidecar, &remove) {
            Ok(file) => deleted.files.push(file),
            Err(err) => deleted.failed.push((sidecar, err)),
        }
    }
    deleted
}

/// The file at `path` removed through `remove`. One already gone counts as removed, so a Version
/// whose file was deleted by hand still leaves the Library.
fn remove_file(path: &Path, remove: impl Fn(&Path) -> io::Result<()>) -> io::Result<File> {
    let gone = |err: &io::Error| err.kind() == io::ErrorKind::NotFound;
    let size = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata.len(),
        Err(err) if gone(&err) => 0,
        Err(err) => return Err(err),
    };
    match remove(path) {
        Err(err) if !gone(&err) => Err(err),
        _ => Ok(File {
            path: path.to_owned(),
            size,
        }),
    }
}

/// The sidecars of `video`: the files beside it whose names start with its own, without its
/// extension, followed by `.` or `-`. Neither a video nor a file that's also named after a video
/// that stays, one not among `gone`, is a sidecar.
fn sidecars_of(video: &Path, gone: &[&Path]) -> io::Result<Vec<PathBuf>> {
    let (Some(folder), Some(stem)) = (video.parent(), video.file_stem()) else {
        return Ok(Vec::new());
    };
    let mut files = Vec::new();
    for entry in fs::read_dir(folder)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            files.push(entry.path());
        }
    }
    let staying: Vec<&OsStr> = (files.iter())
        .filter(|file| is_video(file) && !gone.contains(&file.as_path()))
        .filter_map(|file| file.file_stem())
        .collect();
    let sidecar = |file: &&PathBuf| {
        !is_video(file)
            && named_after(file, stem)
            && !staying.iter().any(|&other| named_after(file, other))
    };
    Ok(files.iter().filter(sidecar).cloned().collect())
}

/// Whether `file`'s name is `stem` followed by `.` or `-` and more.
fn named_after(file: &Path, stem: &OsStr) -> bool {
    after_stem(file, stem).is_some_and(|rest| rest.starts_with(b".") || rest.starts_with(b"-"))
}

/// The innermost of `library`'s folders that `path` is inside, if any.
fn root_of<'a>(library: &'a Library, path: &Path) -> Option<&'a Path> {
    (library.folders.iter())
        .map(Path::new)
        .filter(|&root| path != root && path.starts_with(root))
        .max_by_key(|root| root.components().count())
}

/// Removes `folder` and each folder above it while they're empty, stopping below `root` or any of
/// `kept`: Libraries can overlap, and emptying one mustn't remove another's folder.
fn remove_empty_folders(folder: &Path, root: &Path, kept: &[&Path]) {
    let stop = |above: &&Path| *above == root || kept.contains(above);
    for emptied in folder.ancestors().take_while(|above| !stop(above)) {
        // Fails on a folder that isn't empty, which stops it there.
        if fs::remove_dir(emptied).is_err() {
            break;
        }
    }
}

#[cfg(test)]
#[path = "files_tests.rs"]
mod tests;
