//! The hardware acceleration APIs a Transcode can decode, scale, tone-map and encode its video
//! with. Which work depends on the GPU, its driver and how ffmpeg was built (ADR 0026), so the
//! Server test-encodes with each at startup and offers only those that pass.
//!
//! AMD's AMF and Windows' D3D11VA are left out: VAAPI covers AMD on Linux.

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

/// An API that passed its test encode at startup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hardware {
    /// Which API it is.
    pub accel: Accel,
    /// Whether it also passed tone-mapping HDR to SDR on the GPU.
    pub tone_maps: bool,
}

/// What ffmpeg names the device it opens for a Transcode, so its filters can use it.
const DEVICE: &str = "nama";

/// Keyframes come only where the Transcode forces them, so each cut starts a file and nothing
/// else does: a GOP longer than any Version, about 4 days at 30 frames a second.
const NO_KEYFRAMES_OF_ITS_OWN: &str = "10000000";

impl Accel {
    /// Every API, in the order Automatic prefers them: a dedicated encoder first.
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
    const fn gpu(self) -> Option<Gpu> {
        match self {
            Self::Nvenc => Some(Gpu {
                frames: "cuda",
                upload: "format=nv12|p010le|cuda,hwupload",
                scale: "scale_cuda",
                // ffmpeg's would need `OpenCL`.
                tone_map: None,
            }),
            Self::Qsv => Some(Gpu {
                frames: "qsv",
                upload: "format=nv12|p010le|qsv,hwupload=extra_hw_frames=64",
                scale: "scale_qsv",
                tone_map: Some("vpp_qsv=tonemap=1:format=nv12"),
            }),
            Self::Vaapi => Some(Gpu {
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
        self.gpu().is_some_and(|gpu| gpu.tone_map.is_some())
    }

    /// The arguments before `-i` that open the device and decode on it, keeping the frames on it
    /// if `on_gpu`. A codec it can't decode is decoded in software instead.
    pub(crate) fn input(self, on_gpu: bool) -> Vec<String> {
        let device = match self {
            Self::Nvenc => "cuda",
            Self::Qsv => "qsv",
            Self::Vaapi => "vaapi",
            Self::VideoToolbox => return args(&["-hwaccel", "videotoolbox"]),
        };
        let mut input = args(&[
            "-init_hw_device",
            &format!("{device}={DEVICE}"),
            "-filter_hw_device",
            DEVICE,
            "-hwaccel",
            device,
            "-hwaccel_device",
            DEVICE,
        ]);
        if let Some(gpu) = self.gpu().filter(|_| on_gpu) {
            input.extend(args(&["-hwaccel_output_format", gpu.frames]));
        }
        input
    }

    /// The filters that convert the video on the GPU: tone-mapping HDR to SDR if `tone_map` and
    /// it can, and scaling to `size` if given, ending in 8-bit frames on the GPU. `None` if its
    /// frames don't stay on the GPU.
    pub(crate) fn gpu_filters(self, tone_map: bool, size: Option<(i64, i64)>) -> Option<String> {
        let gpu = self.gpu()?;
        // Frames decoded in software, from a codec the GPU doesn't decode, go up to it first;
        // frames already on it pass through.
        let mut filters = vec![gpu.upload.to_owned()];
        filters.extend(gpu.tone_map.filter(|_| tone_map).map(str::to_owned));
        let scale = gpu.scale;
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
struct Gpu {
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
