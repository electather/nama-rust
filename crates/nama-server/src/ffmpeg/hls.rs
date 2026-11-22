//! Building the runs of ffmpeg that write a Remux's or Transcode's HLS files.

use std::{fs::File, io, io::Read, path::Path, process::Stdio};

use super::{Hardware, finding::is_release_before, hardware::Decode};

/// The filter that tags HDR10 and HLG frames for a tone-map in software, before any scaling.
// HDR10 and HLG are always BT.2020, but probing calls a file HDR by its transfer alone, and
// zscale fails the whole Transcode with "no path between colorspaces" on untagged primaries.
const HDR_TAGS: &str = "setparams=color_primaries=bt2020:colorspace=bt2020nc";

/// The filter that tags Dolby Vision frames with no Base Layer as BT.2020 PQ, which `tonemapx`
/// reads, before any scaling.
// ffmpeg tags them IPTPQc2, which no scaler takes ("Impossible to convert between the formats").
const DOLBY_VISION_TAGS: &str =
    "setparams=range=tv:color_primaries=bt2020:color_trc=smpte2084:colorspace=bt2020nc";

/// The filters that tone-map tagged HDR to SDR with zscale: to linear light, to BT.709's
/// primaries, Hable's curve from HDR's brightness to SDR's 100 nits, then back to BT.709 video.
const ZSCALE_TONE_MAP: &str = "zscale=t=linear:npl=100,format=gbrpf32le,zscale=p=bt709,\
                               tonemap=tonemap=hable:desat=0,zscale=t=bt709:m=bt709:r=tv,\
                               format=yuv420p";

/// The filters that tone-map tagged HDR to SDR with jellyfin-ffmpeg's `tonemapx`, its source's
/// peak pinned at 1000 nits, as Silo does (ADR 0102), applying Dolby Vision's RPU if
/// `apply_dovi`.
// Read from the RPU, a 4000-nit master comes out about a third darker in the midtones.
fn tonemapx(apply_dovi: bool) -> String {
    format!(
        "tonemapx=tonemap=bt2390:peak=100:apply_dovi={},\
         zscale=p=bt709:t=bt709:m=bt709:r=tv,format=yuv420p",
        u8::from(apply_dovi)
    )
}

/// The filter that reshapes tagged Dolby Vision with no Base Layer into HDR10 with `tonemapx`:
/// BT.2020 PQ, limited range, its peak pinned as [`tonemapx`]'s, dropping its RPU.
// `tonemapx` refuses 8-bit PQ, so the frames stay 10-bit up to the upload.
const TONEMAPX_TO_HDR10: &str = "tonemapx=tonemap=bt2390:peak=100:apply_dovi=1:t=smpte2084:\
                                 m=bt2020:p=bt2020:r=tv:format=yuv420p10le";

/// The name of the init segment a Remux or Transcode started at file `n` writes beside its files. Every
/// run's is the same, but each writes its own, since ffmpeg writes it from empty as a run goes.
pub(crate) fn hls_init(n: usize) -> String {
    format!("init-{n}.mp4")
}

/// The playlist a Remux or Transcode started at file `n` writes beside its files, listing each
/// once it's whole, such as `ffmpeg-3-0.m4s`, `ffmpeg-3-1.m4s` and so on, and how long it lasts.
pub(crate) fn hls_playlist(n: usize) -> String {
    format!("ffmpeg-{n}.m3u8")
}

/// The file the run that wrote the file named `name` started at, if it's a run's init segment
/// ([`hls_init`]), playlist ([`hls_playlist`]) or one of the files that lists.
pub(crate) fn hls_run_from(name: &str) -> Option<usize> {
    let rest = (name.strip_prefix("init-")).or_else(|| name.strip_prefix("ffmpeg-"))?;
    rest.split(['-', '.']).next()?.parse().ok()
}

/// What a Remux converts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Remux {
    /// The audio Track it plays, or `None` when the Version has none.
    pub(crate) audio: Option<RemuxAudio>,
    /// What the video it copies is tagged for Apple's players, if anything.
    pub(crate) video_tag: Option<VideoTag>,
}

/// What a Remux tags the HEVC it copies, which Apple's players take in fMP4 only as one of these,
/// where ffmpeg writes `hev1` unless told.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VideoTag {
    /// For HEVC that isn't Dolby Vision 5.
    Hvc1,
    /// For Dolby Vision 5, as Apple's HLS authoring spec has it (rule 1.10).
    Dvh1,
}

/// The audio Track a Remux plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RemuxAudio {
    /// Its place among the file's audio Tracks, counting from 0.
    pub(crate) track: usize,
    /// What it's converted to, or `None` to copy it.
    pub(crate) conversion: Option<AudioConversion>,
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
    /// What it's converted to.
    pub(crate) conversion: AudioConversion,
}

/// What a Remux or Transcode converts the audio Track it plays to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AudioConversion {
    /// The encoder it's converted with.
    pub(crate) encoder: AudioEncoder,
    /// The channels it's converted to, mixing down any more it has.
    pub(crate) channels: i64,
}

/// The ffmpeg encoder a Remux or Transcode converts audio with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AudioEncoder {
    /// AAC-LC, by ffmpeg's own encoder.
    Aac,
    /// AC-3, Dolby Digital.
    Ac3,
    /// E-AC-3, Dolby Digital Plus.
    Eac3,
}

impl AudioEncoder {
    /// Its name as both ffmpeg's encoder and ffprobe call it.
    pub(crate) const fn ffmpeg_name(self) -> &'static str {
        match self {
            Self::Aac => "aac",
            Self::Ac3 => "ac3",
            Self::Eac3 => "eac3",
        }
    }
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
    /// A bitmap, such as PGS, laid over a picture of the width and height given, if known.
    Image {
        subtitle: ImageSubtitle,
        picture: Option<(i64, i64)>,
    },
    /// Text, such as ASS, which libass draws with its styling, from the file or a sidecar.
    Text { subtitle: TextSubtitle },
}

/// Where a [`BurnIn::Image`] subtitle is read from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ImageSubtitle {
    /// At its place among the subtitle streams of the file transcoded, counting from 0.
    Stream(usize),
    /// The PGS file at this path beside it.
    Sidecar(String),
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
    /// The one in the file `path` beside it, read in `charset` if it isn't UTF-8, which running
    /// the plan finds with [`super::sidecar_charset`].
    Sidecar {
        path: String,
        charset: Option<&'static str>,
    },
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
        Some(BurnIn::Image { subtitle, picture }) => {
            let (source, subtitle) = match *subtitle {
                // The input's `-ss` drops a bitmap that began before it, so a run that seeks reads
                // the subtitle from a second input, `transcode` opens, that doesn't.
                ImageSubtitle::Stream(subtitle) => (usize::from(seeks), subtitle),
                // The second input, holding just its one subtitle.
                ImageSubtitle::Sidecar(_) => (1, 0),
            };
            let graph = overlay_graph(before, source, subtitle, *picture, after);
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
            subtitle:
                TextSubtitle::Sidecar {
                    path: sidecar,
                    charset,
                },
        }) => {
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

/// The filtergraph that runs the first video through `before`, lays input `source`'s `subtitle`th
/// subtitle stream, a bitmap, over it, fitted to `picture` if known, then runs it through `after`,
/// ending at `[video]`.
fn overlay_graph(
    before: Option<String>,
    source: usize,
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
    let after: String = after.map_or_default(|after| format!(",{after}"));
    format!(
        "[0:V:0]{before}[picture];[{source}:s:{subtitle}]{fit}[subtitle];\
         [picture][subtitle]overlay=(W-w)/2:(H-h)/2{after}[video]"
    )
}

/// When the first picture of the PGS file at `path` shows, in milliseconds: the 90 kHz time its
/// first segment's header carries after `PG`. Blocks on the disk, briefly.
pub(super) fn pgs_start_ms(path: &str) -> io::Result<i64> {
    let mut header = [0; 6];
    File::open(path)?.read_exact(&mut header)?;
    let [b'P', b'G', ticks @ ..] = header else {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "not a PGS file"));
    };
    Ok(i64::from(u32::from_be_bytes(ticks)) / 90)
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
    /// What tone-maps its HDR to SDR, or reshapes its Dolby Vision to HDR10, if anything does.
    pub(crate) tone_map: Option<ToneMapper>,
    pub(crate) limit: Option<VideoLimit>,
    /// The subtitle it draws into the picture, if any.
    pub(crate) burn_in: Option<BurnIn>,
    /// Whether each file carries the video's headers, so files written by another encoder, as
    /// when hardware fails and software carries on, play after this one's init segment. Never with
    /// `hevc`, which has no software to carry on in.
    pub(crate) in_band_headers: bool,
    /// Whether it encodes 10-bit HEVC on `hardware`, keeping the colours it decodes, HDR's
    /// included, rather than 8-bit H.264. Only on hardware that passed its HEVC test.
    pub(crate) hevc: bool,
}

impl VideoConversion {
    /// The codec it encodes, as ffprobe names it.
    pub(crate) const fn codec(&self) -> &'static str {
        if self.hevc { "hevc" } else { "h264" }
    }

    /// Whether it works the CPU hard: it runs in software, or tone-maps there.
    pub(crate) fn cpu_bound(&self) -> bool {
        self.hardware.is_none() || self.tone_maps_in_software()
    }

    fn tone_maps_in_software(&self) -> bool {
        self.tone_map
            .is_some_and(|tone_map| tone_map != ToneMapper::Hardware)
    }
}

/// What tone-maps a Transcode's HDR to SDR, or reshapes its Dolby Vision to HDR10.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToneMapper {
    /// ffmpeg's zscale, which [`super::CanDo::zscale`] says it has.
    Software,
    /// jellyfin-ffmpeg's `tonemapx`, in software, which [`super::CanDo::tonemapx`] says it has,
    /// reshaping a Dolby Vision picture with no Base Layer by its RPU first if `dolby_vision`.
    Tonemapx { dolby_vision: bool },
    /// jellyfin-ffmpeg's `tonemapx`, in software, reshaping a Dolby Vision picture with no Base
    /// Layer by its RPU into 10-bit HDR10 rather than SDR, for [`VideoConversion::hevc`].
    DolbyVisionToHdr10,
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
    let pixel_format = if video.hevc { "p010" } else { "nv12" };
    let mut codec = match accel {
        None => ["-c:v", "libx264", "-preset", "veryfast", "-pix_fmt", "yuv420p"]
            .into_iter()
            // Keyframes only where forced, so each cut starts a file and nothing else does.
            .chain(["-x264-params", "keyint=infinite:scenecut=0"])
            .map(str::to_owned)
            .collect(),
        Some(accel) => accel.codec(video.hevc),
    };
    if video.hevc {
        // Safari and Chrome take HEVC in fMP4 only as `hvc1`, its headers in the init segment.
        codec.extend(["-tag:v", "hvc1"].map(str::to_owned));
    } else if video.in_band_headers {
        // `avc3` has decoders read the headers before each keyframe rather than the init
        // segment's, which a player fetched once, from whichever encoder wrote it.
        codec.extend(["-bsf:v", "dump_extra=freq=keyframe", "-tag:v", "avc3"].map(str::to_owned));
    }
    // Tone-mapping in software and subtitles work on frames in memory, so with either the GPU
    // only decodes and encodes.
    let on_gpu = accel
        .filter(|_| !video.tone_maps_in_software() && video.burn_in.is_none())
        .and_then(|api| {
            let tone_map = video.tone_map == Some(ToneMapper::Hardware);
            Some((api, api.gpu_filters(tone_map, size, pixel_format)?))
        });
    if let Some((api, filters)) = on_gpu {
        return VideoArgs {
            input: api.input(gpu, Decode::OnGpu),
            video: ["-map", "0:V:0", "-vf", &filters].map(str::to_owned).into(),
            codec,
        };
    }
    let dolby_vision = matches!(
        video.tone_map,
        Some(ToneMapper::Tonemapx { dolby_vision: true } | ToneMapper::DolbyVisionToHdr10)
    );
    // `hevc_qsv` and `hevc_cuvid` drop the RPU that `tonemapx` reshapes by.
    // ponytail: decoding 4K HEVC in software caps how many run at once; decode with `-hwaccel
    // vaapi`, `cuda` or `videotoolbox`, which keep the RPU, once that's measured too slow.
    let decode = if dolby_vision {
        Decode::InSoftware
    } else {
        Decode::ToMemory
    };
    let (tags, tone_map) = video.tone_map.map(software_tone_map).unzip();
    // ponytail: the size is from the stored picture, so a rotated phone clip is stretched; swap
    // its sides once probing reads rotation.
    let scale = size.map(|(width, height)| format!("scale={width}:{height}"));
    let upload = accel.map(|accel| accel.upload(pixel_format));
    // Scaling down first spares tone-mapping the pixels it drops. A subtitle instead goes after
    // tone-mapping, which would dim it, and before scaling, so it shrinks with the picture and an
    // image subtitle still fits the source size `overlay_graph` is given.
    let (before, after) = match video.burn_in {
        None => (filter_chain([tags, scale, tone_map, upload]), None),
        Some(_) => (
            filter_chain([tags, tone_map]),
            filter_chain([scale, upload]),
        ),
    };
    VideoArgs {
        input: accel.map_or_default(|accel| accel.input(gpu, decode)),
        video: transcoded_video(path, before, video.burn_in.as_ref(), after, seeks),
        codec,
    }
}

/// The filter that tags the frames `tone_map` reads, and the filters that then tone-map them in
/// software.
fn software_tone_map(tone_map: ToneMapper) -> (String, String) {
    match tone_map {
        ToneMapper::Tonemapx { dolby_vision: true } => {
            (DOLBY_VISION_TAGS.to_owned(), tonemapx(true))
        }
        ToneMapper::Tonemapx {
            dolby_vision: false,
        } => (HDR_TAGS.to_owned(), tonemapx(false)),
        ToneMapper::DolbyVisionToHdr10 => {
            (DOLBY_VISION_TAGS.to_owned(), TONEMAPX_TO_HDR10.to_owned())
        }
        // The GPU's filter reaches here only when its frames can't stay on it, so zscale stands
        // in.
        ToneMapper::Software | ToneMapper::Hardware => {
            (HDR_TAGS.to_owned(), ZSCALE_TONE_MAP.to_owned())
        }
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

/// Has `command` convert its audio as `conversion` says, if given.
pub(super) fn convert_audio(
    command: &mut tokio::process::Command,
    conversion: Option<AudioConversion>,
) {
    if let Some(AudioConversion { encoder, channels }) = conversion {
        command
            .args(["-c:a", encoder.ffmpeg_name(), "-ac"])
            .arg(channels.to_string());
    }
}

/// `command`, ffmpeg at `version` given its input and codecs, writing HLS with fMP4 to `dir`, as
/// the run started at file `n`: a file per keyframe, listed by [`hls_playlist`], and an init
/// segment named by [`hls_init`].
pub(super) fn hls_output(
    mut command: tokio::process::Command,
    version: &str,
    dir: &Path,
    n: usize,
) -> HlsRun {
    // ffmpeg runs inside `dir` and is also handed paths that include it, which would resolve twice.
    debug_assert!(
        dir.is_absolute(),
        "HLS output folder {} is relative",
        dir.display()
    );
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
        // An event playlist lists each file once it's whole; ffmpeg writes a VOD one only at the
        // end.
        .args(["-hls_time", "0.001", "-hls_playlist_type", "event"])
        .args(["-hls_flags", "temp_file", "-hls_fmp4_init_filename"])
        .arg(hls_init(n))
        // Or each run's fragments would be timed from where it started.
        .args([segment_options, "movflags=+frag_discont"])
        // Named by the run, as a run started elsewhere numbers its files from there.
        .arg("-hls_segment_filename")
        .arg(dir.join(format!("ffmpeg-{n}-%d.m4s")))
        .arg(dir.join(hls_playlist(n)))
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
