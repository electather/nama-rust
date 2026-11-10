//! Playback Plans: how the Server plays one Version on one Device, and why (ADR 0027).
//!
//! [`plan`] gathers what a plan is based on, probing the Version first if its Tracks aren't
//! current, and runs [`STEPS`] over it in order. Each step says which methods it allows and why,
//! and the plan keeps only what every step allows, so no step can widen what an earlier one ruled
//! out.
//!
//! Each playable plan is kept in [`running::RunningPlans`] until it stops, and Remuxes serve their
//! own files there.
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
use nama_proto::nama::v1 as proto;
use tower_http::services::ServeFile;

use crate::{
    contents::Contents,
    ffmpeg::{Ffmpeg, ProbeError, VideoLimit},
    scanner::{ProbeNowError, Scanner},
    tracks::{Hdr, Probe, Track, TrackKind, Tracks, track_to_proto},
    users::{Caller, User},
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

/// A Playback Limit below Original: the picture a Transcode fits the video in, and the bitrate it
/// sends it at, which Users never see.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PlaybackLimit {
    name: &'static str,
    width: i64,
    height: i64,
    bits_per_second: u32,
}

impl PlaybackLimit {
    pub(crate) const P1080: Self = Self::new("1080p", 1920, 1080, 8_000_000);
    pub(crate) const P720: Self = Self::new("720p", 1280, 720, 4_000_000);
    // 854 is 480 at 16:9, made even.
    pub(crate) const P480: Self = Self::new("480p", 854, 480, 1_500_000);

    const fn new(name: &'static str, width: i64, height: i64, bits_per_second: u32) -> Self {
        Self {
            name,
            width,
            height,
            bits_per_second,
        }
    }
}

/// How the Server will play one Version on one Device, and why.
#[derive(Debug, PartialEq)]
pub(crate) struct Plan {
    /// What [`RunningPlans`] keeps it by, once [`plan`] started it; `None` when it can't be
    /// played.
    pub(crate) id: Option<u64>,
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
    /// How many audio channels it plays out at once, such as 6 for 5.1; 0 when it doesn't say.
    pub(crate) audio_channels: u32,
}

impl DeviceDecoding {
    fn decodes(&self, content_type: &str) -> bool {
        self.content_types
            .iter()
            .any(|listed| listed == content_type)
    }
}

/// Plans playing the Version `version` on `caller`'s Device, which decodes `decoding`, by any
/// method but those that just `failed` there, with the audio Track at the place `audio` in the
/// Version's Tracks, or else its own, within the Playback Limit `limit`, `None` being Original,
/// probing the Version first unless its Tracks are current, and keeps a playable plan in
/// `running`, which starts its ffmpeg if it needs one. `None` if there is no such Version.
#[expect(
    clippy::too_many_arguments,
    reason = "each is its own fact about the request or a service it needs, and a struct would \
              only rename them"
)]
pub(crate) async fn plan(
    scanner: &Scanner,
    running: &RunningPlans,
    caller: &Caller,
    version: i64,
    decoding: DeviceDecoding,
    failed: BTreeSet<Method>,
    audio: Option<usize>,
    limit: Option<PlaybackLimit>,
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
        audio,
        limit,
    };
    let mut plan = build(&facts, &STEPS);
    let Some(playback) = playback(caller, version, &plan) else {
        return Ok(Some(plan));
    };
    // Only a probed Version gets this far: without its Tracks, nothing rules out Direct Play.
    let started = match (plan.method, &facts.probe) {
        (Some(Method::Remux), Some(probed)) => {
            let (played, aac) = (played_audio(&facts), channels_as_aac(plan.method, &facts));
            running.remux(playback, probed, played, aac).await?
        }
        (Some(Method::Transcode), Some(probed)) => {
            let converted_audio = played_audio(&facts).zip(channels_as_aac(plan.method, &facts));
            let video = video_limit(&facts);
            running
                .transcode(playback, probed, converted_audio, video)
                .await?
        }
        _ => {
            plan.id = Some(running.direct_play(playback));
            return Ok(Some(plan));
        }
    };
    let Some((id, url)) = started else {
        return Ok(None);
    };
    plan.id = Some(id);
    plan.url = Some(url);
    Ok(Some(plan))
}

/// What Observers are told of `caller` playing the Version `version` by `plan`, or `None` if it
/// can't be played.
fn playback(caller: &Caller, version: i64, plan: &Plan) -> Option<proto::Playback> {
    let method = plan.method?;
    Some(proto::Playback {
        user_id: caller.user.id,
        device_id: caller.device.id,
        version_id: version,
        method: method_to_proto(Some(method)).into(),
        tracks: plan.tracks.iter().map(track_to_proto).collect(),
        reasons: plan.reasons.clone(),
        ..Default::default()
    })
}

/// Whose playback events `viewer` may see: everyone's (`None`) with `ADMIN`, or else only their own.
pub(crate) fn visible_to(viewer: &User) -> Option<i64> {
    (!viewer.permissions.allow(proto::Permission::Admin)).then_some(viewer.id)
}

/// `method` as the API names it, `None` being a plan that can't be played.
pub(crate) const fn method_to_proto(method: Option<Method>) -> proto::PlaybackMethod {
    match method {
        None => proto::PlaybackMethod::PLAYBACK_METHOD_UNSPECIFIED,
        Some(Method::DirectPlay) => proto::PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY,
        Some(Method::Remux) => proto::PlaybackMethod::PLAYBACK_METHOD_REMUX,
        Some(Method::Transcode) => proto::PlaybackMethod::PLAYBACK_METHOD_TRANSCODE,
    }
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
    /// The audio Track the User chose, by its place in the Version's Tracks, if they chose one.
    audio: Option<usize>,
    /// The User's Playback Limit, `None` being Original.
    limit: Option<PlaybackLimit>,
}

/// What one step lets a plan be, and why.
struct Narrowing {
    allows: BTreeSet<Method>,
    reason: String,
}

type Step = fn(&Facts) -> Narrowing;

/// In ADR 0027's order. A new input is a new step here.
const STEPS: [Step; 6] = [
    version_tracks,
    device_decoding,
    server_ffmpeg,
    failed_on_device,
    chosen_audio,
    playback_limit,
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
    let method = allowed.into_iter().next();
    let tracks = played_as(method, facts);
    Plan {
        id: None,
        method,
        // A Remux's or Transcode's URL is its running plan's, which `plan` starts.
        url: (method == Some(Method::DirectPlay)).then(|| direct_play_url(facts.version)),
        tracks,
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
/// and the Tracks the plan plays, and its display shows their HDR, and a Remux only if it decodes
/// the video, since a Remux converts only the audio.
fn device_decoding(facts: &Facts) -> Narrowing {
    let Some(probe) = &facts.probe else {
        // ADR 0026, as for the Tracks step.
        return Narrowing {
            allows: BTreeSet::from(Method::ALL),
            reason: "Its Tracks aren't known, so this Device is taken to decode them".to_owned(),
        };
    };
    let cant: Vec<String> = undecodable_container(probe, &facts.device)
        .into_iter()
        .chain(undecodable_tracks(facts))
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
    let decodes_video =
        played_video(probe).is_none_or(|video| undecodable(video, &facts.device).is_empty());
    if !decodes_video {
        return Narrowing {
            allows: BTreeSet::from([Method::Transcode]),
            reason: format!(
                "{reason} or Remuxed, and a Transcode converts it to {}",
                transcode_in_words(facts)
            ),
        };
    }
    let reason = match aac_channels(facts) {
        Some(channels) => format!(
            "{reason}, and a Remux converts that audio to {}",
            aac_in_words(facts, channels, Method::Remux)
        ),
        None => reason,
    };
    Narrowing {
        allows: BTreeSet::from([Method::Remux, Method::Transcode]),
        reason,
    }
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

/// The step for the audio Track the User chose: Direct Play only if it's the one browsers play
/// from the file, since only Safari switches audio inside a file.
fn chosen_audio(facts: &Facts) -> Narrowing {
    match &facts.probe {
        Some(probe) if played_audio(facts) != file_audio(probe) => Narrowing {
            allows: BTreeSet::from([Method::Remux, Method::Transcode]),
            reason: "The audio Track chosen isn't the one browsers play from its file, so it \
                     can't be Direct Played"
                .to_owned(),
        },
        _ => Narrowing {
            allows: BTreeSet::from(Method::ALL),
            reason: "It plays the file's own audio Track".to_owned(),
        },
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

/// The step for the User's Playback Limit: only a Transcode, which scales the picture down to fit
/// it, when the Version's picture is larger.
fn playback_limit(facts: &Facts) -> Narrowing {
    let Some(PlaybackLimit { name, .. }) = facts.limit else {
        return Narrowing {
            allows: BTreeSet::from(Method::ALL),
            reason: "The Playback Limit is Original".to_owned(),
        };
    };
    if let Some((width, height)) = scaled_size(facts) {
        return Narrowing {
            allows: BTreeSet::from([Method::Transcode]),
            reason: format!(
                "Its picture is larger than the {name} Playback Limit, so a Transcode scales it \
                 down to {width}×{height}"
            ),
        };
    }
    let reason = if picture(facts).is_some() {
        format!("Its picture fits the {name} Playback Limit")
    } else {
        format!("Its picture size isn't known, so it's taken to fit the {name} Playback Limit")
    };
    Narrowing {
        allows: BTreeSet::from(Method::ALL),
        reason,
    }
}

/// How a Transcode keeps the played video within the User's Playback Limit, if they picked one.
fn video_limit(facts: &Facts) -> Option<VideoLimit> {
    let PlaybackLimit {
        bits_per_second, ..
    } = facts.limit?;
    Some(VideoLimit {
        size: scaled_size(facts),
        bits_per_second,
    })
}

/// The size a Transcode scales the played video to, to fit the User's Playback Limit, if it's
/// larger.
fn scaled_size(facts: &Facts) -> Option<(i64, i64)> {
    fitted(picture(facts)?, facts.limit?)
}

/// The width and height of the played video, if the file says.
fn picture(facts: &Facts) -> Option<(i64, i64)> {
    match played_video(facts.probe.as_ref()?)?.kind {
        TrackKind::Video {
            width: Some(width),
            height: Some(height),
            ..
        } => Some((width, height)),
        TrackKind::Video { .. } | TrackKind::Audio { .. } | TrackKind::Subtitle { .. } => None,
    }
}

/// `picture`, a width and height, scaled down to fit `limit` with its shape kept, if it's larger.
fn fitted((width, height): (i64, i64), limit: PlaybackLimit) -> Option<(i64, i64)> {
    if width <= limit.width && height <= limit.height {
        return None;
    }
    let rounded = |numerator: i64, denominator: i64| (numerator + denominator / 2) / denominator;
    let (width, height) = if width * limit.height > height * limit.width {
        (limit.width, rounded(height * limit.width, width))
    } else {
        (rounded(width * limit.height, height), limit.height)
    };
    // H.264 in 4:2:0 needs even sides.
    Some((width / 2 * 2, height / 2 * 2))
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

/// What of the Tracks the plan plays the Device can't decode, in plain words, such as "HEVC
/// video".
fn undecodable_tracks(facts: &Facts) -> Vec<String> {
    (played_tracks(facts).iter())
        .flat_map(|track| undecodable(track, &facts.device))
        .collect()
}

/// What of `track` `device` can't decode, in plain words, such as "HEVC video" and "HDR10 picture".
fn undecodable(track: &Track, device: &DeviceDecoding) -> Vec<String> {
    let mut cant = vec![];
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
    cant
}

/// What a Transcode of the Version makes, in plain words, such as "H.264 and stereo AAC".
fn transcode_in_words(facts: &Facts) -> String {
    let audio = converted_channels(facts).map(|channels| {
        let aac = aac_in_words(facts, channels, Method::Transcode);
        format!(" and {aac}")
    });
    // ponytail: a Transcode doesn't tone-map HDR to SDR yet; #323 does, in software.
    let washed_out = (facts.probe.as_ref().and_then(played_video)).and_then(|video| {
        let TrackKind::Video { hdr: Some(hdr), .. } = video.kind else {
            return None;
        };
        Some(format!(
            ", without tone-mapping its {} picture, so its colours look washed out",
            hdr_name(hdr)
        ))
    });
    format!(
        "H.264{}{}",
        audio.unwrap_or_default(),
        washed_out.unwrap_or_default()
    )
}

/// The Tracks a plan by `method` plays, once converted, or as they
/// are when it can't be played.
fn played_as(method: Option<Method>, facts: &Facts) -> Vec<Track> {
    let tracks = played_tracks(facts);
    let tracks = match channels_as_aac(method, facts) {
        Some(channels) => converted_to_aac(tracks, channels),
        None => tracks,
    };
    if method == Some(Method::Transcode) {
        converted_to_h264(tracks, scaled_size(facts))
    } else {
        tracks
    }
}

/// How many channels a plan by `method` converts the played audio to, as AAC, if any.
fn channels_as_aac(method: Option<Method>, facts: &Facts) -> Option<i64> {
    match method {
        None | Some(Method::DirectPlay) => None,
        Some(Method::Remux) => aac_channels(facts),
        Some(Method::Transcode) => converted_channels(facts),
    }
}

/// How many channels a Remux converts the played audio to, as AAC, if the Device can't decode
/// it, as [`converted_channels`] says.
fn aac_channels(facts: &Facts) -> Option<i64> {
    let audio = played_audio_track(facts)?;
    if undecodable(audio, &facts.device).is_empty() {
        return None;
    }
    converted_channels(facts)
}

/// How many channels the played audio has once converted to AAC for the Device, if it has
/// audio: as many as it has, up to 5.1 if the Device plays 5.1 and stereo otherwise.
fn converted_channels(facts: &Facts) -> Option<i64> {
    played_audio_track(facts)?;
    let most = if facts.device.audio_channels >= 6 {
        6
    } else {
        2
    };
    // Stereo when the file doesn't say, which every Device plays.
    Some(played_channels(facts).unwrap_or(2).min(most))
}

/// How many channels the played audio has, if the file says.
fn played_channels(facts: &Facts) -> Option<i64> {
    let TrackKind::Audio { channels } = played_audio_track(facts)?.kind else {
        return None;
    };
    channels
}

/// `tracks` with their audio converted to AAC with `channels` channels.
fn converted_to_aac(mut tracks: Vec<Track>, channels: i64) -> Vec<Track> {
    for track in &mut tracks {
        if let TrackKind::Audio { channels: played } = &mut track.kind {
            "aac".clone_into(&mut track.codec);
            *played = Some(channels);
        }
    }
    tracks
}

/// `tracks` with their video converted to 8-bit H.264, as a Transcode makes it, scaled to `size`
/// if given.
fn converted_to_h264(mut tracks: Vec<Track>, size: Option<(i64, i64)>) -> Vec<Track> {
    for track in &mut tracks {
        if let TrackKind::Video {
            bit_depth,
            width,
            height,
            ..
        } = &mut track.kind
        {
            "h264".clone_into(&mut track.codec);
            *bit_depth = Some(8);
            if let Some((scaled_width, scaled_height)) = size {
                (*width, *height) = (Some(scaled_width), Some(scaled_height));
            }
        }
    }
    tracks
}

/// AAC with `channels` channels in plain words, such as "5.1 AAC", saying why when that's fewer
/// than the played audio has, which `method` converts it to.
fn aac_in_words(facts: &Facts, channels: i64, method: Method) -> String {
    let name = match channels {
        1 => "mono".to_owned(),
        2 => "stereo".to_owned(),
        6 => "5.1".to_owned(),
        n => format!("{n}-channel"),
    };
    let why = if channels >= played_channels(facts).unwrap_or_default() {
        ""
    } else if facts.device.audio_channels >= 6 {
        &format!(", the most a {} makes", method.name())
    } else if facts.device.audio_channels == 0 {
        ", as this Device doesn't say it plays more"
    } else {
        ", as this Device doesn't play 5.1"
    };
    format!("{name} AAC{why}")
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

/// The Tracks the plan plays: the Version's first video, and its [`played_audio`]. Empty when
/// they aren't known.
fn played_tracks(facts: &Facts) -> Vec<Track> {
    let Some(probe) = &facts.probe else {
        return vec![];
    };
    [played_video(probe), played_audio_track(facts)]
        .into_iter()
        .flatten()
        .cloned()
        .collect()
}

/// The first video Track, which a browser plays.
fn played_video(probe: &Probe) -> Option<&Track> {
    (probe.tracks.iter()).find(|track| matches!(track.kind, TrackKind::Video { .. }))
}

/// The Track [`played_audio`] counts to.
fn played_audio_track(facts: &Facts) -> Option<&Track> {
    let probe = facts.probe.as_ref()?;
    played_audio(facts).and_then(|n| audio_tracks(probe).nth(n))
}

/// Which of the Version's audio Tracks the plan plays, counting only its audio from 0: the one
/// chosen, or else [`file_audio`]. A choice that isn't an audio Track, as when the Version
/// changed since the User saw it, is the same as none.
fn played_audio(facts: &Facts) -> Option<usize> {
    let probe = facts.probe.as_ref()?;
    let chosen = (facts.audio).filter(|&place| probe.tracks.get(place).is_some_and(is_audio));
    let audio_before = |place| {
        (probe.tracks.iter().take(place))
            .filter(|track| is_audio(track))
            .count()
    };
    chosen.map(audio_before).or_else(|| file_audio(probe))
}

/// Which of `probe`'s audio Tracks a browser plays from the file, counting from 0: the one the
/// file marks to play by default, or else its first.
fn file_audio(probe: &Probe) -> Option<usize> {
    (audio_tracks(probe).position(|track| track.default))
        .or_else(|| audio_tracks(probe).next().map(|_| 0))
}

fn audio_tracks(probe: &Probe) -> impl Iterator<Item = &Track> {
    (probe.tracks.iter()).filter(|track| is_audio(track))
}

const fn is_audio(track: &Track) -> bool {
    matches!(track.kind, TrackKind::Audio { .. })
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
/// there is none, marking the caller's Direct Plays of it in `running` asked for, and the text
/// subtitles plans offer, read with `ffmpeg` and cached in `subtitles`. Whether the caller has a
/// session is up to the layers the router is put behind.
pub(crate) fn router(
    contents: Contents,
    running: RunningPlans,
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
        .with_state((contents, running))
        .merge(subtitles)
}

async fn serve(
    State((contents, running)): State<(Contents, RunningPlans)>,
    Path(id): Path<i64>,
    request: Request,
) -> Response {
    if let Some(caller) = request.extensions().get::<Caller>() {
        running.ask_direct_plays(id, caller.device.id);
    }
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
        DeviceDecoding, Facts, Method, Narrowing, Plan, PlaybackLimit, STEPS, Subtitle,
        aac_channels, build, device_decoding, failed_on_device, fitted, played_audio,
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
        audio_with(codec, default, Some(2))
    }

    fn audio_with(codec: &str, default: bool, channels: Option<i64>) -> Track {
        track(codec, default, TrackKind::Audio { channels })
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
            audio_channels: 2,
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
            audio: None,
            limit: None,
        }
    }

    fn subtitle() -> Track {
        track("subrip", true, TrackKind::Subtitle { image: false })
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
                id: None,
                method: Some(Method::DirectPlay),
                url: Some("/media/versions/7".to_owned()),
                tracks: vec![video("h264"), audio("aac", true)],
                reasons: vec![
                    "Its Tracks need no change".to_owned(),
                    "This Device decodes its container and Tracks".to_owned(),
                    "This Server has ffmpeg".to_owned(),
                    "Nothing has failed to play it on this Device".to_owned(),
                    "It plays the file's own audio Track".to_owned(),
                    "The Playback Limit is Original".to_owned(),
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

            assert_eq!(plan.method, Some(Method::Transcode), "{cant}");
            let cant_direct_play =
                format!("This Device can't decode its {cant}, so it can't be Direct Played");
            assert!(
                plan.reasons[1].starts_with(&cant_direct_play),
                "{}",
                plan.reasons[1]
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
    fn only_a_transcode_helps_with_video_the_device_cant_decode() {
        let remux_or_transcode = BTreeSet::from([Method::Remux, Method::Transcode]);
        let transcode = BTreeSet::from([Method::Transcode]);
        let cases = [
            (
                MKV,
                vec![video("h264"), audio("aac", true)],
                remux_or_transcode.clone(),
            ),
            (
                MKV,
                vec![video("hevc"), audio("aac", true)],
                transcode.clone(),
            ),
            (
                MP4,
                vec![video("h264"), audio("dts", true)],
                remux_or_transcode,
            ),
            (
                MP4,
                vec![video("hevc"), audio("dts", true)],
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
                "It plays the file's own audio Track",
                "The Playback Limit is Original",
            ]
        );
    }

    #[test]
    fn the_plan_plays_the_default_audio_track_and_else_the_first() {
        let (first, default) = (audio("mp3", false), audio("aac", true));
        let cases = [
            (vec![video("h264"), first.clone(), default.clone()], default),
            (
                vec![first.clone(), video("h264"), audio("aac", false)],
                first,
            ),
        ];
        for (tracks, played) in cases {
            let with_subtitle = [tracks.clone(), vec![subtitle()]].concat();

            let plan = build(&facts(Some(with_subtitle)), &STEPS);

            assert_eq!(plan.tracks, [video("h264"), played], "{tracks:?}");
        }
    }

    #[test]
    fn a_chosen_audio_track_other_than_the_files_own_is_remuxed_and_the_plan_says_so() {
        let (english, commentary) = (audio("aac", true), audio("aac", false));
        let facts = Facts {
            audio: Some(3),
            ..facts(Some(vec![
                video("h264"),
                english,
                subtitle(),
                commentary.clone(),
            ]))
        };

        let plan = build(&facts, &STEPS);

        assert_eq!(plan.method, Some(Method::Remux));
        assert_eq!(plan.tracks, [video("h264"), commentary]);
        assert_eq!(played_audio(&facts), Some(1), "counting only audio");
        assert_eq!(
            plan.reasons[4],
            "The audio Track chosen isn't the one browsers play from its file, so it can't be \
             Direct Played"
        );
    }

    #[test]
    fn choosing_the_files_own_audio_track_or_no_audio_track_plays_the_files_own() {
        let (mp3, english) = (audio("mp3", false), audio("aac", true));
        let tracks = vec![video("h264"), mp3, english.clone(), subtitle()];
        for chosen in [Some(2), Some(0), Some(3), Some(9)] {
            let facts = Facts {
                audio: chosen,
                ..facts(Some(tracks.clone()))
            };

            let plan = build(&facts, &STEPS);

            assert_eq!(plan.method, Some(Method::DirectPlay), "{chosen:?}");
            assert_eq!(plan.tracks, [video("h264"), english.clone()]);
        }
    }

    #[test]
    fn a_chosen_audio_track_the_device_cant_decode_is_converted_to_aac() {
        let facts = Facts {
            audio: Some(2),
            ..facts(Some(vec![
                video("h264"),
                audio("aac", true),
                audio("dts", false),
            ]))
        };

        let plan = build(&facts, &STEPS);

        assert_eq!(plan.tracks[1], audio("aac", false));
        assert_eq!(aac_channels(&facts), Some(2));
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
                "It plays the file's own audio Track",
                "The Playback Limit is Original",
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
    fn a_direct_play_or_remux_offers_the_text_subtitles_by_their_place_but_not_image_or_teletext() {
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

        let offered = [
            Subtitle {
                track: english,
                url: "/media/versions/7/subtitles/1".to_owned(),
            },
            Subtitle {
                track: french,
                url: "/media/versions/7/subtitles/5".to_owned(),
            },
        ];

        for (container, method) in [(MP4, Method::DirectPlay), (MKV, Method::Remux)] {
            let plan = build(&in_container(container, Some(tracks.clone())), &STEPS);

            assert_eq!(plan.method, Some(method), "{container}");
            assert_eq!(plan.subtitles, offered, "{container}");
        }
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

    #[test]
    fn audio_the_device_cant_decode_is_remuxed_to_aac_and_the_plan_says_so() {
        let dts = audio_with("dts", true, Some(6));
        let mut facts = in_container(MKV, Some(vec![video("h264"), dts.clone()]));
        facts.device.audio_channels = 6;

        let plan = build(&facts, &STEPS);

        assert_eq!(plan.method, Some(Method::Remux));
        let aac = Track {
            codec: "aac".to_owned(),
            ..dts
        };
        assert_eq!(plan.tracks, [video("h264"), aac]);
        assert_eq!(
            plan.reasons[1],
            "This Device can't decode its Matroska container or DTS audio, so it can't be Direct \
             Played, and a Remux converts that audio to 5.1 AAC"
        );
    }

    #[test]
    fn converted_audio_has_as_many_channels_as_the_device_plays_up_to_5_1() {
        let cases = [
            (Some(6), 6, Some(6), "5.1 AAC"),
            (Some(8), 8, Some(6), "5.1 AAC, the most a Remux makes"),
            (
                Some(6),
                2,
                Some(2),
                "stereo AAC, as this Device doesn't play 5.1",
            ),
            (
                Some(6),
                4,
                Some(2),
                "stereo AAC, as this Device doesn't play 5.1",
            ),
            (
                Some(6),
                0,
                Some(2),
                "stereo AAC, as this Device doesn't say it plays more",
            ),
            (Some(2), 6, Some(2), "stereo AAC"),
            (Some(1), 6, Some(1), "mono AAC"),
            (None, 6, Some(2), "stereo AAC"),
        ];
        for (source, plays, converted, to) in cases {
            let mut facts = in_container(
                MP4,
                Some(vec![video("h264"), audio_with("dts", true, source)]),
            );
            facts.device.audio_channels = plays;

            let plan = build(&facts, &STEPS);

            assert_eq!(
                plan.tracks[1],
                audio_with("aac", true, converted),
                "{source:?} on {plays}"
            );
            assert_eq!(
                plan.reasons[1],
                format!(
                    "This Device can't decode its DTS audio, so it can't be Direct Played, and a \
                     Remux converts that audio to {to}"
                ),
            );
        }
    }

    #[test]
    fn video_the_device_cant_decode_is_transcoded_to_h264_and_aac_and_the_plan_says_so() {
        let hevc = Track {
            kind: TrackKind::Video {
                width: Some(1920),
                height: Some(1080),
                frame_rate: Some(24.0),
                bit_depth: Some(10),
                hdr: None,
            },
            ..video("hevc")
        };
        let facts = in_container(MKV, Some(vec![hevc, audio_with("aac", true, Some(6))]));

        let plan = build(&facts, &STEPS);

        assert_eq!(plan.method, Some(Method::Transcode));
        assert_eq!(plan.url, None, "a Transcode's URL is its running plan's");
        assert_eq!(
            plan.tracks,
            [video("h264"), audio_with("aac", true, Some(2))]
        );
        assert_eq!(
            plan.reasons[1],
            "This Device can't decode its Matroska container or HEVC video, so it can't be \
             Direct Played or Remuxed, and a Transcode converts it to H.264 and stereo AAC, as \
             this Device doesn't play 5.1"
        );
    }

    #[test]
    fn a_transcode_of_hdr_video_says_its_colours_look_washed_out() {
        let facts = in_container(MP4, Some(vec![hdr_video("hevc", Some(Hdr::Hdr10))]));

        let plan = build(&facts, &STEPS);

        assert_eq!(plan.method, Some(Method::Transcode));
        assert_eq!(
            plan.reasons[1],
            "This Device can't decode its HEVC video or HDR10 picture, so it can't be Direct \
             Played or Remuxed, and a Transcode converts it to H.264, without tone-mapping its \
             HDR10 picture, so its colours look washed out"
        );
    }

    #[test]
    fn a_transcode_of_a_version_without_audio_says_only_h264() {
        let facts = in_container(MP4, Some(vec![video("hevc")]));

        let plan = build(&facts, &STEPS);

        assert_eq!(plan.method, Some(Method::Transcode));
        assert_eq!(
            plan.reasons[1],
            "This Device can't decode its HEVC video, so it can't be Direct Played or Remuxed, \
             and a Transcode converts it to H.264"
        );
    }

    #[test]
    fn audio_the_device_decodes_is_copied_into_a_remux() {
        let facts = in_container(MKV, Some(vec![video("h264"), audio("aac", true)]));

        let plan = build(&facts, &STEPS);

        assert_eq!(plan.method, Some(Method::Remux));
        assert_eq!(plan.tracks, [video("h264"), audio("aac", true)]);
        assert_eq!(aac_channels(&facts), None);
    }

    #[test]
    fn a_playback_limit_below_the_versions_picture_transcodes_it_down_to_fit_and_says_so() {
        let facts = Facts {
            limit: Some(PlaybackLimit::P720),
            ..facts(Some(vec![video("h264"), audio("aac", true)]))
        };

        let plan = build(&facts, &STEPS);

        assert_eq!(plan.method, Some(Method::Transcode));
        let scaled = TrackKind::Video {
            width: Some(1280),
            height: Some(720),
            frame_rate: Some(24.0),
            bit_depth: Some(8),
            hdr: None,
        };
        assert_eq!(
            plan.tracks,
            [track("h264", false, scaled), audio("aac", true)]
        );
        assert_eq!(
            plan.reasons[5],
            "Its picture is larger than the 720p Playback Limit, so a Transcode scales it down to \
             1280×720"
        );
    }

    #[test]
    fn a_playback_limit_the_versions_picture_fits_or_original_leaves_it_direct_played() {
        let probed = Some(vec![video("h264"), audio("aac", true)]);
        let cases = [
            (
                Some(PlaybackLimit::P1080),
                probed.clone(),
                "Its picture fits the 1080p Playback Limit",
            ),
            (None, probed, "The Playback Limit is Original"),
            (
                Some(PlaybackLimit::P480),
                None,
                "Its picture size isn't known, so it's taken to fit the 480p Playback Limit",
            ),
        ];
        for (limit, tracks, reason) in cases {
            let facts = Facts {
                limit,
                ..facts(tracks)
            };

            let plan = build(&facts, &STEPS);

            assert_eq!(plan.method, Some(Method::DirectPlay), "{reason}");
            assert_eq!(plan.reasons[5], reason);
        }
    }

    #[test]
    fn a_picture_fitted_to_a_playback_limit_keeps_its_shape_with_even_sides() {
        let cases = [
            ((1920, 800), PlaybackLimit::P720, Some((1280, 532))),
            ((3840, 2160), PlaybackLimit::P480, Some((852, 480))),
            ((1440, 1080), PlaybackLimit::P720, Some((960, 720))),
            ((3840, 2160), PlaybackLimit::P1080, Some((1920, 1080))),
            ((1920, 1080), PlaybackLimit::P1080, None),
            ((1280, 536), PlaybackLimit::P720, None),
        ];
        for (picture, limit, fits) in cases {
            assert_eq!(fitted(picture, limit), fits, "{picture:?} at {limit:?}");
        }
    }
}
