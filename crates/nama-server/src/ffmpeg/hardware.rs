//! The hardware acceleration APIs a Transcode can decode, scale, tone-map and encode its video
//! with. Which work depends on the GPU, its driver and how ffmpeg was built (ADR 0026), so the
//! Server test-encodes with each at startup, on each GPU it finds, and offers only those that pass.
//!
//! AMD's AMF and Windows' D3D11VA are left out: VAAPI covers AMD on Linux.

use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use tokio::io::AsyncWriteExt as _;

use super::{Ffmpeg, ToneMapper, VideoArgs, VideoConversion, VideoLimit, video_args};

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
    /// Whether it also passed decoding and encoding 10-bit HEVC, which a Transcode of a 4K or HDR
    /// Version makes on it.
    pub hevc: bool,
}

impl Hardware {
    /// Its API, and its GPU if named, in plain words, such as "VAAPI on the AMD Radeon RX 9070 XT".
    pub(super) fn name(&self) -> String {
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
    pub(super) fn is_intel(&self) -> bool {
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

/// Where a Transcode on hardware decodes its video.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Decode {
    /// On the GPU, keeping the frames there for its filters.
    OnGpu,
    /// On the GPU, taking the frames to memory for filters in software.
    ToMemory,
    /// In software, so the GPU only encodes.
    InSoftware,
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
    pub(super) const fn encoder(self) -> &'static str {
        match self {
            Self::Nvenc => "h264_nvenc",
            Self::Qsv => "h264_qsv",
            Self::Vaapi => "h264_vaapi",
            Self::VideoToolbox => "h264_videotoolbox",
        }
    }

    /// Its HEVC encoder, as `ffmpeg -encoders` lists it.
    pub(super) const fn hevc_encoder(self) -> &'static str {
        match self {
            Self::Nvenc => "hevc_nvenc",
            Self::Qsv => "hevc_qsv",
            Self::Vaapi => "hevc_vaapi",
            Self::VideoToolbox => "hevc_videotoolbox",
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
    pub(super) fn has_tone_map_filter(self) -> bool {
        self.filters()
            .is_some_and(|filters| filters.tone_map.is_some())
    }

    /// The arguments before `-i` that open `gpu`, or ffmpeg's default without one, and decode
    /// as `decode` says. A codec it can't decode is decoded in software instead.
    pub(super) fn input(self, gpu: Option<&Gpu>, decode: Decode) -> Vec<String> {
        let device = match self {
            Self::Nvenc => "cuda",
            Self::Qsv => "qsv",
            Self::Vaapi => "vaapi",
            Self::VideoToolbox if decode == Decode::InSoftware => return vec![],
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
        input.extend(args(&["-filter_hw_device", DEVICE]));
        if decode == Decode::InSoftware {
            return input;
        }
        input.extend(args(&["-hwaccel", device, "-hwaccel_device", DEVICE]));
        if let Some(filters) = self.filters().filter(|_| decode == Decode::OnGpu) {
            input.extend(args(&["-hwaccel_output_format", filters.frames]));
        }
        input
    }

    /// The filters that convert the video on the GPU: tone-mapping HDR to SDR if `tone_map` and
    /// it can, and scaling to `size` if given, ending in frames in `pixel_format`, such as `nv12`,
    /// on the GPU. `None` if its frames don't stay on the GPU.
    pub(super) fn gpu_filters(
        self,
        tone_map: bool,
        size: Option<(i64, i64)>,
        pixel_format: &str,
    ) -> Option<String> {
        let on_gpu = self.filters()?;
        // Frames decoded in software, from a codec the GPU doesn't decode, go up to it first;
        // frames already on it pass through.
        let mut filters = vec![on_gpu.upload.to_owned()];
        filters.extend(on_gpu.tone_map.filter(|_| tone_map).map(str::to_owned));
        let scale = on_gpu.scale;
        filters.push(match size {
            Some((width, height)) => format!("{scale}=w={width}:h={height}:format={pixel_format}"),
            None => format!("{scale}=format={pixel_format}"),
        });
        Some(filters.join(","))
    }

    /// The filter that takes frames converted in software to its encoder, in `pixel_format`, such
    /// as `nv12`.
    pub(super) fn upload(self, pixel_format: &str) -> String {
        match self {
            Self::Vaapi => format!("format={pixel_format},hwupload"),
            Self::Qsv => format!("format={pixel_format},hwupload=extra_hw_frames=64"),
            // Their encoders take frames from memory.
            Self::Nvenc | Self::VideoToolbox => format!("format={pixel_format}"),
        }
    }

    /// The arguments that encode with it, to 10-bit HEVC if `hevc` and else to H.264, making
    /// keyframes only where forced.
    pub(super) fn codec(self, hevc: bool) -> Vec<String> {
        let mut codec = if hevc {
            args(&["-c:v", self.hevc_encoder(), "-profile:v", "main10"])
        } else {
            args(&["-c:v", self.encoder()])
        };
        codec.extend(args(&["-g", NO_KEYFRAMES_OF_ITS_OWN]));
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

/// How long a test encode on hardware may take, opening the device included: a second or two
/// where it works, and the first plan waits for every test.
const HARDWARE_TEST_TIMEOUT: Duration = Duration::from_secs(10);

/// A second of video a test encode on hardware converts, at 720p as a Transcode might get it.
const TEST_CLIP: &str = "testsrc2=size=1280x720:rate=24:duration=1";

/// [`TEST_CLIP`] as 10-bit HDR10, for a test tone-map on hardware.
const TEST_HDR_CLIP: &str = "testsrc2=size=1280x720:rate=24:duration=1,format=yuv420p10le,\
                             setparams=color_primaries=bt2020:color_trc=smpte2084:colorspace=bt2020nc";

/// Six frames of 256×256 HEVC Main 10 (`general_profile_idc` 2), which a test of HEVC on hardware
/// decodes and encodes again. As Silo found, NVIDIA's Pascal decoder refuses anything under
/// 144×144, and a Main 4:4:4 (Rext, profile 4) clip fails every Intel GPU.
pub(super) const TEST_HEVC_CLIP: &[u8] = include_bytes!("test_main10.hevc");

impl Ffmpeg {
    /// The hardware acceleration APIs that ffmpeg lists an encoder for and that pass a test
    /// encode on each GPU they may run on, as a Transcode would run it, in the order Automatic
    /// prefers them, each with whether it passes a test tone-map and a test HEVC encode too, or why
    /// ffmpeg couldn't list its encoders. Takes a few seconds while each opens its GPU.
    pub(super) async fn find_hardware(&self) -> Result<Vec<Hardware>, String> {
        let encoders = self.listed("-encoders").await?;
        let lists = |encoder: &str| encoders.iter().any(|listed| listed == encoder);
        let mut found = vec![];
        for (accel, gpu) in self.pairs().await {
            if !lists(accel.encoder()) {
                continue;
            }
            let tested = self.test_hardware(accel, gpu, lists(accel.hevc_encoder()));
            found.extend(tested.await);
        }
        Ok(found)
    }

    /// Each API on each GPU it may run on, in the order Automatic prefers them: NVENC, each GPU's
    /// in [`Self::gpus`]' order, QSV before VAAPI, then `VideoToolbox`. NVIDIA's GPUs aren't told
    /// apart, so NVENC runs on its first.
    async fn pairs(&self) -> Vec<(Accel, Option<Gpu>)> {
        let mut pairs = vec![(Accel::Nvenc, None)];
        if self.gpus.is_empty() {
            pairs.extend([(Accel::Qsv, None), (Accel::Vaapi, None)]);
        }
        for gpu in &self.gpus {
            let gpu = self.named(gpu).await;
            if gpu.is_intel() {
                pairs.push((Accel::Qsv, Some(gpu.clone())));
            }
            pairs.push((Accel::Vaapi, Some(gpu)));
        }
        pairs.push((Accel::VideoToolbox, None));
        pairs
    }

    /// `gpu` with the model its driver names when ffmpeg opens it with VAAPI, if it does.
    #[tracing::instrument(name = "ffmpeg open GPU", skip_all)]
    async fn named(&self, gpu: &Gpu) -> Gpu {
        let Ok(ffmpeg) = &self.ffmpeg else {
            return gpu.clone();
        };
        let input = Accel::Vaapi.input(Some(gpu), Decode::ToMemory);
        // The test encode runs at `-v error`, which leaves out the line naming the driver.
        let opened = tokio::process::Command::new(&ffmpeg.path)
            .args(["-v", "verbose", "-nostdin"])
            .args(input)
            .args([
                "-f",
                "lavfi",
                "-i",
                "nullsrc",
                "-frames:v",
                "1",
                "-f",
                "null",
                "-",
            ])
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .output();
        match tokio::time::timeout(HARDWARE_TEST_TIMEOUT, opened).await {
            Ok(Ok(output)) => gpu
                .clone()
                .with_model(&String::from_utf8_lossy(&output.stderr)),
            _ => gpu.clone(),
        }
    }

    /// `accel` on `gpu`, if it passes a test encode, with whether it passes a test tone-map too,
    /// and a test of HEVC if ffmpeg `lists_hevc`, its encoder.
    async fn test_hardware(
        &self,
        accel: Accel,
        gpu: Option<Gpu>,
        lists_hevc: bool,
    ) -> Option<Hardware> {
        let mut hardware = Hardware {
            accel,
            gpu,
            tone_maps: false,
            hevc: false,
        };
        let scaled = VideoConversion {
            hardware: Some(hardware.clone()),
            tone_map: None,
            limit: Some(VideoLimit {
                size: Some((640, 360)),
                bits_per_second: 1_000_000,
            }),
            burn_in: None,
            subtitle_white: None,
            in_band_headers: true,
            hevc: false,
        };
        if let Err(err) = self.test_encode(TestClip::Lavfi(TEST_CLIP), &scaled).await {
            tracing::info!("{} didn't pass its test encode: {err}", hardware.name());
            return None;
        }
        let tone_mapped = VideoConversion {
            tone_map: Some(ToneMapper::Hardware),
            ..scaled.clone()
        };
        hardware.tone_maps = accel.has_tone_map_filter()
            && (self
                .test_encode(TestClip::Lavfi(TEST_HDR_CLIP), &tone_mapped)
                .await)
                .inspect_err(|err| {
                    tracing::info!("{} didn't pass its test tone-map: {err}", hardware.name());
                })
                .is_ok();
        let hevc = VideoConversion {
            limit: None,
            in_band_headers: false,
            hevc: true,
            ..scaled
        };
        hardware.hevc = lists_hevc
            && (self.test_encode(TestClip::Hevc, &hevc).await)
                .inspect_err(|err| {
                    tracing::info!("{} didn't pass its test of HEVC: {err}", hardware.name());
                })
                .is_ok();
        Some(hardware)
    }

    /// Converts `clip` as `video` says, and throws it away, or says why it failed.
    #[tracing::instrument(name = "ffmpeg test encode", skip_all)]
    async fn test_encode(&self, clip: TestClip, video: &VideoConversion) -> Result<(), String> {
        let ffmpeg = self.ffmpeg.as_ref().map_err(Clone::clone)?;
        let (format, source) = match clip {
            TestClip::Lavfi(source) => ("lavfi", source),
            TestClip::Hevc => ("hevc", "pipe:0"),
        };
        // The clip's path matters only to a subtitle burned in, which a test has none of.
        let VideoArgs {
            input,
            video,
            codec,
        } = video_args(source, video, 0);
        let mut child = tokio::process::Command::new(&ffmpeg.path)
            .args(["-v", "error", "-nostdin"])
            .args(input)
            .args(["-f", format, "-i", source])
            .args(video)
            .args(codec)
            .args(["-force_key_frames", "0", "-f", "null", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|err| err.to_string())?;
        let stdin = child.stdin.take();
        let output = async move {
            if let (TestClip::Hevc, Some(mut stdin)) = (clip, stdin) {
                // An ffmpeg that fails before reading it all closes the pipe; its exit says why.
                let _: io::Result<()> = stdin.write_all(TEST_HEVC_CLIP).await;
            }
            child.wait_with_output().await
        };
        match tokio::time::timeout(HARDWARE_TEST_TIMEOUT, output).await {
            Ok(Ok(output)) if output.status.success() => Ok(()),
            Ok(Ok(output)) => Err(format!(
                "ffmpeg exited with {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )),
            Ok(Err(err)) => Err(err.to_string()),
            Err(_) => Err(format!("it took over {HARDWARE_TEST_TIMEOUT:?}")),
        }
    }
}

/// What a test encode on hardware converts.
#[derive(Clone, Copy)]
enum TestClip {
    /// A lavfi source, such as [`TEST_CLIP`].
    Lavfi(&'static str),
    /// [`TEST_HEVC_CLIP`], piped in.
    Hevc,
}

fn args(args: &[&str]) -> Vec<String> {
    args.iter().map(|&arg| arg.to_owned()).collect()
}

#[cfg(all(test, unix))]
#[path = "hardware_tests.rs"]
mod tests;
