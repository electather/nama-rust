//! The words ffmpeg prints when the folder it writes into refuses a write.

/// Why the folder ffmpeg wrote into refused its write, which says nothing about what it read.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RefusedWrite {
    /// The folder's disk is full, or its owner is over quota.
    Full,
    /// The folder's file system is read-only.
    ReadOnly,
}

impl RefusedWrite {
    /// Says whether ffmpeg's `stderr` reports a refused write, and why.
    // ponytail: matches the C library's English messages, as in the Docker image; read ffmpeg's
    // exit code instead if a Server in another locale needs it.
    pub(crate) fn in_stderr(stderr: &str) -> Option<Self> {
        if stderr.contains("No space left on device") || stderr.contains("Disk quota exceeded") {
            Some(Self::Full)
        } else if stderr.contains("Read-only file system") {
            Some(Self::ReadOnly)
        } else {
            None
        }
    }
}

#[cfg(test)]
#[path = "refused_write_tests.rs"]
mod tests;
