//! Fake programs, such as an ffmpeg, written as shell scripts for tests to run.

use std::{
    io::Write as _,
    path::Path,
    process::{Command, Stdio},
};

use super::FoundProgram;

/// The program `name`, the shell script `body` written to `bin`.
///
/// A child `sh` writes it, lest a fork hold it open and make it "Text file busy" (`AGENTS.md`).
///
/// # Panics
///
/// If writing it fails.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
#[expect(
    clippy::must_use_candidate,
    reason = "writing the program is the point; a caller may not need it found"
)]
pub fn script(bin: &Path, name: &str, body: &str) -> FoundProgram {
    let path = bin.join(name);
    let mut writer = Command::new("sh")
        .args(["-c", r#"cat > "$0" && chmod 755 "$0""#])
        .arg(&path)
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    // Dropping stdin closes it, so `cat` ends.
    writer
        .stdin
        .take()
        .unwrap()
        .write_all(body.as_bytes())
        .unwrap();
    let status = writer.wait().unwrap();
    assert!(
        status.success(),
        "writing {} failed: {status}",
        path.display()
    );
    FoundProgram {
        path,
        version: "7.1.1".to_owned(),
    }
}
