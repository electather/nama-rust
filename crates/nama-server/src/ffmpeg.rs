//! Finding the ffmpeg and ffprobe the Server Remuxes, Transcodes and probes Versions with, which
//! come from the system, not the release (ADR 0026).

mod abilities;
#[cfg(any(test, feature = "testing"))]
pub mod fake;
mod finding;
mod hardware;
mod hls;
mod refused_write;
#[cfg(any(test, feature = "testing"))]
mod test_media;

use std::{
    ffi::{OsStr, OsString},
    io,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

pub(crate) use self::abilities::{Abilities, CanDo};
use self::finding::{find, not_on_path};
pub use self::hardware::{Accel, Gpu, Hardware, find_gpus};
#[cfg(test)]
use self::hls::filter_value;
pub(crate) use self::hls::{
    AudioConversion, AudioEncoder, BurnIn, FromCut, HlsRun, ImageSubtitle, Remux, RemuxAudio,
    TextSubtitle, ToneMapper, Transcode, TranscodeAudio, VideoConversion, VideoLimit, hls_init,
    hls_playlist, hls_run_from,
};
use self::hls::{
    VideoArgs, convert_audio, hls_output, map_audio, pgs_start_ms, seconds, video_args,
};
pub(crate) use self::refused_write::RefusedWrite;
#[cfg(any(test, feature = "testing"))]
pub use self::test_media::{
    FRENCH_SRT, PERSIAN_SRT, write_4k_with_5_1, write_17_minutes_h264, write_1080p_h264,
    write_black_with_pgs, write_black_with_pgs_sidecar, write_h264_with_5_1, write_hevc_with_5_1,
};
use crate::config::Config;

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

/// How far before its first keyframe a Remux keeps the video's packets at most, in milliseconds.
/// Its audio starts at [`REMUX_AUDIO_BACK_MS`] instead, unless the run starts the file.
const REMUX_BACK_AT_MOST_MS: i64 = 500;

/// How far before its first keyframe a Remux that doesn't start the file starts its audio, in
/// milliseconds: at the keyframe's decode time, where a run that wrote the file before ended its
/// audio. That's two frames before the keyframe shows at 24 fps, as most films' H.264 and HEVC are.
// ponytail: at other frame rates or reorderings, runs that meet overlap or skip up to a few tens
// of ms of audio; read the keyframe's decode time when Probing if that's heard.
const REMUX_AUDIO_BACK_MS: i64 = 83;

/// How much later a Remux starts its audio when converting it to AAC than when copying it, in
/// milliseconds: the encoder starts its first packet a frame of 1024 samples, 21 to 23 ms at 48 or
/// 44.1 kHz, before its first sample.
const AAC_PRIMING_MS: i64 = 24;

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

/// How long ffmpeg may take to read one subtitle, or a Version's Preview frames, out of a file.
/// Matroska interleaves a subtitle with the video, so the whole file is read. Raised rather than
/// grown with the file's size: an hour reads 60 GB at 17 MB/s, slower than a spinning disk or a
/// NAS on gigabit, so only a stuck read outlasts it.
const READ_OUT_TIMEOUT: Duration = Duration::from_hours(1);

/// Subtitle codecs, as ffprobe names them, that are text the Server serves as WebVTT. Others that
/// aren't images either, such as teletext, need a library ffmpeg may lack, or are unknown.
pub(crate) const TEXT_SUBTITLES: &[&str] = &["subrip", "mov_text", "webvtt", "ass", "ssa"];

/// Why reading a subtitle, a font for one, or Preview frames out of a file failed.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ReadOutError {
    #[error("the Server has no ffmpeg")]
    Unavailable,
    #[error("ffmpeg couldn't be run: {0}")]
    Run(#[from] io::Error),
    #[error("ffmpeg took longer than {}s", READ_OUT_TIMEOUT.as_secs())]
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
    #[tracing::instrument(name = "ffmpeg list", skip_all, fields(ffmpeg.list = list))]
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
    #[tracing::instrument(name = "ffmpeg open GPU", skip_all)]
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
    #[tracing::instrument(name = "ffmpeg test encode", skip_all)]
    async fn test_encode(&self, clip: &str, video: &VideoConversion) -> Result<(), String> {
        let ffmpeg = self.ffmpeg.as_ref().map_err(Clone::clone)?;
        // The clip's path matters only to a subtitle burned in, which a test has none of.
        let VideoArgs {
            input,
            video,
            codec,
        } = video_args(clip, video, false);
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
    /// Track if any, untouched into fMP4 in `dir`, but for converting that audio as `remux`
    /// says, from the keyframe `from` to the end. It writes a file per keyframe of the file, listed
    /// or not, which [`hls_playlist`] lists with how long each lasts, and an init segment named by
    /// [`hls_init`] that's whole once its first file is. A run started at any keyframe cuts at the
    /// same ones, so one can pick up where another stopped. Each file appears only once it's whole.
    /// HEVC video is tagged `hvc1` so Apple's players play it. Fails without ffmpeg.
    ///
    /// The video starts up to [`REMUX_BACK_AT_MOST_MS`] before `from`, the audio as
    /// [`REMUX_AUDIO_BACK_MS`] says, so it doesn't step back where this run picks up another's.
    /// Copied audio is cut there by `file_start_ms`, when the file starts as Probing reads it.
    #[expect(
        clippy::too_many_arguments,
        reason = "the plan makes `remux` before Probing lists the file's start with its keyframes"
    )]
    pub(crate) fn remux(
        &self,
        path: &str,
        remux: &Remux,
        dir: &Path,
        from: Keyframe,
        file_start_ms: i64,
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
        let audio_start_ms = (remux.audio.is_some() && from.n > 0)
            .then(|| (from.ms - REMUX_AUDIO_BACK_MS).max(start_ms));
        let seek_ms = audio_start_ms.unwrap_or(start_ms);
        let conversion = remux.audio.and_then(|audio| audio.conversion);
        if seek_ms > 0 {
            let cuts_copied_audio = audio_start_ms.is_some() && conversion.is_none();
            if cuts_copied_audio && file_start_ms != 0 {
                // `-copypriorss` drops packets by the input's `-ss` as a time in the file, not
                // from its start as ffmpeg seeks by, so in one that starts late, such as an
                // MPEG-TS, both are given in the file's own times.
                command.args(["-seek_timestamp", "1"]);
                command.args(["-ss", &seconds(file_start_ms + seek_ms)]);
            } else {
                // Jumps to a keyframe at or before it, which in Matroska can be a few before.
                command.args(["-ss", &seconds(seek_ms)]);
            }
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
        if remux.hevc {
            // Apple's players take HEVC in fMP4 only as `hvc1`, and ffmpeg writes `hev1` unless
            // told. Only for HEVC: ffmpeg refuses the tag on any other codec.
            command.args(["-tag:v", "hvc1"]);
        }
        convert_audio(&mut command, conversion);
        if let Some(audio_start_ms) = audio_start_ms {
            start_audio_at(&mut command, audio_start_ms, conversion.is_some());
        }
        Ok(hls_output(command, &ffmpeg.version, dir, from.n))
    }

    /// The run of ffmpeg that converts the first video of the file at `path` to 8-bit H.264, and
    /// its audio Track if any, as `transcode` says, into fMP4 in `dir`, from the cut `from`
    /// to the end. It cuts a file at each cut, which [`hls_playlist`] lists, and writes an init
    /// segment named by [`hls_init`] that's whole once the first file is. A run started at
    /// any cut writes the same files, so one can pick up where another stopped. Each file appears
    /// only once it's whole. Fails without ffmpeg, or if a PGS sidecar it burns in can't be read.
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
        let start_ms = from.cuts.first().copied().unwrap_or_default();
        let seeks = start_ms > 0;
        let VideoArgs {
            input,
            video: video_input,
            codec,
        } = video_args(path, video, seeks);
        command.args(input);
        if seeks {
            // Decodes from the keyframe before it and drops the frames before it.
            command.args(["-ss", &seconds(start_ms)]);
        }
        command.arg("-i").arg(path);
        match &video.burn_in {
            Some(BurnIn::Image {
                subtitle: ImageSubtitle::Sidecar(sidecar),
                ..
            }) => {
                // `-start_at_zero` moves each input back by its own start, a `.sup`'s being its
                // first picture's time, which `-itsoffset` undoes. No `-ss`: see `overlay_graph`'s
                // caller.
                command
                    .arg("-itsoffset")
                    .arg(seconds(pgs_start_ms(sidecar)?))
                    .arg("-i")
                    .arg(sidecar);
            }
            Some(BurnIn::Image {
                subtitle: ImageSubtitle::Stream(_),
                ..
            }) if seeks => {
                command.arg("-i").arg(path);
            }
            _ => {}
        }
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
        convert_audio(&mut command, transcode.audio.map(|audio| audio.conversion));
        command.args(["-output_ts_offset", &seconds(TRANSCODE_LATER_BY_MS)]);
        Ok(hls_output(command, &ffmpeg.version, dir, from.n))
    }

    /// The file at `path`'s subtitle stream `subtitle`, counting only its subtitle streams from 0,
    /// or with `None` the one subtitle of the text sidecar at `path`, read in the charset it's in,
    /// encoded with ffmpeg's `codec`, such as `webvtt`, or `copy` to keep it as it is, and written
    /// by its `muxer`, such as `webvtt` or `sup`.
    pub(crate) async fn subtitle(
        &self,
        path: &str,
        subtitle: Option<usize>,
        codec: &str,
        muxer: &str,
    ) -> Result<Vec<u8>, ReadOutError> {
        let charset = match subtitle {
            // Matroska and MP4 store text subtitles as UTF-8.
            Some(_) => None,
            None => sidecar_charset(path).await,
        };
        let mut args = charset.map_or_default(|charset| vec!["-sub_charenc", charset]);
        let map = format!("0:s:{}", subtitle.unwrap_or_default());
        args.extend(["-i", path, "-map", &map, "-c:s", codec, "-f", muxer, "-"]);
        let output = self.read_out(&args, Priority::Normal).await?;
        if !output.status.success() {
            return Err(failed(&output));
        }
        Ok(output.stdout)
    }

    /// Reads, in one read of the file at `path`, each of `subtitles`, a subtitle stream by its
    /// place among the file's subtitle streams, encoded and written in a format ffmpeg names the
    /// same as its encoder, such as `webvtt` or `ass`, and each of `attachments`, such as a font,
    /// by its place among the file's attachments, into the file each names.
    ///
    /// A file may be left partly written when it fails.
    pub(crate) async fn extract(
        &self,
        path: &str,
        subtitles: &[(usize, &str, PathBuf)],
        attachments: &[(usize, PathBuf)],
    ) -> Result<(), ReadOutError> {
        let mut args: Vec<OsString> = vec![];
        for (attachment, to) in attachments {
            args.push(format!("-dump_attachment:t:{attachment}").into());
            args.push(to.into());
        }
        args.extend(["-i".into(), path.into()]);
        for (subtitle, format, to) in subtitles {
            let map = format!("0:s:{subtitle}");
            args.extend(["-map", &map, "-c:s", format, "-f", format].map(OsString::from));
            args.push(to.into());
        }
        let output = self.read_out(&args, Priority::Normal).await?;
        // With no output file named, ffmpeg dumps the attachments and then fails for want of one,
        // so only what it wrote says whether that worked.
        if output.status.success() || subtitles.is_empty() && all_there(attachments).await? {
            Ok(())
        } else {
            Err(failed(&output))
        }
    }

    /// Writes a still `width` pixels wide, keeping its aspect ratio, of the file at `path`'s first
    /// video every `every_s` seconds, into `dir` as `1.jpg`, `2.jpg` and on. It decodes only the
    /// keyframes, so each still is of the last keyframe at or before its time, and runs on one
    /// thread at a low OS priority, as it's background work beside playback.
    pub(crate) async fn keyframe_stills(
        &self,
        path: &str,
        every_s: u32,
        width: u32,
        dir: &Path,
    ) -> Result<(), ReadOutError> {
        let mut args: Vec<OsString> = ["-threads", "1", "-filter_threads", "1"]
            .into_iter()
            .chain(["-skip_frame", "nokey", "-i", path])
            // `V`, not `v`: a cover's picture isn't the video.
            .chain(["-map", "0:V:0", "-an", "-sn", "-vf"])
            .map(OsString::from)
            .collect();
        // `round=up` gives each time the last keyframe at or before it; ffmpeg's nearest takes
        // one up to half an interval later. The height comes from the display aspect ratio, with
        // square pixels out, as a sheet's frames are shown square: `-2` would keep an anamorphic
        // video's stored shape, squashed.
        args.push(
            format!("fps=1/{every_s}:round=up,scale={width}:trunc(ow/dar/2)*2,setsar=1").into(),
        );
        args.push(dir.join("%d.jpg").into());
        let output = self.read_out(&args, Priority::Low).await?;
        if output.status.success() {
            Ok(())
        } else {
            Err(failed(&output))
        }
    }

    /// What ffmpeg, quiet but for errors, writes given `args` at `priority`, within
    /// [`READ_OUT_TIMEOUT`].
    #[tracing::instrument(name = "ffmpeg extract", skip_all)]
    async fn read_out(
        &self,
        args: &[impl AsRef<OsStr>],
        priority: Priority,
    ) -> Result<std::process::Output, ReadOutError> {
        let Ok(ffmpeg) = &self.ffmpeg else {
            return Err(ReadOutError::Unavailable);
        };
        let mut command = tokio::process::Command::new(&ffmpeg.path);
        command
            .args(["-v", "error", "-nostdin"])
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // A fetch given up on, or a read cancelled at shutdown, stops ffmpeg too.
            .kill_on_drop(true);
        #[cfg(windows)]
        if priority == Priority::Low {
            // `BELOW_NORMAL_PRIORITY_CLASS`, from `windows-sys`, which the Server doesn't depend on.
            command.creation_flags(0x0000_4000);
        }
        let child = command.spawn()?;
        #[cfg(unix)]
        if priority == Priority::Low {
            lower_priority(&child);
        }
        Ok(tokio::time::timeout(READ_OUT_TIMEOUT, child.wait_with_output()).await??)
    }
}

/// How much CPU time ffmpeg gets beside the rest of the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Priority {
    Normal,
    /// For background work, which yields to playback.
    Low,
}

/// Lowers `child`'s OS priority to `nice`'s default, or logs why it couldn't.
// Lowered once it has started, as lowering it before needs the `unsafe` `pre_exec`. On Linux this
// lowers one thread, and those it starts later inherit it, so a thread ffmpeg starts in the moment
// before keeps the old priority.
#[cfg(unix)]
fn lower_priority(child: &tokio::process::Child) {
    const NICE: i32 = 10;
    let pid = (child.id())
        .and_then(|id| i32::try_from(id).ok())
        .and_then(rustix::process::Pid::from_raw);
    if let Some(pid) = pid
        && let Err(err) = rustix::process::setpriority_process(Some(pid), NICE)
    {
        tracing::warn!("could not lower ffmpeg's priority: {err}");
    }
}

async fn all_there(written: &[(usize, PathBuf)]) -> io::Result<bool> {
    for (_, path) in written {
        if !tokio::fs::try_exists(path).await? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// The charset the sidecar at `path` is in, as ffmpeg's `-sub_charenc` or the `subtitles` filter's
/// `charenc` names it, if it isn't UTF-8, which ffmpeg reads by default.
pub(crate) async fn sidecar_charset(path: &str) -> Option<&'static str> {
    // One that can't be read gets none, and ffmpeg says why it can't read it either.
    let bytes = tokio::fs::read(path).await.ok()?;
    charset_of(&bytes)
}

/// [`sidecar_charset`] for a sidecar holding `bytes`.
fn charset_of(bytes: &[u8]) -> Option<&'static str> {
    // ffmpeg reads UTF-8, with or without a BOM, and UTF-16 by its BOM, on its own.
    if str::from_utf8(bytes).is_ok() || encoding_rs::Encoding::for_bom(bytes).is_some() {
        return None;
    }
    let mut detector = chardetng::EncodingDetector::new(chardetng::Iso2022JpDetection::Deny);
    detector.feed(bytes, true);
    Some(detector.guess(None, chardetng::Utf8Detection::Deny).name())
}

/// A failed read-out, saying what ffmpeg wrote to stderr.
fn failed(output: &std::process::Output) -> ReadOutError {
    let stderr = String::from_utf8_lossy(&output.stderr);
    ReadOutError::Failed(stderr.trim().to_owned())
}

/// Has a Remux's `command`, whose input seeks to `start_ms`, drop the audio from before it,
/// which it's converting if `converted`.
fn start_audio_at(command: &mut tokio::process::Command, start_ms: i64, converted: bool) {
    if converted {
        // Under `-copyts` the input's `-ss` trims decoded audio without its `-itsoffset`, so
        // too early; this trims in the output's times.
        // ponytail: AC-3 and E-AC-3 prime 256 samples, not AAC's 1024, so a restarted run of
        // either drops about 18 ms more; trim by codec if that's heard.
        let start = seconds(start_ms + REMUX_LATER_BY_MS + AAC_PRIMING_MS);
        command.arg("-af").arg(format!("atrim=start={start}"));
    } else {
        // Copied packets from before the input's `-ss` are kept under `-copyts` unless told.
        // ponytail: before ffmpeg 8 a Matroska file starting at its AAC's priming, before 0,
        // skips a frame, as Probing reads a start before 0 as 0; keep it if that's heard.
        command.args(["-copypriorss:a", "0"]);
    }
}

// The fakes are shell scripts.
#[cfg(all(test, unix))]
#[path = "ffmpeg/ffmpeg_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "ffmpeg/hls_run_tests.rs"]
mod hls_run_tests;

#[cfg(test)]
#[path = "ffmpeg/hls_tests.rs"]
mod hls_tests;
