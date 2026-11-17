//! Running ffprobe on a stored file and reading what it prints, private to Probing
//! (the area's `AGENTS.md`, "One owner for probing").

use std::{collections::HashMap, time::Duration};

use serde::Deserialize;

use super::ProbeError;
use crate::{
    ffmpeg::Ffmpeg,
    playback::tracks::{BaseLayer, Hdr, Probe, Track, TrackKind},
};

/// How long ffprobe may take over one file, such as one on a network share that stopped answering.
const PROBE_TIMEOUT: Duration = Duration::from_secs(60);

/// How long ffprobe may take to read all of a file, as listing its keyframes or finding where its
/// video ends does, such as a 60 GB file from a slow network share.
pub(super) const WHOLE_FILE_TIMEOUT: Duration = Duration::from_mins(10);

/// Subtitle codecs, as ffprobe names them, that are bitmaps rather than text.
const IMAGE_SUBTITLES: &[&str] = &["dvb_subtitle", "dvd_subtitle", "hdmv_pgs_subtitle", "xsub"];

/// The ends of the file names of fonts, for an attachment whose MIME type doesn't say it's one.
const FONT_EXTENSIONS: &[&str] = &[".ttf", ".otf", ".ttc"];

/// The facts of the file at `path`, its video, audio and subtitle Tracks and the fonts it carries,
/// as ffprobe reads them. Reads the whole file when its format doesn't say how long it is, or says
/// so only from a placeholder header.
pub(super) async fn probe(ffmpeg: &Ffmpeg, path: &str) -> Result<Probe, ProbeError> {
    let args = ["-print_format", "json", "-show_format", "-show_streams"];
    let output = run(ffmpeg, &args, path, PROBE_TIMEOUT).await?;
    let mut probe = parse_probe(&output)?;
    if probe.duration_ms.is_none() {
        probe.duration_ms = match video_end(ffmpeg, path).await {
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
async fn video_end(ffmpeg: &Ffmpeg, path: &str) -> Result<Option<i64>, ProbeError> {
    let packets = "packet=pts_time,dts_time,duration_time:format=start_time";
    let args = [
        "-select_streams",
        "V:0",
        "-show_entries",
        packets,
        "-of",
        "csv=p=0",
    ];
    let output = run(ffmpeg, &args, path, WHOLE_FILE_TIMEOUT).await?;
    Ok(parse_video_end(&String::from_utf8_lossy(&output)))
}

/// When each keyframe of the first video in the file at `path` falls, in milliseconds from
/// its start, in order. Reads the whole file.
pub(super) async fn keyframes(ffmpeg: &Ffmpeg, path: &str) -> Result<Vec<i64>, ProbeError> {
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
    let output = run(ffmpeg, &args, path, WHOLE_FILE_TIMEOUT).await?;
    Ok(parse_keyframes(&String::from_utf8_lossy(&output)))
}

/// What ffprobe prints given `args` and the file at `path`, within `timeout`.
async fn run(
    ffmpeg: &Ffmpeg,
    args: &[&str],
    path: &str,
    timeout: Duration,
) -> Result<Vec<u8>, ProbeError> {
    let Ok(ffprobe) = &ffmpeg.ffprobe else {
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
/// packet=pts_time,dts_time,duration_time:format=start_time -of csv=p=0` printed, packets such
/// as `2.002000,1.960000,0.041000` and then the file's start time: the latest end of a packet, in
/// milliseconds from that start, or `None` if there is none past it.
fn parse_video_end(csv: &str) -> Option<i64> {
    let start = file_start(csv);
    let ends = csv.lines().filter_map(|line| {
        let mut times = line.split(',');
        let (pts, dts, duration) = (times.next()?, times.next()?, times.next()?);
        // An AVI's packets carry only when they decode.
        let at = millis(pts).or_else(|| millis(dts))?;
        Some(at + millis(duration).unwrap_or_default())
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
    /// How many bytes of codec header the file carries for it, such as VobSub's `.idx`.
    extradata_size: Option<i64>,
    nb_frames: Option<String>,
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

/// The frame count ffmpeg's AVI muxer writes until it knows the real one, 2^30.
const AVI_PLACEHOLDER_FRAMES: &str = "1073741824";

/// The [`Probe`] ffprobe's JSON output describes.
fn parse_probe(json: &[u8]) -> Result<Probe, serde_json::Error> {
    let output: ProbeOutput = serde_json::from_slice(json)?;
    let format = output.format;
    // An AVI written to a pipe can't go back to fill in its header, which keeps ffmpeg's
    // placeholder frame counts, and ffprobe works its duration out from those.
    let header_unfilled = format.format_name == "avi"
        && (output.streams.iter())
            .any(|stream| stream.nb_frames.as_deref() == Some(AVI_PLACEHOLDER_FRAMES));
    // ffmpeg's `t:<n>` counts only these, so a font's place is among them.
    let font_attachments = (output.streams.iter())
        .filter(|stream| stream.codec_type.as_deref() == Some("attachment"))
        .enumerate()
        .filter(|(_, attachment)| is_font(attachment))
        .map(|(place, _)| place)
        .collect();
    Ok(Probe {
        container: format.format_name,
        duration_ms: (format.duration.as_deref())
            .filter(|_| !header_unfilled)
            .and_then(millis)
            // A playlist can't be cut from a length of 0, so it reads as unknown.
            .filter(|&ms| ms > 0),
        bitrate: format.bit_rate.and_then(|bitrate| bitrate.parse().ok()),
        tracks: output.streams.into_iter().filter_map(track).collect(),
        font_attachments,
    })
}

/// Whether `attachment` is a font: its MIME type says so, or failing that its file name ends as a
/// font's does. Anything else, such as a picture, isn't.
fn is_font(attachment: &ProbedStream) -> bool {
    let tag = |name: &str| (attachment.tags.get(name)).map(|value| value.to_ascii_lowercase());
    // Muxers name fonts `font/ttf`, `application/x-truetype-font`, `application/vnd.ms-opentype`
    // and more besides.
    tag("mimetype").is_some_and(|mime| mime.contains("font") || mime.contains("opentype"))
        || tag("filename").is_some_and(|name| FONT_EXTENSIONS.iter().any(|end| name.ends_with(end)))
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
            // ponytail: a VobSub header without a `palette:` line passes and draws black; read the
            // extradata itself if one turns up.
            header: stream.extradata_size.is_some_and(|size| size > 0),
        },
        _ => return None,
    };
    let (default, forced, flagged_sdh) =
        (flag("default"), flag("forced"), flag("hearing_impaired"));
    let title = stream.tags.remove("title");
    // Only a subtitle's title is read: "CC" or "SDH" on an audio title means something else.
    let titled_sdh =
        matches!(kind, TrackKind::Subtitle { .. }) && title.as_deref().is_some_and(title_says_sdh);
    Some(Track {
        codec: stream.codec_name,
        language: stream.tags.remove("language"),
        title,
        default,
        forced,
        sdh: flagged_sdh || titled_sdh,
        kind,
    })
}

/// Whether `title` names a subtitle for the deaf and hard of hearing, such as "English (SDH)",
/// "English CC" or "hearing-impaired". Only whole words count, so "Accent" doesn't.
fn title_says_sdh(title: &str) -> bool {
    let words: Vec<String> = (title.split(|c: char| !c.is_alphanumeric()))
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect();
    words.iter().any(|word| word == "sdh" || word == "cc")
        || words.windows(2).any(|pair| pair == ["hearing", "impaired"])
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

#[cfg(test)]
#[path = "ffprobe_tests.rs"]
mod tests;
