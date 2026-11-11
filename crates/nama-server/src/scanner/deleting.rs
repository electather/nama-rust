//! Deleting media from disk: each Version's file, its sidecars, and the folders that leaves empty,
//! never a Library's own folder or one above it (ADR 0038). Blocks, so call it from
//! `spawn_blocking`.

use std::{
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
};

use super::is_video;
use crate::{contents::Version, libraries::Library};

/// Versions of one title of a Library, which a delete removes, with the title, for its log.
#[derive(Debug)]
pub(crate) struct Doomed {
    pub(crate) library: Library,
    pub(crate) title: String,
    pub(crate) versions: Vec<Version>,
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

/// Deletes each Version's file, then its sidecars, then the folders that leaves empty. A Version
/// whose file can't be deleted keeps its sidecars; the rest still go, and nothing is put back.
pub(crate) fn delete(doomed: &Doomed) -> Deleted {
    let deleted = each_file(doomed, |path| fs::remove_file(path));
    for version in &doomed.versions {
        let video = Path::new(&version.path);
        if let (Some(folder), Some(root)) = (video.parent(), root_of(&doomed.library, video)) {
            remove_empty_folders(folder, root);
        }
    }
    deleted
}

/// Removes each Version's file through `remove`, then, once it's gone, its sidecars.
fn each_file(doomed: &Doomed, remove: impl Fn(&Path) -> io::Result<()>) -> Deleted {
    let mut deleted = Deleted::default();
    let videos: Vec<&Path> = (doomed.versions.iter())
        .map(|version| Path::new(&version.path))
        .collect();
    for (version, &video) in doomed.versions.iter().zip(&videos) {
        if root_of(&doomed.library, video).is_none() {
            let outside = io::Error::other("it isn't in any of its Library's folders");
            deleted.failed.push((video.to_owned(), outside));
            continue;
        }
        match remove_file(video, &remove) {
            Ok(file) => deleted.files.push(file),
            Err(err) => {
                deleted.failed.push((video.to_owned(), err));
                continue;
            }
        }
        deleted.versions.push(version.file.id);
        let sidecars = match sidecars(video, &videos) {
            Ok(sidecars) => sidecars,
            Err(err) => {
                let folder = video.parent().unwrap_or(video).to_owned();
                deleted.failed.push((folder, err));
                continue;
            }
        };
        for sidecar in sidecars {
            match remove_file(&sidecar, &remove) {
                Ok(file) => deleted.files.push(file),
                Err(err) => deleted.failed.push((sidecar, err)),
            }
        }
    }
    deleted
}

/// The file at `path` removed through `remove`. One already gone counts as removed, as when a
/// Version and its sidecar are named alike.
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
/// that stays, one not among `doomed`, is a sidecar.
fn sidecars(video: &Path, doomed: &[&Path]) -> io::Result<Vec<PathBuf>> {
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
        .filter(|file| is_video(file) && !doomed.contains(&file.as_path()))
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

/// Removes `folder` and each folder above it while they're empty, stopping below `root`.
fn remove_empty_folders(folder: &Path, root: &Path) {
    for emptied in folder.ancestors().take_while(|&above| above != root) {
        // Fails on a folder that isn't empty, which stops it there.
        if fs::remove_dir(emptied).is_err() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use tempfile::TempDir;

    use super::{Doomed, delete, preview};
    use crate::{
        contents::{Stored, Version},
        libraries::{Kind, Library},
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
                },
                title: "Title".to_owned(),
                versions: versions.collect(),
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

        let deleted = delete(&fx.doomed(&["Up/Up.mkv"]));

        assert_eq!(names(&deleted.files), ["Up.mkv", "Up.srt"]);
        assert!(fx.exists("Up/Up-1080p.mkv") && fx.exists("Up/Up-1080p.srt"));
    }

    #[test]
    fn folders_left_empty_go_up_to_but_not_including_the_librarys_folder() {
        let fx = Fixture::new();
        fx.write("Up/Disc 1/Up.mkv");

        let deleted = delete(&fx.doomed(&["Up/Disc 1/Up.mkv"]));

        assert_eq!(deleted.versions, [1]);
        assert!(!fx.exists("Up"));
        assert!(fx.root.path().is_dir());
    }

    #[test]
    fn a_folder_with_something_else_left_in_it_stays() {
        let fx = Fixture::new();
        fx.write("Up/Disc 1/Up.mkv");
        fx.write("Up/folder.jpg");

        delete(&fx.doomed(&["Up/Disc 1/Up.mkv"]));

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

        let deleted = delete(&doomed);

        assert!(Path::new(&outside).exists());
        assert_eq!(deleted.failed.len(), 1);
        assert_eq!(deleted.versions, [0; 0]);
    }
}
