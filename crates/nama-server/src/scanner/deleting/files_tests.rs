use std::{fs, io, path::Path};

use nama_plugin_api::metadata::ExternalId;
use tempfile::TempDir;

use super::{Doomed, delete, each_file, preview};
use crate::{
    library::{Library, LibraryKind, LibraryVersion, Stored},
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
        let versions = (1..).zip(videos).map(|(id, video)| LibraryVersion {
            file: Stored {
                id,
                size: 0,
                modified_ns: 0,
                parser_version: 0,
            },
            path: self.path(video),
            edition: None,
        });
        Doomed {
            library: Library {
                id: 1,
                name: "Media".to_owned(),
                kind: LibraryKind::Movies,
                folders: vec![self.path("")],
                read_only: Vec::new(),
                empty_folders: Vec::new(),
                title_sources: vec!["tmdb".to_owned()],
                recognisers: Vec::new(),
                segment_sources: Vec::new(),
                preview_frames: true,
                last_scan: None,
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
        id: ExternalId::tmdb(1),
        season: Some(number),
    };
    doomed.requested = vec![(1, season(1)), (2, season(1)), (3, season(2))];

    assert_eq!(doomed.request_titles(&[1, 2]), [season(1)]);
}
