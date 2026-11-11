//! Fake programs, such as an ffmpeg, written as shell scripts.

use std::{fs, os::unix::fs::PermissionsExt, path::Path};

use nama_server::ffmpeg::FoundProgram;

/// The program `name`, the shell script `body` written to `bin`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
pub(crate) fn script(bin: &Path, name: &str, body: &str) -> FoundProgram {
    let path = bin.join(name);
    fs::write(&path, body).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    FoundProgram {
        path,
        version: "7.1.1".to_owned(),
    }
}
