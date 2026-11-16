//! Deleting media from disk: each Version's file, its sidecars, and the folders that leaves empty,
//! never a Library's own folder or one above it (ADR 0038). Blocks, so call it from
//! `spawn_blocking`. Also asking a Fulfiller again to stop monitoring what a delete took, when it
//! said to try again.

use std::{
    collections::BTreeSet,
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use nama_plugin_api::{
    PluginId,
    fulfiller::{Fulfiller, StopMonitoring, stop_monitoring::Title},
};
use nama_proto::nama::v1::Request;

use super::is_video;
use crate::{
    library::contents::{Episode, Version},
    library::libraries::Library,
    plugins::{ExtensionPoints, Failure, RETRIES},
    requests::RequestTitle,
};

// Kept: `scanner.rs` takes it from here for the first stop-monitoring ask `StopAgain` retries.
pub(super) use crate::plugins::Retryable;

/// A deleted title the Plugin `plugin`'s Fulfiller said to ask it again to stop monitoring:
/// `title`, which the log calls `name`.
pub(super) struct StopAgain {
    pub(super) plugin: PluginId,
    pub(super) title: StopMonitoring,
    pub(super) name: String,
}

impl StopAgain {
    /// Asks the Fulfiller of `K` again after `wait`, as it said to, the `retry`th time in a row.
    /// Waits in the Plugin's scope in `plugins`, so disabling it ends the wait at once (ADR 0047),
    /// and logs giving up.
    pub(super) fn after<K: 'static>(self, plugins: &ExtensionPoints, wait: Duration, retry: u32) {
        let points = plugins.clone();
        let plugin = self.plugin.clone();
        // Not spawned if the Plugin was disabled meanwhile, which leaves nothing to tell.
        plugins.spawn_in(&plugin, async move {
            tokio::time::sleep(wait).await;
            let title = &self.title;
            let stop = |fulfiller: Arc<dyn Fulfiller<K>>| async move {
                fulfiller.stop_monitoring(title).await.map_err(Retryable)
            };
            let answer = points.ask(&self.plugin, stop).await;
            let reason = match answer.map(|answer| answer.result) {
                Some(Ok(())) => return,
                Some(Err(Failure {
                    kind: Some(again), ..
                })) if retry < RETRIES => {
                    self.after::<K>(&points, again, retry + 1);
                    return;
                }
                Some(Err(failure)) => failure.message,
                None => "it panicked or was disabled".to_owned(),
            };
            tracing::warn!(
                plugin = %self.plugin,
                title = self.name,
                reason,
                retry,
                "a Fulfiller didn't stop monitoring a deleted title, so it may download it again"
            );
        });
    }
}

/// Versions of one title of a Library, which a delete removes, with the title, for its log.
#[derive(Debug)]
pub(crate) struct Doomed {
    pub(crate) library: Library,
    pub(crate) title: String,
    /// What the Library's Fulfillers are asked to stop monitoring. `None` for a title Matched
    /// without the ids they know it by.
    pub(crate) to_unmonitor: Option<Title>,
    /// Each file once.
    pub(crate) versions: Vec<Version>,
    /// The Episodes the title isn't, that a file of `versions` holds too.
    pub(crate) taken_along: Vec<Episode>,
    /// What each Version is of, by its id, as a Request names it: its Movie, or the Season of each
    /// Episode it holds.
    pub(crate) requested: Vec<(i64, RequestTitle)>,
}

impl Doomed {
    pub(super) fn version_ids(&self) -> Vec<i64> {
        self.versions
            .iter()
            .map(|version| version.file.id)
            .collect()
    }

    /// What the Versions `gone` are of, as Requests name it, each once.
    pub(super) fn request_titles(&self, gone: &[i64]) -> Vec<RequestTitle> {
        let titles = (self.requested.iter()).filter(|(version, _)| gone.contains(version));
        let titles: BTreeSet<RequestTitle> = titles.map(|(_, title)| title.clone()).collect();
        titles.into_iter().collect()
    }
}

impl Doomed {
    /// The folder of the Library holding one of the Versions that the Library's last scan found
    /// the Server can't write to, which refuses the delete.
    pub(crate) fn read_only_folder(&self) -> Option<&str> {
        read_only_folder(&self.library, self.versions.iter().map(|v| v.path.as_str()))
    }
}

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

/// What a delete did, or a preview would do.
#[derive(Debug, Default)]
pub(crate) struct Deleted {
    pub(crate) files: Vec<File>,
    /// Each file that couldn't be deleted, and why.
    pub(crate) failed: Vec<(PathBuf, io::Error)>,
    /// The Versions whose own files went.
    pub(super) versions: Vec<i64>,
    /// The name of each Fulfiller that couldn't be told to stop monitoring the title.
    pub(crate) fulfillers_not_told: Vec<String>,
    /// How many Playback Plans play the Versions, as a preview finds them.
    pub(crate) playing: usize,
    /// The Requests deleting would cancel, as a preview finds them.
    pub(crate) requests: Vec<Request>,
}

impl Deleted {
    pub(crate) fn bytes_freed(&self) -> u64 {
        self.files.iter().map(|file| file.size).sum()
    }
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
    (file.file_name())
        .and_then(|name| {
            name.as_encoded_bytes()
                .strip_prefix(stem.as_encoded_bytes())
        })
        .is_some_and(|rest| rest.starts_with(b".") || rest.starts_with(b"-"))
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
#[path = "deleting_tests.rs"]
mod tests;
