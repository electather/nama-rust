//! The hardware acceleration APIs a Transcode can decode, scale, tone-map and encode its video
//! with. Which work depends on the GPU, its driver and how ffmpeg was built (ADR 0026), so the
//! Server test-encodes with each at startup, on each GPU it finds, and offers only those that pass.
//!
//! AMD's AMF and Windows' D3D11VA are left out: VAAPI covers AMD on Linux.

use std::{
    fs,
    path::{Path, PathBuf},
};

/// A hardware acceleration API ffmpeg may offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accel {
    /// NVIDIA's NVENC and NVDEC, through CUDA.
    Nvenc,
    /// Intel's Quick Sync Video.
    Qsv,
    /// VAAPI, for Intel and AMD on Linux.
    Vaapi,
    /// Apple's `VideoToolbox`.
    VideoToolbox,
}

/// An API on a GPU that passed its test encode at startup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hardware {
    /// Which API it is.
    pub accel: Accel,
    /// The GPU it runs on, or `None` where ffmpeg picks one: NVENC's first CUDA GPU, `VideoToolbox`,
    /// and VAAPI and QSV when the Server found no GPU.
    pub gpu: Option<Gpu>,
    /// Whether it also passed tone-mapping HDR to SDR on the GPU.
    pub tone_maps: bool,
}

impl Hardware {
    /// Its API, and its GPU if named, in plain words, such as "VAAPI on the AMD Radeon RX 9070 XT".
    pub(crate) fn name(&self) -> String {
        match &self.gpu {
            Some(gpu) => format!("{} on the {}", self.accel.name(), gpu.name()),
            None => self.accel.name().to_owned(),
        }
    }

    /// Whether it is `accel` on the GPU at the PCI address `gpu_id`.
    pub(crate) fn is_pair(&self, accel: Accel, gpu_id: &str) -> bool {
        self.accel == accel && (self.gpu.as_ref()).is_some_and(|gpu| gpu.id() == gpu_id)
    }
}

/// A GPU the Server found: a render node backed by a PCI function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gpu {
    /// Its PCI address: see [`Self::id`].
    id: String,
    /// Its render node, such as `/dev/dri/renderD128`.
    node: PathBuf,
    /// Its PCI vendor, such as `0x1002`.
    vendor: String,
    integrated: bool,
    /// The model its driver names, if it does.
    model: Option<String>,
}

const AMD: &str = "0x1002";
const INTEL: &str = "0x8086";
const NVIDIA: &str = "0x10de";
/// AMD the CPU maker, whose functions share a bus with its integrated GPU.
const AMD_CPU: &str = "0x1022";

/// The GPUs whose render nodes are in `dri` (`/dev/dri`), as `drm` (`/sys/class/drm`) describes
/// them, in the order Automatic prefers: dedicated before integrated, then by PCI address. None
/// where `drm` can't be read, as off Linux or in a container that hides `/sys`.
#[must_use]
pub fn find_gpus(dri: &Path, drm: &Path) -> Vec<Gpu> {
    let Ok(nodes) = fs::read_dir(drm) else {
        return vec![];
    };
    let mut gpus: Vec<Gpu> = (nodes.flatten())
        .map(|node| node.file_name())
        .filter(|node| node.to_string_lossy().starts_with("renderD"))
        // A container sees every GPU in `/sys` but may use only the nodes passed to it.
        .filter(|node| dri.join(node).exists())
        .filter_map(|node| Gpu::at(dri.join(&node), &drm.join(&node).join("device")))
        .collect();
    gpus.sort_by(|a, b| (a.integrated, &a.id).cmp(&(b.integrated, &b.id)));
    gpus
}

impl Gpu {
    /// Its PCI address, such as `0000:03:00.0`, which names it across boots: its render node's
    /// number can differ from one boot to the next.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The GPU with the render node `node`, whose `device` link in `/sys/class/drm` is `link`, if
    /// that resolves to a PCI function.
    fn at(node: PathBuf, link: &Path) -> Option<Self> {
        let function = fs::canonicalize(link).ok()?;
        let id = function.file_name()?.to_str()?.to_owned();
        let vendor = pci_vendor(&function)?;
        // A domain past `ffff`, as behind Intel's VMD, takes more digits.
        let is_pci_address = id.split_once(':').is_some_and(|(domain, slot)| {
            domain.len() >= 4
                && domain.bytes().all(|byte| byte.is_ascii_hexdigit())
                && (slot.bytes())
                    .map(|byte| if byte.is_ascii_hexdigit() { b'x' } else { byte })
                    .eq(*b"xx:xx.x")
        });
        is_pci_address.then(|| Self {
            integrated: is_integrated(&function, &id, &vendor),
            id,
            node,
            vendor,
            model: None,
        })
    }

    /// Whether it is Intel's, which QSV runs on too.
    pub(crate) fn is_intel(&self) -> bool {
        self.vendor == INTEL
    }

    /// It with the model its driver names in `log`, what ffmpeg logs at `-v verbose` opening it
    /// with VAAPI, if a Mesa driver names one.
    pub(crate) fn with_model(self, log: &str) -> Self {
        Self {
            model: mesa_model(log),
            ..self
        }
    }

    /// Its name in plain words, such as "AMD Radeon RX 9070 XT", "integrated GPU (AMD Ryzen 9
    /// 7950X3D 16-Core Processor)" or "Intel GPU at 0000:03:00.0".
    #[must_use]
    pub fn name(&self) -> String {
        let maker = match self.vendor.as_str() {
            AMD => "AMD ",
            INTEL => "Intel ",
            NVIDIA => "NVIDIA ",
            _ => "",
        };
        match (&self.model, self.integrated) {
            (Some(model), false) => model.clone(),
            (Some(model), true) => format!("integrated GPU ({model})"),
            (None, true) => format!("{maker}integrated GPU"),
            (None, false) => format!("{maker}GPU at {}", self.id),
        }
    }
}

/// The PCI vendor of the function in the folder `function`, such as `0x1002`.
fn pci_vendor(function: &Path) -> Option<String> {
    Some(
        fs::read_to_string(function.join("vendor"))
            .ok()?
            .trim()
            .to_owned(),
    )
}

/// Whether the GPU at the PCI address `id` from `vendor`, in the folder `function`, is built into
/// the CPU.
// Not `boot_vga`: it marks the GPU driving the display, which is the dedicated one on a desktop but
// the integrated one on a laptop or a headless box. Nor a benchmark, which would slow every startup
// and vary with whatever else the GPUs are doing.
fn is_integrated(function: &Path, id: &str, vendor: &str) -> bool {
    match vendor {
        // Intel's integrated GPU is always at this address.
        INTEL => id == "0000:00:02.0",
        // An AMD CPU's integrated GPU shares its bus with the CPU's own functions.
        AMD => {
            let Some((bus, _)) = id.rsplit_once(':') else {
                return false;
            };
            let Some(Ok(siblings)) = function.parent().map(fs::read_dir) else {
                return false;
            };
            siblings.flatten().any(|sibling| {
                sibling
                    .file_name()
                    .to_string_lossy()
                    .starts_with(&format!("{bus}:"))
                    && pci_vendor(&sibling.path()).as_deref() == Some(AMD_CPU)
            })
        }
        _ => false,
    }
}

/// The model a Mesa driver names in `log`, ffmpeg's at `-v verbose` opening a GPU with VAAPI,
/// which has a line such as `VAAPI driver: Mesa Gallium driver 26.2.6-arch3.1 for AMD Radeon RX
/// 9070 XT (radeonsi, gfx1201, ACO, DRM 3.64, 7.3.3-1-cachyos).`
// Intel's driver names no model, only "Intel iHD driver for Intel(R) Gen Graphics".
fn mesa_model(log: &str) -> Option<String> {
    let driver = log
        .lines()
        .find_map(|line| line.split_once("VAAPI driver: Mesa "))?
        .1;
    let (_, model) = driver.split_once(" for ")?;
    Some(model.rsplit_once(" (")?.0.to_owned())
}

/// What ffmpeg names the device it opens for a Transcode, so its filters can use it.
const DEVICE: &str = "nama";

/// Keyframes come only where the Transcode forces them, so each cut starts a file and nothing
/// else does: a GOP longer than any Version, about 4 days at 30 frames a second.
const NO_KEYFRAMES_OF_ITS_OWN: &str = "10000000";

impl Accel {
    /// Every API.
    pub const ALL: [Self; 4] = [Self::Nvenc, Self::Qsv, Self::Vaapi, Self::VideoToolbox];

    /// Its name in plain words, such as "Intel QSV".
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Nvenc => "NVIDIA NVENC",
            Self::Qsv => "Intel QSV",
            Self::Vaapi => "VAAPI",
            Self::VideoToolbox => "Apple VideoToolbox",
        }
    }

    /// Its H.264 encoder, as `ffmpeg -encoders` lists it.
    pub(crate) const fn encoder(self) -> &'static str {
        match self {
            Self::Nvenc => "h264_nvenc",
            Self::Qsv => "h264_qsv",
            Self::Vaapi => "h264_vaapi",
            Self::VideoToolbox => "h264_videotoolbox",
        }
    }

    /// The filters that keep its frames on the GPU between decoding and encoding, or `None` for
    /// `VideoToolbox`, whose scaler needs ffmpeg 6.1, so its frames are scaled in software.
    const fn filters(self) -> Option<Filters> {
        match self {
            Self::Nvenc => Some(Filters {
                frames: "cuda",
                upload: "format=nv12|p010le|cuda,hwupload",
                scale: "scale_cuda",
                // ffmpeg's would need `OpenCL`.
                tone_map: None,
            }),
            Self::Qsv => Some(Filters {
                frames: "qsv",
                upload: "format=nv12|p010le|qsv,hwupload=extra_hw_frames=64",
                scale: "scale_qsv",
                tone_map: Some("vpp_qsv=tonemap=1:format=nv12"),
            }),
            Self::Vaapi => Some(Filters {
                frames: "vaapi",
                upload: "format=nv12|p010le|vaapi,hwupload",
                scale: "scale_vaapi",
                // Intel's driver has it; AMD's says "doesn't support HDR" and fails its test.
                tone_map: Some("tonemap_vaapi=format=nv12:t=bt709:m=bt709:p=bt709"),
            }),
            Self::VideoToolbox => None,
        }
    }

    /// Whether ffmpeg has a filter that tone-maps on its GPU, which its test then tries.
    pub(crate) fn has_tone_map_filter(self) -> bool {
        self.filters()
            .is_some_and(|filters| filters.tone_map.is_some())
    }

    /// The arguments before `-i` that open `gpu`, or ffmpeg's default without one, and decode on
    /// it, keeping the frames on it if `on_gpu`. A codec it can't decode is decoded in software
    /// instead.
    pub(crate) fn input(self, gpu: Option<&Gpu>, on_gpu: bool) -> Vec<String> {
        let device = match self {
            Self::Nvenc => "cuda",
            Self::Qsv => "qsv",
            Self::Vaapi => "vaapi",
            Self::VideoToolbox => return args(&["-hwaccel", "videotoolbox"]),
        };
        let mut input = match gpu.map(|gpu| gpu.node.display()) {
            None => args(&["-init_hw_device", &format!("{device}={DEVICE}")]),
            // ffmpeg's documented way to put QSV on one GPU: through VAAPI on its node.
            Some(node) if self == Self::Qsv => args(&[
                "-init_hw_device",
                &format!("vaapi=va:{node}"),
                "-init_hw_device",
                &format!("qsv={DEVICE}@va"),
            ]),
            Some(node) => args(&["-init_hw_device", &format!("{device}={DEVICE}:{node}")]),
        };
        input.extend(args(&[
            "-filter_hw_device",
            DEVICE,
            "-hwaccel",
            device,
            "-hwaccel_device",
            DEVICE,
        ]));
        if let Some(filters) = self.filters().filter(|_| on_gpu) {
            input.extend(args(&["-hwaccel_output_format", filters.frames]));
        }
        input
    }

    /// The filters that convert the video on the GPU: tone-mapping HDR to SDR if `tone_map` and
    /// it can, and scaling to `size` if given, ending in 8-bit frames on the GPU. `None` if its
    /// frames don't stay on the GPU.
    pub(crate) fn gpu_filters(self, tone_map: bool, size: Option<(i64, i64)>) -> Option<String> {
        let on_gpu = self.filters()?;
        // Frames decoded in software, from a codec the GPU doesn't decode, go up to it first;
        // frames already on it pass through.
        let mut filters = vec![on_gpu.upload.to_owned()];
        filters.extend(on_gpu.tone_map.filter(|_| tone_map).map(str::to_owned));
        let scale = on_gpu.scale;
        filters.push(match size {
            Some((width, height)) => format!("{scale}=w={width}:h={height}:format=nv12"),
            None => format!("{scale}=format=nv12"),
        });
        Some(filters.join(","))
    }

    /// The filter that takes 8-bit frames converted in software to its encoder.
    pub(crate) const fn upload(self) -> &'static str {
        match self {
            Self::Vaapi => "format=nv12,hwupload",
            Self::Qsv => "format=nv12,hwupload=extra_hw_frames=64",
            // Their encoders take frames from memory.
            Self::Nvenc | Self::VideoToolbox => "format=nv12",
        }
    }

    /// The arguments that encode with it, making keyframes only where forced.
    pub(crate) fn codec(self) -> Vec<String> {
        let mut codec = args(&["-c:v", self.encoder(), "-g", NO_KEYFRAMES_OF_ITS_OWN]);
        match self {
            Self::Nvenc => codec.extend(args(&["-forced-idr", "1", "-no-scenecut", "1"])),
            Self::Qsv => codec.extend(args(&["-forced_idr", "1"])),
            Self::Vaapi | Self::VideoToolbox => {}
        }
        codec
    }
}

/// The filters that keep an API's frames on its GPU.
struct Filters {
    /// What `-hwaccel_output_format` calls frames on it.
    frames: &'static str,
    /// Takes frames to it, passing through those already on it.
    upload: &'static str,
    /// Its scaler, which also converts to 8 bits.
    scale: &'static str,
    /// Its filter that tone-maps HDR to SDR, if ffmpeg has one.
    tone_map: Option<&'static str>,
}

fn args(args: &[&str]) -> Vec<String> {
    args.iter().map(|&arg| arg.to_owned()).collect()
}

#[cfg(all(test, unix))]
mod tests {
    use std::path::Path;

    use super::{Gpu, find_gpus};
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
}
