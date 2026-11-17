//! Building the runs of ffmpeg that write a Remux's or Transcode's HLS files.

use std::{io, path::Path, process::Stdio};

use super::{Hardware, finding::is_release_before, sidecar_charset};

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

/// What a Remux converts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Remux {
    /// The audio Track it plays, or `None` when the Version has none.
    pub(crate) audio: Option<RemuxAudio>,
    /// Whether the video it copies is HEVC, so it's tagged `hvc1` for Apple's players.
    pub(crate) hevc: bool,
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
pub(crate) struct HlsRun(pub(super) tokio::process::Command);

impl HlsRun {
    /// Starts ffmpeg. Dropping the child stops it.
    #[tracing::instrument(name = "ffmpeg launch", skip_all)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BurnIn {
    /// A bitmap, such as PGS, at its place among the file's subtitle streams, counting from 0, laid
    /// over a picture of the width and height given, if known.
    Image {
        subtitle: usize,
        picture: Option<(i64, i64)>,
    },
    /// Text, such as ASS, which libass draws with its styling, from the file or a sidecar.
    Text { subtitle: TextSubtitle },
}

/// Where a [`BurnIn::Text`] subtitle is read from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TextSubtitle {
    /// At its place `subtitle` among the subtitle streams of the file transcoded, counting from 0.
    Stream {
        subtitle: usize,
        /// Its place in the Version's Tracks, and how many fonts the file carries, by which the
        /// subtitle cache finds them.
        track: usize,
        fonts: usize,
    },
    /// The one in this file beside it.
    Sidecar(String),
    /// A [`Self::Stream`] read out of the file into the ASS file `ass`, drawn with the fonts in
    /// the folder `fonts`.
    Cached { ass: String, fonts: String },
}

/// The arguments that take the first video of the file at `path` into a Transcode, through
/// `before`, then with `burn_in` drawn into it, then through `after`.
fn transcoded_video(
    path: &str,
    before: Option<String>,
    burn_in: Option<&BurnIn>,
    after: Option<String>,
    seeks: bool,
) -> Vec<String> {
    let drawn = match burn_in {
        None => None,
        Some(&BurnIn::Image { subtitle, picture }) => {
            let graph = overlay_graph(before, subtitle, picture, after, seeks);
            return ["-filter_complex", &graph, "-map", "[video]"]
                .map(str::to_owned)
                .into();
        }
        // ponytail: assumes ffmpeg has libass. Until the subtitle cache holds it, each run reads
        // the whole file for the subtitle first, attached fonts and all.
        Some(BurnIn::Text {
            subtitle: TextSubtitle::Stream { subtitle, .. },
        }) => Some(format!(
            "subtitles=filename={}:si={subtitle}",
            filter_value(path)
        )),
        Some(BurnIn::Text {
            subtitle: TextSubtitle::Sidecar(sidecar),
        }) => {
            // Blocks on the disk, briefly, as each run's start already does for the cache. One
            // that can't be read gets no `charenc`, as in `Ffmpeg::subtitle`.
            let charset = (std::fs::read(sidecar).ok()).and_then(|bytes| sidecar_charset(&bytes));
            let charenc =
                charset.map_or_default(|charset| format!(":charenc={}", filter_value(charset)));
            Some(format!(
                "subtitles=filename={}{charenc}",
                filter_value(sidecar)
            ))
        }
        Some(BurnIn::Text {
            subtitle: TextSubtitle::Cached { ass, fonts },
        }) => Some(format!(
            "subtitles=filename={}:fontsdir={}",
            filter_value(ass),
            filter_value(fonts)
        )),
    };
    let mut args: Vec<String> = ["-map", "0:V:0"].map(str::to_owned).into();
    if let Some(filters) = filter_chain([before, drawn, after]) {
        // One `-vf`: a second would replace the first.
        args.extend(["-vf".to_owned(), filters]);
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
    seeks: bool,
) -> String {
    // The input's `-ss` drops a bitmap that began before it, so a run that seeks reads the
    // subtitle from a second input, `transcode` opens, that doesn't.
    let source = usize::from(seeks);
    // A bitmap's canvas can be smaller than the picture, as a 4K Blu-ray's 1080p PGS is, or
    // another shape, as a cropped film's 1080p PGS is: scaled evenly and centred, its text is
    // neither stretched nor cut off.
    let fit = picture.map(|(width, height)| {
        format!("scale={width}:{height}:force_original_aspect_ratio=decrease")
    });
    let or_null = |filter: Option<String>| filter.unwrap_or_else(|| "null".to_owned());
    let (before, fit) = (or_null(before), or_null(fit));
    let after: String = after.map_or_default(|after| format!(",{after}"));
    format!(
        "[0:V:0]{before}[picture];[{source}:s:{subtitle}]{fit}[subtitle];\
         [picture][subtitle]overlay=(W-w)/2:(H-h)/2{after}[video]"
    )
}

/// `value` as a filter's option value in a filtergraph, which ffmpeg unescapes twice: once
/// parsing the graph, then once parsing the filter's options (ffmpeg-filters(1), "Notes on
/// filtergraph escaping").
pub(super) fn filter_value(value: &str) -> String {
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

impl VideoConversion {
    /// Whether it works the CPU hard: it runs in software, or tone-maps there.
    pub(crate) fn cpu_bound(&self) -> bool {
        self.hardware.is_none() || self.tone_map == Some(ToneMapper::Software)
    }
}

/// What tone-maps a Transcode's HDR to SDR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToneMapper {
    /// ffmpeg's zscale, which [`super::CanDo::zscale`] says it has.
    Software,
    /// The GPU of [`VideoConversion::hardware`], which [`Hardware::tone_maps`] says it can, while
    /// nothing is burned in.
    Hardware,
}

/// The arguments for ffmpeg that convert video as a [`VideoConversion`] says.
pub(super) struct VideoArgs {
    /// Before `-i`.
    pub(super) input: Vec<String>,
    /// After `-i`: which video, and the filters it goes through.
    pub(super) video: Vec<String>,
    /// The encoder, making keyframes only where forced.
    pub(super) codec: Vec<String>,
}

/// The arguments for ffmpeg that convert the first video of the file at `path` as `video` says.
///
/// `seeks` is whether the input is opened at a point past its start.
pub(super) fn video_args(path: &str, video: &VideoConversion, seeks: bool) -> VideoArgs {
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
    let tone_map = (video.tone_map.is_some()).then(|| TONE_MAP.to_owned());
    // ponytail: the size is from the stored picture, so a rotated phone clip is stretched; swap
    // its sides once probing reads rotation.
    let scale = size.map(|(width, height)| format!("scale={width}:{height}"));
    let upload = accel.map(|accel| accel.upload().to_owned());
    // Scaling down first spares tone-mapping the pixels it drops. A subtitle instead goes after
    // tone-mapping, which would dim it, and before scaling, so it shrinks with the picture and an
    // image subtitle still fits the source size `overlay_graph` is given.
    let (before, after) = match video.burn_in {
        None => (filter_chain([scale, tone_map, upload]), None),
        Some(_) => (tone_map, filter_chain([scale, upload])),
    };
    VideoArgs {
        input: accel.map_or_default(|accel| accel.input(gpu, false)),
        video: transcoded_video(path, before, video.burn_in.as_ref(), after, seeks),
        codec,
    }
}

/// The `filters` given, comma-joined in order into one chain, or `None` if none is.
fn filter_chain<const N: usize>(filters: [Option<String>; N]) -> Option<String> {
    let filters: Vec<String> = filters.into_iter().flatten().collect();
    (!filters.is_empty()).then(|| filters.join(","))
}

/// Has `command` take the `audio`th audio Track, if given.
pub(super) fn map_audio(command: &mut tokio::process::Command, audio: Option<usize>) {
    if let Some(audio) = audio {
        command.arg("-map").arg(format!("0:a:{audio}"));
    }
}

/// Has `command` convert its audio to AAC with `channels` channels, if given.
pub(super) fn aac(command: &mut tokio::process::Command, channels: Option<i64>) {
    if let Some(channels) = channels {
        command
            .args(["-c:a", "aac", "-ac"])
            .arg(channels.to_string());
    }
}

/// `command`, ffmpeg at `version` given its input and codecs, writing HLS with fMP4 to `dir`: a
/// file per keyframe, numbered from `n`, and an init segment named by [`hls_init`].
pub(super) fn hls_output(
    mut command: tokio::process::Command,
    version: &str,
    dir: &Path,
    n: usize,
) -> HlsRun {
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
pub(super) fn seconds(ms: i64) -> String {
    format!("{}.{:03}", ms / 1000, ms % 1000)
}
