//! What a failed read of a Version's file says about the file, for whoever reads it.

use std::path::Path;

use crate::ffmpeg::{RefusedWrite, RunError};

/// How a Version's file was read, which decides what a timeout means.
#[derive(Debug, Clone, Copy)]
pub(super) enum Read {
    /// A probe of its headers, tried again up to `PROBE_ATTEMPTS` times.
    Probe,
    /// A read of the whole file: listing keyframes, the text pass or Preview frames.
    WholeFile,
}

/// What a failed read of a Version's file means for reading it again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ReadFailure {
    /// The Server has no such program, so nothing was tried.
    NoProgram,
    /// The program couldn't be started, so every other file would fail the same.
    WontRun,
    /// Says nothing about this file, which may read fine next time.
    NotTheFile,
    /// May not happen next time, so it's tried again a few times.
    Passing,
    /// Will happen again until the file changes.
    Lasting,
}

impl ReadFailure {
    /// What `error`, from a `read` of the Version's file at `path`, means.
    pub(super) async fn of(error: &RunError, read: Read, path: impl AsRef<Path>) -> Self {
        match (error, read) {
            (RunError::Unavailable(_), _) => Self::NoProgram,
            (RunError::Run(..), _) => Self::WontRun,
            (RunError::Failed(_, stderr), _) if RefusedWrite::in_stderr(stderr).is_some() => {
                Self::NotTheFile
            }
            // ffmpeg and ffprobe fail fast on a file they can't open, as on a share that dropped
            // or an NFS handle gone stale since the scan, which may read fine once it's back.
            (RunError::Failed(..), _) if !readable(path).await => Self::NotTheFile,
            // A slow or unmounted share, or a busy disk, may not be slow next time.
            (RunError::TimedOut(_), Read::Probe) => Self::Passing,
            // A whole-file read that timed out held a read slot for up to an hour (#850).
            (RunError::Failed(..), _) | (RunError::TimedOut(_), Read::WholeFile) => Self::Lasting,
        }
    }
}

/// Whether the file at `path` can be opened and read from now.
async fn readable(path: impl AsRef<Path>) -> bool {
    use tokio::io::AsyncReadExt;

    match tokio::fs::File::open(path).await {
        Ok(mut file) => file.read(&mut [0; 1]).await.is_ok(),
        Err(_) => false,
    }
}

#[cfg(test)]
#[path = "read_failures_tests.rs"]
mod tests;
