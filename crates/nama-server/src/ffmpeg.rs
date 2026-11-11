//! Finding the ffmpeg and ffprobe the Server Remuxes, Transcodes and probes Versions with, which
//! come from the system, not the release (ADR 0026).

mod hardware;

use std::{
    collections::HashMap,
    env,
    ffi::OsStr,
    io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

use serde::Deserialize;

pub use self::hardware::{Accel, Hardware};
use crate::{
    config::Config,
    tracks::{BaseLayer, Hdr, Probe, Track, TrackKind},
};

/// The oldest ffmpeg and ffprobe the Server uses, as major and minor version. HLS with fMP4
/// segments, the zscale and tonemap filters and the VAAPI, QSV and NVENC encoders that Remuxing
/// and Transcoding use all predate 4.4, and it's the oldest a supported LTS distribution ships
/// (Ubuntu 22.04), so nothing older gets tested.
const MINIMUM: (u32, u32) = (4, 4);

/// How long ffprobe may take over one file, such as one on a network share that stopped answering.
const PROBE_TIMEOUT: Duration = Duration::from_secs(60);

/// How long ffprobe may take to read all of a file, as listing its keyframes or finding where its
/// video ends does, such as a 60 GB file from a slow network share.
const WHOLE_FILE_TIMEOUT: Duration = Duration::from_mins(10);

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

/// How long ffmpeg may take to read one subtitle out of a file. Matroska interleaves it with the
/// video, so the whole file is read: about a minute from a slow disk, so this leaves room for a
/// slower one.
const SUBTITLE_TIMEOUT: Duration = Duration::from_secs(300);

/// Subtitle codecs, as ffprobe names them, that are bitmaps rather than text.
const IMAGE_SUBTITLES: &[&str] = &["dvb_subtitle", "dvd_subtitle", "hdmv_pgs_subtitle", "xsub"];

/// Subtitle codecs, as ffprobe names them, that are text the Server serves as WebVTT. Others that
/// aren't images either, such as teletext, need a library ffmpeg may lack, or are unknown.
pub(crate) const TEXT_SUBTITLES: &[&str] = &["subrip", "mov_text", "webvtt", "ass", "ssa"];

/// Why probing a file failed.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ProbeError {
    #[error("the Server has no ffprobe")]
    Unavailable,
    #[error("ffprobe couldn't be run: {0}")]
    Run(#[from] io::Error),
    #[error("ffprobe took too long")]
    TimedOut(#[from] tokio::time::error::Elapsed),
    #[error("ffprobe failed: {0}")]
    Failed(String),
    #[error("ffprobe's output couldn't be read: {0}")]
    Output(#[from] serde_json::Error),
}

/// Why reading a subtitle out of a file failed.
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

/// The ffmpeg and ffprobe the Server found at startup. Without both it only Direct Plays.
#[derive(Debug)]
pub struct Ffmpeg {
    /// ffmpeg, or why the Server can't use one, in plain words.
    pub ffmpeg: Result<FoundProgram, String>,
    /// ffprobe, or why the Server can't use one, in plain words.
    pub ffprobe: Result<FoundProgram, String>,
}

/// Neither found on `PATH`.
impl Default for Ffmpeg {
    fn default() -> Self {
        Self {
            ffmpeg: Err(not_on_path("ffmpeg")),
            ffprobe: Err(not_on_path("ffprobe")),
        }
    }
}

impl Ffmpeg {
    /// Runs ffmpeg and ffprobe at the paths `config` sets, or else the first of each in the
    /// directories of `path`, a `PATH` value, to read their versions. Blocks while they run.
    #[must_use]
    pub fn find(config: &Config, path: &OsStr) -> Self {
        Self {
            ffmpeg: find("ffmpeg", config.ffmpeg.as_deref(), path),
            ffprobe: find("ffprobe", config.ffprobe.as_deref(), path),
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
    pub(crate) async fn has_zscale(&self) -> Result<bool, String> {
        // Each filter is a line such as ` .SC zscale  V->V  Apply resizing, ...`.
        Ok(self
            .listed("-filters")
            .await?
            .contains(&"zscale".to_owned()))
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
    /// encode, as a Transcode would run it, in [`Accel::ALL`]'s order, each with whether it passes
    /// a test tone-map too. Takes a few seconds while each opens its device.
    pub(crate) async fn find_hardware(&self) -> Vec<Hardware> {
        let encoders = match self.listed("-encoders").await {
            Ok(encoders) => encoders,
            Err(err) => {
                tracing::warn!("could not ask ffmpeg for its encoders: {err}");
                return vec![];
            }
        };
        let mut found = vec![];
        for accel in Accel::ALL {
            if !encoders.iter().any(|encoder| encoder == accel.encoder()) {
                continue;
            }
            let scaled = VideoConversion {
                hardware: Some(accel),
                tone_map: None,
                limit: Some(VideoLimit {
                    size: Some((640, 360)),
                    bits_per_second: 1_000_000,
                }),
                burn_in: None,
                in_band_headers: true,
            };
            if let Err(err) = self.test_encode(TEST_CLIP, &scaled).await {
                tracing::info!("{} didn't pass its test encode: {err}", accel.name());
                continue;
            }
            let tone_mapped = VideoConversion {
                tone_map: Some(ToneMapper::Hardware),
                ..scaled
            };
            let tone_maps = accel.has_tone_map_filter()
                && (self.test_encode(TEST_HDR_CLIP, &tone_mapped).await)
                    .inspect_err(|err| {
                        tracing::info!("{} didn't pass its test tone-map: {err}", accel.name());
                    })
                    .is_ok();
            found.push(Hardware { accel, tone_maps });
        }
        found
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

    /// The facts of the file at `path` and its video, audio and subtitle Tracks, as ffprobe reads
    /// them. Reads the whole file when its format doesn't say how long it is.
    pub(crate) async fn probe(&self, path: &str) -> Result<Probe, ProbeError> {
        let args = ["-print_format", "json", "-show_format", "-show_streams"];
        let output = self.run_ffprobe(&args, path, PROBE_TIMEOUT).await?;
        let mut probe = parse_probe(&output)?;
        if probe.duration_ms.is_none() {
            probe.duration_ms = match self.video_end(path).await {
                // Its Tracks were read, so a packet it can't read, as in a damaged file, leaves
                // only its length unknown rather than failing the probe for good.
                Err(ProbeError::Failed(_)) => None,
                end => end?,
            };
        }
        Ok(probe)
    }

    /// When the first video in the file at `path` ends, in milliseconds from its start, or `None`
    /// if it has none. Reads the whole file, for one such as a Matroska written to a pipe, which
    /// couldn't seek back to write its length.
    async fn video_end(&self, path: &str) -> Result<Option<i64>, ProbeError> {
        let packets = "packet=pts_time,duration_time:format=start_time";
        let args = [
            "-select_streams",
            "V:0",
            "-show_entries",
            packets,
            "-of",
            "csv=p=0",
        ];
        let output = self.run_ffprobe(&args, path, WHOLE_FILE_TIMEOUT).await?;
        Ok(parse_video_end(&String::from_utf8_lossy(&output)))
    }

    /// When each keyframe of the first video in the file at `path` falls, in milliseconds from
    /// its start, in order. Reads the whole file.
    pub(crate) async fn keyframes(&self, path: &str) -> Result<Vec<i64>, ProbeError> {
        // Packets rather than frames, so nothing is decoded. `V` skips cover art.
        let packets = "packet=pts_time,flags:format=start_time";
        let args = [
            "-select_streams",
            "V:0",
            "-show_entries",
            packets,
            "-of",
            "csv=p=0",
        ];
        let output = self.run_ffprobe(&args, path, WHOLE_FILE_TIMEOUT).await?;
        Ok(parse_keyframes(&String::from_utf8_lossy(&output)))
    }

    /// What ffprobe prints given `args` and the file at `path`, within `timeout`.
    async fn run_ffprobe(
        &self,
        args: &[&str],
        path: &str,
        timeout: Duration,
    ) -> Result<Vec<u8>, ProbeError> {
        let Ok(ffprobe) = &self.ffprobe else {
            return Err(ProbeError::Unavailable);
        };
        let output = tokio::process::Command::new(&ffprobe.path)
            .args(["-v", "error"])
            .args(args)
            .arg(path)
            // A probe given up on, or cancelled at shutdown, stops ffprobe too.
            .kill_on_drop(true)
            .output();
        let output = tokio::time::timeout(timeout, output).await??;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ProbeError::Failed(stderr.trim().to_owned()));
        }
        Ok(output.stdout)
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
    /// in `format`, which names both ffmpeg's encoder and its muxer, such as `webvtt` or `ass`.
    pub(crate) async fn subtitle(
        &self,
        path: &str,
        subtitle: usize,
        format: &str,
    ) -> Result<Vec<u8>, SubtitleError> {
        let Ok(ffmpeg) = &self.ffmpeg else {
            return Err(SubtitleError::Unavailable);
        };
        let output = tokio::process::Command::new(&ffmpeg.path)
            .args(["-v", "error", "-nostdin", "-i"])
            .arg(path)
            .args(["-map", &format!("0:s:{subtitle}")])
            .args(["-c:s", format, "-f", format, "-"])
            // A fetch given up on, or cancelled at shutdown, stops ffmpeg too.
            .kill_on_drop(true)
            .output();
        let output = tokio::time::timeout(SUBTITLE_TIMEOUT, output).await??;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SubtitleError::Failed(stderr.trim().to_owned()));
        }
        Ok(output.stdout)
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
        // ponytail: assumes ffmpeg has libass, as it does libx264 (#446). Each run reads the
        // whole file for the subtitle first, attached fonts and all; read it once into the
        // subtitle cache instead if seeking in a styled subtitle's Transcode is too slow.
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VideoConversion {
    /// The API it decodes, converts and encodes on, or `None` for software.
    pub(crate) hardware: Option<Accel>,
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
    /// ffmpeg's zscale, which [`Ffmpeg::has_zscale`] says it has.
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
    let mut codec = match video.hardware {
        // ponytail: assumes the Server's ffmpeg has libx264; check it as ADR 0026 says (#446).
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
    let on_gpu = (video.hardware)
        .filter(|_| video.tone_map != Some(ToneMapper::Software) && video.burn_in.is_none())
        .and_then(|accel| {
            let tone_map = video.tone_map == Some(ToneMapper::Hardware);
            Some((accel, accel.gpu_filters(tone_map, size)?))
        });
    if let Some((accel, filters)) = on_gpu {
        return VideoArgs {
            input: accel.input(true),
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
    let upload = video.hardware.map(|accel| accel.upload().to_owned());
    let after: Vec<String> = [scale, upload].into_iter().flatten().collect();
    VideoArgs {
        input: video
            .hardware
            .map(|accel| accel.input(false))
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
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    HlsRun(command)
}

/// The keyframes in what `ffprobe -show_entries packet=pts_time,flags:format=start_time -of
/// csv=p=0` printed, packets such as `2.002000,K__` and then the file's start time, in
/// milliseconds from that start, in order.
fn parse_keyframes(csv: &str) -> Vec<i64> {
    let start = file_start(csv);
    let mut keyframes: Vec<i64> = csv
        .lines()
        .filter_map(|line| {
            let (seconds, flags) = line.split_once(',')?;
            flags.contains('K').then(|| millis(seconds))?
        })
        .map(|keyframe| keyframe - start)
        .collect();
    // Packets come in decoding order.
    keyframes.sort_unstable();
    keyframes
}

/// When the video ends in what `ffprobe -show_entries
/// packet=pts_time,duration_time:format=start_time -of csv=p=0` printed, packets such as
/// `2.002000,0.041000` and then the file's start time: the latest end of a packet, in
/// milliseconds from that start, or `None` if there is none past it.
fn parse_video_end(csv: &str) -> Option<i64> {
    let start = file_start(csv);
    let ends = csv.lines().filter_map(|line| {
        let (pts, duration) = line.split_once(',')?;
        Some(millis(pts)? + millis(duration).unwrap_or_default())
    });
    // Packets come in decoding order, so the last isn't always the latest.
    ends.max()
        .map(|end| end - start)
        .filter(|&length| length > 0)
}

/// The file's start time in ffprobe's CSV of packets, the one line that isn't a packet, in
/// milliseconds.
fn file_start(csv: &str) -> i64 {
    // ffmpeg shifts a file that starts late, as an MPEG-TS often does, to start at 0.
    (csv.lines())
        .find(|line| !line.contains(','))
        .and_then(millis)
        .unwrap_or_default()
}

/// `ms` milliseconds as seconds, such as `"5.120"`, as ffmpeg takes a time.
fn seconds(ms: i64) -> String {
    format!("{}.{:03}", ms / 1000, ms % 1000)
}

/// Seconds such as `"5.120000"` in whole milliseconds, rounded down, or `None` for anything else,
/// such as `N/A` or a negative time.
fn millis(seconds: &str) -> Option<i64> {
    let duration = Duration::try_from_secs_f64(seconds.parse().ok()?).ok()?;
    i64::try_from(duration.as_millis()).ok()
}

/// What `ffprobe -print_format json -show_format -show_streams` printed.
#[derive(Deserialize)]
struct ProbeOutput {
    format: ProbedFormat,
    #[serde(default)]
    streams: Vec<ProbedStream>,
}

#[derive(Deserialize)]
struct ProbedFormat {
    format_name: String,
    /// In seconds, such as `"5.120000"`.
    duration: Option<String>,
    bit_rate: Option<String>,
}

#[derive(Deserialize)]
struct ProbedStream {
    codec_type: Option<String>,
    #[serde(default)]
    codec_name: String,
    width: Option<i64>,
    height: Option<i64>,
    /// Such as `"24000/1001"`, or `"0/0"` when unknown.
    avg_frame_rate: Option<String>,
    /// Such as `yuv420p10le`, which says its bit depth.
    pix_fmt: Option<String>,
    /// Such as `smpte2084` for HDR10's PQ or `arib-std-b67` for HLG.
    color_transfer: Option<String>,
    #[serde(default)]
    side_data_list: Vec<ProbedSideData>,
    channels: Option<i64>,
    #[serde(default)]
    disposition: HashMap<String, u8>,
    #[serde(default)]
    tags: HashMap<String, String>,
}

#[derive(Deserialize)]
struct ProbedSideData {
    #[serde(default)]
    side_data_type: String,
    #[serde(default)]
    dv_profile: i64,
    /// Its Base Layer: 1 is HDR10 and 6 Ultra HD Blu-ray's HDR10, while 0 is none, 2 SDR and 4
    /// HLG.
    dv_bl_signal_compatibility_id: Option<i64>,
}

/// The [`Probe`] ffprobe's JSON output describes.
fn parse_probe(json: &[u8]) -> Result<Probe, serde_json::Error> {
    let output: ProbeOutput = serde_json::from_slice(json)?;
    let format = output.format;
    Ok(Probe {
        container: format.format_name,
        // A playlist can't be cut from a length of 0, so it reads as unknown.
        duration_ms: (format.duration.as_deref())
            .and_then(millis)
            .filter(|&ms| ms > 0),
        bitrate: format.bit_rate.and_then(|bitrate| bitrate.parse().ok()),
        tracks: output.streams.into_iter().filter_map(track).collect(),
    })
}

/// `stream` as a Track, or `None` for a stream that isn't one, such as cover art or a data
/// stream.
fn track(mut stream: ProbedStream) -> Option<Track> {
    let flag = |name: &str| stream.disposition.get(name).is_some_and(|&set| set == 1);
    let kind = match stream.codec_type.as_deref()? {
        // Cover art is a one-picture video stream.
        "video" if flag("attached_pic") => return None,
        "video" => TrackKind::Video {
            width: stream.width,
            height: stream.height,
            frame_rate: stream.avg_frame_rate.as_deref().and_then(frame_rate),
            bit_depth: stream.pix_fmt.as_deref().and_then(bit_depth),
            hdr: hdr(&stream),
        },
        "audio" => TrackKind::Audio {
            channels: stream.channels,
        },
        "subtitle" => TrackKind::Subtitle {
            image: IMAGE_SUBTITLES.contains(&stream.codec_name.as_str()),
        },
        _ => return None,
    };
    let (default, forced, sdh) = (flag("default"), flag("forced"), flag("hearing_impaired"));
    Some(Track {
        codec: stream.codec_name,
        language: stream.tags.remove("language"),
        title: stream.tags.remove("title"),
        default,
        forced,
        sdh,
        kind,
    })
}

/// Frames per second from a fraction such as `24000/1001`, or `None` for `0/0`.
fn frame_rate(fraction: &str) -> Option<f64> {
    let (numerator, denominator) = fraction.split_once('/')?;
    let (numerator, denominator): (f64, f64) = (numerator.parse().ok()?, denominator.parse().ok()?);
    (denominator > 0.0).then(|| numerator / denominator)
}

/// Bits per sample of a planar pixel format such as `yuv420p10le` or `p210le`, or `None` for one
/// such as `rgb24` or `nv12`, whose number is bits per pixel.
fn bit_depth(pix_fmt: &str) -> Option<i64> {
    let format = ["le", "be"]
        .into_iter()
        .find_map(|endian| pix_fmt.strip_suffix(endian))
        .unwrap_or(pix_fmt);
    // A planar format names its bits after its `p`, and has 8 when it names none. A semi-planar one
    // such as `p210` puts its chroma subsampling first, so only the last two digits are bits.
    match format.rsplit_once('p')?.1 {
        "" => Some(8),
        bits => bits.get(bits.len().saturating_sub(2)..)?.parse().ok(),
    }
}

/// How a video `stream`'s picture is HDR, or `None` for SDR.
fn hdr(stream: &ProbedStream) -> Option<Hdr> {
    let dolby_vision = stream
        .side_data_list
        .iter()
        .find(|data| data.side_data_type == "DOVI configuration record");
    if let Some(record) = dolby_vision {
        return Some(Hdr::DolbyVision {
            profile: record.dv_profile,
            base_layer: match record.dv_bl_signal_compatibility_id {
                Some(1 | 6) => Some(BaseLayer::Hdr10),
                Some(2) => Some(BaseLayer::Sdr),
                Some(4) => Some(BaseLayer::Hlg),
                _ => None,
            },
        });
    }
    match stream.color_transfer.as_deref()? {
        "smpte2084" => Some(Hdr::Hdr10),
        "arib-std-b67" => Some(Hdr::Hlg),
        _ => None,
    }
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
mod tests {
    use std::{
        ffi::OsStr,
        fs,
        os::unix::fs::PermissionsExt,
        path::Path,
        sync::{PoisonError, RwLock},
    };

    use super::{Accel, Ffmpeg, FoundProgram, Hardware};
    use crate::config::Config;

    /// Held to write a fake, and shared to run them. A child forked while another test has a fake
    /// open for writing keeps it open until it execs, and running that fake then fails with
    /// "Text file busy".
    static WRITING_FAKES: RwLock<()> = RwLock::new(());

    /// Writes a fake `name` into `dir` that prints `version_line` as `-version` does.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    fn fake(dir: &Path, name: &str, version_line: &str) {
        let _writing = WRITING_FAKES
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        let path = dir.join(name);
        fs::write(&path, format!("#!/bin/sh\necho '{version_line}'\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    /// [`Ffmpeg::find`], while no fake is being written.
    fn find(config: &Config, path: &OsStr) -> Ffmpeg {
        let _running = WRITING_FAKES.read().unwrap_or_else(PoisonError::into_inner);
        Ffmpeg::find(config, path)
    }

    /// Looks only on `path`, with no paths configured.
    fn find_on(path: &OsStr) -> Ffmpeg {
        find(&Config::default(), path)
    }

    #[test]
    fn both_on_path_are_found_with_their_path_and_version() {
        let empty = tempfile::tempdir().unwrap();
        let bin = tempfile::tempdir().unwrap();
        fake(
            bin.path(),
            "ffmpeg",
            "ffmpeg version 7.1.1 Copyright (c) 2000-2025",
        );
        fake(
            bin.path(),
            "ffprobe",
            "ffprobe version n7.0 Copyright (c) 2007-2024",
        );
        let path = std::env::join_paths([empty.path(), bin.path()]).unwrap();

        let found = find_on(&path);

        assert_eq!(
            found.ffmpeg,
            Ok(FoundProgram {
                path: bin.path().join("ffmpeg"),
                version: "7.1.1".to_owned(),
            })
        );
        assert_eq!(
            found.ffprobe,
            Ok(FoundProgram {
                path: bin.path().join("ffprobe"),
                version: "n7.0".to_owned(),
            })
        );
        assert_eq!(found.direct_play_only_because(), None);
    }

    #[test]
    fn with_neither_on_path_the_server_can_only_direct_play() {
        let empty = tempfile::tempdir().unwrap();

        let found = find_on(empty.path().as_os_str());

        assert_eq!(found.ffmpeg, Err("ffmpeg wasn't found on PATH".to_owned()));
        assert_eq!(
            found.direct_play_only_because(),
            Some("ffmpeg wasn't found on PATH and ffprobe wasn't found on PATH".to_owned())
        );
    }

    #[test]
    fn one_that_doesnt_run_is_skipped_for_a_later_one_on_path() {
        let broken = tempfile::tempdir().unwrap();
        let bin = tempfile::tempdir().unwrap();
        fake(broken.path(), "ffmpeg", "something else entirely");
        fake(bin.path(), "ffmpeg", "ffmpeg version 7.1.1");
        let path = std::env::join_paths([broken.path(), bin.path()]).unwrap();

        let found = find_on(&path);

        assert_eq!(
            found.ffmpeg,
            Ok(FoundProgram {
                path: bin.path().join("ffmpeg"),
                version: "7.1.1".to_owned(),
            })
        );
    }

    #[test]
    fn a_program_that_doesnt_say_its_ffmpeg_version_isnt_found() {
        let bin = tempfile::tempdir().unwrap();
        fake(bin.path(), "ffmpeg", "something else entirely");
        fake(bin.path(), "ffprobe", "ffprobe version 7.1.1");

        let found = find_on(bin.path().as_os_str());

        assert_eq!(found.ffmpeg, Err("ffmpeg wasn't found on PATH".to_owned()));
        assert_eq!(
            found.direct_play_only_because(),
            Some("ffmpeg wasn't found on PATH".to_owned())
        );
    }

    #[test]
    fn a_configured_path_is_used_instead_of_path() {
        let on_path = tempfile::tempdir().unwrap();
        let configured = tempfile::tempdir().unwrap();
        fake(on_path.path(), "ffmpeg", "ffmpeg version 7.1.1");
        fake(configured.path(), "my-ffmpeg", "ffmpeg version 6.1");
        let config = Config {
            ffmpeg: Some(configured.path().join("my-ffmpeg")),
            ..Config::default()
        };

        let found = find(&config, on_path.path().as_os_str());

        assert_eq!(
            found.ffmpeg,
            Ok(FoundProgram {
                path: configured.path().join("my-ffmpeg"),
                version: "6.1".to_owned(),
            })
        );
    }

    #[test]
    fn a_configured_path_that_is_missing_or_wont_run_is_reported_with_that_path() {
        let configured = tempfile::tempdir().unwrap();
        fake(configured.path(), "ffprobe", "something else entirely");
        let config = Config {
            ffmpeg: Some("/opt/ffmpeg".into()),
            ffprobe: Some(configured.path().join("ffprobe")),
            ..Config::default()
        };

        let found = find(&config, configured.path().as_os_str());

        assert_eq!(
            found.ffmpeg,
            Err("ffmpeg wasn't found at the configured path /opt/ffmpeg".to_owned())
        );
        assert_eq!(
            found.ffprobe,
            Err(format!(
                "ffprobe at the configured path {} won't run or isn't ffprobe",
                configured.path().join("ffprobe").display()
            ))
        );
    }

    #[test]
    fn a_relative_configured_path_is_from_the_working_directory_not_path() {
        let on_path = tempfile::tempdir().unwrap();
        fake(on_path.path(), "ffmpeg", "ffmpeg version 7.1.1");
        let config = Config {
            ffmpeg: Some("ffmpeg".into()),
            ..Config::default()
        };

        let found = find(&config, on_path.path().as_os_str());

        assert_eq!(
            found.ffmpeg,
            Err(format!(
                "ffmpeg wasn't found at the configured path {}",
                std::env::current_dir().unwrap().join("ffmpeg").display()
            ))
        );
    }

    #[test]
    fn a_version_below_the_minimum_is_too_old() {
        let bin = tempfile::tempdir().unwrap();
        fake(bin.path(), "ffmpeg", "ffmpeg version 4.3.1-static");
        fake(bin.path(), "ffprobe", "ffprobe version n4.4");

        let found = find_on(bin.path().as_os_str());

        assert_eq!(
            found.ffmpeg,
            Err(format!(
                "ffmpeg 4.3.1-static at {} is older than 4.4, the oldest this Server supports",
                bin.path().join("ffmpeg").display()
            ))
        );
        assert!(found.ffprobe.is_ok(), "{:?}", found.ffprobe);
    }

    #[test]
    fn a_distribution_or_git_build_is_new_enough() {
        let bin = tempfile::tempdir().unwrap();
        fake(
            bin.path(),
            "ffmpeg",
            "ffmpeg version 4.4.2-0ubuntu0.22.04.1",
        );
        fake(bin.path(), "ffprobe", "ffprobe version N-113021-g5b3fc5d");

        let found = find_on(bin.path().as_os_str());

        assert_eq!(found.direct_play_only_because(), None);
    }

    #[tokio::test]
    #[expect(
        clippy::await_holding_lock,
        reason = "the fakes' lock is held while ffmpeg runs, which forks; a test waits on no other"
    )]
    async fn without_ffmpeg_or_zscale_among_its_filters_the_server_cant_tone_map() {
        // Prints its arguments, not a filter.
        let echo = Ffmpeg {
            ffmpeg: Ok(FoundProgram {
                path: "/bin/echo".into(),
                version: "7.1.1".to_owned(),
            }),
            ..Ffmpeg::default()
        };

        let _running = WRITING_FAKES.read().unwrap_or_else(PoisonError::into_inner);
        assert_eq!(echo.has_zscale().await, Ok(false));
        assert_eq!(Ffmpeg::default().has_zscale().await, Ok(false));
    }

    /// Writes a fake ffmpeg into `dir` that runs `script`, a shell `case` over its arguments, and
    /// returns it as the Server's ffmpeg.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    fn fake_ffmpeg(dir: &Path, script: &str) -> Ffmpeg {
        let path = dir.join("ffmpeg");
        {
            let _writing = WRITING_FAKES
                .write()
                .unwrap_or_else(PoisonError::into_inner);
            fs::write(
                &path,
                format!("#!/bin/sh\ncase \"$*\" in\n{script}\nesac\n"),
            )
            .unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        Ffmpeg {
            ffmpeg: Ok(FoundProgram {
                path,
                version: "7.1.1".to_owned(),
            }),
            ..Ffmpeg::default()
        }
    }

    #[tokio::test]
    #[expect(
        clippy::await_holding_lock,
        reason = "the fakes' lock is held while ffmpeg runs, which forks; a test waits on no other"
    )]
    async fn only_hardware_ffmpeg_lists_and_that_passes_its_test_encode_is_found() {
        let bin = tempfile::tempdir().unwrap();
        let ffmpeg = fake_ffmpeg(
            bin.path(),
            // NVENC is listed but fails, as without NVIDIA's driver; QSV isn't listed at all.
            "*-encoders*) echo ' V....D h264_vaapi  H.264/AVC (VAAPI)'; \
                          echo ' V....D h264_nvenc  NVIDIA NVENC H.264 encoder' ;;
             *tonemap_vaapi*) exit 1 ;;
             *h264_vaapi*) exit 0 ;;
             *) exit 1 ;;",
        );

        let found = {
            let _running = WRITING_FAKES.read().unwrap_or_else(PoisonError::into_inner);
            ffmpeg.find_hardware().await
        };

        assert_eq!(
            found,
            [Hardware {
                accel: Accel::Vaapi,
                tone_maps: false,
            }]
        );
    }

    #[tokio::test]
    #[expect(
        clippy::await_holding_lock,
        reason = "the fakes' lock is held while ffmpeg runs, which forks; a test waits on no other"
    )]
    async fn hardware_that_passes_its_tone_map_test_tone_maps() {
        let bin = tempfile::tempdir().unwrap();
        let ffmpeg = fake_ffmpeg(
            bin.path(),
            "*-encoders*) echo ' V..... h264_qsv  H.264 (Intel Quick Sync Video acceleration)' ;;
             *h264_qsv*) exit 0 ;;
             *) exit 1 ;;",
        );

        let found = {
            let _running = WRITING_FAKES.read().unwrap_or_else(PoisonError::into_inner);
            ffmpeg.find_hardware().await
        };

        assert_eq!(
            found,
            [Hardware {
                accel: Accel::Qsv,
                tone_maps: true,
            }]
        );
    }

    #[tokio::test]
    async fn without_ffmpeg_no_hardware_is_found() {
        assert_eq!(Ffmpeg::default().find_hardware().await, []);
    }
}

#[cfg(test)]
mod hls_run_tests {
    use std::path::Path;

    use super::{
        BurnIn, Ffmpeg, FoundProgram, FromCut, HlsRun, Keyframe, Remux, RemuxAudio, ToneMapper,
        Transcode, TranscodeAudio, VideoConversion, VideoLimit,
    };

    /// An ffmpeg at `version` that isn't on this machine, which building arguments never runs.
    fn ffmpeg(version: &str) -> Ffmpeg {
        Ffmpeg {
            ffmpeg: Ok(FoundProgram {
                path: "ffmpeg".into(),
                version: version.to_owned(),
            }),
            ..Ffmpeg::default()
        }
    }

    fn args(run: &HlsRun) -> Vec<String> {
        (run.0.as_std().get_args())
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    /// A Transcode's video converted in software, with nothing else asked of it.
    const SOFTWARE: VideoConversion = VideoConversion {
        hardware: None,
        tone_map: None,
        limit: None,
        burn_in: None,
        in_band_headers: false,
    };

    /// The arguments for a run of `remux` of `alien.mkv` from `from`, with ffmpeg 7.1.1.
    fn remux_args(remux: Remux, from: Keyframe) -> Vec<String> {
        let run = ffmpeg("7.1.1").remux("alien.mkv", &remux, Path::new("plan"), from);
        args(&run.unwrap())
    }

    /// The arguments for a run of a silent Transcode of `alien.mkv` whose video `video` converts,
    /// from its first cut of three, with ffmpeg 7.1.1.
    fn transcode_args(video: VideoConversion) -> Vec<String> {
        let transcode = Transcode { audio: None, video };
        let from = FromCut {
            n: 0,
            cuts: &[0, 6000, 12_000],
        };
        let run = ffmpeg("7.1.1").transcode("alien.mkv", &transcode, Path::new("plan"), from);
        args(&run.unwrap())
    }

    /// What `args` gives the first `option`, if anything.
    fn value_of<'a>(args: &'a [String], option: &str) -> Option<&'a str> {
        let at = args.iter().position(|arg| arg == option)?;
        args.get(at + 1).map(String::as_str)
    }

    const fn keyframe(n: usize, ms: i64, previous_ms: Option<i64>) -> Keyframe {
        Keyframe { n, ms, previous_ms }
    }

    /// `name` in the folder the tests write to, as ffmpeg is given it on this platform.
    fn in_plan(name: &str) -> String {
        Path::new("plan").join(name).display().to_string()
    }

    #[test]
    fn a_remux_copies_the_video_and_converts_the_audio_from_its_keyframe_on() {
        let remux = Remux {
            audio: Some(RemuxAudio {
                track: 1,
                aac_channels: Some(2),
            }),
        };

        let args = remux_args(remux, keyframe(3, 6000, Some(4000)));

        let expected = [
            "-v",
            "error",
            "-nostdin",
            "-ss",
            "5.500",
            "-itsoffset",
            "1.000",
            "-i",
            "alien.mkv",
            "-copyts",
            "-start_at_zero",
            "-ss",
            "6.500",
            "-output_ts_offset",
            "6.500",
            "-map",
            "0:V:0",
            "-map",
            "0:a:1",
            "-c",
            "copy",
            "-c:a",
            "aac",
            "-ac",
            "2",
            "-f",
            "hls",
            "-hls_segment_type",
            "fmp4",
            "-hls_time",
            "0.001",
            "-hls_playlist_type",
            "vod",
            "-hls_flags",
            "temp_file",
            "-hls_fmp4_init_filename",
            "init-3.mp4",
            "-hls_segment_options",
            "movflags=+frag_discont",
            "-start_number",
            "3",
            "-hls_segment_filename",
            &in_plan("%d.m4s"),
            &in_plan("ffmpeg.m3u8"),
        ];
        assert_eq!(args, expected);
    }

    #[test]
    fn a_transcode_converts_the_video_and_audio_from_its_cut_on() {
        let transcode = Transcode {
            audio: Some(TranscodeAudio {
                track: 0,
                aac_channels: 6,
            }),
            video: SOFTWARE,
        };
        let from = FromCut {
            n: 2,
            cuts: &[12_000, 18_000],
        };

        let run = ffmpeg("7.1.1").transcode("alien.mkv", &transcode, Path::new("plan"), from);
        let args = args(&run.unwrap());

        let expected = [
            "-v",
            "error",
            "-nostdin",
            "-ss",
            "12.000",
            "-i",
            "alien.mkv",
            "-copyts",
            "-start_at_zero",
            "-map",
            "0:V:0",
            "-map",
            "0:a:0",
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-pix_fmt",
            "yuv420p",
            "-x264-params",
            "keyint=infinite:scenecut=0",
            "-force_key_frames",
            "12.000,18.000",
            "-c:a",
            "aac",
            "-ac",
            "6",
            "-output_ts_offset",
            "1.000",
            "-f",
            "hls",
            "-hls_segment_type",
            "fmp4",
            "-hls_time",
            "0.001",
            "-hls_playlist_type",
            "vod",
            "-hls_flags",
            "temp_file",
            "-hls_fmp4_init_filename",
            "init-2.mp4",
            "-hls_segment_options",
            "movflags=+frag_discont",
            "-start_number",
            "2",
            "-hls_segment_filename",
            &in_plan("%d.m4s"),
            &in_plan("ffmpeg.m3u8"),
        ];
        assert_eq!(args, expected);
    }

    #[test]
    fn a_remux_seeks_halfway_back_to_the_keyframe_before_and_at_most_half_a_second() {
        let input_seek = |from| {
            let args = remux_args(Remux { audio: None }, from);
            let input = args.iter().position(|arg| arg == "-i").unwrap();
            value_of(&args[..input], "-ss").map(str::to_owned)
        };

        assert_eq!(
            input_seek(keyframe(3, 6000, Some(5600))).as_deref(),
            Some("5.800")
        );
        assert_eq!(
            input_seek(keyframe(3, 6000, Some(2000))).as_deref(),
            Some("5.500")
        );
        assert_eq!(
            input_seek(keyframe(0, 400, None)),
            None,
            "from the file's start"
        );
    }

    #[test]
    fn hdr_is_tone_mapped_to_sdr_in_software_with_zscale() {
        let tone_mapped = VideoConversion {
            tone_map: Some(ToneMapper::Software),
            ..SOFTWARE
        };

        let args = transcode_args(tone_mapped);

        let filters = "setparams=color_primaries=bt2020:colorspace=bt2020nc,\
                       zscale=t=linear:npl=100,format=gbrpf32le,zscale=p=bt709,\
                       tonemap=tonemap=hable:desat=0,zscale=t=bt709:m=bt709:r=tv,format=yuv420p";
        assert_eq!(value_of(&args, "-vf"), Some(filters));
    }

    #[test]
    fn a_playback_limit_scales_the_picture_and_caps_the_bitrate() {
        let limited = VideoConversion {
            limit: Some(VideoLimit {
                size: Some((1280, 720)),
                bits_per_second: 4_000_000,
            }),
            ..SOFTWARE
        };

        let args = transcode_args(limited);

        assert_eq!(value_of(&args, "-vf"), Some("scale=1280:720"));
        assert_eq!(value_of(&args, "-b:v"), Some("4000000"));
        assert_eq!(value_of(&args, "-maxrate"), Some("4000000"));
        assert_eq!(value_of(&args, "-bufsize"), Some("8000000"));
    }

    #[test]
    fn an_image_subtitle_is_burned_in_fitted_and_centred_over_the_picture() {
        let burned = VideoConversion {
            burn_in: Some(BurnIn::Image {
                subtitle: 1,
                picture: Some((1920, 1080)),
            }),
            ..SOFTWARE
        };

        let args = transcode_args(burned);

        let graph = "[0:V:0]null[picture];\
                     [0:s:1]scale=1920:1080:force_original_aspect_ratio=decrease[subtitle];\
                     [picture][subtitle]overlay=(W-w)/2:(H-h)/2[video]";
        assert_eq!(value_of(&args, "-filter_complex"), Some(graph));
        assert_eq!(value_of(&args, "-map"), Some("[video]"));
    }

    #[test]
    fn a_text_subtitle_is_burned_in_from_the_file_by_libass() {
        let burned = VideoConversion {
            burn_in: Some(BurnIn::Text { subtitle: 2 }),
            ..SOFTWARE
        };

        let args = transcode_args(burned);

        assert_eq!(
            value_of(&args, "-vf"),
            Some(r"subtitles=filename=\'alien.mkv\':si=2")
        );
    }

    #[test]
    fn audio_is_converted_to_aac_with_its_channels_or_else_copied() {
        let audio = |aac_channels| Remux {
            audio: Some(RemuxAudio {
                track: 0,
                aac_channels,
            }),
        };
        let first = keyframe(0, 0, None);

        let converted = remux_args(audio(Some(6)), first);
        let copied = remux_args(audio(None), first);

        assert_eq!(value_of(&converted, "-c:a"), Some("aac"));
        assert_eq!(value_of(&converted, "-ac"), Some("6"));
        assert_eq!(value_of(&copied, "-c:a"), None);
        assert_eq!(value_of(&copied, "-ac"), None);
    }

    #[test]
    fn a_transcode_forces_a_keyframe_at_each_cut_from_its_first() {
        let args = transcode_args(SOFTWARE);

        assert_eq!(
            value_of(&args, "-force_key_frames"),
            Some("0.000,6.000,12.000")
        );
    }

    #[test]
    fn ffmpeg_before_5_0_is_given_segment_options_by_their_old_name() {
        let segment_options = |version| {
            let silent = Remux { audio: None };
            let run = ffmpeg(version).remux(
                "alien.mkv",
                &silent,
                Path::new("plan"),
                keyframe(0, 0, None),
            );
            let args = args(&run.unwrap());
            ["-hls_ts_options", "-hls_segment_options"]
                .into_iter()
                .filter(|option| value_of(&args, option) == Some("movflags=+frag_discont"))
                .collect::<Vec<_>>()
        };

        assert_eq!(segment_options("4.4.2"), ["-hls_ts_options"]);
        assert_eq!(segment_options("n5.0"), ["-hls_segment_options"]);
        assert_eq!(
            segment_options("N-113021-g5b3fc5d"),
            ["-hls_segment_options"]
        );
    }
}

#[cfg(test)]
mod probe_output_tests {
    use super::parse_probe;
    use crate::tracks::{BaseLayer, Hdr, Probe, Track, TrackKind};

    /// Cut down from ffprobe 7.0's output for a Blu-ray remux with cover art.
    const BLU_RAY_REMUX: &str = r#"{
        "streams": [
            {"codec_name": "hevc", "codec_type": "video", "width": 3840, "height": 1600,
             "avg_frame_rate": "24000/1001", "disposition": {"default": 1, "forced": 0}},
            {"codec_name": "truehd", "codec_type": "audio", "channels": 8,
             "disposition": {"default": 1, "forced": 0},
             "tags": {"language": "eng", "title": "TrueHD Atmos 7.1"}},
            {"codec_name": "hdmv_pgs_subtitle", "codec_type": "subtitle",
             "disposition": {"default": 0, "forced": 1}, "tags": {"language": "fre"}},
            {"codec_name": "subrip", "codec_type": "subtitle",
             "disposition": {"default": 0, "forced": 0, "hearing_impaired": 1},
             "tags": {"language": "eng"}},
            {"codec_name": "mjpeg", "codec_type": "video", "width": 600, "height": 900,
             "avg_frame_rate": "0/0", "disposition": {"attached_pic": 1}},
            {"codec_type": "attachment", "tags": {"filename": "font.ttf"}}
        ],
        "format": {"format_name": "matroska,webm", "duration": "7299.4565", "bit_rate": "61734512"}
    }"#;

    fn track(codec: &str, language: Option<&str>, default: bool, kind: TrackKind) -> Track {
        Track {
            codec: codec.to_owned(),
            language: language.map(str::to_owned),
            title: None,
            default,
            forced: false,
            sdh: false,
            kind,
        }
    }

    #[test]
    fn ffprobe_output_reads_as_tracks_without_cover_art_or_attachments() {
        let probe = parse_probe(BLU_RAY_REMUX.as_bytes()).unwrap();

        let Some(TrackKind::Video { frame_rate, .. }) = probe.tracks.first().map(|t| &t.kind)
        else {
            panic!("no video Track first: {probe:?}");
        };
        assert!(frame_rate.is_some_and(|fps| (fps - 23.976).abs() < 0.001));
        let ntsc = frame_rate.unwrap();
        assert_eq!(
            probe,
            Probe {
                container: "matroska,webm".to_owned(),
                duration_ms: Some(7_299_456),
                bitrate: Some(61_734_512),
                tracks: vec![
                    track(
                        "hevc",
                        None,
                        true,
                        TrackKind::Video {
                            width: Some(3840),
                            height: Some(1600),
                            frame_rate: Some(ntsc),
                            bit_depth: None,
                            hdr: None,
                        },
                    ),
                    Track {
                        title: Some("TrueHD Atmos 7.1".to_owned()),
                        ..track(
                            "truehd",
                            Some("eng"),
                            true,
                            TrackKind::Audio { channels: Some(8) },
                        )
                    },
                    Track {
                        forced: true,
                        ..track(
                            "hdmv_pgs_subtitle",
                            Some("fre"),
                            false,
                            TrackKind::Subtitle { image: true },
                        )
                    },
                    Track {
                        sdh: true,
                        ..track(
                            "subrip",
                            Some("eng"),
                            false,
                            TrackKind::Subtitle { image: false },
                        )
                    },
                ],
            }
        );
    }

    /// ffprobe 7.0's output for a Dolby Vision file of `profile`, whose base layer's
    /// `dv_bl_signal_compatibility_id` is `compatibility` and transfer is `transfer`, cut down to
    /// its video stream.
    fn dolby_vision(profile: u8, compatibility: u8, transfer: &str) -> String {
        format!(
            r#"{{
            "streams": [
                {{"codec_name": "hevc", "codec_type": "video", "width": 3840, "height": 2160,
                 "pix_fmt": "yuv420p10le", "color_transfer": "{transfer}",
                 "side_data_list": [
                    {{"side_data_type": "DOVI configuration record", "dv_version_major": 1,
                     "dv_version_minor": 0, "dv_profile": {profile}, "dv_level": 6,
                     "rpu_present_flag": 1, "el_present_flag": 0, "bl_present_flag": 1,
                     "dv_bl_signal_compatibility_id": {compatibility}}}
                 ]}}
            ],
            "format": {{"format_name": "matroska,webm"}}
        }}"#
        )
    }

    fn video_of(json: &str) -> TrackKind {
        let probe = parse_probe(json.as_bytes()).unwrap();
        probe.tracks.into_iter().next().unwrap().kind
    }

    fn hdr_of(json: &str) -> Option<Hdr> {
        let TrackKind::Video { hdr, .. } = video_of(json) else {
            panic!("not a video Track");
        };
        hdr
    }

    #[test]
    fn dolby_vision_is_read_with_its_profile_and_base_layer() {
        let dolby_vision_with = |profile, base_layer| {
            Some(Hdr::DolbyVision {
                profile,
                base_layer,
            })
        };
        assert_eq!(
            hdr_of(&dolby_vision(8, 1, "smpte2084")),
            dolby_vision_with(8, Some(BaseLayer::Hdr10))
        );
        // Ultra HD Blu-ray's, with an enhancement layer over HDR10.
        assert_eq!(
            hdr_of(&dolby_vision(7, 6, "smpte2084")),
            dolby_vision_with(7, Some(BaseLayer::Hdr10))
        );
        // Streaming's, which only a Dolby Vision player shows right.
        assert_eq!(
            hdr_of(&dolby_vision(5, 0, "unknown")),
            dolby_vision_with(5, None)
        );
        // Broadcast's, over HLG.
        assert_eq!(
            hdr_of(&dolby_vision(8, 4, "arib-std-b67")),
            dolby_vision_with(8, Some(BaseLayer::Hlg))
        );
        // Over SDR.
        assert_eq!(
            hdr_of(&dolby_vision(8, 2, "bt709")),
            dolby_vision_with(8, Some(BaseLayer::Sdr))
        );
        // A reserved id says nothing a player without Dolby Vision could show.
        assert_eq!(
            hdr_of(&dolby_vision(8, 3, "unknown")),
            dolby_vision_with(8, None)
        );
    }

    #[test]
    fn bit_depth_is_read_from_the_pixel_format() {
        let depth_of = |pix_fmt: &str| {
            let json = format!(
                r#"{{"streams": [{{"codec_name": "hevc", "codec_type": "video",
                    "pix_fmt": "{pix_fmt}"}}], "format": {{"format_name": "matroska,webm"}}}}"#
            );
            let TrackKind::Video { bit_depth, .. } = video_of(&json) else {
                panic!("not a video Track");
            };
            bit_depth
        };

        assert_eq!(depth_of("yuv420p"), Some(8));
        assert_eq!(depth_of("yuvj420p"), Some(8));
        assert_eq!(depth_of("yuv420p10le"), Some(10));
        assert_eq!(depth_of("yuv422p12be"), Some(12));
        assert_eq!(depth_of("p010le"), Some(10));
        assert_eq!(depth_of("p210le"), Some(10));
        assert_eq!(depth_of("p416le"), Some(16));
        // These name bits per pixel, not per sample.
        assert_eq!(depth_of("rgb24"), None);
        assert_eq!(depth_of("nv12"), None);
    }

    #[test]
    fn a_path_in_a_filter_option_survives_both_of_ffmpegs_unescapings() {
        let path = r"/films/it's: a [test], x; y\z.mkv";

        assert_eq!(
            super::filter_value(path),
            r"\'/films/it\'\\\'\'s: a \[test\]\, x\; y\\z.mkv\'"
        );
    }

    #[test]
    fn keyframes_are_the_packets_flagged_k_in_time_order() {
        let csv = "0.000000,K__\n0.083000,___\n2.002000,K_D\n1.001000,K__\nN/A,K__\n";

        assert_eq!(super::parse_keyframes(csv), [0, 1001, 2002]);
    }

    #[test]
    fn a_missing_or_zero_format_duration_reads_as_unknown() {
        let with_format = |format: &str| {
            let json = format!(r#"{{"format": {{"format_name": "matroska,webm"{format}}}}}"#);
            parse_probe(json.as_bytes()).unwrap().duration_ms
        };

        assert_eq!(with_format(""), None);
        assert_eq!(with_format(r#", "duration": "0.000000""#), None);
        assert_eq!(with_format(r#", "duration": "2.000000""#), Some(2000));
    }

    #[test]
    fn video_ends_where_its_latest_packet_does_from_the_files_start() {
        // In decoding order, with a packet whose duration isn't known.
        let csv = "1.523000,0.041000\n3.523000,0.041000\n3.481000,N/A\nN/A,0.041000\n1.500000\n";

        assert_eq!(super::parse_video_end(csv), Some(2064));
        assert_eq!(super::parse_video_end("0.000000\n"), None);
    }

    /// ffmpeg shifts a file that starts late to start at 0, and cuts its segments from there.
    #[test]
    fn keyframes_are_timed_from_the_files_start() {
        let csv = "1.523000,K__\n4.023000,K__\n1.500000\n";

        assert_eq!(super::parse_keyframes(csv), [23, 2523]);
    }
}

#[cfg(test)]
mod hls_tests {
    use std::{fs, path::Path};

    use super::{
        Ffmpeg, FromCut, Keyframe, Remux, RemuxAudio, ToneMapper, Transcode, TranscodeAudio,
        VideoConversion, VideoLimit, write_h264_with_5_1, write_hevc_with_5_1, write_test_media,
    };
    use crate::config::Config;

    /// The ffmpeg and ffprobe on `PATH`, which these tests need.
    fn system_ffmpeg() -> Ffmpeg {
        let ffmpeg = Ffmpeg::find(&Config::default(), &std::env::var_os("PATH").unwrap());
        assert!(
            ffmpeg.can_remux(),
            "this test needs ffmpeg and ffprobe on PATH"
        );
        ffmpeg
    }

    /// Remuxes the file at `path` into `dir` from `from`, until ffmpeg is done.
    async fn remux(ffmpeg: &Ffmpeg, path: &Path, dir: &Path, from: Keyframe) {
        fs::create_dir_all(dir).unwrap();
        let path = path.to_str().unwrap();
        let remux = Remux {
            audio: Some(RemuxAudio {
                track: 0,
                aac_channels: None,
            }),
        };
        let output = ffmpeg.remux(path, &remux, dir, from).unwrap();
        let output = output.spawn().unwrap().wait_with_output().await.unwrap();
        assert!(output.status.success(), "{output:?}");
    }

    /// When the first fragment of an fMP4 segment starts, in its track's own units.
    fn starts_at(segment: &[u8]) -> u64 {
        let at = segment.windows(4).position(|name| name == b"tfdt").unwrap() + 4;
        let (version, time) = (segment[at], &segment[at + 4..]);
        if version == 1 {
            u64::from_be_bytes(time[..8].try_into().unwrap())
        } else {
            u32::from_be_bytes(time[..4].try_into().unwrap()).into()
        }
    }

    /// Transcodes the file at `path` into `dir`, cut every 6 seconds of its 20, from cut `n`,
    /// until ffmpeg is done.
    async fn transcode(ffmpeg: &Ffmpeg, path: &Path, dir: &Path, n: usize) {
        fs::create_dir_all(dir).unwrap();
        let cuts = [0, 6000, 12_000, 18_000];
        let transcode = Transcode {
            audio: Some(TranscodeAudio {
                track: 0,
                aac_channels: 2,
            }),
            video: VideoConversion {
                hardware: None,
                tone_map: None,
                limit: None,
                burn_in: None,
                in_band_headers: false,
            },
        };
        let from = FromCut {
            n,
            cuts: &cuts[n..],
        };
        let output = ffmpeg
            .transcode(path.to_str().unwrap(), &transcode, dir, from)
            .unwrap();
        let output = output.spawn().unwrap().wait_with_output().await.unwrap();
        assert!(output.status.success(), "{output:?}");
    }

    #[tokio::test]
    async fn hdr10_tagged_only_by_its_transfer_is_tone_mapped() {
        let ffmpeg = system_ffmpeg();
        let work = tempfile::tempdir().unwrap();
        let file = work.path().join("hdr10.mkv");
        write_test_media(&file, |command| {
            command
                .args([
                    "-f",
                    "lavfi",
                    "-i",
                    // The frames carry the transfer, since ffmpeg 9 drops `-color_trc`.
                    "testsrc=size=320x240:rate=24,setparams=color_trc=smpte2084",
                    "-t",
                    "1",
                ])
                .args([
                    "-c:v",
                    "libx265",
                    "-preset",
                    "ultrafast",
                    "-pix_fmt",
                    "yuv420p10le",
                ])
                .args(["-x265-params", "log-level=error"])
                .args(["-f", "matroska"])
        })
        .unwrap();
        let dir = work.path().join("transcoded");
        fs::create_dir_all(&dir).unwrap();
        let tone_mapped = Transcode {
            audio: None,
            video: VideoConversion {
                hardware: None,
                tone_map: Some(ToneMapper::Software),
                limit: None,
                burn_in: None,
                in_band_headers: false,
            },
        };
        let from = FromCut { n: 0, cuts: &[0] };

        let output = ffmpeg
            .transcode(file.to_str().unwrap(), &tone_mapped, &dir, from)
            .unwrap();

        let output = output.spawn().unwrap().wait_with_output().await.unwrap();
        assert!(output.status.success(), "{output:?}");
    }

    /// Run by hand where there is a GPU: `cargo test -p nama-server --lib -- --ignored hardware`.
    #[tokio::test]
    #[ignore = "needs a GPU that ffmpeg can use"]
    async fn hardware_found_on_this_host_transcodes_cut_at_every_cut_and_tone_maps_hdr10() {
        let ffmpeg = system_ffmpeg();
        let found = ffmpeg.find_hardware().await;
        let hardware = *found.first().expect("no hardware passed its test encode");
        let work = tempfile::tempdir().unwrap();
        let file = work.path().join("hdr10.mkv");
        write_test_media(&file, |command| {
            command
                .args(["-f", "lavfi", "-i", "testsrc2=size=3840x2160:rate=24"])
                .args(["-t", "20", "-c:v", "libx265", "-preset", "ultrafast"])
                .args(["-pix_fmt", "yuv420p10le", "-x265-params"])
                .arg("log-level=error:colorprim=bt2020:transfer=smpte2084:colormatrix=bt2020nc")
                .args(["-f", "matroska"])
        })
        .unwrap();
        let tone_map = if hardware.tone_maps {
            ToneMapper::Hardware
        } else {
            ToneMapper::Software
        };
        let video = VideoConversion {
            hardware: Some(hardware.accel),
            tone_map: Some(tone_map),
            limit: Some(VideoLimit {
                size: Some((1920, 1080)),
                bits_per_second: 8_000_000,
            }),
            burn_in: None,
            in_band_headers: true,
        };
        let dir = work.path().join("transcoded");
        fs::create_dir_all(&dir).unwrap();

        let transcode = Transcode { audio: None, video };
        let from = FromCut {
            n: 0,
            cuts: &[0, 6000, 12_000, 18_000],
        };

        let output = ffmpeg
            .transcode(file.to_str().unwrap(), &transcode, &dir, from)
            .unwrap();

        let output = output.spawn().unwrap().wait_with_output().await.unwrap();
        assert!(output.status.success(), "{hardware:?}: {output:?}");
        for n in 0..4 {
            assert!(dir.join(format!("{n}.m4s")).exists(), "{n}.m4s");
        }
        assert!(!dir.join("4.m4s").exists(), "a keyframe of its own");
    }

    #[tokio::test]
    async fn an_mkv_written_to_a_pipe_probes_to_its_real_length() {
        let ffmpeg = system_ffmpeg();
        let work = tempfile::tempdir().unwrap();
        let file = work.path().join("captured.mkv");
        // A source that just ends, unlike `-t`, leaves the muxer no length to write up front, and
        // on a pipe it can't seek back to write it at the end, as with a stream capture.
        let status = std::process::Command::new("ffmpeg")
            .args(["-v", "error", "-f", "lavfi"])
            .args(["-i", "testsrc=size=320x240:rate=24:duration=2"])
            .args([
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-f",
                "matroska",
                "pipe:1",
            ])
            .stdout(fs::File::create(&file).unwrap())
            .status()
            .unwrap();
        assert!(status.success());

        let probe = ffmpeg.probe(file.to_str().unwrap()).await.unwrap();

        // Within a frame, a 24th of a second, of 2 seconds.
        assert!(
            probe
                .duration_ms
                .is_some_and(|ms| (1958..=2042).contains(&ms)),
            "{probe:?}"
        );
    }

    #[tokio::test]
    async fn a_transcode_started_at_a_cut_writes_the_same_files_from_there_on() {
        let ffmpeg = system_ffmpeg();
        let work = tempfile::tempdir().unwrap();
        let file = work.path().join("alien.mkv");
        write_hevc_with_5_1(&file).unwrap();
        let (whole, restarted) = (work.path().join("whole"), work.path().join("restarted"));
        transcode(&ffmpeg, &file, &whole, 0).await;

        transcode(&ffmpeg, &file, &restarted, 2).await;

        assert!(!restarted.join("1.m4s").exists());
        for n in 2..4 {
            let segment = |dir: &Path| fs::read(dir.join(format!("{n}.m4s"))).unwrap();
            let (expected, actual) = (starts_at(&segment(&whole)), starts_at(&segment(&restarted)));
            assert_eq!(actual, expected, "{n}.m4s");
        }
        assert!(!restarted.join("4.m4s").exists());
    }

    #[tokio::test]
    async fn a_remux_started_at_a_keyframe_writes_the_same_files_from_there_on() {
        let ffmpeg = system_ffmpeg();
        let work = tempfile::tempdir().unwrap();
        let file = work.path().join("alien.mkv");
        write_h264_with_5_1(&file, "aac").unwrap();
        // A keyframe every 2 seconds of 14, each starting a file.
        let keyframes = ffmpeg.keyframes(file.to_str().unwrap()).await.unwrap();
        assert_eq!(keyframes.len(), 7);
        let (whole, restarted) = (work.path().join("whole"), work.path().join("restarted"));
        let first = Keyframe {
            n: 0,
            ms: keyframes[0],
            previous_ms: None,
        };
        remux(&ffmpeg, &file, &whole, first).await;

        let third = Keyframe {
            n: 3,
            ms: keyframes[3],
            previous_ms: Some(keyframes[2]),
        };
        remux(&ffmpeg, &file, &restarted, third).await;

        assert!(!restarted.join("2.m4s").exists());
        for n in 3..7 {
            let segment = |dir: &Path| fs::read(dir.join(format!("{n}.m4s"))).unwrap();
            let (expected, actual) = (starts_at(&segment(&whole)), starts_at(&segment(&restarted)));
            // Within 2 ms, at 12288 units a second: a run's first file can round differently.
            assert!(
                expected.abs_diff(actual) <= 25,
                "{n}.m4s: {actual} against {expected}"
            );
        }
        assert!(!restarted.join("7.m4s").exists());
    }
}
