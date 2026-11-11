//! Fake programs, such as an ffmpeg, written as shell scripts for tests to run.

use std::{
    io::Write as _,
    path::Path,
    process::{Command, Stdio},
};

use super::FoundProgram;

/// The program `name`, the shell script `body` written to `bin`.
///
/// A child `sh` writes it, lest a fork hold it open and make it "Text file busy"
/// (`crates/nama-server/AGENTS.md`, "Fake programs").
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

/// A GPU at the PCI `address` from `vendor`, such as `0x1002` for AMD, as a fake `/sys` in `sys`
/// shows it, with the render node `node`, such as `renderD128`, in `dri`, a fake `/dev/dri`. Its
/// `/sys/class/drm` is `sys/class/drm`.
///
/// # Panics
///
/// If writing it fails.
#[cfg(unix)]
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
pub fn gpu(sys: &Path, dri: &Path, node: &str, address: &str, vendor: &str) {
    let function = pci_function(sys, address, vendor);
    let drm = sys.join("class/drm").join(node);
    std::fs::create_dir_all(&drm).unwrap();
    std::os::unix::fs::symlink(function, drm.join("device")).unwrap();
    std::fs::create_dir_all(dri).unwrap();
    std::fs::write(dri.join(node), "").unwrap();
}

/// A PCI function at `address` from `vendor`, in a fake `/sys` in `sys`, beside the others on its
/// bus as the kernel puts them, and its folder.
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
    reason = "writing the function is the point; a caller may not need its folder"
)]
pub fn pci_function(sys: &Path, address: &str, vendor: &str) -> std::path::PathBuf {
    let (bus, _) = address.rsplit_once(':').unwrap();
    let function = sys.join("devices").join(bus).join(address);
    std::fs::create_dir_all(&function).unwrap();
    std::fs::write(function.join("vendor"), format!("{vendor}\n")).unwrap();
    function
}
