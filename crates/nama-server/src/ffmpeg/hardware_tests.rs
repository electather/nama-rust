use std::path::Path;

use super::{Gpu, TEST_HEVC_CLIP, find_gpus};
use crate::ffmpeg::fake;

/// The GPUs found in `root`, which holds a fake `/sys` and `/dev/dri`.
fn found(root: &Path) -> Vec<Gpu> {
    find_gpus(&root.join("dri"), &root.join("sys/class/drm"))
}

/// Writes a GPU into `root`'s fake `/sys` and `/dev/dri`.
fn gpu(root: &Path, node: &str, address: &str, vendor: &str) {
    fake::gpu(&root.join("sys"), &root.join("dri"), node, address, vendor);
}

fn ids_and_integrated(gpus: &[Gpu]) -> Vec<(&str, bool)> {
    gpus.iter()
        .map(|gpu| (gpu.id.as_str(), gpu.integrated))
        .collect()
}

#[test]
fn intel_at_00_02_0_is_integrated_and_elsewhere_dedicated() {
    let root = tempfile::tempdir().unwrap();
    gpu(root.path(), "renderD128", "0000:00:02.0", "0x8086");
    gpu(root.path(), "renderD129", "0000:03:00.0", "0x8086");

    let gpus = found(root.path());

    assert_eq!(
        ids_and_integrated(&gpus),
        [("0000:03:00.0", false), ("0000:00:02.0", true)]
    );
    assert!(gpus.iter().all(Gpu::is_intel));
}

#[test]
fn amd_with_an_amd_cpu_function_on_its_bus_is_integrated_and_without_one_dedicated() {
    let root = tempfile::tempdir().unwrap();
    // As on a Ryzen 7950X3D with an RX 9070 XT: the integrated GPU's node comes first.
    gpu(root.path(), "renderD128", "0000:18:00.0", "0x1002");
    fake::pci_function(&root.path().join("sys"), "0000:18:00.2", "0x1022");
    gpu(root.path(), "renderD129", "0000:03:00.0", "0x1002");
    // An AMD GPU's own functions, such as its audio, don't make it integrated: only a CPU's do.
    fake::pci_function(&root.path().join("sys"), "0000:03:00.1", "0x1002");

    let gpus = found(root.path());

    assert_eq!(
        ids_and_integrated(&gpus),
        [("0000:03:00.0", false), ("0000:18:00.0", true)]
    );
    assert!(!gpus.iter().any(Gpu::is_intel));
}

#[test]
fn two_dedicated_gpus_go_by_pci_address_lowest_first() {
    let root = tempfile::tempdir().unwrap();
    gpu(root.path(), "renderD128", "0000:0a:00.0", "0x1002");
    gpu(root.path(), "renderD129", "0000:03:00.0", "0x1002");

    let gpus = found(root.path());

    assert_eq!(
        ids_and_integrated(&gpus),
        [("0000:03:00.0", false), ("0000:0a:00.0", false)]
    );
}

#[test]
fn a_render_node_missing_from_dev_dri_isnt_found() {
    let root = tempfile::tempdir().unwrap();
    gpu(root.path(), "renderD128", "0000:03:00.0", "0x1002");
    gpu(root.path(), "renderD129", "0000:18:00.0", "0x1002");
    std::fs::remove_file(root.path().join("dri/renderD128")).unwrap();

    let gpus = found(root.path());

    assert_eq!(ids_and_integrated(&gpus), [("0000:18:00.0", false)]);
    assert_eq!(gpus[0].node, root.path().join("dri/renderD129"));
}

#[test]
fn a_gpu_in_a_pci_domain_past_ffff_is_found() {
    let root = tempfile::tempdir().unwrap();
    gpu(root.path(), "renderD128", "10000:e1:00.0", "0x8086");

    assert_eq!(
        ids_and_integrated(&found(root.path())),
        [("10000:e1:00.0", false)]
    );
}

#[test]
fn without_sys_no_gpu_is_found() {
    let root = tempfile::tempdir().unwrap();

    assert_eq!(found(root.path()), []);
}

/// What a Mesa driver logs opening `model` with VAAPI, among other lines.
fn mesa_log(model: &str) -> String {
    format!(
        "[AVHWDeviceContext @ 0x5581] Opened VA display via DRM: /dev/dri/renderD128.\n\
         [AVHWDeviceContext @ 0x5581] VAAPI driver: Mesa Gallium driver 26.2.6-arch3.1 for \
         {model} (radeonsi, gfx1201, ACO, DRM 3.64, 7.3.3-1-cachyos).\n\
         [AVHWDeviceContext @ 0x5581] Driver not found in known nonstandard list, using \
         standard behaviour.\n"
    )
}

/// The name of the one GPU at `address` from `vendor`, integrated if `integrated`, whose
/// driver logs `log` when ffmpeg opens it.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn name(address: &str, vendor: &str, integrated: bool, log: &str) -> String {
    let root = tempfile::tempdir().unwrap();
    gpu(root.path(), "renderD128", address, vendor);
    if integrated && vendor == super::AMD {
        let (slot, _) = address.rsplit_once('.').unwrap();
        fake::pci_function(&root.path().join("sys"), &format!("{slot}.2"), "0x1022");
    }
    let [gpu] = found(root.path()).try_into().unwrap();
    assert_eq!(gpu.integrated, integrated);
    gpu.with_model(log).name()
}

#[test]
fn a_dedicated_gpu_whose_mesa_driver_names_a_model_takes_the_model() {
    assert_eq!(
        name(
            "0000:03:00.0",
            "0x1002",
            false,
            &mesa_log("AMD Radeon RX 9070 XT")
        ),
        "AMD Radeon RX 9070 XT"
    );
}

#[test]
fn an_integrated_gpu_is_called_that_with_its_model_if_named() {
    let ryzen = mesa_log("AMD Ryzen 9 7950X3D 16-Core Processor");
    let intel = "VAAPI driver: Intel iHD driver for Intel(R) Gen Graphics - 24.1.0 ()";

    assert_eq!(
        name("0000:18:00.0", "0x1002", true, &ryzen),
        "integrated GPU (AMD Ryzen 9 7950X3D 16-Core Processor)"
    );
    assert_eq!(
        name("0000:00:02.0", "0x8086", true, intel),
        "Intel integrated GPU"
    );
}

#[test]
fn a_dedicated_gpu_without_a_model_is_its_maker_and_address() {
    let intel = "VAAPI driver: Intel iHD driver for Intel(R) Gen Graphics - 24.1.0 ()";

    assert_eq!(
        name("0000:03:00.0", "0x8086", false, intel),
        "Intel GPU at 0000:03:00.0"
    );
    assert_eq!(
        name("0000:04:00.0", "0x1a03", false, ""),
        "GPU at 0000:04:00.0"
    );
}

#[test]
fn the_hevc_test_clip_is_main_10_at_256_by_256() {
    // Its first NAL unit with type 33, a sequence parameter set: its start code, its two-byte
    // header, a byte of ids, then the byte with `general_profile_idc` in its low five bits.
    let sps = (TEST_HEVC_CLIP.windows(7))
        .find(|bytes| bytes[..3] == [0, 0, 1] && (bytes[3] >> 1) & 0x3f == 33)
        .expect("a sequence parameter set");
    assert_eq!(sps[6] & 0x1f, 2, "general_profile_idc");
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), TEST_HEVC_CLIP).unwrap();
    let probed = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=profile,width,height",
        ])
        .args(["-of", "csv=p=0"])
        .arg(file.path())
        .output()
        .expect("this test needs ffprobe on PATH");
    assert_eq!(
        String::from_utf8_lossy(&probed.stdout).trim(),
        "Main 10,256,256"
    );
}
