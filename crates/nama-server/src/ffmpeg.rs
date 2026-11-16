//! Finding the ffmpeg and ffprobe the Server Remuxes, Transcodes and probes Versions with, which
//! come from the system, not the release (ADR 0026).

#[cfg(any(test, feature = "testing"))]
pub mod fake;
mod hardware;

use std::{
    env,
    ffi::OsStr,
    io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
    time::Duration,
};

use tokio_util::{sync::CancellationToken, task::TaskTracker};

pub use self::hardware::{Accel, Gpu, Hardware, find_gpus};
use crate::config::Config;

/// The oldest ffmpeg and ffprobe the Server uses, as major and minor version. HLS with fMP4
/// segments, the zscale and tonemap filters and the VAAPI, QSV and NVENC encoders that Remuxing
/// and Transcoding use all predate 4.4, and it's the oldest a supported LTS distribution ships
/// (Ubuntu 22.04), so nothing older gets tested.
const MINIMUM: (u32, u32) = (4, 4);

/// A keyframe of a Version's video that a Remux starts at.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Keyframe {
    /// Its number, from 0.
    pub(crate) n: usize,
    /// When it falls, in milliseconds from the file's start.
    pub(crate) ms: i64,
    /// When the keyframe before it falls, or `None` for the first.
    pub(crate) previous_ms: Option<i64>,
}

/// How far before its first keyframe a Remux keeps packets at most, in milliseconds.
const REMUX_BACK_AT_MOST_MS: i64 = 500;

/// How much later than in its file a Remux times everything, in milliseconds: more than
/// [`REMUX_BACK_AT_MOST_MS`], since ffmpeg shifts the times of a run that would start before 0,
/// which would put it out of step with runs started later.
const REMUX_LATER_BY_MS: i64 = 1000;

/// How much later than in its file a Transcode times everything, in milliseconds, so the
/// encoder's delay never times a frame before 0, which ffmpeg would shift, putting the run started
/// at the file's start out of step with those started later.
const TRANSCODE_LATER_BY_MS: i64 = 1000;

/// How long ffmpeg may take to list its filters or encoders.
const LIST_TIMEOUT: Duration = Duration::from_secs(10);

/// How long a test encode on hardware may take, opening the device included: a second or two
/// where it works, and the first plan waits for every test.
const HARDWARE_TEST_TIMEOUT: Duration = Duration::from_secs(10);

/// A second of video a test encode on hardware converts, at 720p as a Transcode might get it.
const TEST_CLIP: &str = "testsrc2=size=1280x720:rate=24:duration=1";

/// [`TEST_CLIP`] as 10-bit HDR10, for a test tone-map on hardware.
const TEST_HDR_CLIP: &str = "testsrc2=size=1280x720:rate=24:duration=1,format=yuv420p10le,\
                             setparams=color_primaries=bt2020:color_trc=smpte2084:colorspace=bt2020nc";

/// The software filters a Transcode tone-maps HDR to SDR with: to linear light, to BT.709's
/// primaries, Hable's curve from HDR's brightness to SDR's 100 nits, then back to BT.709 video.
// HDR10 and HLG are always BT.2020, but probing calls a file HDR by its transfer alone, and
// zscale fails the whole Transcode with "no path between colorspaces" on untagged primaries.
const TONE_MAP: &str = "setparams=color_primaries=bt2020:colorspace=bt2020nc,\
                        zscale=t=linear:npl=100,format=gbrpf32le,zscale=p=bt709,\
                        tonemap=tonemap=hable:desat=0,zscale=t=bt709:m=bt709:r=tv,format=yuv420p";

/// The name of the init segment a Remux or Transcode started at file `n` writes beside its files. Every
/// run's is the same, but each writes its own, since ffmpeg writes it from empty as a run goes.
pub(crate) fn hls_init(n: usize) -> String {
    format!("init-{n}.mp4")
}

/// The file the run whose init segment [`hls_init`] named `name` started at, if it's one.
pub(crate) fn hls_init_from(name: &str) -> Option<usize> {
    name.strip_prefix("init-")?
        .strip_suffix(".mp4")?
        .parse()
        .ok()
}

/// How long ffmpeg may take to read one subtitle out of a file. Matroska interleaves it with the
/// video, so the whole file is read. Raised rather than grown with the file's size: an hour reads
/// 60 GB at 17 MB/s, slower than a spinning disk or a NAS on gigabit, so only a stuck read
/// outlasts it.
const SUBTITLE_TIMEOUT: Duration = Duration::from_hours(1);

/// Subtitle codecs, as ffprobe names them, that are text the Server serves as WebVTT. Others that
/// aren't images either, such as teletext, need a library ffmpeg may lack, or are unknown.
pub(crate) const TEXT_SUBTITLES: &[&str] = &["subrip", "mov_text", "webvtt", "ass", "ssa"];

/// Why reading a subtitle, or a font for one, out of a file failed.
#[derive(Debug, thiserror::Error)]
pub(crate) enum SubtitleError {
    #[error("the Server has no ffmpeg")]
    Unavailable,
    #[error("ffmpeg couldn't be run: {0}")]
    Run(#[from] io::Error),
    #[error("ffmpeg took longer than {}s", SUBTITLE_TIMEOUT.as_secs())]
    TimedOut(#[from] tokio::time::error::Elapsed),
    #[error("ffmpeg failed: {0}")]
    Failed(String),
}

/// A program the Server found and ran.
#[derive(Debug, PartialEq, Eq)]
pub struct FoundProgram {
    /// Where it is.
    pub path: PathBuf,
    /// Its version as it reports it, such as `7.1.1`.
    pub version: String,
}

/// The ffmpeg and ffprobe the Server found at startup, and the GPUs ffmpeg may Transcode on.
/// Without both programs it only Direct Plays.
#[derive(Debug)]
pub struct Ffmpeg {
    /// ffmpeg, or why the Server can't use one, in plain words.
    pub ffmpeg: Result<FoundProgram, String>,
    /// ffprobe, or why the Server can't use one, in plain words.
    pub ffprobe: Result<FoundProgram, String>,
    /// The GPUs VAAPI and QSV are tested on, as [`find_gpus`] finds them. Without any, they are
    /// tested on ffmpeg's default.
    pub gpus: Vec<Gpu>,
}

/// Neither found on `PATH`, and no GPU.
impl Default for Ffmpeg {
    fn default() -> Self {
        Self {
            ffmpeg: Err(not_on_path("ffmpeg")),
            ffprobe: Err(not_on_path("ffprobe")),
            gpus: vec![],
        }
    }
}

impl Ffmpeg {
    /// Runs ffmpeg and ffprobe at the paths `config` sets, or else the first of each in the
    /// directories of `path`, a `PATH` value, to read their versions, and finds the GPUs with
    /// [`find_gpus`]. Blocks while they run.
    #[must_use]
    pub fn find(config: &Config, path: &OsStr) -> Self {
        Self {
            ffmpeg: find("ffmpeg", config.ffmpeg.as_deref(), path),
            ffprobe: find("ffprobe", config.ffprobe.as_deref(), path),
            gpus: find_gpus(Path::new("/dev/dri"), Path::new("/sys/class/drm")),
        }
    }

    /// Why the Server can only Direct Play, in plain words, or `None` when it can use both.
    #[must_use]
    pub fn direct_play_only_because(&self) -> Option<String> {
        let reasons: Vec<&str> = [&self.ffmpeg, &self.ffprobe]
            .into_iter()
            .filter_map(|program| program.as_ref().err())
            .map(String::as_str)
            .collect();
        (!reasons.is_empty()).then(|| reasons.join(" and "))
    }

    /// Whether the Server can Remux, which needs both: ffprobe to list keyframes, ffmpeg to copy.
    pub(crate) const fn can_remux(&self) -> bool {
        self.ffmpeg.is_ok() && self.ffprobe.is_ok()
    }

    /// Whether the Server can probe files, which needs only ffprobe.
    pub(crate) const fn can_probe(&self) -> bool {
        self.ffprobe.is_ok()
    }

    /// Whether ffmpeg has the zscale filter, which [`Self::transcode`] tone-maps with in software
    /// and a system ffmpeg may lack (ADR 0026), or why ffmpeg couldn't say, such as a timeout on a
    /// busy host.
    async fn has_zscale(&self) -> Result<bool, String> {
        // Each filter is a line such as ` .SC zscale  V->V  Apply resizing, ...`.
        Ok(self
            .listed("-filters")
            .await?
            .contains(&"zscale".to_owned()))
    }

    /// Whether ffmpeg has the libx264 encoder, which [`Self::transcode`] encodes with in software
    /// and a system ffmpeg may lack (ADR 0026), or why ffmpeg couldn't say.
    async fn has_libx264(&self) -> Result<bool, String> {
        Ok(self
            .listed("-encoders")
            .await?
            .contains(&"libx264".to_owned()))
    }

    /// The names `ffmpeg -hide_banner <list>` lists, such as its filters for `-filters`, or why it
    /// couldn't say. Empty without ffmpeg.
    async fn listed(&self, list: &str) -> Result<Vec<String>, String> {
        let Ok(ffmpeg) = &self.ffmpeg else {
            return Ok(vec![]);
        };
        let output = tokio::process::Command::new(&ffmpeg.path)
            .args(["-hide_banner", list])
            .kill_on_drop(true)
            .output();
        let output = match tokio::time::timeout(LIST_TIMEOUT, output).await {
            Ok(Ok(output)) if output.status.success() => output,
            Ok(Ok(output)) => return Err(format!("it exited with {}", output.status)),
            Ok(Err(err)) => return Err(err.to_string()),
            Err(_) => return Err(format!("it took over {LIST_TIMEOUT:?}")),
        };
        // Each is a line of flags, its name and what it is.
        Ok((String::from_utf8_lossy(&output.stdout).lines())
            .filter_map(|line| line.split_whitespace().nth(1))
            .map(str::to_owned)
            .collect())
    }

    /// The hardware acceleration APIs that ffmpeg lists an encoder for and that pass a test
    /// encode on each GPU they may run on, as a Transcode would run it, in the order Automatic
    /// prefers them, each with whether it passes a test tone-map too, or why ffmpeg couldn't list
    /// its encoders. Takes a few seconds while each opens its GPU.
    async fn find_hardware(&self) -> Result<Vec<Hardware>, String> {
        let encoders = self.listed("-encoders").await?;
        let mut found = vec![];
        for (accel, gpu) in self.pairs().await {
            if !encoders.iter().any(|encoder| encoder == accel.encoder()) {
                continue;
            }
            found.extend(self.test_hardware(accel, gpu).await);
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
    async fn named(&self, gpu: &Gpu) -> Gpu {
        let Ok(ffmpeg) = &self.ffmpeg else {
            return gpu.clone();
        };
        let input = Accel::Vaapi.input(Some(gpu), false);
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

    /// `accel` on `gpu`, if it passes a test encode, with whether it passes a test tone-map too.
    async fn test_hardware(&self, accel: Accel, gpu: Option<Gpu>) -> Option<Hardware> {
        let mut hardware = Hardware {
            accel,
            gpu,
            tone_maps: false,
        };
        let scaled = VideoConversion {
            hardware: Some(hardware.clone()),
            tone_map: None,
            limit: Some(VideoLimit {
                size: Some((640, 360)),
                bits_per_second: 1_000_000,
            }),
            burn_in: None,
            in_band_headers: true,
        };
        if let Err(err) = self.test_encode(TEST_CLIP, &scaled).await {
            tracing::info!("{} didn't pass its test encode: {err}", hardware.name());
            return None;
        }
        let tone_mapped = VideoConversion {
            tone_map: Some(ToneMapper::Hardware),
            ..scaled
        };
        hardware.tone_maps = accel.has_tone_map_filter()
            && (self.test_encode(TEST_HDR_CLIP, &tone_mapped).await)
                .inspect_err(|err| {
                    tracing::info!("{} didn't pass its test tone-map: {err}", hardware.name());
                })
                .is_ok();
        Some(hardware)
    }

    /// Converts `clip`, a lavfi source, as `video` says, and throws it away, or says why it failed.
    async fn test_encode(&self, clip: &str, video: &VideoConversion) -> Result<(), String> {
        let ffmpeg = self.ffmpeg.as_ref().map_err(Clone::clone)?;
        // The clip's path matters only to a subtitle burned in, which a test has none of.
        let VideoArgs {
            input,
            video,
            codec,
        } = video_args(clip, video);
        let output = tokio::process::Command::new(&ffmpeg.path)
            .args(["-v", "error", "-nostdin"])
            .args(input)
            .args(["-f", "lavfi", "-i", clip])
            .args(video)
            .args(codec)
            .args(["-force_key_frames", "0", "-f", "null", "-"])
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .output();
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

    /// The run of ffmpeg that copies the first video of the file at `path`, and `remux`'s audio
    /// Track if any, untouched into fMP4 in `dir`, but for converting that audio to AAC as `remux`
    /// says, from the keyframe `from` to the end. It writes a file per keyframe, named by its
    /// number such as `3.m4s`, and an init segment named by [`hls_init`] that's whole once
    /// `from`'s file is. A run started at any keyframe writes the same files, so one can pick up
    /// where another stopped. Each file appears only once it's whole. Fails without ffmpeg.
    pub(crate) fn remux(
        &self,
        path: &str,
        remux: &Remux,
        dir: &Path,
        from: Keyframe,
    ) -> io::Result<HlsRun> {
        let ffmpeg = (self.ffmpeg.as_ref()).map_err(|why| io::Error::other(why.clone()))?;
        let mut command = tokio::process::Command::new(&ffmpeg.path);
        command.args(["-v", "error", "-nostdin"]);
        // The output `-ss` below drops packets by when they decode, and with B-frames a keyframe
        // decodes a few frames before it shows, so it starts back towards the keyframe before.
        let back_ms = from
            .previous_ms
            .map_or(REMUX_BACK_AT_MOST_MS, |previous| (from.ms - previous) / 2);
        let start_ms = from.ms - back_ms.min(REMUX_BACK_AT_MOST_MS);
        if start_ms > 0 {
            // Jumps to a keyframe at or before it, which in Matroska can be a few before.
            command.args(["-ss", &seconds(start_ms)]);
        }
        command.args(["-itsoffset", &seconds(REMUX_LATER_BY_MS)]);
        command.arg("-i").arg(path);
        // Times as in the file, from its start. The output `-ss` drops what's before `start` and
        // starts the times from it, so `-output_ts_offset` puts them back.
        let start = seconds(start_ms + REMUX_LATER_BY_MS);
        let times = [
            "-copyts",
            "-start_at_zero",
            "-ss",
            &start,
            "-output_ts_offset",
            &start,
        ];
        command.args(times);
        command.args(["-map", "0:V:0"]);
        map_audio(&mut command, remux.audio.map(|audio| audio.track));
        command.args(["-c", "copy"]);
        aac(
            &mut command,
            remux.audio.and_then(|audio| audio.aac_channels),
        );
        Ok(hls_output(command, &ffmpeg.version, dir, from.n))
    }

    /// The run of ffmpeg that converts the first video of the file at `path` to 8-bit H.264, and
    /// its audio Track if any to AAC, as `transcode` says, into fMP4 in `dir`, from the cut `from`
    /// to the end. It cuts a file at each cut, numbered from `from`'s, such as `3.m4s`, and writes
    /// an init segment named by [`hls_init`] that's whole once the first file is. A run started at
    /// any cut writes the same files, so one can pick up where another stopped. Each file appears
    /// only once it's whole. Fails without ffmpeg.
    pub(crate) fn transcode(
        &self,
        path: &str,
        transcode: &Transcode,
        dir: &Path,
        from: FromCut<'_>,
    ) -> io::Result<HlsRun> {
        let ffmpeg = (self.ffmpeg.as_ref()).map_err(|why| io::Error::other(why.clone()))?;
        let mut command = tokio::process::Command::new(&ffmpeg.path);
        command.args(["-v", "error", "-nostdin"]);
        let video = &transcode.video;
        let VideoArgs {
            input,
            video: video_input,
            codec,
        } = video_args(path, video);
        command.args(input);
        let start_ms = from.cuts.first().copied().unwrap_or_default();
        if start_ms > 0 {
            // Decodes from the keyframe before it and drops the frames before it.
            command.args(["-ss", &seconds(start_ms)]);
        }
        command.arg("-i").arg(path);
        // Times as in the file, so the keyframes forced below fall at the same frames whichever
        // cut a run starts at: `-force_key_frames` expressions time frames from the run's first.
        command.args(["-copyts", "-start_at_zero"]);
        command.args(video_input);
        map_audio(&mut command, transcode.audio.map(|audio| audio.track));
        // ponytail: a 5-hour Version's cuts near Windows' 32K command line; read them from a
        // file if longer ones fail to Transcode there.
        let cuts: Vec<String> = from.cuts.iter().map(|&cut| seconds(cut)).collect();
        command
            .args(codec)
            .arg("-force_key_frames")
            .arg(cuts.join(","));
        // ponytail: at Original, a hardware encoder picks its own rate, which is low on NVENC and
        // VideoToolbox; set one from the Version's bitrate if that shows.
        if let Some(VideoLimit {
            bits_per_second, ..
        }) = video.limit
        {
            let rate = bits_per_second.to_string();
            // Two seconds' worth of buffer lets a busy scene borrow from a quiet one.
            command
                .args(["-b:v", &rate, "-maxrate", &rate, "-bufsize"])
                .arg((2 * u64::from(bits_per_second)).to_string());
        }
        aac(
            &mut command,
            transcode.audio.map(|audio| audio.aac_channels),
        );
        command.args(["-output_ts_offset", &seconds(TRANSCODE_LATER_BY_MS)]);
        Ok(hls_output(command, &ffmpeg.version, dir, from.n))
    }

    /// The file at `path`'s subtitle stream `subtitle`, counting only its subtitle streams from 0,
    /// encoded with ffmpeg's `codec`, such as `webvtt`, or `copy` to keep it as it is, and written
    /// by its `muxer`, such as `webvtt` or `sup`.
    pub(crate) async fn subtitle(
        &self,
        path: &str,
        subtitle: usize,
        codec: &str,
        muxer: &str,
    ) -> Result<Vec<u8>, SubtitleError> {
        let map = format!("0:s:{subtitle}");
        let args = ["-i", path, "-map", &map, "-c:s", codec, "-f", muxer, "-"];
        let output = self.read_out(&args).await?;
        if !output.status.success() {
            return Err(failed(&output));
        }
        Ok(output.stdout)
    }

    /// The file at `path`'s attachment `attachment`, such as a font, counting only its
    /// attachments from 0.
    pub(crate) async fn attachment(
        &self,
        path: &str,
        attachment: usize,
    ) -> Result<Vec<u8>, SubtitleError> {
        let dump = format!("-dump_attachment:t:{attachment}");
        let output = self.read_out(&[&dump, "pipe:1", "-i", path]).await?;
        // With no output file named, ffmpeg dumps it and then fails for want of one, so only
        // what it wrote says whether it worked.
        if output.stdout.is_empty() {
            return Err(failed(&output));
        }
        Ok(output.stdout)
    }

    /// What ffmpeg, quiet but for errors, writes given `args`, within [`SUBTITLE_TIMEOUT`].
    async fn read_out(&self, args: &[&str]) -> Result<std::process::Output, SubtitleError> {
        let Ok(ffmpeg) = &self.ffmpeg else {
            return Err(SubtitleError::Unavailable);
        };
        let output = tokio::process::Command::new(&ffmpeg.path)
            .args(["-v", "error", "-nostdin"])
            .args(args)
            // A fetch given up on, or cancelled at shutdown, stops ffmpeg too.
            .kill_on_drop(true)
            .output();
        Ok(tokio::time::timeout(SUBTITLE_TIMEOUT, output).await??)
    }
}

/// A failed read-out, saying what ffmpeg wrote to stderr.
fn failed(output: &std::process::Output) -> SubtitleError {
    let stderr = String::from_utf8_lossy(&output.stderr);
    SubtitleError::Failed(stderr.trim().to_owned())
}

/// What the Server's ffmpeg can do, as [`Abilities`] hands it back.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each is a separate thing ffmpeg can do, not a state"
)]
pub(crate) struct CanDo {
    /// Whether the Server has ffmpeg and ffprobe, which every method but Direct Play needs.
    pub(crate) runs: bool,
    /// Whether ffmpeg has zscale, which a Transcode tone-maps HDR to SDR with in software.
    pub(crate) zscale: bool,
    /// Whether ffmpeg has libx264, which a Transcode encodes its video with in software.
    pub(crate) libx264: bool,
    /// The APIs on GPUs that passed their test, in the order Automatic picks.
    pub(crate) hardware: Vec<Hardware>,
    /// The GPUs the Server found at startup, whether or not anything passed on them.
    pub(crate) gpus: Vec<Gpu>,
}

impl CanDo {
    /// Whether a Transcode runs at all: it needs ffmpeg and ffprobe, and libx264 even on hardware.
    pub(crate) const fn transcodes(&self) -> bool {
        // ponytail: a Transcode on hardware needs no libx264, but its fallback to software does;
        // allow one once a failed hardware run can stop instead of falling back, and change
        // `server_ffmpeg`'s reason with it.
        self.runs && self.libx264
    }

    /// The hardware Automatic picks, if any passed its test.
    pub(crate) fn automatic(&self) -> Option<&Hardware> {
        self.hardware.first()
    }

    /// What tone-maps HDR to SDR in a Transcode on `gpu`, or in software for `None`: the GPU if
    /// it passed tone-mapping, or else zscale if ffmpeg has it.
    pub(crate) const fn tone_mapper(&self, gpu: Option<&Hardware>) -> Option<ToneMapper> {
        match gpu {
            Some(Hardware {
                tone_maps: true, ..
            }) => Some(ToneMapper::Hardware),
            _ if self.zscale => Some(ToneMapper::Software),
            _ => None,
        }
    }
}

/// Asks the Server's ffmpeg what it can do and remembers each answer, so planning and the
/// Playback status read the same [`CanDo`]. The only asker: see `ffmpeg/AGENTS.md`, "What
/// ffmpeg can do is asked once".
#[derive(Clone)]
pub(crate) struct Abilities {
    inner: Arc<AbilitiesInner>,
}

struct AbilitiesInner {
    ffmpeg: Arc<Ffmpeg>,
    zscale: tokio::sync::OnceCell<bool>,
    libx264: tokio::sync::OnceCell<bool>,
    hardware: tokio::sync::OnceCell<Vec<Hardware>>,
    /// The hardware test, so shutting down can wait for it.
    tasks: TaskTracker,
    /// Cancelled when the Server shuts down.
    cancel: CancellationToken,
}

impl Abilities {
    /// Starts testing `ffmpeg`'s hardware in the background, as it takes a few seconds; the first
    /// [`Self::can_do`] waits for it, and asks again if ffmpeg couldn't list its encoders.
    pub(crate) fn new(ffmpeg: Arc<Ffmpeg>) -> Self {
        let abilities = Self {
            inner: Arc::new(AbilitiesInner {
                ffmpeg,
                zscale: tokio::sync::OnceCell::new(),
                libx264: tokio::sync::OnceCell::new(),
                hardware: tokio::sync::OnceCell::new(),
                tasks: TaskTracker::new(),
                cancel: CancellationToken::new(),
            }),
        };
        // Holds nothing but ffmpeg, so the Server shutting down mid-test still closes the
        // database cleanly (#554).
        let testing = abilities.clone();
        abilities.inner.tasks.spawn(async move {
            // Only for testing early; a task's output must be owned.
            let _: Option<usize> = (testing.inner.cancel)
                .run_until_cancelled(async { testing.hardware().await.len() })
                .await;
        });
        abilities
    }

    /// Cancels the hardware test if it still runs, killing its test encode, and waits for it to
    /// stop.
    pub(crate) async fn shutdown(&self) {
        self.inner.cancel.cancel();
        self.inner.tasks.close();
        self.inner.tasks.wait().await;
    }

    /// What ffmpeg can do. An answer ffmpeg couldn't give, such as on a busy host whose listing
    /// timed out, counts as `false` or no hardware and is asked again next time.
    pub(crate) async fn can_do(&self) -> CanDo {
        let ffmpeg = &self.inner.ffmpeg;
        // Together, so a busy host waits out one listing's timeout, not two.
        let (zscale, libx264, hardware) = tokio::join!(
            answered_once(&self.inner.zscale, "zscale", ffmpeg.has_zscale()),
            answered_once(&self.inner.libx264, "libx264", ffmpeg.has_libx264()),
            self.hardware(),
        );
        CanDo {
            runs: ffmpeg.can_remux(),
            zscale,
            libx264,
            hardware: hardware.to_vec(),
            gpus: ffmpeg.gpus.clone(),
        }
    }

    /// The hardware acceleration that passed its test encode, tested the first time it's asked
    /// for and kept, or none while ffmpeg couldn't list its encoders, which is asked again next
    /// time.
    async fn hardware(&self) -> &[Hardware] {
        let ffmpeg = &self.inner.ffmpeg;
        let tested = self.inner.hardware.get_or_try_init(|| async {
            let found = ffmpeg.find_hardware().await?;
            let names: Vec<String> = found.iter().map(Hardware::name).collect();
            tracing::info!(hardware = ?names, "tested hardware acceleration");
            Ok::<_, String>(found)
        });
        // As `answered_once`: a listing that timed out on a busy host isn't kept as no hardware.
        match tested.await {
            Ok(found) => found,
            Err(err) => {
                tracing::warn!("could not ask ffmpeg for its encoders: {err}");
                &[]
            }
        }
    }
}

/// Whether ffmpeg has `feature`, as `cell` keeps from `ask`'s first answer, or `false` while it
/// can't say.
async fn answered_once(
    cell: &tokio::sync::OnceCell<bool>,
    feature: &str,
    ask: impl Future<Output = Result<bool, String>>,
) -> bool {
    // Only an answer is kept, so a check that timed out on a busy host is asked again.
    match cell.get_or_try_init(|| ask).await {
        Ok(&has) => has,
        Err(err) => {
            tracing::warn!("could not ask ffmpeg whether it has {feature}: {err}");
            false
        }
    }
}

/// What a Remux converts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Remux {
    /// The audio Track it plays, or `None` when the Version has none.
    pub(crate) audio: Option<RemuxAudio>,
}

/// The audio Track a Remux plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RemuxAudio {
    /// Its place among the file's audio Tracks, counting from 0.
    pub(crate) track: usize,
    /// The channels it's converted to AAC with, or `None` to copy it.
    pub(crate) aac_channels: Option<i64>,
}

/// What a Transcode converts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Transcode {
    /// The audio Track it plays, or `None` when the Version has none.
    pub(crate) audio: Option<TranscodeAudio>,
    /// How it converts the first video.
    pub(crate) video: VideoConversion,
}

/// The audio Track a Transcode plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TranscodeAudio {
    /// Its place among the file's audio Tracks, counting from 0.
    pub(crate) track: usize,
    /// The channels it's converted to AAC with.
    pub(crate) aac_channels: i64,
}

/// The cut a run of a Transcode starts at.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FromCut<'a> {
    /// Its number, from 0.
    pub(crate) n: usize,
    /// When it and each cut after it fall, in milliseconds from the file's start.
    pub(crate) cuts: &'a [i64],
}

/// One run of ffmpeg writing a Remux's or Transcode's files, built but not started. Only this
/// module builds one, so ffmpeg only ever runs with arguments built here.
pub(crate) struct HlsRun(tokio::process::Command);

impl HlsRun {
    /// Starts ffmpeg. Dropping the child stops it.
    pub(crate) fn spawn(mut self) -> io::Result<tokio::process::Child> {
        self.0.spawn()
    }
}

/// How a Transcode keeps its video within a Playback Limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VideoLimit {
    /// The width and height it scales the picture to, if it does.
    pub(crate) size: Option<(i64, i64)>,
    pub(crate) bits_per_second: u32,
}

/// A subtitle Track a Transcode burns into the picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BurnIn {
    /// A bitmap, such as PGS, at its place among the file's subtitle streams, counting from 0, laid
    /// over a picture of the width and height given, if known.
    Image {
        subtitle: usize,
        picture: Option<(i64, i64)>,
    },
    /// Text, such as ASS, at its place among the file's subtitle streams, counting from 0, which
    /// libass draws with its styling.
    Text { subtitle: usize },
}

/// The arguments that take the first video of the file at `path` into a Transcode, through
/// `before`, then with `burn_in` drawn into it, then through `after`.
fn transcoded_video(
    path: &str,
    before: Option<String>,
    burn_in: Option<BurnIn>,
    after: Option<String>,
) -> Vec<String> {
    let drawn = match burn_in {
        None => None,
        Some(BurnIn::Image { subtitle, picture }) => {
            let graph = overlay_graph(before, subtitle, picture, after);
            return ["-filter_complex", &graph, "-map", "[video]"]
                .map(str::to_owned)
                .into();
        }
        // ponytail: assumes ffmpeg has libass. Each run reads the whole file for the subtitle
        // first, attached fonts and all; read it once into the subtitle cache instead if seeking
        // in a styled subtitle's Transcode is too slow.
        Some(BurnIn::Text { subtitle }) => Some(format!(
            "subtitles=filename={}:si={subtitle}",
            filter_value(path)
        )),
    };
    let filters: Vec<String> = [before, drawn, after].into_iter().flatten().collect();
    let mut args: Vec<String> = ["-map", "0:V:0"].map(str::to_owned).into();
    if !filters.is_empty() {
        // One `-vf`: a second would replace the first.
        args.extend(["-vf".to_owned(), filters.join(",")]);
    }
    args
}

/// The filtergraph that runs the first video through `before`, lays its `subtitle`th subtitle
/// stream, a bitmap, over it, fitted to `picture` if known, then runs it through `after`, ending at
/// `[video]`.
fn overlay_graph(
    before: Option<String>,
    subtitle: usize,
    picture: Option<(i64, i64)>,
    after: Option<String>,
) -> String {
    // A bitmap's canvas can be smaller than the picture, as a 4K Blu-ray's 1080p PGS is, or
    // another shape, as a cropped film's 1080p PGS is: scaled evenly and centred, its text is
    // neither stretched nor cut off.
    let fit = picture.map(|(width, height)| {
        format!("scale={width}:{height}:force_original_aspect_ratio=decrease")
    });
    let or_null = |filter: Option<String>| filter.unwrap_or_else(|| "null".to_owned());
    let (before, fit) = (or_null(before), or_null(fit));
    let after: String = after.map(|after| format!(",{after}")).unwrap_or_default();
    format!(
        "[0:V:0]{before}[picture];[0:s:{subtitle}]{fit}[subtitle];\
         [picture][subtitle]overlay=(W-w)/2:(H-h)/2{after}[video]"
    )
}

/// `value` as a filter's option value in a filtergraph, which ffmpeg unescapes twice: once
/// parsing the graph, then once parsing the filter's options (ffmpeg-filters(1), "Notes on
/// filtergraph escaping").
fn filter_value(value: &str) -> String {
    let quoted = format!("'{}'", value.replace('\'', r"'\''"));
    let mut escaped = String::with_capacity(quoted.len());
    for character in quoted.chars() {
        if r"\'[],;".contains(character) {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

/// How a Transcode converts its video.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VideoConversion {
    /// The API, on its GPU, it decodes, converts and encodes on, or `None` for software.
    pub(crate) hardware: Option<Hardware>,
    /// What tone-maps its HDR to SDR, if anything does.
    pub(crate) tone_map: Option<ToneMapper>,
    pub(crate) limit: Option<VideoLimit>,
    /// The subtitle it draws into the picture, if any.
    pub(crate) burn_in: Option<BurnIn>,
    /// Whether each file carries the video's headers, so files written by another encoder, as
    /// when hardware fails and software carries on, play after this one's init segment.
    pub(crate) in_band_headers: bool,
}

/// What tone-maps a Transcode's HDR to SDR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToneMapper {
    /// ffmpeg's zscale, which [`CanDo::zscale`] says it has.
    Software,
    /// The GPU of [`VideoConversion::hardware`], which [`Hardware::tone_maps`] says it can, while
    /// nothing is burned in.
    Hardware,
}

/// The arguments for ffmpeg that convert video as a [`VideoConversion`] says.
struct VideoArgs {
    /// Before `-i`.
    input: Vec<String>,
    /// After `-i`: which video, and the filters it goes through.
    video: Vec<String>,
    /// The encoder, making keyframes only where forced.
    codec: Vec<String>,
}

/// The arguments for ffmpeg that convert the first video of the file at `path` as `video` says.
fn video_args(path: &str, video: &VideoConversion) -> VideoArgs {
    let size = video.limit.and_then(|limit| limit.size);
    let accel = video.hardware.as_ref().map(|hardware| hardware.accel);
    let gpu = video
        .hardware
        .as_ref()
        .and_then(|hardware| hardware.gpu.as_ref());
    let mut codec = match accel {
        None => ["-c:v", "libx264", "-preset", "veryfast", "-pix_fmt", "yuv420p"]
            .into_iter()
            // Keyframes only where forced, so each cut starts a file and nothing else does.
            .chain(["-x264-params", "keyint=infinite:scenecut=0"])
            .map(str::to_owned)
            .collect(),
        Some(accel) => accel.codec(),
    };
    if video.in_band_headers {
        // `avc3` has decoders read the headers before each keyframe rather than the init
        // segment's, which a player fetched once, from whichever encoder wrote it.
        codec.extend(["-bsf:v", "dump_extra=freq=keyframe", "-tag:v", "avc3"].map(str::to_owned));
    }
    // zscale and subtitles work on frames in memory, so with either the GPU only decodes and
    // encodes.
    let on_gpu = accel
        .filter(|_| video.tone_map != Some(ToneMapper::Software) && video.burn_in.is_none())
        .and_then(|api| {
            let tone_map = video.tone_map == Some(ToneMapper::Hardware);
            Some((api, api.gpu_filters(tone_map, size)?))
        });
    if let Some((api, filters)) = on_gpu {
        return VideoArgs {
            input: api.input(gpu, true),
            video: ["-map", "0:V:0", "-vf", &filters].map(str::to_owned).into(),
            codec,
        };
    }
    // Subtitles are drawn after tone-mapping, which would dim them, and before scaling, so they
    // shrink with the picture.
    let before = (video.tone_map.is_some()).then(|| TONE_MAP.to_owned());
    // ponytail: the size is from the stored picture, so a rotated phone clip is stretched; swap
    // its sides once probing reads rotation.
    let scale = size.map(|(width, height)| format!("scale={width}:{height}"));
    let upload = accel.map(|accel| accel.upload().to_owned());
    let after: Vec<String> = [scale, upload].into_iter().flatten().collect();
    VideoArgs {
        input: accel
            .map(|accel| accel.input(gpu, false))
            .unwrap_or_default(),
        video: transcoded_video(
            path,
            before,
            video.burn_in,
            (!after.is_empty()).then(|| after.join(",")),
        ),
        codec,
    }
}

/// Has `command` take the `audio`th audio Track, if given.
fn map_audio(command: &mut tokio::process::Command, audio: Option<usize>) {
    if let Some(audio) = audio {
        command.arg("-map").arg(format!("0:a:{audio}"));
    }
}

/// Has `command` convert its audio to AAC with `channels` channels, if given.
fn aac(command: &mut tokio::process::Command, channels: Option<i64>) {
    if let Some(channels) = channels {
        command
            .args(["-c:a", "aac", "-ac"])
            .arg(channels.to_string());
    }
}

/// `command`, ffmpeg at `version` given its input and codecs, writing HLS with fMP4 to `dir`: a
/// file per keyframe, numbered from `n`, and an init segment named by [`hls_init`].
fn hls_output(mut command: tokio::process::Command, version: &str, dir: &Path, n: usize) -> HlsRun {
    // ffmpeg 5.0 renamed it; 6.0 dropped the old name.
    let segment_options = if is_release_before(version, (5, 0)) {
        "-hls_ts_options"
    } else {
        "-hls_segment_options"
    };
    command
        .args(["-f", "hls", "-hls_segment_type", "fmp4"])
        // ffmpeg cuts at multiples of `-hls_time` from where it started, so a run started
        // elsewhere would cut elsewhere: cutting at every keyframe keeps the cuts put.
        .args(["-hls_time", "0.001", "-hls_playlist_type", "vod"])
        .args(["-hls_flags", "temp_file", "-hls_fmp4_init_filename"])
        .arg(hls_init(n))
        // Or each run's fragments would be timed from where it started.
        .args([segment_options, "movflags=+frag_discont"])
        .arg("-start_number")
        .arg(n.to_string())
        .arg("-hls_segment_filename")
        .arg(dir.join("%d.m4s"))
        // The Server serves a playlist of its own, listing the whole Version up front.
        .arg(dir.join("ffmpeg.m3u8"))
        // ffmpeg puts the init segment in the playlist's folder, which it finds by the last `/`
        // alone, so on Windows it'd put it in its working folder instead.
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    HlsRun(command)
}

/// `ms` milliseconds as seconds, such as `"5.120"`, as ffmpeg takes a time.
fn seconds(ms: i64) -> String {
    format!("{}.{:03}", ms / 1000, ms % 1000)
}

/// `name` at `configured`, or else on `path`, if it's new enough, or why the Server can't use it.
fn find(name: &str, configured: Option<&Path>, path: &OsStr) -> Result<FoundProgram, String> {
    let found = match configured {
        Some(configured) => at_configured_path(name, configured)?,
        None => on_path(name, path).ok_or_else(|| not_on_path(name))?,
    };
    if is_release_before(&found.version, MINIMUM) {
        let (major, minor) = MINIMUM;
        return Err(format!(
            "{name} {} at {} is older than {major}.{minor}, the oldest this Server supports",
            found.version,
            found.path.display()
        ));
    }
    Ok(found)
}

fn not_on_path(name: &str) -> String {
    format!("{name} wasn't found on PATH")
}

/// `name` at the path the config sets, or why it can't be run.
fn at_configured_path(name: &str, path: &Path) -> Result<FoundProgram, String> {
    // Relative to the working directory, like `data_dir`. Left relative, a bare `ffmpeg` would
    // be looked up on `PATH` when run.
    let path = std::path::absolute(path).unwrap_or_else(|_| path.to_owned());
    if !path.is_file() {
        return Err(format!(
            "{name} wasn't found at the configured path {}",
            path.display()
        ));
    }
    let shown = path.display().to_string();
    run(name, path)
        .ok_or_else(|| format!("{name} at the configured path {shown} won't run or isn't {name}"))
}

/// The first `name` in the directories of `path` that runs and says which version it is.
fn on_path(name: &str, path: &OsStr) -> Option<FoundProgram> {
    let file = format!("{name}{}", env::consts::EXE_SUFFIX);
    env::split_paths(path)
        // A relative entry, such as an empty one, would run whatever is in the working directory.
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(&file))
        .filter(|candidate| candidate.is_file())
        .find_map(|candidate| run(name, candidate))
}

/// `name` at `path`, if it runs and says which version it is.
fn run(name: &str, path: PathBuf) -> Option<FoundProgram> {
    let output = Command::new(&path).arg("-version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    // The first line reads like `ffmpeg version 7.1.1 Copyright (c) 2000-2025 the FFmpeg developers`.
    let version = String::from_utf8_lossy(&output.stdout)
        .strip_prefix(&format!("{name} version "))?
        .split_whitespace()
        .next()?
        .to_owned();
    Some(FoundProgram { path, version })
}

/// Whether `version` is a release older than `release`, as major and minor version.
fn is_release_before(version: &str, release: (u32, u32)) -> bool {
    // A release says `7.1.1`, `n7.0` or `4.4.2-0ubuntu0.22.04.1`. A build from git says
    // `N-113021-g5b3fc5d`, with no release number, so it gets the benefit of the doubt.
    let mut numbers = version
        .trim_start_matches('n')
        .split(|c: char| !c.is_ascii_digit())
        .map(|number| number.parse::<u32>().ok());
    let Some(Some(major)) = numbers.next() else {
        return false;
    };
    let minor = numbers.next().flatten().unwrap_or(0);
    (major, minor) < release
}

/// Writes a fifth of a second of 10-bit 4K video with English 5.1 audio, as Matroska, to `path`,
/// with the ffmpeg on `PATH`, for a test to probe. The video is tagged with `transfer`, as ffmpeg
/// names it: `smpte2084` for HDR10, `arib-std-b67` for HLG or `bt709` for SDR.
///
/// # Errors
///
/// If ffmpeg can't be run or fails.
#[cfg(any(test, feature = "testing"))]
pub fn write_4k_with_5_1(path: &Path, transfer: &str) -> io::Result<()> {
    write_test_media(path, |ffmpeg| {
        // The frames carry the colours, since ffmpeg 9 drops `-color_trc` and the like for FFV1.
        let picture = format!(
            "color=size=3840x2160:rate=24,format=yuv420p10le,\
             setparams=color_primaries=bt2020:color_trc={transfer}:colorspace=bt2020nc"
        );
        ffmpeg
            .args(["-f", "lavfi", "-i", &picture])
            .args(["-f", "lavfi", "-i", "anullsrc=channel_layout=5.1"])
            // Encoders every ffmpeg build has; of those, FFV1 is one that takes 10 bits.
            .args(["-t", "0.2", "-c:v", "ffv1", "-pix_fmt", "yuv420p10le", "-c:a", "ac3"])
            .args(["-metadata:s:a:0", "language=eng", "-f", "matroska"])
    })
}

/// Writes 14 seconds of H.264 video with B-frames and a keyframe every 2 seconds, and 5.1 audio
/// encoded with `audio`, such as `aac` or `dca` for DTS, as Matroska, to `path`, with the ffmpeg on
/// `PATH`, for a test to Remux.
///
/// # Errors
///
/// If ffmpeg can't be run or fails.
#[cfg(any(test, feature = "testing"))]
pub fn write_h264_with_5_1(path: &Path, audio: &str) -> io::Result<()> {
    write_test_media(path, |ffmpeg| {
        ffmpeg
            .args(["-f", "lavfi", "-i", "testsrc=size=320x240:rate=24"])
            .args(["-f", "lavfi", "-i", "sine,aformat=channel_layouts=5.1", "-t", "14"])
            .args([
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                // As most H.264 has, so a keyframe decodes before it shows.
                "-bf",
                "2",
                "-g",
                "48",
                "-sc_threshold",
                "0",
            ])
            // ffmpeg's DTS encoder is marked experimental.
            .args(["-c:a", audio, "-strict", "experimental", "-f", "matroska"])
    })
}

/// Writes 2 seconds of 1080p H.264 video with stereo AAC audio, as Matroska, to `path`, with the
/// ffmpeg on `PATH`, for a test to play within a Playback Limit.
///
/// # Errors
///
/// If ffmpeg can't be run or fails.
#[cfg(any(test, feature = "testing"))]
pub fn write_1080p_h264(path: &Path) -> io::Result<()> {
    write_test_media(path, |ffmpeg| {
        ffmpeg
            .args(["-f", "lavfi", "-i", "testsrc=size=1920x1080:rate=24"])
            .args(["-f", "lavfi", "-i", "sine", "-t", "2"])
            .args(["-c:v", "libx264", "-preset", "ultrafast", "-c:a", "aac"])
            .args(["-f", "matroska"])
    })
}

/// Writes 20 seconds of HEVC video at 23.976 frames a second, whose frames don't fall on whole
/// seconds, with 5.1 AC-3 audio, as Matroska, to `path`, with the ffmpeg on `PATH`, for a test to
/// Transcode.
///
/// # Errors
///
/// If ffmpeg can't be run or fails.
#[cfg(any(test, feature = "testing"))]
pub fn write_hevc_with_5_1(path: &Path) -> io::Result<()> {
    write_test_media(path, |ffmpeg| {
        ffmpeg
            .args(["-f", "lavfi", "-i", "testsrc=size=320x240:rate=24000/1001"])
            .args([
                "-f",
                "lavfi",
                "-i",
                "sine,aformat=channel_layouts=5.1",
                "-t",
                "20",
            ])
            .args(["-c:v", "libx265", "-preset", "ultrafast"])
            .args(["-x265-params", "log-level=error"])
            .args(["-c:a", "ac3", "-f", "matroska"])
    })
}

/// Writes 20 seconds of black 320×240 H.264 video with a PGS subtitle Track showing a 16×16 white
/// square at 100, 100 from its start to 19 seconds, as Matroska, to `path`, with the ffmpeg on
/// `PATH`, for a test to burn in.
///
/// # Errors
///
/// If the subtitle can't be written beside `path`, or ffmpeg can't be run or fails.
#[cfg(any(test, feature = "testing"))]
pub fn write_black_with_pgs(path: &Path) -> io::Result<()> {
    // ffmpeg can't encode PGS, so it muxes one written by hand.
    let sup = path.with_extension("sup");
    std::fs::write(&sup, white_square_pgs())?;
    let written = write_test_media(path, |ffmpeg| {
        ffmpeg
            .args([
                "-f",
                "lavfi",
                "-i",
                "color=black:size=320x240:rate=24",
                "-i",
            ])
            .arg(&sup)
            .args(["-t", "20", "-map", "0", "-map", "1"])
            // A keyframe each second, so a Transcode started mid-way seeks past where the
            // subtitle starts, as it would in a film.
            .args(["-c:v", "libx264", "-preset", "ultrafast", "-g", "24", "-c:s", "copy"])
            .args(["-f", "matroska"])
    });
    std::fs::remove_file(&sup)?;
    written
}

/// A PGS stream, as a `.sup` file holds it, on a 320×240 canvas: a 16×16 white square at 100, 100
/// shown at 0 seconds and cleared at 19.
#[cfg(any(test, feature = "testing"))]
fn white_square_pgs() -> Vec<u8> {
    // Each segment: "PG", when it shows and decodes in 90 kHz ticks, its type and its length.
    let segment = |seconds: u32, kind: u8, body: &[u8]| -> Vec<u8> {
        let length = u16::try_from(body.len()).unwrap_or(u16::MAX);
        let shows = (seconds * 90_000).to_be_bytes();
        [
            &b"PG"[..],
            &shows,
            &[0; 4],
            &[kind],
            &length.to_be_bytes(),
            body,
        ]
        .concat()
    };
    // Canvas width and height, frame rate, composition number, then its state: 0x80 starts one.
    let composition = |state: u8| vec![1, 64, 0, 240, 0x10, 0, 0, state, 0, 0];
    // Then its objects: one, in window 0 at 100, 100, or none.
    let shown = [composition(0x80), vec![1, 0, 0, 0, 0, 0, 100, 0, 100]].concat();
    let cleared = [composition(0), vec![0]].concat();
    let window = [1, 0, 0, 100, 0, 100, 0, 16, 0, 16];
    // Colour 1 is opaque white, as Y, Cr, Cb and alpha.
    let palette = [0, 0, 1, 235, 128, 128, 255];
    // Each row is 16 pixels of colour 1, then the end of the row.
    let rows = [0, 0x80 | 16, 1, 0, 0].repeat(16);
    let object = [&[0, 0, 0, 0xC0, 0, 0, 84, 0, 16, 0, 16][..], &rows].concat();
    [
        segment(0, 0x16, &shown),
        segment(0, 0x17, &window),
        segment(0, 0x14, &palette),
        segment(0, 0x15, &object),
        segment(0, 0x80, &[]),
        segment(19, 0x16, &cleared),
        segment(19, 0x17, &window),
        segment(19, 0x80, &[]),
    ]
    .concat()
}

/// Runs the ffmpeg on `PATH` with the inputs and outputs `args` adds, writing to `path`.
#[cfg(any(test, feature = "testing"))]
fn write_test_media(
    path: &Path,
    args: impl FnOnce(&mut Command) -> &mut Command,
) -> io::Result<()> {
    let mut ffmpeg = Command::new("ffmpeg");
    let status = args(ffmpeg.args(["-v", "error", "-y"]))
        .arg(path)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("ffmpeg failed: {status}")))
    }
}

// The fakes are shell scripts.
#[cfg(all(test, unix))]
#[path = "ffmpeg/ffmpeg_tests.rs"]
pub(crate) mod tests;

#[cfg(test)]
#[path = "ffmpeg/hls_run_tests.rs"]
mod hls_run_tests;

#[cfg(test)]
#[path = "ffmpeg/hls_tests.rs"]
mod hls_tests;
