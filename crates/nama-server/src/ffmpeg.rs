//! Finding the ffmpeg and ffprobe the Server Remuxes, Transcodes and probes Versions with, which
//! come from the system, not the release (ADR 0026).

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

use crate::{
    config::Config,
    tracks::{Hdr, Probe, Track, TrackKind},
};

/// The oldest ffmpeg and ffprobe the Server uses, as major and minor version. HLS with fMP4
/// segments, the zscale and tonemap filters and the VAAPI, QSV and NVENC encoders that Remuxing
/// and Transcoding use all predate 4.4, and it's the oldest a supported LTS distribution ships
/// (Ubuntu 22.04), so nothing older gets tested.
const MINIMUM: (u32, u32) = (4, 4);

/// How long ffprobe may take over one file, such as one on a network share that stopped answering.
const PROBE_TIMEOUT: Duration = Duration::from_secs(60);

/// How long ffprobe may take to list a file's keyframes, which reads all of it, such as a 60 GB
/// file from a slow network share.
const KEYFRAMES_TIMEOUT: Duration = Duration::from_mins(10);

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

    /// The facts of the file at `path` and its video, audio and subtitle Tracks, as ffprobe reads
    /// them.
    pub(crate) async fn probe(&self, path: &str) -> Result<Probe, ProbeError> {
        let args = ["-print_format", "json", "-show_format", "-show_streams"];
        let output = self.run_ffprobe(&args, path, PROBE_TIMEOUT).await?;
        Ok(parse_probe(&output)?)
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
        let output = self.run_ffprobe(&args, path, KEYFRAMES_TIMEOUT).await?;
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

    /// Starts ffmpeg copying the first video, and the `audio`th audio Track if any, of the file
    /// at `path` untouched into fMP4 in `dir`, but for converting that audio to AAC with
    /// `aac_channels` channels if given, from the keyframe `from` to the end. It writes a
    /// file per keyframe, named by its number such as `3.m4s`, and an init segment named by
    /// [`hls_init`] that's whole once `from`'s file is. A run started at any keyframe writes the
    /// same files, so one can pick up where another stopped. Each file appears only once it's
    /// whole. Dropping the child stops ffmpeg.
    #[expect(
        clippy::too_many_arguments,
        reason = "the plan fixes the first three and each run the last two; a struct would only rename them"
    )]
    pub(crate) fn remux(
        &self,
        path: &str,
        audio: Option<usize>,
        aac_channels: Option<i64>,
        dir: &Path,
        from: Keyframe,
    ) -> io::Result<tokio::process::Child> {
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
        map(&mut command, audio);
        command.args(["-c", "copy"]);
        aac(&mut command, aac_channels);
        spawn_hls(command, &ffmpeg.version, dir, from.n)
    }

    /// Starts ffmpeg converting the first video of the file at `path` to 8-bit H.264 in software,
    /// and its `audio`th audio Track, if given, to AAC with the channels given beside it, into fMP4
    /// in `dir`, from `cuts[0]` to the end. It cuts a file at each of `cuts`, in milliseconds,
    /// numbered from `n`, such as `3.m4s`, and writes an init segment named by [`hls_init`]
    /// that's whole once the first file is. A run started at any cut writes the same files, so one
    /// can pick up where another stopped. Each file appears only once it's whole. Dropping the
    /// child stops ffmpeg.
    #[expect(
        clippy::too_many_arguments,
        reason = "as for `remux`: the plan fixes the first two and each run the rest"
    )]
    pub(crate) fn transcode(
        &self,
        path: &str,
        audio: Option<(usize, i64)>,
        dir: &Path,
        n: usize,
        cuts: &[i64],
    ) -> io::Result<tokio::process::Child> {
        let ffmpeg = (self.ffmpeg.as_ref()).map_err(|why| io::Error::other(why.clone()))?;
        let mut command = tokio::process::Command::new(&ffmpeg.path);
        command.args(["-v", "error", "-nostdin"]);
        let start_ms = cuts.first().copied().unwrap_or_default();
        if start_ms > 0 {
            // Decodes from the keyframe before it and drops the frames before it.
            command.args(["-ss", &seconds(start_ms)]);
        }
        command.arg("-i").arg(path);
        // Times as in the file, so the keyframes forced below fall at the same frames whichever
        // cut a run starts at: `-force_key_frames` expressions time frames from the run's first.
        command.args(["-copyts", "-start_at_zero"]);
        map(&mut command, audio.map(|(audio, _)| audio));
        // ponytail: a 5-hour Version's cuts near Windows' 32K command line; read them from a
        // file if longer ones fail to Transcode there.
        let cuts: Vec<String> = cuts.iter().map(|&cut| seconds(cut)).collect();
        command
            // ponytail: assumes the Server's ffmpeg has libx264; check it as ADR 0026 says (#446).
            .args(["-c:v", "libx264", "-preset", "veryfast", "-pix_fmt", "yuv420p"])
            // Keyframes only where forced, so each cut starts a file and nothing else does.
            .args(["-x264-params", "keyint=infinite:scenecut=0"])
            .arg("-force_key_frames")
            .arg(cuts.join(","));
        aac(&mut command, audio.map(|(_, channels)| channels));
        command.args(["-output_ts_offset", &seconds(TRANSCODE_LATER_BY_MS)]);
        spawn_hls(command, &ffmpeg.version, dir, n)
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

/// Has `command` take the first video, and the `audio`th audio Track if given.
fn map(command: &mut tokio::process::Command, audio: Option<usize>) {
    command.args(["-map", "0:V:0"]);
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

/// Starts `command`, ffmpeg at `version` given its input and codecs, writing HLS with fMP4 to
/// `dir`: a file per keyframe, numbered from `n`, and an init segment named by [`hls_init`].
fn spawn_hls(
    mut command: tokio::process::Command,
    version: &str,
    dir: &Path,
    n: usize,
) -> io::Result<tokio::process::Child> {
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
        .kill_on_drop(true)
        .spawn()
}

/// The keyframes in what `ffprobe -show_entries packet=pts_time,flags:format=start_time -of
/// csv=p=0` printed, packets such as `2.002000,K__` and then the file's start time, in
/// milliseconds from that start, in order.
fn parse_keyframes(csv: &str) -> Vec<i64> {
    // ffmpeg shifts a file that starts late, as an MPEG-TS often does, to start at 0.
    let start = (csv.lines())
        .find(|line| !line.contains(','))
        .and_then(millis)
        .unwrap_or_default();
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
    /// What a player without Dolby Vision shows: 1 is HDR10 and 6 Ultra HD Blu-ray's HDR10,
    /// while 0 is nothing, 2 SDR and 4 HLG.
    dv_bl_signal_compatibility_id: Option<i64>,
}

/// The [`Probe`] ffprobe's JSON output describes.
fn parse_probe(json: &[u8]) -> Result<Probe, serde_json::Error> {
    let output: ProbeOutput = serde_json::from_slice(json)?;
    let format = output.format;
    Ok(Probe {
        container: format.format_name,
        duration_ms: format.duration.as_deref().and_then(millis),
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
            hdr10_base_layer: matches!(record.dv_bl_signal_compatibility_id, Some(1 | 6)),
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
        ffmpeg
            .args(["-f", "lavfi", "-i", "color=size=3840x2160:rate=24"])
            .args(["-f", "lavfi", "-i", "anullsrc=channel_layout=5.1"])
            // Encoders every ffmpeg build has; of those, FFV1 is one that takes 10 bits.
            .args(["-t", "0.2", "-c:v", "ffv1", "-pix_fmt", "yuv420p10le", "-c:a", "ac3"])
            .args(["-color_primaries", "bt2020", "-colorspace", "bt2020nc"])
            .args(["-color_trc", transfer])
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

    use super::{Ffmpeg, FoundProgram, Keyframe};
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
    async fn ffmpeg_before_5_0_is_given_segment_options_by_their_old_name() {
        let bin = tempfile::tempdir().unwrap();
        let args = bin.path().join("args");
        let script = format!(
            "#!/bin/sh\n[ \"$1\" = -version ] && echo 'ffmpeg version 4.4.2' \
             || echo \"$*\" > '{}'\n",
            args.display()
        );
        let ffmpeg = {
            let _writing = WRITING_FAKES
                .write()
                .unwrap_or_else(PoisonError::into_inner);
            let path = bin.path().join("ffmpeg");
            fs::write(&path, script).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
            path
        };
        let ffmpeg = Ffmpeg {
            ffmpeg: Ok(FoundProgram {
                path: ffmpeg,
                version: "4.4.2".to_owned(),
            }),
            ..Ffmpeg::default()
        };

        let remux = {
            let _running = WRITING_FAKES.read().unwrap_or_else(PoisonError::into_inner);
            let first = Keyframe {
                n: 0,
                ms: 0,
                previous_ms: None,
            };
            ffmpeg.remux("alien.mkv", None, None, bin.path(), first)
        };

        remux.unwrap().wait().await.unwrap();
        let args = fs::read_to_string(args).unwrap();
        assert!(
            args.contains("-hls_ts_options movflags=+frag_discont"),
            "{args}"
        );
    }
}

#[cfg(test)]
mod probe_output_tests {
    use super::parse_probe;
    use crate::tracks::{Hdr, Probe, Track, TrackKind};

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
    fn dolby_vision_is_read_with_its_profile_and_whether_its_base_layer_is_hdr10() {
        assert_eq!(
            hdr_of(&dolby_vision(8, 1, "smpte2084")),
            Some(Hdr::DolbyVision {
                profile: 8,
                hdr10_base_layer: true,
            })
        );
        // Ultra HD Blu-ray's, with an enhancement layer over HDR10.
        assert_eq!(
            hdr_of(&dolby_vision(7, 6, "smpte2084")),
            Some(Hdr::DolbyVision {
                profile: 7,
                hdr10_base_layer: true,
            })
        );
        // Streaming's, which only a Dolby Vision player shows right.
        assert_eq!(
            hdr_of(&dolby_vision(5, 0, "unknown")),
            Some(Hdr::DolbyVision {
                profile: 5,
                hdr10_base_layer: false,
            })
        );
        // Broadcast's, over HLG.
        assert_eq!(
            hdr_of(&dolby_vision(8, 4, "arib-std-b67")),
            Some(Hdr::DolbyVision {
                profile: 8,
                hdr10_base_layer: false,
            })
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
    fn keyframes_are_the_packets_flagged_k_in_time_order() {
        let csv = "0.000000,K__\n0.083000,___\n2.002000,K_D\n1.001000,K__\nN/A,K__\n";

        assert_eq!(super::parse_keyframes(csv), [0, 1001, 2002]);
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

    use super::{Ffmpeg, Keyframe, write_h264_with_5_1, write_hevc_with_5_1};
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
        let output = ffmpeg.remux(path, Some(0), None, dir, from).unwrap();
        let output = output.wait_with_output().await.unwrap();
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
        let output = ffmpeg
            .transcode(path.to_str().unwrap(), Some((0, 2)), dir, n, &cuts[n..])
            .unwrap();
        let output = output.wait_with_output().await.unwrap();
        assert!(output.status.success(), "{output:?}");
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
