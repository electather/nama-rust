//! Deleting media from disk: each Version's file, its sidecars, and the folders that leaves empty,
//! never a Library's own folder or one above it (ADR 0038). Blocks, so call it from
//! `spawn_blocking`.

use std::{
    collections::BTreeSet,
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
};

use nama_plugin_api::fulfiller::stop_monitoring::Title;
use nama_proto::nama::v1::Request;

use super::is_video;
use crate::{
    contents::{Episode, Version},
    libraries::Library,
    requests::RequestTitle,
};

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
    /// Episode it holds. One with no TMDB id, which no Request names, isn't here.
    pub(crate) requested: Vec<(i64, RequestTitle)>,
}

impl Doomed {
    pub(crate) fn version_ids(&self) -> Vec<i64> {
        self.versions
            .iter()
            .map(|version| version.file.id)
            .collect()
    }

    /// What the Versions `gone` are of, as Requests name it, each once.
    pub(crate) fn request_titles(&self, gone: &[i64]) -> Vec<RequestTitle> {
        let titles = (self.requested.iter()).filter(|(version, _)| gone.contains(version));
        let titles: BTreeSet<RequestTitle> = titles.map(|&(_, title)| title).collect();
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
    pub(crate) versions: Vec<i64>,
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
pub(crate) fn preview(doomed: &Doomed) -> Deleted {
    each_file(doomed, |_| Ok(()))
}

/// Deletes each Version's file, then its sidecars, then the folders that leaves empty, stopping
/// below any of `kept`, every Library's folders. A Version whose file can't be deleted keeps its
/// sidecars; the rest still go, and nothing is put back.
pub(crate) fn delete(doomed: &Doomed, kept: &[&Path]) -> Deleted {
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
mod tests {
    use std::{fs, io, path::Path};

    use tempfile::TempDir;

    use super::{Doomed, delete, each_file, preview};
    use crate::{
        contents::{Stored, Version},
        libraries::{Kind, Library},
        requests::RequestTitle,
    };

    /// A Library over a scratch folder.
    struct Fixture {
        root: TempDir,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                root: tempfile::tempdir().unwrap(),
            }
        }

        fn write(&self, relative: &str) {
            let path = self.root.path().join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, relative).unwrap();
        }

        fn exists(&self, relative: &str) -> bool {
            self.root.path().join(relative).exists()
        }

        /// The Versions at `videos` of this Library, numbered from 1.
        fn doomed(&self, videos: &[&str]) -> Doomed {
            let versions = (1..).zip(videos).map(|(id, video)| Version {
                file: Stored {
                    id,
                    size: 0,
                    modified_ns: 0,
                    parser_version: 0,
                },
                path: self.path(video),
            });
            Doomed {
                library: Library {
                    id: 1,
                    name: "Media".to_owned(),
                    kind: Kind::Movies,
                    folders: vec![self.path("")],
                    read_only: Vec::new(),
                },
                title: "Title".to_owned(),
                to_unmonitor: None,
                versions: versions.collect(),
                taken_along: Vec::new(),
                requested: Vec::new(),
            }
        }

        fn path(&self, relative: &str) -> String {
            let path = self.root.path().join(relative);
            path.to_str().unwrap().trim_end_matches('/').to_owned()
        }
    }

    /// The names of `files`, sorted.
    fn names(files: &[super::File]) -> Vec<String> {
        let mut names: Vec<_> = (files.iter())
            .map(|file| file.path.file_name().unwrap().to_str().unwrap().to_owned())
            .collect();
        names.sort_unstable();
        names
    }

    #[test]
    fn a_sidecar_is_named_after_the_video_then_a_dot_or_a_dash() {
        let fx = Fixture::new();
        for file in [
            "Up/Up.mkv",
            "Up/Up.en.srt",
            "Up/Up-thumb.jpg",
            "Up/Up.nfo",
            "Up/Upside.nfo",
            "Up/Up Close.jpg",
            "Up/folder.jpg",
        ] {
            fx.write(file);
        }

        let planned = preview(&fx.doomed(&["Up/Up.mkv"]));

        assert_eq!(
            names(&planned.files),
            ["Up-thumb.jpg", "Up.en.srt", "Up.mkv", "Up.nfo"]
        );
    }

    #[test]
    fn a_video_or_a_file_named_after_a_video_that_stays_is_no_sidecar() {
        let fx = Fixture::new();
        for file in [
            "Up/Up.mkv",
            "Up/Up.srt",
            "Up/Up-1080p.mkv",
            "Up/Up-1080p.srt",
        ] {
            fx.write(file);
        }

        let deleted = delete(&fx.doomed(&["Up/Up.mkv"]), &[]);

        assert_eq!(names(&deleted.files), ["Up.mkv", "Up.srt"]);
        assert!(fx.exists("Up/Up-1080p.mkv") && fx.exists("Up/Up-1080p.srt"));
    }

    #[test]
    fn a_sidecar_named_after_two_versions_is_listed_once() {
        let fx = Fixture::new();
        for file in ["Up/Up.mkv", "Up/Up-1080p.mkv", "Up/Up-1080p.srt"] {
            fx.write(file);
        }

        let planned = preview(&fx.doomed(&["Up/Up.mkv", "Up/Up-1080p.mkv"]));

        assert_eq!(
            names(&planned.files),
            ["Up-1080p.mkv", "Up-1080p.srt", "Up.mkv"]
        );
    }

    #[test]
    fn a_version_that_cant_be_deleted_keeps_a_sidecar_named_after_another_too() {
        let fx = Fixture::new();
        for file in ["Up/Up.mkv", "Up/Up-1080p.mkv", "Up/Up-1080p.srt"] {
            fx.write(file);
        }
        let stuck = Path::new("Up-1080p.mkv");

        let deleted = each_file(&fx.doomed(&["Up/Up.mkv", "Up/Up-1080p.mkv"]), |path| {
            if path.ends_with(stuck) {
                Err(io::Error::from(io::ErrorKind::PermissionDenied))
            } else {
                fs::remove_file(path)
            }
        });

        assert_eq!(names(&deleted.files), ["Up.mkv"]);
        assert_eq!(deleted.versions, [1]);
        assert!(fx.exists("Up/Up-1080p.srt"));
    }

    #[test]
    fn another_librarys_folder_left_empty_stays() {
        let fx = Fixture::new();
        fx.write("Movies/Up/Up.mkv");
        let other = fx.root.path().join("Movies");

        delete(&fx.doomed(&["Movies/Up/Up.mkv"]), &[&other]);

        assert!(!fx.exists("Movies/Up"));
        assert!(other.is_dir());
    }

    #[test]
    fn folders_left_empty_go_up_to_but_not_including_the_librarys_folder() {
        let fx = Fixture::new();
        fx.write("Up/Disc 1/Up.mkv");

        let deleted = delete(&fx.doomed(&["Up/Disc 1/Up.mkv"]), &[]);

        assert_eq!(deleted.versions, [1]);
        assert!(!fx.exists("Up"));
        assert!(fx.root.path().is_dir());
    }

    #[test]
    fn a_folder_with_something_else_left_in_it_stays() {
        let fx = Fixture::new();
        fx.write("Up/Disc 1/Up.mkv");
        fx.write("Up/folder.jpg");

        delete(&fx.doomed(&["Up/Disc 1/Up.mkv"]), &[]);

        assert!(!fx.exists("Up/Disc 1"));
        assert!(fx.exists("Up/folder.jpg"));
    }

    #[test]
    fn a_file_outside_the_librarys_folders_is_left_and_reported() {
        let fx = Fixture::new();
        let elsewhere = tempfile::tempdir().unwrap();
        let outside = elsewhere.path().join("Up.mkv");
        fs::write(&outside, "video").unwrap();
        let mut doomed = fx.doomed(&["Up.mkv"]);
        outside
            .to_str()
            .unwrap()
            .clone_into(&mut doomed.versions.first_mut().unwrap().path);

        let deleted = delete(&doomed, &[]);

        assert!(Path::new(&outside).exists());
        assert_eq!(deleted.failed.len(), 1);
        assert_eq!(deleted.versions, [0; 0]);
    }

    #[test]
    fn only_the_versions_that_went_name_the_requests_to_cancel_each_once() {
        let fx = Fixture::new();
        let mut doomed = fx.doomed(&["S01E01-E02.mkv", "S01E01.mkv", "S02E01.mkv"]);
        let season = |number| RequestTitle {
            tmdb_id: 1,
            season: Some(number),
        };
        doomed.requested = vec![(1, season(1)), (2, season(1)), (3, season(2))];

        assert_eq!(doomed.request_titles(&[1, 2]), [season(1)]);
    }
}
