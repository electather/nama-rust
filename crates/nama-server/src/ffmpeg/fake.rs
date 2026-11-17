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

/// VAAPI on a dedicated AMD Radeon RX 9070 XT, then on an integrated AMD GPU, as Automatic
/// orders them, with the integrated GPU and the dedicated one on the render nodes `nodes`, in a
/// fake `/sys` and `/dev/dri` in `root`.
///
/// # Panics
///
/// If writing them fails.
#[cfg(unix)]
#[must_use]
pub fn vaapi_on_two_gpus_at(root: &Path, nodes: [&str; 2]) -> Vec<super::Hardware> {
    use super::{Accel, Hardware, find_gpus};

    let (sys, dri) = (root.join("sys"), root.join("dri"));
    let [integrated, dedicated] = nodes;
    gpu(&sys, &dri, integrated, "0000:18:00.0", "0x1002");
    pci_function(&sys, "0000:18:00.2", "0x1022");
    gpu(&sys, &dri, dedicated, "0000:03:00.0", "0x1002");
    let mesa = "VAAPI driver: Mesa Gallium driver 26.2.6 for AMD Radeon RX 9070 XT (radeonsi)";
    find_gpus(&dri, &sys.join("class/drm"))
        .into_iter()
        .map(|gpu| Hardware {
            accel: Accel::Vaapi,
            gpu: Some(if gpu.id() == "0000:03:00.0" {
                gpu.with_model(mesa)
            } else {
                gpu
            }),
            tone_maps: false,
        })
        .collect()
}
