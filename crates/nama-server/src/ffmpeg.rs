//! Finding the ffmpeg and ffprobe the Server Remuxes, Transcodes and probes Versions with, which
//! come from the system, not the release (ADR 0026).

use std::{env, ffi::OsStr, path::PathBuf, process::Command};

/// A program the Server found and ran.
#[derive(Debug, PartialEq, Eq)]
pub struct FoundProgram {
    /// Where it is.
    pub path: PathBuf,
    /// Its version as it reports it, such as `7.1.1`.
    pub version: String,
}

/// The ffmpeg and ffprobe the Server found at startup. Without both it only Direct Plays.
#[derive(Debug, Default)]
pub struct Ffmpeg {
    /// ffmpeg, or `None` when it wasn't found.
    pub ffmpeg: Option<FoundProgram>,
    /// ffprobe, or `None` when it wasn't found.
    pub ffprobe: Option<FoundProgram>,
}

impl Ffmpeg {
    /// Looks for ffmpeg and ffprobe in the directories of `path`, a `PATH` value, and runs each
    /// to read its version. Blocks while they run.
    #[must_use]
    pub fn find(path: &OsStr) -> Self {
        Self {
            ffmpeg: find("ffmpeg", path),
            ffprobe: find("ffprobe", path),
        }
    }

    /// Why the Server can only Direct Play, in plain words, or `None` when it found both.
    #[must_use]
    pub fn direct_play_only_because(&self) -> Option<&'static str> {
        match (&self.ffmpeg, &self.ffprobe) {
            (Some(_), Some(_)) => None,
            (None, None) => Some("ffmpeg and ffprobe weren't found on PATH"),
            (None, Some(_)) => Some("ffmpeg wasn't found on PATH"),
            (Some(_), None) => Some("ffprobe wasn't found on PATH"),
        }
    }
}

/// The first `name` in the directories of `path` that runs and says which version it is.
fn find(name: &str, path: &OsStr) -> Option<FoundProgram> {
    let file = format!("{name}{}", env::consts::EXE_SUFFIX);
    env::split_paths(path)
        // A relative entry, such as an empty one, would run whatever is in the working directory.
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(&file))
        .filter(|candidate| candidate.is_file())
        .find_map(|candidate| run(name, candidate))
}

/// `name` at `path`, if it runs and says which version it is.
fn run(name: &str, path: PathBuf) -> Option<FoundProgram> {
    let output = Command::new(&path).arg("-version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    // The first line reads like `ffmpeg version 7.1.1 Copyright (c) 2000-2025 the FFmpeg developers`.
    let version = String::from_utf8_lossy(&output.stdout)
        .strip_prefix(&format!("{name} version "))?
        .split_whitespace()
        .next()?
        .to_owned();
    Some(FoundProgram { path, version })
}

// The fakes are shell scripts.
#[cfg(all(test, unix))]
mod tests {
    use std::{fs, os::unix::fs::PermissionsExt, path::Path};

    use super::{Ffmpeg, FoundProgram};

    /// Writes a fake `name` into `dir` that prints `version_line` as `-version` does.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    fn fake(dir: &Path, name: &str, version_line: &str) {
        let path = dir.join(name);
        fs::write(&path, format!("#!/bin/sh\necho '{version_line}'\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn both_on_path_are_found_with_their_path_and_version() {
        let empty = tempfile::tempdir().unwrap();
        let bin = tempfile::tempdir().unwrap();
        fake(
            bin.path(),
            "ffmpeg",
            "ffmpeg version 7.1.1 Copyright (c) 2000-2025",
        );
        fake(
            bin.path(),
            "ffprobe",
            "ffprobe version n7.0 Copyright (c) 2007-2024",
        );
        let path = std::env::join_paths([empty.path(), bin.path()]).unwrap();

        let found = Ffmpeg::find(&path);

        assert_eq!(
            found.ffmpeg,
            Some(FoundProgram {
                path: bin.path().join("ffmpeg"),
                version: "7.1.1".to_owned(),
            })
        );
        assert_eq!(
            found.ffprobe,
            Some(FoundProgram {
                path: bin.path().join("ffprobe"),
                version: "n7.0".to_owned(),
            })
        );
        assert_eq!(found.direct_play_only_because(), None);
    }

    #[test]
    fn with_neither_on_path_the_server_can_only_direct_play() {
        let empty = tempfile::tempdir().unwrap();

        let found = Ffmpeg::find(empty.path().as_os_str());

        assert_eq!(found.ffmpeg, None);
        assert_eq!(found.ffprobe, None);
        assert_eq!(
            found.direct_play_only_because(),
            Some("ffmpeg and ffprobe weren't found on PATH")
        );
    }

    #[test]
    fn one_that_doesnt_run_is_skipped_for_a_later_one_on_path() {
        let broken = tempfile::tempdir().unwrap();
        let bin = tempfile::tempdir().unwrap();
        fake(broken.path(), "ffmpeg", "something else entirely");
        fake(bin.path(), "ffmpeg", "ffmpeg version 7.1.1");
        let path = std::env::join_paths([broken.path(), bin.path()]).unwrap();

        let found = Ffmpeg::find(&path);

        assert_eq!(
            found.ffmpeg,
            Some(FoundProgram {
                path: bin.path().join("ffmpeg"),
                version: "7.1.1".to_owned(),
            })
        );
    }

    #[test]
    fn a_program_that_doesnt_say_its_ffmpeg_version_isnt_found() {
        let bin = tempfile::tempdir().unwrap();
        fake(bin.path(), "ffmpeg", "something else entirely");
        fake(bin.path(), "ffprobe", "ffprobe version 7.1.1");

        let found = Ffmpeg::find(bin.path().as_os_str());

        assert_eq!(found.ffmpeg, None);
        assert_eq!(
            found.direct_play_only_because(),
            Some("ffmpeg wasn't found on PATH")
        );
    }
}
