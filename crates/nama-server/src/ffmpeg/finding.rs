//! Finding ffmpeg and ffprobe at the configured path or on `PATH`, and checking their versions.

use std::{
    env,
    ffi::OsStr,
    path::{Component, Path, PathBuf},
    process::Command,
};

use super::FoundProgram;

/// The oldest ffmpeg and ffprobe the Server uses, as major and minor version. HLS with fMP4
/// segments, the zscale and tonemap filters and the VAAPI, QSV and NVENC encoders that Remuxing
/// and Transcoding use all predate 4.4, and it's the oldest a supported LTS distribution ships
/// (Ubuntu 22.04), so nothing older gets tested.
const MINIMUM: (u32, u32) = (4, 4);

/// `name` at `configured`, or on `path` when `configured` is unset or a bare program name, if
/// it's new enough, or why the Server can't use it.
pub(super) fn find(
    name: &str,
    configured: Option<&Path>,
    path: &OsStr,
) -> Result<FoundProgram, String> {
    let found = match configured {
        None => on_path(name, OsStr::new(name), path).ok_or_else(|| not_on_path(name))?,
        Some(configured) => match bare_name(configured) {
            Some(file) => on_path(name, file, path).ok_or_else(|| {
                format!(
                    "{name} (configured as {}) wasn't found on PATH",
                    configured.display()
                )
            })?,
            None => at_configured_path(name, configured)?,
        },
    };
    if is_release_before(&found.version, MINIMUM) {
        let (major, minor) = MINIMUM;
        return Err(format!(
            "{name} {} at {} is older than {major}.{minor}, the oldest this Server supports",
            found.version,
            found.path.display()
        ));
    }
    Ok(found)
}

pub(super) fn not_on_path(name: &str) -> String {
    format!("{name} wasn't found on PATH")
}

/// `path` as a file name with no folder, which is looked up on `PATH` as a shell would.
fn bare_name(path: &Path) -> Option<&OsStr> {
    match path.components().collect::<Vec<_>>()[..] {
        [Component::Normal(name)] => Some(name),
        _ => None,
    }
}

/// `name` at the path the config sets, or why it can't be run.
fn at_configured_path(name: &str, path: &Path) -> Result<FoundProgram, String> {
    // `Config::load` made a path with a folder absolute; left relative, it would break where
    // ffmpeg runs in another folder (#1120).
    if !path.is_file() {
        return Err(format!(
            "{name} wasn't found at the configured path {}",
            path.display()
        ));
    }
    let shown = path.display().to_string();
    run(name, path.to_owned())
        .ok_or_else(|| format!("{name} at the configured path {shown} won't run or isn't {name}"))
}

/// The first `file` in the directories of `path` that runs and says it's `name` and which version.
fn on_path(name: &str, file: &OsStr, path: &OsStr) -> Option<FoundProgram> {
    let mut file = PathBuf::from(file);
    // As `Command::new` finds programs on Windows, so `ffmpeg.exe` doesn't become `ffmpeg.exe.exe`.
    if file.extension().is_none() {
        file.set_extension(env::consts::EXE_EXTENSION);
    }
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

/// Whether `version` is a release older than `release`, as major and minor version.
pub(super) fn is_release_before(version: &str, release: (u32, u32)) -> bool {
    // A release says `7.1.1`, `n7.0` or `4.4.2-0ubuntu0.22.04.1`. A build from git says
    // `N-113021-g5b3fc5d`, with no release number, so it gets the benefit of the doubt.
    let mut numbers = version
        .trim_start_matches('n')
        .split(|c: char| !c.is_ascii_digit())
        .map(|number| number.parse::<u32>().ok());
    let Some(Some(major)) = numbers.next() else {
        return false;
    };
    let minor = numbers.next().flatten().unwrap_or(0);
    (major, minor) < release
}
