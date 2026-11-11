//! Running ffprobe on a stored file and reading what it prints, private to Probing
//! (`AGENTS.md`, "One owner for probing").

use std::{collections::HashMap, time::Duration};

use serde::Deserialize;

use super::ProbeError;
use crate::{
    ffmpeg::Ffmpeg,
    tracks::{BaseLayer, Hdr, Probe, Track, TrackKind},
};

/// How long ffprobe may take over one file, such as one on a network share that stopped answering.
const PROBE_TIMEOUT: Duration = Duration::from_secs(60);

/// How long ffprobe may take to read all of a file, as listing its keyframes or finding where its
/// video ends does, such as a 60 GB file from a slow network share.
const WHOLE_FILE_TIMEOUT: Duration = Duration::from_mins(10);

/// Subtitle codecs, as ffprobe names them, that are bitmaps rather than text.
const IMAGE_SUBTITLES: &[&str] = &["dvb_subtitle", "dvd_subtitle", "hdmv_pgs_subtitle", "xsub"];

/// The facts of the file at `path` and its video, audio and subtitle Tracks, as ffprobe reads
/// them. Reads the whole file when its format doesn't say how long it is, or says so only from
/// a placeholder header.
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
    Ok(Probe {
        container: format.format_name,
        duration_ms: (format.duration.as_deref())
            .filter(|_| !header_unfilled)
            .and_then(millis)
            // A playlist can't be cut from a length of 0, so it reads as unknown.
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

#[cfg(test)]
mod tests {
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
    fn an_avi_with_the_placeholder_frame_count_has_no_known_duration() {
        let avi_with = |nb_frames: &str| {
            let json = format!(
                r#"{{"format": {{"format_name": "avi", "duration": "2866.375000"}},
                "streams": [{{"codec_type": "video", "nb_frames": "{nb_frames}"}}]}}"#
            );
            parse_probe(json.as_bytes()).unwrap().duration_ms
        };

        assert_eq!(avi_with("1073741824"), None);
        assert_eq!(avi_with("68793"), Some(2_866_375));
    }

    #[test]
    fn only_an_avi_reads_the_placeholder_frame_count_as_unfilled() {
        let json = r#"{"format": {"format_name": "matroska,webm", "duration": "2.000000"},
            "streams": [{"codec_type": "video", "nb_frames": "1073741824"}]}"#;

        assert_eq!(
            parse_probe(json.as_bytes()).unwrap().duration_ms,
            Some(2000)
        );
    }

    #[test]
    fn video_ends_where_its_latest_packet_does_from_the_files_start() {
        // In decoding order, with a packet whose duration isn't known.
        let csv = "1.523000,1.440000,0.041000\n3.523000,3.440000,0.041000\n\
                   3.481000,3.481000,N/A\nN/A,N/A,0.041000\n1.500000\n";

        assert_eq!(super::parse_video_end(csv), Some(2064));
        assert_eq!(super::parse_video_end("0.000000\n"), None);
    }

    #[test]
    fn a_packet_without_a_presentation_time_ends_by_its_decoding_time() {
        // As an AVI's packets have.
        let csv = "N/A,0.000000,0.040000\nN/A,19.960000,0.040000\n0.000000\n";

        assert_eq!(super::parse_video_end(csv), Some(20_000));
    }

    /// ffmpeg shifts a file that starts late to start at 0, and cuts its segments from there.
    #[test]
    fn keyframes_are_timed_from_the_files_start() {
        let csv = "1.523000,K__\n4.023000,K__\n1.500000\n";

        assert_eq!(super::parse_keyframes(csv), [23, 2523]);
    }
}
