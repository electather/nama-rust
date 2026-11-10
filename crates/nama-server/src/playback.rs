//! Playback Plans: how the Server plays one Version on one Device, and why (ADR 0027).
//!
//! [`plan`] gathers what a plan is based on, probing the Version first if its Tracks aren't
//! current, and runs [`STEPS`] over it in order. Each step says which methods it allows and why,
//! and the plan keeps only what every step allows, so no step can widen what an earlier one ruled
//! out.
//!
//! Remuxes run as [`running::RunningPlans`], which serve their own files.
//!
//! Direct Play serves a Version's file as it is, over plain HTTP at `/media/versions/<id>` (ADR
//! 0007: media bytes are not RPC), to anyone with a session. A browser's `<video src>` presents
//! that session in its cookie (ADR 0014). Range requests let a `<video>` seek. Only Versions,
//! files a scan stored for a Library and Matched, are served, by id, so a client can never name a
//! path. The text subtitles a plan offers are served beside it ([`subtitles`]).

pub(crate) mod running;
pub(crate) mod subtitles;

use std::{collections::BTreeSet, sync::Arc};

use axum::{
    Router,
    extract::{Path, Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use tower_http::services::ServeFile;

use crate::{
    contents::Contents,
    ffmpeg::{Ffmpeg, ProbeError},
    scanner::{ProbeNowError, Scanner},
    tracks::{Hdr, Probe, Track, TrackKind, Tracks},
};
use running::{RunningPlans, StartError};

/// How a plan plays a Version, the one the Server prefers first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Method {
    DirectPlay,
    Remux,
    Transcode,
}

impl Method {
    const ALL: [Self; 3] = [Self::DirectPlay, Self::Remux, Self::Transcode];

    const fn name(self) -> &'static str {
        match self {
            Self::DirectPlay => "Direct Play",
            Self::Remux => "Remux",
            Self::Transcode => "Transcode",
        }
    }
}

/// How the Server will play one Version on one Device, and why.
#[derive(Debug, PartialEq)]
pub(crate) struct Plan {
    /// `None` when it can't be played.
    pub(crate) method: Option<Method>,
    /// Where to play it from, on this Server. `None` when it can't be played.
    pub(crate) url: Option<String>,
    /// Its video, then its audio. Empty when its Tracks aren't known.
    pub(crate) tracks: Vec<Track>,
    /// Why, one for each step, in the order they ran.
    pub(crate) reasons: Vec<String>,
    /// The text subtitles a player can turn on without a new plan.
    pub(crate) subtitles: Vec<Subtitle>,
}

/// A text subtitle a player shows as a WebVTT `<track>`.
#[derive(Debug, PartialEq)]
pub(crate) struct Subtitle {
    pub(crate) track: Track,
    /// Where it's served as WebVTT, on this Server.
    pub(crate) url: String,
}

/// What a Device says it can decode, as it found out just now. The Server never stores it.
#[derive(Debug, Default)]
pub(crate) struct DeviceDecoding {
    /// Those of the content types [`container`] and [`CODECS`] name that it decodes, such as
    /// `video/mp4` for a container or `video/mp4; codecs="avc1.640028"` for a codec.
    pub(crate) content_types: Vec<String>,
    /// Whether its display shows HDR.
    pub(crate) hdr: bool,
    /// Whether it plays HLS, natively or through Media Source Extensions, which every method but
    /// Direct Play needs.
    pub(crate) hls: bool,
}

impl DeviceDecoding {
    fn decodes(&self, content_type: &str) -> bool {
        self.content_types
            .iter()
            .any(|listed| listed == content_type)
    }
}

/// Plans playing the Version `version` on the Device `device`, which decodes `decoding`, by any
/// method but those that just `failed` there, probing the Version first unless its Tracks are
/// current, and starts the plan running in `running` if it needs ffmpeg. `None` if there is no
/// such Version.
#[expect(
    clippy::too_many_arguments,
    reason = "each is its own fact about the request, and a struct would only rename them"
)]
pub(crate) async fn plan(
    scanner: &Scanner,
    running: &RunningPlans,
    version: i64,
    device: i64,
    decoding: DeviceDecoding,
    failed: BTreeSet<Method>,
) -> Result<Option<Plan>, StartError> {
    let probe = match scanner.probe_now(version).await {
        Ok(probe) => Some(probe),
        Err(ProbeNowError::Probe(err)) => {
            if !matches!(*err, ProbeError::Unavailable) {
                tracing::warn!(
                    version,
                    "could not probe a Version to plan playing it: {err}"
                );
            }
            None
        }
        Err(ProbeNowError::NoSuchVersion(_)) => return Ok(None),
        Err(ProbeNowError::Database(err)) => return Err(err.into()),
    };
    let facts = Facts {
        version,
        probe,
        device: decoding,
        ffmpeg: running.can_run(),
        failed,
    };
    let mut plan = build(&facts, &STEPS);
    // Only a probed Version gets this far: without its Tracks, nothing rules out Direct Play.
    if plan.method == Some(Method::Remux)
        && let Some(probed) = &facts.probe
    {
        let Some(url) = running.remux(version, device, probed).await? else {
            return Ok(None);
        };
        plan.url = Some(url);
    }
    Ok(Some(plan))
}

/// What a plan is based on.
struct Facts {
    version: i64,
    /// What probing found in the Version, or `None` if it couldn't be probed.
    probe: Option<Probe>,
    device: DeviceDecoding,
    /// Whether the Server has ffmpeg and ffprobe, which every method but Direct Play needs
    /// (ADR 0026).
    ffmpeg: bool,
    /// The methods that just failed to play it on the Device. The Server never stores them.
    failed: BTreeSet<Method>,
}

/// What one step lets a plan be, and why.
struct Narrowing {
    allows: BTreeSet<Method>,
    reason: String,
}

type Step = fn(&Facts) -> Narrowing;

/// In ADR 0027's order. A new input is a new step here.
const STEPS: [Step; 4] = [
    version_tracks,
    device_decoding,
    server_ffmpeg,
    failed_on_device,
];

/// The plan for `facts` that every one of `steps` allows.
fn build(facts: &Facts, steps: &[Step]) -> Plan {
    let mut allowed = BTreeSet::from(Method::ALL);
    let mut reasons = Vec::with_capacity(steps.len());
    for step in steps {
        let Narrowing { allows, reason } = step(facts);
        allowed.retain(|method| allows.contains(method));
        reasons.push(reason);
    }
    // ponytail: Transcodes aren't served yet (#321).
    let method = allowed
        .into_iter()
        .find(|method| *method != Method::Transcode);
    Plan {
        method,
        // A Remux's URL is its running plan's, which `plan` starts.
        url: (method == Some(Method::DirectPlay)).then(|| direct_play_url(facts.version)),
        tracks: facts.probe.as_ref().map(played_tracks).unwrap_or_default(),
        reasons,
        subtitles: facts
            .probe
            .as_ref()
            .map(|probe| text_subtitles(facts.version, probe))
            .unwrap_or_default(),
    }
}

/// The step for the Version's own Tracks. None needs changing to play yet, so it allows every
/// method.
fn version_tracks(facts: &Facts) -> Narrowing {
    let reason = if facts.probe.is_some() {
        "Its Tracks need no change"
    } else {
        // ADR 0026: without ffprobe, a Server Direct Plays.
        "Its Tracks aren't known, so they're taken to need no change"
    };
    Narrowing {
        allows: BTreeSet::from(Method::ALL),
        reason: reason.to_owned(),
    }
}

/// The step for what the Device decodes: Direct Play only if it decodes the Version's container
/// and the Tracks the plan plays, and its display shows their HDR.
fn device_decoding(facts: &Facts) -> Narrowing {
    let Some(probe) = &facts.probe else {
        // ADR 0026, as for the Tracks step.
        return Narrowing {
            allows: BTreeSet::from(Method::ALL),
            reason: "Its Tracks aren't known, so this Device is taken to decode them".to_owned(),
        };
    };
    let container = undecodable_container(probe, &facts.device);
    let tracks = undecodable_tracks(probe, &facts.device);
    let cant: Vec<String> = container
        .into_iter()
        .chain(tracks.iter().cloned())
        .collect();
    let Some(cant) = joined(&cant, "or") else {
        return Narrowing {
            allows: BTreeSet::from(Method::ALL),
            reason: "This Device decodes its container and Tracks".to_owned(),
        };
    };
    let reason = format!("This Device can't decode its {cant}, so it can't be Direct Played");
    if !facts.device.hls {
        return Narrowing {
            allows: BTreeSet::new(),
            reason: format!("{reason}, and it can't play HLS, which anything else needs"),
        };
    }
    // A Remux keeps the Tracks as they are, so it helps only with the container.
    let allows = if tracks.is_empty() {
        BTreeSet::from([Method::Remux, Method::Transcode])
    } else {
        BTreeSet::from([Method::Transcode])
    };
    Narrowing { allows, reason }
}

/// The step for the Server's ffmpeg: without it, only Direct Play (ADR 0026).
fn server_ffmpeg(facts: &Facts) -> Narrowing {
    if facts.ffmpeg {
        Narrowing {
            allows: BTreeSet::from(Method::ALL),
            reason: "This Server has ffmpeg".to_owned(),
        }
    } else {
        Narrowing {
            allows: BTreeSet::from([Method::DirectPlay]),
            reason: "This Server lacks ffmpeg or ffprobe, so it can only Direct Play".to_owned(),
        }
    }
}

/// The step for what just failed to play on the Device: it rules out each method that did, so
/// the player can ask again without looping.
fn failed_on_device(facts: &Facts) -> Narrowing {
    let failed: Vec<String> = (facts.failed.iter())
        .map(|method| method.name().to_owned())
        .collect();
    let reason = match joined(&failed, "and") {
        Some(failed) => format!("{failed} failed on this Device"),
        None => "Nothing has failed to play it on this Device".to_owned(),
    };
    Narrowing {
        allows: BTreeSet::from(Method::ALL)
            .difference(&facts.failed)
            .copied()
            .collect(),
        reason,
    }
}

/// Each codec a Device can say it decodes, as ffprobe names it, with the content type a Device
/// lists for it and its name in plain words. `web/src/lib/device-decoding.ts` asks the browser
/// about these same types.
// ponytail: one common profile per codec, so a 10-bit or 4K Track may pass on a Device that
// decodes only the 8-bit HD one; check the Track's own profile once probing reads it.
const CODECS: [(&str, &str, &str); 14] = [
    ("h264", r#"video/mp4; codecs="avc1.640028""#, "H.264"),
    ("hevc", r#"video/mp4; codecs="hvc1.1.6.L120.90""#, "HEVC"),
    ("vp8", r#"video/webm; codecs="vp8""#, "VP8"),
    ("vp9", r#"video/webm; codecs="vp09.00.40.08""#, "VP9"),
    ("av1", r#"video/mp4; codecs="av01.0.08M.08""#, "AV1"),
    ("aac", r#"audio/mp4; codecs="mp4a.40.2""#, "AAC"),
    ("mp3", "audio/mpeg", "MP3"),
    ("ac3", r#"audio/mp4; codecs="ac-3""#, "AC-3"),
    ("eac3", r#"audio/mp4; codecs="ec-3""#, "E-AC-3"),
    ("opus", r#"audio/webm; codecs="opus""#, "Opus"),
    ("vorbis", r#"audio/webm; codecs="vorbis""#, "Vorbis"),
    ("flac", r#"audio/mp4; codecs="flac""#, "FLAC"),
    ("dts", r#"audio/mp4; codecs="dtsc""#, "DTS"),
    ("truehd", r#"audio/mp4; codecs="mlpa""#, "TrueHD"),
];

/// `probe`'s container in plain words, such as "Matroska container", if `device` can't decode it.
fn undecodable_container(probe: &Probe, device: &DeviceDecoding) -> Option<String> {
    match container(probe) {
        Some((content_type, _)) if device.decodes(content_type) => None,
        Some((_, name)) => Some(format!("{name} container")),
        None => Some(format!("{} container", probe.container)),
    }
}

/// What of the Tracks the plan plays `device` can't decode, in plain words, such as "HEVC video".
fn undecodable_tracks(probe: &Probe, device: &DeviceDecoding) -> Vec<String> {
    let mut cant = vec![];
    for track in played_tracks(probe) {
        let (kind, hdr) = match track.kind {
            TrackKind::Video { hdr, .. } => ("video", hdr),
            TrackKind::Audio { .. } => ("audio", None),
            TrackKind::Subtitle { .. } => ("subtitle", None),
        };
        match CODECS.into_iter().find(|(codec, ..)| *codec == track.codec) {
            Some((_, content_type, _)) if device.decodes(content_type) => {}
            Some((_, _, name)) => cant.push(format!("{name} {kind}")),
            None => cant.push(format!("{} {kind}", track.codec)),
        }
        if let Some(hdr) = hdr.filter(|_| !device.hdr) {
            cant.push(format!("{} picture", hdr_name(hdr)));
        }
    }
    cant
}

/// The content type of `probe`'s container and its name in plain words, if a browser can play it
/// at all. `web/src/lib/device-decoding.ts` asks the browser about these same types.
fn container(probe: &Probe) -> Option<(&'static str, &'static str)> {
    // ffprobe names WebM and Matroska alike; WebM is Matroska holding only these codecs.
    const WEBM: [&str; 6] = ["vp8", "vp9", "av1", "opus", "vorbis", "webvtt"];
    let webm = (probe.tracks.iter()).all(|track| WEBM.contains(&track.codec.as_str()));
    match probe.container.as_str() {
        "mov,mp4,m4a,3gp,3g2,mj2" => Some(("video/mp4", "MP4")),
        "matroska,webm" if webm => Some(("video/webm", "WebM")),
        "matroska,webm" => Some(("video/x-matroska", "Matroska")),
        _ => None,
    }
}

const fn hdr_name(hdr: Hdr) -> &'static str {
    match hdr {
        Hdr::Hdr10 => "HDR10",
        Hdr::Hlg => "HLG",
        Hdr::DolbyVision { .. } => "Dolby Vision",
    }
}

/// `items` as "a, b <conjunction> c", or `None` if there are none.
fn joined(items: &[String], conjunction: &str) -> Option<String> {
    let (last, rest) = items.split_last()?;
    Some(if rest.is_empty() {
        last.clone()
    } else {
        format!("{} {conjunction} {last}", rest.join(", "))
    })
}

/// The Tracks a browser plays from the file: its first video, and its [`played_audio`].
fn played_tracks(probe: &Probe) -> Vec<Track> {
    let video = (probe.tracks.iter()).find(|track| matches!(track.kind, TrackKind::Video { .. }));
    let audio = played_audio(probe).and_then(|n| audio_tracks(probe).nth(n));
    [video, audio].into_iter().flatten().cloned().collect()
}

/// Which of `probe`'s audio Tracks a browser plays, counting from 0: the one the file marks to
/// play by default, or else its first.
fn played_audio(probe: &Probe) -> Option<usize> {
    (audio_tracks(probe).position(|track| track.default))
        .or_else(|| audio_tracks(probe).next().map(|_| 0))
}

fn audio_tracks(probe: &Probe) -> impl Iterator<Item = &Track> {
    (probe.tracks.iter()).filter(|track| matches!(track.kind, TrackKind::Audio { .. }))
}

/// The Version's text subtitle Tracks.
fn text_subtitles(version: i64, probe: &Probe) -> Vec<Subtitle> {
    (probe.tracks.iter().enumerate())
        .filter(|(_, track)| track.is_text_subtitle())
        .map(|(position, track)| Subtitle {
            track: track.clone(),
            url: subtitles::url(version, position),
        })
        .collect()
}

fn direct_play_url(version: i64) -> String {
    format!("/media/versions/{version}")
}

/// Serves `GET /media/versions/<id>` with the file of the Version `id` in `contents`, and 404 if
/// there is none, and the text subtitles plans offer, read with `ffmpeg` and cached in
/// `subtitles`. Whether the caller has a session is up to the layers the router is put behind.
pub(crate) fn router(
    contents: Contents,
    tracks: Tracks,
    ffmpeg: Arc<Ffmpeg>,
    subtitles: subtitles::Cache,
) -> Router {
    let subtitles = subtitles::router(subtitles::Subtitles {
        contents: contents.clone(),
        tracks,
        ffmpeg,
        cache: subtitles,
    });
    Router::new()
        .route("/media/versions/{id}", get(serve))
        .with_state(contents)
        .merge(subtitles)
}

async fn serve(
    State(contents): State<Contents>,
    Path(id): Path<i64>,
    request: Request,
) -> Response {
    let path = match contents.version_file(id).await {
        Ok(Some((path, _))) => path,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            tracing::error!("could not look up the Version {id}: {err}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    // `ServeFile` answers `Range` (206 and `Content-Range`), sets `Accept-Ranges`, `ETag` and
    // `Last-Modified`, takes the `Content-Type` from the extension, and is 404 for a file gone
    // since the last scan.
    match ServeFile::new(&path).try_call(request).await {
        Ok(response) => response.into_response(),
        Err(err) => {
            tracing::error!("could not serve {path}: {err}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{
        DeviceDecoding, Facts, Method, Narrowing, Plan, STEPS, Subtitle, build, device_decoding,
        failed_on_device,
    };
    use crate::tracks::{Hdr, Probe, Track, TrackKind};

    fn track(codec: &str, default: bool, kind: TrackKind) -> Track {
        Track {
            codec: codec.to_owned(),
            language: None,
            title: None,
            default,
            forced: false,
            sdh: false,
            kind,
        }
    }

    fn video(codec: &str) -> Track {
        hdr_video(codec, None)
    }

    fn hdr_video(codec: &str, hdr: Option<Hdr>) -> Track {
        let kind = TrackKind::Video {
            width: Some(1920),
            height: Some(1080),
            frame_rate: Some(24.0),
            bit_depth: Some(8),
            hdr,
        };
        track(codec, false, kind)
    }

    fn audio(codec: &str, default: bool) -> Track {
        track(codec, default, TrackKind::Audio { channels: Some(2) })
    }

    const MP4: &str = "mov,mp4,m4a,3gp,3g2,mj2";
    const MKV: &str = "matroska,webm";

    /// What Firefox on an SDR display says it decodes.
    fn firefox() -> DeviceDecoding {
        let content_types = [
            "video/mp4",
            "video/webm",
            r#"video/mp4; codecs="avc1.640028""#,
            r#"video/webm; codecs="vp8""#,
            r#"video/webm; codecs="vp09.00.40.08""#,
            r#"video/mp4; codecs="av01.0.08M.08""#,
            r#"audio/mp4; codecs="mp4a.40.2""#,
            "audio/mpeg",
            r#"audio/webm; codecs="opus""#,
            r#"audio/webm; codecs="vorbis""#,
            r#"audio/mp4; codecs="flac""#,
        ];
        DeviceDecoding {
            content_types: content_types.map(str::to_owned).to_vec(),
            hdr: false,
            hls: true,
        }
    }

    fn facts(tracks: Option<Vec<Track>>) -> Facts {
        in_container(MP4, tracks)
    }

    fn in_container(container: &str, tracks: Option<Vec<Track>>) -> Facts {
        Facts {
            version: 7,
            probe: tracks.map(|tracks| Probe {
                container: container.to_owned(),
                duration_ms: Some(7_200_000),
                bitrate: Some(5_000_000),
                tracks,
            }),
            device: firefox(),
            ffmpeg: true,
            failed: BTreeSet::new(),
        }
    }

    #[test]
    fn an_h264_and_aac_mp4_direct_plays_from_its_versions_url_because_its_tracks_need_no_change() {
        let plan = build(
            &facts(Some(vec![video("h264"), audio("aac", true)])),
            &STEPS,
        );

        assert_eq!(
            plan,
            Plan {
                method: Some(Method::DirectPlay),
                url: Some("/media/versions/7".to_owned()),
                tracks: vec![video("h264"), audio("aac", true)],
                reasons: vec![
                    "Its Tracks need no change".to_owned(),
                    "This Device decodes its container and Tracks".to_owned(),
                    "This Server has ffmpeg".to_owned(),
                    "Nothing has failed to play it on this Device".to_owned(),
                ],
                subtitles: vec![],
            }
        );
    }

    #[test]
    fn a_version_the_device_cant_decode_cant_be_direct_played_and_the_plan_names_what() {
        let hdr10 = Some(Hdr::Hdr10);
        let cases = [
            (
                MKV,
                vec![video("hevc"), audio("aac", true)],
                "Matroska container or HEVC video",
            ),
            (MP4, vec![video("h264"), audio("dts", true)], "DTS audio"),
            (
                MP4,
                vec![hdr_video("av1", hdr10), audio("aac", true)],
                "HDR10 picture",
            ),
            (
                "avi",
                vec![video("mpeg4"), audio("pcm_s16le", true)],
                "avi container, mpeg4 video or pcm_s16le audio",
            ),
        ];
        for (container, tracks, cant) in cases {
            let plan = build(&in_container(container, Some(tracks)), &STEPS);

            assert_eq!(plan.method, None, "{cant}");
            assert_eq!(
                plan.reasons[1],
                format!("This Device can't decode its {cant}, so it can't be Direct Played")
            );
        }
    }

    #[test]
    fn a_device_decodes_what_its_content_types_and_display_say() {
        let mut hdr_display = firefox();
        hdr_display.hdr = true;
        let cases = [
            (MKV, vec![video("vp9"), audio("opus", true)], firefox()),
            (MKV, vec![video("vp8"), audio("vorbis", true)], firefox()),
            (MP4, vec![video("h264"), audio("mp3", true)], firefox()),
            (
                MP4,
                vec![hdr_video("av1", Some(Hdr::Hlg)), audio("flac", true)],
                hdr_display,
            ),
            // Only the Tracks the plan plays need decoding.
            (
                MP4,
                vec![video("h264"), audio("dts", false), audio("aac", true)],
                firefox(),
            ),
        ];
        for (container, tracks, device) in cases {
            let facts = Facts {
                device,
                ..in_container(container, Some(tracks.clone()))
            };

            let plan = build(&facts, &STEPS);

            assert_eq!(plan.method, Some(Method::DirectPlay), "{tracks:?}");
        }
    }

    #[test]
    fn only_a_transcode_helps_with_tracks_the_device_cant_decode() {
        let remux_or_transcode = BTreeSet::from([Method::Remux, Method::Transcode]);
        let transcode = BTreeSet::from([Method::Transcode]);
        let cases = [
            (
                MKV,
                vec![video("h264"), audio("aac", true)],
                remux_or_transcode,
            ),
            (
                MKV,
                vec![video("hevc"), audio("aac", true)],
                transcode.clone(),
            ),
            (
                MP4,
                vec![video("h264"), audio("dts", true)],
                transcode.clone(),
            ),
            (
                MP4,
                vec![hdr_video("av1", Some(Hdr::Hdr10)), audio("aac", true)],
                transcode,
            ),
        ];
        for (container, tracks, allowed) in cases {
            let narrowing = device_decoding(&in_container(container, Some(tracks.clone())));

            assert_eq!(narrowing.allows, allowed, "{tracks:?}");
        }
    }

    #[test]
    fn a_version_whose_tracks_arent_known_direct_plays_and_says_so() {
        let plan = build(&facts(None), &STEPS);

        assert_eq!(plan.method, Some(Method::DirectPlay));
        assert_eq!(plan.tracks, []);
        assert_eq!(
            plan.reasons,
            [
                "Its Tracks aren't known, so they're taken to need no change",
                "Its Tracks aren't known, so this Device is taken to decode them",
                "This Server has ffmpeg",
                "Nothing has failed to play it on this Device",
            ]
        );
    }

    #[test]
    fn the_plan_plays_the_default_audio_track_and_else_the_first() {
        let subtitle = track("subrip", true, TrackKind::Subtitle { image: false });
        let (first, default) = (audio("ac3", false), audio("aac", true));
        let cases = [
            (vec![video("h264"), first.clone(), default.clone()], default),
            (
                vec![first.clone(), video("h264"), audio("aac", false)],
                first,
            ),
        ];
        for (tracks, played) in cases {
            let with_subtitle = [tracks.clone(), vec![subtitle.clone()]].concat();

            let plan = build(&facts(Some(with_subtitle)), &STEPS);

            assert_eq!(plan.tracks, [video("h264"), played], "{tracks:?}");
        }
    }

    #[test]
    fn a_version_only_its_container_keeps_from_direct_playing_is_remuxed() {
        let plan = build(
            &in_container(MKV, Some(vec![video("h264"), audio("aac", true)])),
            &STEPS,
        );

        assert_eq!(plan.method, Some(Method::Remux));
        assert_eq!(plan.url, None, "a Remux's URL is its running plan's");
        assert_eq!(
            plan.reasons,
            [
                "Its Tracks need no change",
                "This Device can't decode its Matroska container, so it can't be Direct Played",
                "This Server has ffmpeg",
                "Nothing has failed to play it on this Device",
            ]
        );
    }

    #[test]
    fn each_method_that_just_failed_on_the_device_is_ruled_out_and_the_plan_says_so() {
        use Method::{DirectPlay, Remux, Transcode};
        let cases = [
            (
                vec![],
                vec![DirectPlay, Remux, Transcode],
                "Nothing has failed to play it on this Device",
            ),
            (
                vec![DirectPlay],
                vec![Remux, Transcode],
                "Direct Play failed on this Device",
            ),
            (
                vec![Remux, DirectPlay],
                vec![Transcode],
                "Direct Play and Remux failed on this Device",
            ),
        ];
        for (failed, allowed, reason) in cases {
            let facts = Facts {
                failed: BTreeSet::from_iter(failed),
                ..facts(None)
            };

            let narrowing = failed_on_device(&facts);

            assert_eq!(narrowing.allows, BTreeSet::from_iter(allowed), "{reason}");
            assert_eq!(narrowing.reason, reason);
        }
    }

    #[test]
    fn the_plan_offers_the_text_subtitles_by_their_place_in_the_version_but_not_image_or_teletext()
    {
        let english = track("subrip", false, TrackKind::Subtitle { image: false });
        let pgs = track(
            "hdmv_pgs_subtitle",
            false,
            TrackKind::Subtitle { image: true },
        );
        let teletext = track("dvb_teletext", false, TrackKind::Subtitle { image: false });
        let french = Track {
            language: Some("fre".to_owned()),
            ..english.clone()
        };
        let tracks = vec![
            video("h264"),
            english.clone(),
            audio("aac", true),
            pgs,
            teletext,
            french.clone(),
        ];

        let plan = build(&facts(Some(tracks)), &STEPS);

        assert_eq!(
            plan.subtitles,
            [
                Subtitle {
                    track: english,
                    url: "/media/versions/7/subtitles/1".to_owned(),
                },
                Subtitle {
                    track: french,
                    url: "/media/versions/7/subtitles/5".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn without_ffmpeg_on_the_server_or_hls_on_the_device_a_version_isnt_remuxed() {
        let tracks = Some(vec![video("h264"), audio("aac", true)]);
        let no_ffmpeg = Facts {
            ffmpeg: false,
            ..in_container(MKV, tracks.clone())
        };
        let mut no_hls = in_container(MKV, tracks);
        no_hls.device.hls = false;

        let without_ffmpeg = build(&no_ffmpeg, &STEPS);
        let without_hls = build(&no_hls, &STEPS);

        assert_eq!(without_ffmpeg.method, None);
        assert_eq!(
            without_ffmpeg.reasons[2],
            "This Server lacks ffmpeg or ffprobe, so it can only Direct Play"
        );
        assert_eq!(without_hls.method, None);
        assert_eq!(
            without_hls.reasons[1],
            "This Device can't decode its Matroska container, so it can't be Direct Played, \
             and it can't play HLS, which anything else needs"
        );
    }

    fn rules_out_direct_play(_: &Facts) -> Narrowing {
        Narrowing {
            allows: BTreeSet::from([Method::Remux, Method::Transcode]),
            reason: "ruled out Direct Play".to_owned(),
        }
    }

    fn allows_every_method(_: &Facts) -> Narrowing {
        Narrowing {
            allows: BTreeSet::from(Method::ALL),
            reason: "allowed every method".to_owned(),
        }
    }

    #[test]
    fn a_later_step_cant_allow_what_an_earlier_step_ruled_out() {
        let plan = build(&facts(None), &[rules_out_direct_play, allows_every_method]);

        assert_eq!(plan.method, Some(Method::Remux));
        assert_eq!(
            plan.reasons,
            ["ruled out Direct Play", "allowed every method"]
        );
    }
}
