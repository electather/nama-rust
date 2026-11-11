//! Playback Plans: how the Server plays one Version on one Device, and why (ADR 0027).
//!
//! [`plan`] gathers what a plan is based on, probing the Version first if its Tracks aren't
//! current, and runs the steps over it in order. A step gives a Playback Reason for each fact that
//! rules a method out, changes what plays, drops a choice or assumes what it couldn't know, and
//! none when it changes nothing (ADR 0034). The plan keeps only the methods no reason rules out,
//! so no step can widen what an earlier one ruled out. The steps for the User's choices run last,
//! and a choice no method is left for is dropped instead (ADR 0031).
//!
//! Each playable plan is kept in [`running::RunningPlans`] until it stops, and Remuxes serve their
//! own files there.
//!
//! Direct Play serves a Version's file as it is, over plain HTTP at `/media/versions/<id>` (ADR
//! 0007: media bytes are not RPC), to anyone with a session. A browser's `<video src>` presents
//! that session in its cookie (ADR 0014). Range requests let a `<video>` seek. Only Versions,
//! files a scan stored for a Library and Matched, are served, by id, so a client can never name a
//! path. The subtitles a plan offers are served beside it ([`subtitles`]).

pub(crate) mod running;
pub(crate) mod subtitles;

use std::{collections::BTreeSet, net::IpAddr, num::NonZeroU32, sync::Arc};

use axum::{
    Router,
    extract::{Path, Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use buffa::Enumeration;
use nama_proto::nama::v1 as proto;
use proto::PlaybackReasonKind as Kind;
use proto::SubtitleFormat::{SUBTITLE_FORMAT_ASS as ASS, SUBTITLE_FORMAT_WEBVTT as WEBVTT};
use tower_http::services::ServeFile;

use crate::{
    contents::Contents,
    ffmpeg::{
        Abilities, Accel, BurnIn, CanDo, Ffmpeg, Gpu, Hardware, Remux, RemuxAudio, ToneMapper,
        Transcode, TranscodeAudio, VideoConversion, VideoLimit,
    },
    probing::{self, ProbeError, Probing},
    server_settings::{HardwareChoice, PlaybackValues},
    tracks::{Hdr, Probe, Track, TrackKind, Tracks, hdr_to_proto},
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

    const ALL: [Self; 3] = [Self::P1080, Self::P720, Self::P480];

    /// What it's called, such as "720p", which is also how it's stored.
    pub(crate) const fn name(self) -> &'static str {
        self.name
    }

    /// The limit called `name`, if there is one.
    pub(crate) fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|limit| limit.name == name)
    }

    const fn new(name: &'static str, width: i64, height: i64, bits_per_second: u32) -> Self {
        Self {
            name,
            width,
            height,
            bits_per_second,
        }
    }
}

/// The Playback Limit a User picked in the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PickedLimit {
    /// They haven't picked one, so outside the local network the default for remote playback
    /// applies.
    Unpicked,
    /// They picked this one, `None` being Original.
    Picked(Option<PlaybackLimit>),
}

/// Where a Device plays from, as its address says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Network {
    Local,
    /// Outside the local network, with the default Playback Limit for remote playback an admin
    /// set, `None` being Original.
    Remote(Option<PlaybackLimit>),
}

/// How the Server will play one Version on one Device, and why.
#[derive(Debug, PartialEq)]
pub(crate) struct Plan {
    /// The Version it plays, by id.
    pub(crate) version: i64,
    /// How it plays the Version and what that converts, `None` when it can't be played.
    pub(crate) playing: Option<Playing>,
    /// Its video, then its audio. Empty when its Tracks aren't known.
    pub(crate) tracks: Vec<Track>,
    /// The facts that shaped it, in the order the steps that found them ran.
    pub(crate) reasons: Vec<Reason>,
    /// The subtitles the Device shows itself, without a new plan.
    pub(crate) subtitles: Vec<Subtitle>,
    /// The Playback Limit it keeps to, `None` being Original.
    pub(crate) limit: Option<PlaybackLimit>,
    /// The place in the Version's Tracks of the audio Track it plays, if it has audio.
    pub(crate) audio: Option<usize>,
    /// The place in the Version's Tracks of the subtitle Track it burns in, if any.
    pub(crate) burn_in: Option<usize>,
    pub(crate) offers: Offers,
}

impl Plan {
    /// How it plays the Version, `None` when it can't be played.
    pub(crate) fn method(&self) -> Option<Method> {
        self.playing.as_ref().map(Playing::method)
    }
}

/// One fact that shaped a plan, a Playback Reason (ADR 0034).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Reason {
    pub(crate) kind: Kind,
    /// The values it mentions, by their key in the API, as its kind's comment in the schema
    /// lists them.
    pub(crate) params: Vec<(&'static str, String)>,
    /// The Server's English sentence for it, which isn't API contract.
    pub(crate) text: String,
    /// The methods it rules out, as its kind's comment in the schema names them.
    rules_out: BTreeSet<Method>,
    /// Whether it decided the plan, rather than being context.
    pub(crate) decided: bool,
}

impl Reason {
    /// A reason that rules nothing out and is context until [`Self::ruling_out`] or
    /// [`Self::deciding`] says otherwise.
    fn new(kind: Kind, text: impl Into<String>) -> Self {
        Self {
            kind,
            params: vec![],
            text: text.into(),
            rules_out: BTreeSet::new(),
            decided: false,
        }
    }

    fn with(mut self, key: &'static str, value: &impl ToString) -> Self {
        self.params.push((key, value.to_string()));
        self
    }

    fn ruling_out<const N: usize>(mut self, methods: [Method; N]) -> Self {
        self.rules_out.extend(methods);
        self
    }

    /// Marks it as saying what the method the plan plays changes, or that a choice was dropped,
    /// which always decides the plan.
    const fn deciding(mut self) -> Self {
        self.decided = true;
        self
    }

    /// Marks it decided if it rules out a method the plan would rather have used than the one it
    /// `plays`, or any method when it plays none (ADR 0034).
    fn decide(&mut self, plays: Option<Method>) {
        self.decided |=
            (self.rules_out.iter()).any(|&ruled_out| plays.is_none_or(|plays| ruled_out < plays));
    }
}

/// A plan [`RunningPlans`] started, which it keeps until it stops.
#[derive(Debug)]
pub(crate) struct StartedPlan {
    /// What [`RunningPlans`] keeps it by.
    pub(crate) id: u64,
    /// Where to play it from, on this Server.
    pub(crate) url: String,
    pub(crate) plan: Plan,
}

/// What [`plan`] answers.
#[derive(Debug)]
pub(crate) enum Planned {
    Started(StartedPlan),
    /// A plan that can't be played, which nothing keeps.
    Unplayable(Plan),
}

/// How a plan plays a Version, with what its method converts, for a Remux or a Transcode the
/// Version's length, which their playlists list, and for a Transcode on hardware its fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::large_enum_variant,
    reason = "a Server holds a plan per Device playing, a handful, so boxing saves nothing"
)]
pub(crate) enum Playing {
    DirectPlay,
    Remux {
        remux: Remux,
        duration_ms: i64,
    },
    Transcode {
        transcode: Transcode,
        duration_ms: i64,
        /// What it goes on with if it runs on hardware, kept apart from what `Ffmpeg::transcode`
        /// reads.
        fallback: Option<Fallback>,
    },
}

impl Playing {
    const fn method(&self) -> Method {
        match self {
            Self::DirectPlay => Method::DirectPlay,
            Self::Remux { .. } => Method::Remux,
            Self::Transcode { .. } => Method::Transcode,
        }
    }
}

/// What a Transcode on hardware goes on with in software once the hardware fails partway.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Fallback {
    pub(crate) transcode: Transcode,
    /// Its plan's reasons from then on: those that still hold, and why it fell back.
    pub(crate) reasons: Vec<Reason>,
}

/// The choices a plan for a Version can ever honour on a Server and Device: what the lasting
/// facts allow, whatever a busy Server or a method that failed on the Device rules out for now.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Offers {
    /// The audio Tracks it can play, by their place in the Version's Tracks.
    pub(crate) audio: Vec<usize>,
    /// The subtitle Tracks it can burn in, by their place in the Version's Tracks.
    pub(crate) burn_in: Vec<usize>,
    /// The Playback Limits below Original it can scale the picture down to, largest first.
    pub(crate) limits: Vec<PlaybackLimit>,
}

/// One of the User's choices, which a plan drops when no method is left to honour it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Choice {
    Audio,
    Limit,
    Subtitle,
}

/// A subtitle the Device shows itself: a text one as WebVTT, or one in a format it draws.
#[derive(Debug, PartialEq)]
pub(crate) struct Subtitle {
    pub(crate) track: Track,
    pub(crate) format: proto::SubtitleFormat,
    /// Where it's served in `format`, on this Server.
    pub(crate) url: String,
    /// Where the fonts its file carries are served, for one drawn as ASS; empty for any other.
    pub(crate) font_urls: Vec<String>,
}

/// What a Device says it can decode, as it found out just now. The Server never stores it.
#[derive(Debug, Default)]
pub(crate) struct DeviceDecoding {
    /// The containers it plays.
    pub(crate) containers: Vec<proto::Container>,
    /// The video codecs it decodes.
    pub(crate) video_codecs: Vec<proto::VideoCodec>,
    /// The audio codecs it decodes.
    pub(crate) audio_codecs: Vec<proto::AudioCodec>,
    /// Whether its display shows HDR.
    pub(crate) hdr: bool,
    /// Whether it plays HLS, natively or through Media Source Extensions, which every method but
    /// Direct Play needs.
    pub(crate) hls: bool,
    /// How many audio channels it plays out at once, such as 6 for 5.1; 0 when it doesn't say.
    pub(crate) audio_channels: u32,
    /// The subtitle formats it draws itself, which never lists WebVTT, since every Device shows it.
    pub(crate) subtitle_formats: Vec<proto::SubtitleFormat>,
}

/// What a request for a plan asks.
#[derive(Debug)]
pub(crate) struct Asked {
    /// The Version to play, by id.
    pub(crate) version: i64,
    pub(crate) decoding: DeviceDecoding,
    /// The methods that just failed to play it on the Device.
    pub(crate) failed: BTreeSet<Method>,
    /// The audio Track chosen, by its place in the Version's Tracks, if one was.
    pub(crate) audio: Option<usize>,
    /// The subtitle Track chosen to burn into the picture, by its place in the Version's Tracks,
    /// if one was.
    pub(crate) subtitle: Option<usize>,
    pub(crate) limit: PickedLimit,
    /// The Device's address, which says whether it's on the local network.
    pub(crate) client: IpAddr,
}

/// Plans what `asked` asks on `caller`'s Device, by the Playback settings `settings`, and starts
/// it in `running` if it plays. It probes the Version first unless its Tracks are current. `None`
/// if there is no such Version, and [`StartError::Busy`] if only a Transcode would play it and
/// the Server is running as many as it may.
#[expect(
    clippy::too_many_arguments,
    reason = "the handles are the PlaybackService handler's own, and the rest is the call; \
              passing the handler would tie planning to the API"
)]
pub(crate) async fn plan(
    probing: &Probing,
    abilities: &Abilities,
    running: &RunningPlans,
    caller: &Caller,
    asked: Asked,
    settings: PlaybackValues,
) -> Result<Option<Planned>, StartError> {
    let Asked {
        version,
        decoding,
        failed,
        audio,
        subtitle,
        limit,
        client,
    } = asked;
    let probe = match probing.current(version).await {
        Ok(probe) => Some(probe),
        Err(probing::Error::Probe(err)) => {
            if !matches!(*err, ProbeError::Unavailable) {
                tracing::warn!(
                    version,
                    "could not probe a Version to plan playing it: {err}"
                );
            }
            None
        }
        Err(probing::Error::NoSuchVersion(_)) => return Ok(None),
        Err(probing::Error::Database(err)) => return Err(err.into()),
    };
    let mut facts = Facts {
        version,
        probe,
        device: decoding,
        can_do: abilities.can_do().await,
        failed,
        audio,
        transcodes_full: None,
        picked_limit: limit,
        network: if on_local_network(client) {
            Network::Local
        } else {
            Network::Remote(settings.remote_limit)
        },
        subtitle,
        hardware_choice: settings.hardware,
        dropped: BTreeSet::new(),
    };
    let max_transcodes =
        (settings.max_transcodes).map_or_else(|| default_max_transcodes(&facts), NonZeroU32::get);
    facts.transcodes_full = running.transcodes_full(caller.device.id, max_transcodes);
    let plan = build(&mut facts);
    if plan.playing.is_none() {
        return match too_busy_to_play(facts) {
            Some(max) => Err(StartError::Busy(max)),
            None => Ok(Some(Planned::Unplayable(plan))),
        };
    }
    let started = running.start(caller, plan).await?;
    Ok(started.map(Planned::Started))
}

/// Whose playback events `viewer` may see: everyone's (`None`) with `ADMIN`, or else only their own.
pub(crate) fn visible_to(viewer: &User) -> Option<i64> {
    (!viewer.permissions.allow(proto::Permission::Admin)).then_some(viewer.id)
}

/// `reason` as the API sends it.
pub(crate) fn reason_to_proto(reason: &Reason) -> proto::PlaybackReason {
    proto::PlaybackReason {
        kind: reason.kind.into(),
        params: (reason.params.iter())
            .map(|(key, value)| ((*key).to_owned(), value.clone()))
            .collect(),
        text: reason.text.clone(),
        decided: reason.decided,
        ..Default::default()
    }
}

/// Each Playback Limit below Original, by the API's name for it.
pub(crate) const NAMED_LIMITS: [(PlaybackLimit, proto::PlaybackLimit); 3] = [
    (
        PlaybackLimit::P1080,
        proto::PlaybackLimit::PLAYBACK_LIMIT_1080P,
    ),
    (
        PlaybackLimit::P720,
        proto::PlaybackLimit::PLAYBACK_LIMIT_720P,
    ),
    (
        PlaybackLimit::P480,
        proto::PlaybackLimit::PLAYBACK_LIMIT_480P,
    ),
];

/// `limit` as the API names it, `None` being Original.
pub(crate) fn limit_to_proto(limit: Option<PlaybackLimit>) -> proto::PlaybackLimit {
    (NAMED_LIMITS.into_iter())
        .find(|(ours, _)| Some(*ours) == limit)
        .map_or(
            proto::PlaybackLimit::PLAYBACK_LIMIT_ORIGINAL,
            |(_, theirs)| theirs,
        )
}

/// `accel` as the API names it.
pub(crate) const fn accel_to_proto(accel: Accel) -> proto::HardwareAcceleration {
    match accel {
        Accel::Vaapi => proto::HardwareAcceleration::HARDWARE_ACCELERATION_VAAPI,
        Accel::Qsv => proto::HardwareAcceleration::HARDWARE_ACCELERATION_QSV,
        Accel::Nvenc => proto::HardwareAcceleration::HARDWARE_ACCELERATION_NVENC,
        Accel::VideoToolbox => proto::HardwareAcceleration::HARDWARE_ACCELERATION_VIDEOTOOLBOX,
    }
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
    /// What the Server's ffmpeg can do: every method but Direct Play needs ffmpeg and ffprobe,
    /// and a system ffmpeg may lack zscale or libx264 (ADR 0026).
    can_do: CanDo,
    /// The methods that just failed to play it on the Device. The Server never stores them.
    failed: BTreeSet<Method>,
    /// The audio Track the User chose, by its place in the Version's Tracks, if they chose one.
    audio: Option<usize>,
    /// The most Transcodes the Server may run at once, if other Devices already run that many.
    transcodes_full: Option<u32>,
    picked_limit: PickedLimit,
    network: Network,
    /// The subtitle Track the User chose to burn into the picture, by its place in the Version's
    /// Tracks, if they chose one.
    subtitle: Option<usize>,
    /// What the admin chose for Transcodes to run on.
    hardware_choice: HardwareChoice,
    /// The User's choices the plan doesn't honour, as [`build`] found.
    dropped: BTreeSet<Choice>,
}

/// A step gives the reasons it found, and the plan may be what none of them rules out.
type Step = fn(&Facts, Option<&Candidates>) -> Vec<Reason>;

/// What a Remux and a Transcode of a probed Version would each convert, worked out before the
/// steps, which read them, so a plan's reasons and what it plays can't disagree (the crate's
/// `AGENTS.md`).
struct Candidates {
    /// The place in the Version's Tracks of the audio Track a plan plays, whatever its method.
    audio: Option<usize>,
    remux: Remux,
    transcode: Transcode,
}

/// The [`Candidates`] for `facts`. `None` for a Version that isn't probed, which neither a Remux
/// nor a Transcode can play.
fn candidates(facts: &Facts) -> Option<Candidates> {
    let probe = facts.probe.as_ref()?;
    let audio = played_audio_place(facts);
    let played =
        audio.and_then(|place| Some((probe.ffmpeg_index(place)?, probe.tracks.get(place)?)));
    Some(Candidates {
        audio,
        remux: remux(facts, played),
        transcode: transcode(facts, played),
    })
}

// The steps run in ADR 0027's order, split by ADR 0031. A new input is a new step in one of these.
// What the method the plan plays changes isn't one: it rules nothing out, so `build` gives its
// reasons last, once the plan has a method ([`converting`]).

/// The steps for what the Version, Device and Server allow that lasts, so what they allow is what
/// the User's choices can ever have.
const LASTING_STEPS: [Step; 4] = [
    version_tracks,
    version_length,
    device_decoding,
    server_ffmpeg,
];

/// The steps for what rules a method out only for now.
const NOW_STEPS: [Step; 2] = [failed_on_device, server_busy];

/// The steps for the User's choices, which narrow only while a method is left.
const CHOICE_STEPS: [(Choice, Step); 4] = [
    (Choice::Audio, chosen_audio),
    (Choice::Limit, network),
    (Choice::Limit, playback_limit),
    (Choice::Subtitle, chosen_subtitle),
];

/// The plan for `facts` that every step allows. A choice that would leave no method is added to
/// `facts.dropped` and the steps run again without it, since what the earlier steps see, such as
/// the audio the Device must decode, may change with it.
fn build(facts: &mut Facts) -> Plan {
    let candidates = candidates(facts);
    let candidates = candidates.as_ref();
    let mut reasons = vec![];
    let lasting = narrowed(
        facts,
        candidates,
        &LASTING_STEPS,
        BTreeSet::from(Method::ALL),
        &mut reasons,
    );
    let mut allowed = narrowed(facts, candidates, &NOW_STEPS, lasting.clone(), &mut reasons);
    for (choice, step) in CHOICE_STEPS {
        let given = step(facts, candidates);
        let allows = allowed_by(&given);
        // A step allowing every method has no choice in effect to drop.
        if allowed.is_disjoint(&allows) && allows != BTreeSet::from(Method::ALL) {
            facts.dropped.insert(choice);
            return build(facts);
        }
        allowed.retain(|method| allows.contains(method));
        reasons.extend(given);
    }
    let mut playing = allowed
        .first()
        .and_then(|&method| playing_by(method, facts, candidates));
    reasons.extend(converting(facts, candidates, playing.as_ref()));
    let plays = playing.as_ref().map(Playing::method);
    for reason in &mut reasons {
        reason.decide(plays);
    }
    if let Some(Playing::Transcode {
        transcode,
        fallback: on_failure,
        ..
    }) = &mut playing
    {
        *on_failure = fallback(facts, transcode, &reasons);
    }
    let audio = candidates.and_then(|candidates| candidates.audio);
    let burns_in = matches!(
        &playing,
        Some(Playing::Transcode { transcode, .. }) if transcode.video.burn_in.is_some()
    );
    let tracks = (facts.probe.as_ref())
        .map(|probe| played_by(playing.as_ref(), played_tracks(probe, audio)))
        .unwrap_or_default();
    Plan {
        version: facts.version,
        playing,
        tracks,
        reasons,
        subtitles: facts
            .probe
            .as_ref()
            .map(|probe| shown_subtitles(facts.version, probe, &facts.device))
            .unwrap_or_default(),
        limit: limit_in_effect(facts),
        audio,
        burn_in: facts.subtitle.filter(|_| burns_in),
        offers: offers(facts, &lasting),
    }
}

/// How the plan for `facts` plays by `method`, with what it converts, before its fallback is
/// known. Never `None` once a Remux or Transcode is allowed: `version_length` rules both out first
/// and says why.
fn playing_by(method: Method, facts: &Facts, candidates: Option<&Candidates>) -> Option<Playing> {
    let duration_ms = facts.probe.as_ref().and_then(|probe| probe.duration_ms);
    match method {
        Method::DirectPlay => Some(Playing::DirectPlay),
        Method::Remux => Some(Playing::Remux {
            remux: candidates?.remux,
            duration_ms: duration_ms?,
        }),
        Method::Transcode => Some(Playing::Transcode {
            transcode: candidates?.transcode.clone(),
            duration_ms: duration_ms?,
            fallback: None,
        }),
    }
}

/// What `allowed` leaves once each of `steps` narrows it, adding their reasons to `reasons`.
fn narrowed(
    facts: &Facts,
    candidates: Option<&Candidates>,
    steps: &[Step],
    mut allowed: BTreeSet<Method>,
    reasons: &mut Vec<Reason>,
) -> BTreeSet<Method> {
    for step in steps {
        let given = step(facts, candidates);
        let allows = allowed_by(&given);
        allowed.retain(|method| allows.contains(method));
        reasons.extend(given);
    }
    allowed
}

/// The methods none of `reasons` rules out.
fn allowed_by(reasons: &[Reason]) -> BTreeSet<Method> {
    (Method::ALL.into_iter())
        .filter(|method| {
            !reasons
                .iter()
                .any(|reason| reason.rules_out.contains(method))
        })
        .collect()
}

/// What a plan for `facts` can ever honour, with only the methods in `lasting` allowed.
fn offers(facts: &Facts, lasting: &BTreeSet<Method>) -> Offers {
    let Some(probe) = &facts.probe else {
        return Offers::default();
    };
    let transcodes = lasting.contains(&Method::Transcode);
    let places = |keep: &dyn Fn(&Track) -> bool| {
        (probe.tracks.iter().enumerate())
            .filter(|(_, track)| keep(track))
            .map(|(place, _)| place)
            .collect()
    };
    // Only the file's own audio plays without a Remux or a Transcode.
    let audio = if transcodes || lasting.contains(&Method::Remux) {
        places(&is_audio)
    } else {
        file_audio(probe).into_iter().collect()
    };
    if !transcodes {
        return Offers {
            audio,
            ..Offers::default()
        };
    }
    let picture = picture(facts);
    Offers {
        audio,
        burn_in: places(&|track| burns_in(&facts.device, probe, track)),
        limits: (PlaybackLimit::ALL.into_iter())
            .filter(|&limit| picture.and_then(|picture| fitted(picture, limit)).is_some())
            .collect(),
    }
}

/// The step for the Version's own Tracks: a Remux or a Transcode needs them.
fn version_tracks(facts: &Facts, _: Option<&Candidates>) -> Vec<Reason> {
    if facts.probe.is_some() {
        return vec![];
    }
    vec![
        Reason::new(
            Kind::PLAYBACK_REASON_KIND_TRACKS_UNKNOWN,
            "Its Tracks aren't known, so it can only be Direct Played",
        )
        .ruling_out([Method::Remux, Method::Transcode]),
    ]
}

/// The step for the Version's length: the HLS playlist of a Remux or a Transcode lists how long
/// each segment is.
fn version_length(facts: &Facts, _: Option<&Candidates>) -> Vec<Reason> {
    if facts
        .probe
        .as_ref()
        .is_some_and(|probe| probe.duration_ms.is_some())
    {
        return vec![];
    }
    vec![
        Reason::new(
            Kind::PLAYBACK_REASON_KIND_LENGTH_UNKNOWN,
            "Its length isn't known, which a Remux's or a Transcode's playlist needs, so it can \
             only be Direct Played",
        )
        .ruling_out([Method::Remux, Method::Transcode]),
    ]
}

/// The step for what the Device decodes: Direct Play only if it decodes the Version's container
/// and the Tracks the plan plays, and its display shows their HDR, a Remux only if it decodes the
/// video, since a Remux converts only the audio, and neither a Remux nor a Transcode unless it
/// plays HLS.
fn device_decoding(facts: &Facts, candidates: Option<&Candidates>) -> Vec<Reason> {
    // `version_tracks` already rules out all but Direct Play, which is then taken to decode them.
    let (Some(probe), Some(candidates)) = (&facts.probe, candidates) else {
        return vec![];
    };
    let mut reasons: Vec<Reason> = undecodable_container(probe, &facts.device)
        .into_iter()
        .chain(
            (played_tracks(probe, candidates.audio).iter())
                .flat_map(|track| undecodable(track, &facts.device)),
        )
        .collect();
    if !facts.device.hls {
        reasons.push(
            Reason::new(
                Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_PLAY_HLS,
                "This Device can't play HLS, which a Remux and a Transcode need",
            )
            .ruling_out([Method::Remux, Method::Transcode]),
        );
    }
    reasons
}

/// The step for the Server's ffmpeg: without it, only Direct Play (ADR 0026); without libx264, no
/// Transcode; and no Transcode of a Dolby Vision picture with no Base Layer.
fn server_ffmpeg(facts: &Facts, _: Option<&Candidates>) -> Vec<Reason> {
    let reason = if !facts.can_do.runs {
        Reason::new(
            Kind::PLAYBACK_REASON_KIND_SERVER_LACKS_FFMPEG,
            "This Server lacks ffmpeg or ffprobe, so it can only Direct Play",
        )
        .ruling_out([Method::Remux, Method::Transcode])
    } else if !facts.can_do.transcodes() {
        Reason::new(
            Kind::PLAYBACK_REASON_KIND_SERVER_CANT_ENCODE_VIDEO,
            "This Server's ffmpeg lacks libx264, so it can't Transcode",
        )
        .ruling_out([Method::Transcode])
    } else if let Some(Hdr::DolbyVision {
        base_layer: None, ..
    }) = played_hdr(facts)
    {
        // Its picture is in Dolby Vision's own colours, which only a Dolby Vision player knows.
        // ponytail: Dolby Vision profile 5 stays out of reach: none of the hardware APIs' ffmpeg
        // filters reshape its colours, which only libplacebo does.
        Reason::new(
            Kind::PLAYBACK_REASON_KIND_DOLBY_VISION_NEEDS_BASE_LAYER,
            "This Server can't Transcode a Dolby Vision picture with no Base Layer, as in \
             profile 5",
        )
        .ruling_out([Method::Transcode])
    } else {
        return vec![];
    };
    vec![reason]
}

/// What `transcode` goes on with in software if it runs on hardware: the same conversion,
/// tone-mapping only if ffmpeg has zscale, with the plan's `reasons` but those that no longer
/// hold, and why it fell back (ADR 0034).
fn fallback(facts: &Facts, transcode: &Transcode, reasons: &[Reason]) -> Option<Fallback> {
    let accel = transcode.video.hardware.as_ref()?.accel;
    let tone_map = (transcode.video.tone_map).and_then(|_| facts.can_do.tone_mapper(None));
    let stops_tone_mapping = tone_map.is_none() && transcode.video.tone_map.is_some();
    let still_holds = |reason: &&Reason| {
        reason.kind != Kind::PLAYBACK_REASON_KIND_TRANSCODES_ON_HARDWARE
            && (reason.kind != Kind::PLAYBACK_REASON_KIND_HDR_TONE_MAPPED || !stops_tone_mapping)
    };
    let failed = accel_reason(
        Kind::PLAYBACK_REASON_KIND_HARDWARE_FAILED_PARTWAY,
        accel,
        format!(
            "{} failed partway, so the rest of it Transcodes in software",
            accel.name()
        ),
    );
    let reasons = (reasons.iter().filter(still_holds).cloned())
        .chain([failed])
        .chain(
            played_hdr(facts)
                .filter(|_| stops_tone_mapping)
                .map(not_tone_mapped),
        )
        .collect();
    Some(Fallback {
        transcode: Transcode {
            video: VideoConversion {
                hardware: None,
                tone_map,
                ..transcode.video
            },
            ..*transcode
        },
        reasons,
    })
}

/// What tone-maps the played video's HDR to SDR in a Transcode, when the Device can't show it: the
/// GPU, if a Transcode runs on one that passed its tone-map test and burns nothing in, which
/// takes the frames off it, or else zscale, if ffmpeg has it.
fn tone_mapper(facts: &Facts) -> Option<ToneMapper> {
    if facts.device.hdr || played_hdr(facts).and_then(transcoded_hdr).is_none() {
        return None;
    }
    let gpu = transcodes_on(facts)
        .ok()
        .filter(|_| burn_in(facts).is_none());
    facts.can_do.tone_mapper(gpu.as_ref())
}

/// The step for what the method `playing` changes, which runs once the plan has one, as it rules
/// nothing out: the audio channels it drops, and for a Transcode, how it does with HDR the Device
/// can't show and what it runs on.
fn converting(
    facts: &Facts,
    candidates: Option<&Candidates>,
    playing: Option<&Playing>,
) -> Vec<Reason> {
    let (method, channels) = match playing {
        Some(Playing::Remux { remux, .. }) => (
            Method::Remux,
            remux.audio.and_then(|audio| audio.aac_channels),
        ),
        Some(Playing::Transcode { transcode, .. }) => (
            Method::Transcode,
            transcode.audio.map(|audio| audio.aac_channels),
        ),
        None | Some(Playing::DirectPlay) => return vec![],
    };
    let mut reasons: Vec<Reason> = channels
        .and_then(|channels| fewer_channels(facts, candidates, channels, method))
        .into_iter()
        .collect();
    if let Some(Playing::Transcode { transcode, .. }) = playing {
        reasons.extend(tone_mapping(facts, transcode.video.tone_map));
        reasons.extend(match &transcode.video.hardware {
            Some(hardware) => {
                let gpu = (hardware.gpu.as_ref()).map_or_else(|| "GPU".to_owned(), Gpu::name);
                Some(accel_reason(
                    Kind::PLAYBACK_REASON_KIND_TRANSCODES_ON_HARDWARE,
                    hardware.accel,
                    format!("It Transcodes on the {gpu} with {}", hardware.accel.name()),
                ))
            }
            None => transcodes_on(facts).err(),
        });
    }
    reasons
}

/// Why `method` converts the played audio to `channels` channels, if that's fewer than it has.
fn fewer_channels(
    facts: &Facts,
    candidates: Option<&Candidates>,
    channels: i64,
    method: Method,
) -> Option<Reason> {
    let played = (candidates?.audio)
        .and_then(|place| facts.probe.as_ref()?.tracks.get(place))
        .and_then(channels_of)?;
    if channels >= played {
        return None;
    }
    let (kind, why) = match facts.device.audio_channels {
        6.. => (
            Kind::PLAYBACK_REASON_KIND_CONVERSION_CHANNELS_CAPPED,
            format!("the most a {} makes", method.name()),
        ),
        0 => (
            Kind::PLAYBACK_REASON_KIND_DEVICE_CHANNELS_UNKNOWN,
            "as this Device doesn't say it plays more".to_owned(),
        ),
        _ => (
            Kind::PLAYBACK_REASON_KIND_DEVICE_PLAYS_FEWER_CHANNELS,
            "as this Device doesn't play 5.1".to_owned(),
        ),
    };
    let name = match channels {
        1 => "mono".to_owned(),
        2 => "stereo".to_owned(),
        6 => "5.1".to_owned(),
        n => format!("{n}-channel"),
    };
    let text = format!("Its audio is converted to {name}, {why}");
    Some(
        Reason::new(kind, text)
            .with("channels", &channels)
            .deciding(),
    )
}

/// Whether a Transcode tone-maps the played picture to SDR with `tone_map`, when it's HDR the
/// Device can't show and the Transcode keeps.
fn tone_mapping(facts: &Facts, tone_map: Option<ToneMapper>) -> Option<Reason> {
    let hdr =
        played_hdr(facts).filter(|&hdr| !facts.device.hdr && transcoded_hdr(hdr).is_some())?;
    if tone_map.is_none() {
        return Some(not_tone_mapped(hdr));
    }
    let text = format!("Its {} picture is tone-mapped to SDR", hdr_name(hdr));
    Some(hdr_reason(Kind::PLAYBACK_REASON_KIND_HDR_TONE_MAPPED, hdr, text).deciding())
}

fn not_tone_mapped(hdr: Hdr) -> Reason {
    let text = format!(
        "Nothing on this Server tone-maps its {} picture to SDR, so its colours look washed out",
        hdr_name(hdr)
    );
    hdr_reason(Kind::PLAYBACK_REASON_KIND_HDR_NOT_TONE_MAPPED, hdr, text).deciding()
}

/// A reason of `kind` about the hardware acceleration API `accel`.
fn accel_reason(kind: Kind, accel: Accel, text: String) -> Reason {
    Reason::new(kind, text).with("hardwareAcceleration", &accel_to_proto(accel).proto_name())
}

/// A reason of `kind` about a picture that's `hdr`.
fn hdr_reason(kind: Kind, hdr: Hdr, text: String) -> Reason {
    Reason::new(kind, text).with("hdrFormat", &hdr_to_proto(Some(hdr)).0.proto_name())
}

/// The hardware a Transcode runs on, as the admin chose, or why it runs in software.
fn transcodes_on(facts: &Facts) -> Result<Hardware, Reason> {
    let found = |accel| (facts.can_do.hardware.iter()).find(|found| found.accel == accel);
    match facts.hardware_choice {
        HardwareChoice::Off => Err(Reason::new(
            Kind::PLAYBACK_REASON_KIND_HARDWARE_ACCELERATION_OFF,
            "Hardware acceleration is off, so it Transcodes in software",
        )),
        HardwareChoice::Automatic => facts.can_do.automatic().cloned().ok_or_else(|| {
            Reason::new(
                Kind::PLAYBACK_REASON_KIND_NO_HARDWARE_ACCELERATION_PASSED,
                "No hardware acceleration passed its test when this Server started, so it \
                 Transcodes in software",
            )
        }),
        // Stored before GPUs were told apart, it means the API on whichever GPU Automatic prefers.
        HardwareChoice::Api(accel) => found(accel).cloned().ok_or_else(|| {
            let text = format!(
                "{} didn't pass its test when this Server started, so it Transcodes in software",
                accel.name()
            );
            accel_reason(
                Kind::PLAYBACK_REASON_KIND_HARDWARE_ACCELERATION_FAILED_TEST,
                accel,
                text,
            )
        }),
    }
}

/// The most Transcodes the Server runs at once when the admin hasn't set it: no limit while they
/// run on hardware, which a modest Server has room for, and 2 in software.
fn default_max_transcodes(facts: &Facts) -> u32 {
    /// As many as a modest Server's CPU keeps up with.
    const IN_SOFTWARE: u32 = 2;
    if transcodes_on(facts).is_ok() {
        u32::MAX
    } else {
        IN_SOFTWARE
    }
}

/// How a Transcode for `facts` converts the played video.
fn video_conversion(facts: &Facts) -> VideoConversion {
    let hardware = transcodes_on(facts).ok();
    VideoConversion {
        // Hardware may fail, and software carry on after its init segment.
        in_band_headers: hardware.is_some(),
        hardware,
        tone_map: tone_mapper(facts),
        limit: video_limit(facts),
        burn_in: burn_in(facts),
    }
}

/// What a Remux for `facts` converts, playing `audio`: the index ffmpeg knows an audio Track by,
/// and that Track.
fn remux(facts: &Facts, audio: Option<(usize, &Track)>) -> Remux {
    let audio = audio.map(|(track, played)| RemuxAudio {
        track,
        aac_channels: aac_channels(&facts.device, played),
    });
    Remux { audio }
}

/// What a Transcode for `facts` converts, playing `audio` as [`remux`] does.
fn transcode(facts: &Facts, audio: Option<(usize, &Track)>) -> Transcode {
    let audio = audio.map(|(track, played)| TranscodeAudio {
        track,
        aac_channels: converted_channels(&facts.device, played),
    });
    Transcode {
        audio,
        video: video_conversion(facts),
    }
}

/// The step for the audio Track the User chose: Direct Play only if it's the one browsers play
/// from the file, since only Safari switches audio inside a file.
fn chosen_audio(facts: &Facts, candidates: Option<&Candidates>) -> Vec<Reason> {
    if facts.dropped.contains(&Choice::Audio) {
        return vec![
            Reason::new(
                Kind::PLAYBACK_REASON_KIND_CHOSEN_AUDIO_DROPPED,
                "The audio Track chosen can't be played here, so the file's own plays instead",
            )
            .deciding(),
        ];
    }
    match (&facts.probe, candidates) {
        (Some(probe), Some(candidates)) if candidates.audio != file_audio(probe) => vec![
            Reason::new(
                Kind::PLAYBACK_REASON_KIND_CHOSEN_AUDIO_NOT_FILES_OWN,
                "The audio Track chosen isn't the one browsers play from its file, so it can't \
                 be Direct Played",
            )
            .ruling_out([Method::DirectPlay]),
        ],
        _ => vec![],
    }
}

/// The step for how busy the Server is: no Transcode while other Devices run as many as it may.
fn server_busy(facts: &Facts, _: Option<&Candidates>) -> Vec<Reason> {
    let busy = facts.transcodes_full.map(|max| {
        Reason::new(
            Kind::PLAYBACK_REASON_KIND_SERVER_BUSY,
            format!(
                "This Server is busy, running as many Transcodes as it may at once ({max}), so \
                 it can't Transcode it"
            ),
        )
        .with("maxTranscodes", &max)
        .ruling_out([Method::Transcode])
    });
    busy.into_iter().collect()
}

/// The most Transcodes the Server may run at once, if running that many is all that keeps the
/// plan for `facts` from playing: a Transcode would play it otherwise. A plan nothing else plays
/// is refused as busy rather than shown as unplayable, since trying again later may play it. Only
/// what the Device needs counts: a choice that needs a Transcode is dropped first.
fn too_busy_to_play(facts: Facts) -> Option<u32> {
    let max = facts.transcodes_full?;
    let mut not_busy = Facts {
        transcodes_full: None,
        ..facts
    };
    (build(&mut not_busy).method() == Some(Method::Transcode)).then_some(max)
}

/// The step for what just failed to play on the Device: it rules out each method that did, so
/// the player can ask again without looping.
fn failed_on_device(facts: &Facts, _: Option<&Candidates>) -> Vec<Reason> {
    (facts.failed.iter())
        .map(|&method| {
            Reason::new(
                Kind::PLAYBACK_REASON_KIND_METHOD_FAILED_ON_DEVICE,
                format!("{} failed on this Device", method.name()),
            )
            .with("method", &method_to_proto(Some(method)).proto_name())
            .ruling_out([method])
        })
        .collect()
}

/// The step for where the Device is: it rules nothing out, and says when the default Playback
/// Limit for remote playback chose a limit the next step scales the picture down to or drops.
fn network(facts: &Facts, _: Option<&Candidates>) -> Vec<Reason> {
    let scales = |limit| {
        picture(facts)
            .and_then(|picture| fitted(picture, limit))
            .is_some()
    };
    match (facts.network, facts.picked_limit) {
        (Network::Remote(Some(limit)), PickedLimit::Unpicked) if scales(limit) => {
            vec![limit_reason(
                Kind::PLAYBACK_REASON_KIND_REMOTE_DEFAULT_LIMIT,
                limit,
                format!(
                    "This Device is outside the local network, so the default Playback Limit for \
                     remote playback, {}, applies",
                    limit.name
                ),
            )]
        }
        _ => vec![],
    }
}

/// A reason of `kind` about the Playback Limit `limit`.
fn limit_reason(kind: Kind, limit: PlaybackLimit, text: String) -> Reason {
    Reason::new(kind, text).with("playbackLimit", &limit_to_proto(Some(limit)).proto_name())
}

/// Whether `address` is on the local network: loopback, private (RFC 1918), link-local, or IPv6
/// unique-local.
fn on_local_network(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        IpAddr::V6(v6) => v6.is_loopback() || v6.is_unicast_link_local() || v6.is_unique_local(),
    }
}

/// The Playback Limit in effect: [`limit_asked`], unless the plan dropped it. `None` is Original.
fn limit_in_effect(facts: &Facts) -> Option<PlaybackLimit> {
    limit_asked(facts).filter(|_| !facts.dropped.contains(&Choice::Limit))
}

/// The Playback Limit the User picked, or else, outside the local network, the remote default.
/// `None` is Original.
fn limit_asked(facts: &Facts) -> Option<PlaybackLimit> {
    match (facts.picked_limit, facts.network) {
        (PickedLimit::Picked(limit), _) | (PickedLimit::Unpicked, Network::Remote(limit)) => limit,
        (PickedLimit::Unpicked, Network::Local) => None,
    }
}

/// The step for the Playback Limit in effect: only a Transcode, which scales the picture down to
/// fit it, when the Version's picture is larger.
fn playback_limit(facts: &Facts, candidates: Option<&Candidates>) -> Vec<Reason> {
    if let Some(limit) = limit_asked(facts)
        && facts.dropped.contains(&Choice::Limit)
    {
        let text = format!(
            "The {} Playback Limit can't be kept to here, so it's dropped for Original",
            limit.name
        );
        return vec![
            limit_reason(
                Kind::PLAYBACK_REASON_KIND_PLAYBACK_LIMIT_DROPPED,
                limit,
                text,
            )
            .deciding(),
        ];
    }
    let Some(limit) = limit_in_effect(facts) else {
        return vec![];
    };
    let scaled = candidates.and_then(|candidates| candidates.transcode.video.limit?.size);
    if let Some((width, height)) = scaled {
        let text = format!(
            "Its picture is larger than the {} Playback Limit, so a Transcode scales it down to \
             {width}×{height}",
            limit.name
        );
        return vec![
            limit_reason(Kind::PLAYBACK_REASON_KIND_PICTURE_SCALED_DOWN, limit, text)
                .with("widthPixels", &width)
                .with("heightPixels", &height)
                .ruling_out([Method::DirectPlay, Method::Remux]),
        ];
    }
    if picture(facts).is_some() {
        return vec![];
    }
    let text = format!(
        "Its picture size isn't known, so it's taken to fit the {} Playback Limit",
        limit.name
    );
    vec![limit_reason(
        Kind::PLAYBACK_REASON_KIND_PICTURE_SIZE_UNKNOWN,
        limit,
        text,
    )]
}

/// The step for the subtitle Track the User chose to burn in, one the Device can't draw: only a
/// Transcode draws it into the picture.
fn chosen_subtitle(facts: &Facts, candidates: Option<&Candidates>) -> Vec<Reason> {
    if facts.dropped.contains(&Choice::Subtitle) {
        return vec![
            Reason::new(
                Kind::PLAYBACK_REASON_KIND_CHOSEN_SUBTITLES_DROPPED,
                "The subtitles chosen can't be burned in here, so they're dropped",
            )
            .deciding(),
        ];
    }
    let transcode_burns_in =
        candidates.is_some_and(|candidates| candidates.transcode.video.burn_in.is_some());
    if let Some(track) = chosen_subtitle_track(facts).filter(|_| transcode_burns_in) {
        let name = subtitle_format_name(&track.codec);
        // It draws ASS only with fonts its file carries (`draws_with_styling`).
        let (kind, why) = if track.is_ass_subtitle() && lists(&facts.device, ASS) {
            (
                Kind::PLAYBACK_REASON_KIND_SUBTITLES_FILE_CARRIES_NO_FONTS,
                format!("Its file carries no fonts to draw its {name} subtitles with"),
            )
        } else {
            (
                Kind::PLAYBACK_REASON_KIND_SUBTITLES_BURNED_IN,
                format!("This Device can't draw {name} subtitles"),
            )
        };
        return vec![
            Reason::new(
                kind,
                format!(
                    "{why}, so the ones chosen are burned into its picture, which only a \
                     Transcode does"
                ),
            )
            .ruling_out([Method::DirectPlay, Method::Remux]),
        ];
    }
    vec![]
}

/// The subtitle Track the User chose, if the Version has it.
fn chosen_subtitle_track(facts: &Facts) -> Option<&Track> {
    facts.probe.as_ref()?.tracks.get(facts.subtitle?)
}

/// What a subtitle format whose codec ffprobe names `codec` is called, such as PGS.
fn subtitle_format_name(codec: &str) -> &str {
    match codec {
        "hdmv_pgs_subtitle" => "PGS",
        "dvd_subtitle" => "VobSub",
        "dvb_subtitle" => "DVB",
        "xsub" => "XSUB",
        "ass" => "ASS",
        "ssa" => "SSA",
        other => other,
    }
}

/// How a Transcode burns in the subtitle Track the User chose, unless the plan dropped it, if it's
/// one that [`burns_in`] on the Device. Any other choice, such as one it draws itself, or one made
/// before the Version changed, is the same as none.
fn burn_in(facts: &Facts) -> Option<BurnIn> {
    let probe = facts.probe.as_ref()?;
    let place = facts
        .subtitle
        .filter(|_| !facts.dropped.contains(&Choice::Subtitle))?;
    let track = (probe.tracks.get(place)).filter(|track| burns_in(&facts.device, probe, track))?;
    let subtitle = probe.ffmpeg_index(place)?;
    Some(
        if matches!(track.kind, TrackKind::Subtitle { image: true }) {
            BurnIn::Image {
                subtitle,
                picture: picture(facts),
            }
        } else {
            BurnIn::Text { subtitle }
        },
    )
}

/// Whether `track`, in the file `probe` read, is a subtitle `device` can't show as it should, so a
/// Transcode burns it in: an image one it doesn't draw, such as PGS, or an ASS or SSA one it
/// doesn't draw, whose styling its WebVTT drops.
fn burns_in(device: &DeviceDecoding, probe: &Probe, track: &Track) -> bool {
    match track.kind {
        TrackKind::Subtitle { image } => {
            let drawn = shown_as(device, probe, track).is_some_and(|format| format != WEBVTT);
            (image || track.is_ass_subtitle()) && !drawn
        }
        TrackKind::Video { .. } | TrackKind::Audio { .. } => false,
    }
}

/// The format `device` shows `track`, in the file `probe` read, in itself, if it does: ASS for
/// one it [`draws_with_styling`], WebVTT for a text one, which every Device shows, or else the
/// Track's own, if the Device draws it (ADR 0033).
fn shown_as(
    device: &DeviceDecoding,
    probe: &Probe,
    track: &Track,
) -> Option<proto::SubtitleFormat> {
    if draws_with_styling(device, probe, track) {
        return Some(ASS);
    }
    subtitles::served_as(track).filter(|&format| format == WEBVTT || lists(device, format))
}

/// Whether `device` draws `track`, in the file `probe` read, as ASS with its styling: only an ASS
/// or SSA one, on a Device that lists ASS, in a file that carries fonts. A renderer on the Device
/// sees only the fonts it's handed and one fallback for Latin, Greek and Cyrillic, so a Track in
/// Persian, Arabic, Chinese or Japanese with no fonts of its own would draw as empty boxes, where
/// its WebVTT shows in the Device's own fonts.
fn draws_with_styling(device: &DeviceDecoding, probe: &Probe, track: &Track) -> bool {
    track.is_ass_subtitle() && lists(device, ASS) && !probe.font_attachments.is_empty()
}

/// Whether `device` says it draws `format` itself.
fn lists(device: &DeviceDecoding, format: proto::SubtitleFormat) -> bool {
    device.subtitle_formats.contains(&format)
}

/// How a Transcode keeps the played video within the Playback Limit in effect, if there's one.
fn video_limit(facts: &Facts) -> Option<VideoLimit> {
    let limit = limit_in_effect(facts)?;
    Some(VideoLimit {
        size: picture(facts).and_then(|picture| fitted(picture, limit)),
        bits_per_second: limit.bits_per_second,
    })
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

/// A picture `width` by `height`, scaled down to fit `limit` with its shape kept, if it's larger.
fn fitted((width, height): (i64, i64), limit: PlaybackLimit) -> Option<(i64, i64)> {
    // A limit names the short side, so a portrait 1080×1920 picture is already 1080p.
    let (limit_width, limit_height) = if height > width {
        (limit.height, limit.width)
    } else {
        (limit.width, limit.height)
    };
    if width <= limit_width && height <= limit_height {
        return None;
    }
    let rounded = |numerator: i64, denominator: i64| (numerator + denominator / 2) / denominator;
    let (width, height) = if width * limit_height > height * limit_width {
        (limit_width, rounded(height * limit_width, width))
    } else {
        (rounded(width * limit_height, height), limit_height)
    };
    // H.264 in 4:2:0 needs even sides.
    Some((width / 2 * 2, height / 2 * 2))
}

/// Each video codec a Device can say it decodes, as ffprobe names it, with its name in plain words.
const VIDEO_CODECS: [(&str, proto::VideoCodec, &str); 5] = [
    ("h264", proto::VideoCodec::VIDEO_CODEC_H264, "H.264"),
    ("hevc", proto::VideoCodec::VIDEO_CODEC_HEVC, "HEVC"),
    ("vp8", proto::VideoCodec::VIDEO_CODEC_VP8, "VP8"),
    ("vp9", proto::VideoCodec::VIDEO_CODEC_VP9, "VP9"),
    ("av1", proto::VideoCodec::VIDEO_CODEC_AV1, "AV1"),
];

/// Each audio codec a Device can say it decodes, as ffprobe names it, with its name in plain words.
const AUDIO_CODECS: [(&str, proto::AudioCodec, &str); 9] = [
    ("aac", proto::AudioCodec::AUDIO_CODEC_AAC, "AAC"),
    ("mp3", proto::AudioCodec::AUDIO_CODEC_MP3, "MP3"),
    ("ac3", proto::AudioCodec::AUDIO_CODEC_AC3, "AC-3"),
    ("eac3", proto::AudioCodec::AUDIO_CODEC_EAC3, "E-AC-3"),
    ("opus", proto::AudioCodec::AUDIO_CODEC_OPUS, "Opus"),
    ("vorbis", proto::AudioCodec::AUDIO_CODEC_VORBIS, "Vorbis"),
    ("flac", proto::AudioCodec::AUDIO_CODEC_FLAC, "FLAC"),
    ("dts", proto::AudioCodec::AUDIO_CODEC_DTS, "DTS"),
    ("truehd", proto::AudioCodec::AUDIO_CODEC_TRUEHD, "TrueHD"),
];

/// Why `device` can't play `probe`'s container, if it can't.
fn undecodable_container(probe: &Probe, device: &DeviceDecoding) -> Option<Reason> {
    let (param, name) = match container(probe) {
        Some((container, _)) if device.containers.contains(&container) => return None,
        Some((container, name)) => (container.proto_name(), name),
        None => (probe.container.as_str(), probe.container.as_str()),
    };
    let reason = Reason::new(
        Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_CONTAINER,
        format!("This Device can't decode its {name} container, so it can't be Direct Played"),
    );
    Some(
        reason
            .with("container", &param)
            .ruling_out([Method::DirectPlay]),
    )
}

/// What of `track` `device` can't decode or show, one reason for each.
fn undecodable(track: &Track, device: &DeviceDecoding) -> Vec<Reason> {
    let codec = &track.codec;
    match track.kind {
        TrackKind::Video { hdr, .. } => {
            let video = undecodable_codec(codec, &VIDEO_CODECS, &device.video_codecs).map(|name| {
                Reason::new(
                    Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_VIDEO,
                    format!(
                        "This Device can't decode its {name} video, so it can't be Direct Played \
                         or Remuxed"
                    ),
                )
                .with("codec", &codec)
                .ruling_out([Method::DirectPlay, Method::Remux])
            });
            let hdr = hdr.filter(|_| !device.hdr).map(|hdr| {
                let text = format!(
                    "This Device can't show its {} picture, so it can't be Direct Played or \
                     Remuxed",
                    hdr_name(hdr)
                );
                hdr_reason(Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_SHOW_HDR, hdr, text)
                    .ruling_out([Method::DirectPlay, Method::Remux])
            });
            video.into_iter().chain(hdr).collect()
        }
        TrackKind::Audio { .. } => {
            let audio = undecodable_codec(codec, &AUDIO_CODECS, &device.audio_codecs).map(|name| {
                Reason::new(
                    Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_AUDIO,
                    format!(
                        "This Device can't decode its {name} audio, so it can't be Direct Played"
                    ),
                )
                .with("codec", &codec)
                .ruling_out([Method::DirectPlay])
            });
            audio.into_iter().collect()
        }
        // A plan plays only a video and an audio Track as Tracks of their own.
        TrackKind::Subtitle { .. } => vec![],
    }
}

/// The codec ffprobe names `ffprobe_name`, in plain words from `codecs` if it has it, unless
/// `decoded` lists it.
fn undecodable_codec<'a, C: PartialEq>(
    ffprobe_name: &'a str,
    codecs: &[(&str, C, &'static str)],
    decoded: &[C],
) -> Option<&'a str> {
    match codecs.iter().find(|(name, ..)| *name == ffprobe_name) {
        Some((_, codec, _)) if decoded.contains(codec) => None,
        Some((_, _, name)) => Some(name),
        None => Some(ffprobe_name),
    }
}

/// `tracks`, the ones a plan plays as they are in the file, as `playing` plays them once
/// converted, or as they are when it can't be played.
fn played_by(playing: Option<&Playing>, tracks: Vec<Track>) -> Vec<Track> {
    match playing {
        Some(Playing::Remux {
            remux:
                Remux {
                    audio:
                        Some(RemuxAudio {
                            aac_channels: Some(channels),
                            ..
                        }),
                },
            ..
        }) => converted_to_aac(tracks, *channels),
        Some(Playing::Transcode {
            transcode: Transcode { audio, video },
            ..
        }) => {
            let tracks = match audio {
                Some(audio) => converted_to_aac(tracks, audio.aac_channels),
                None => tracks,
            };
            converted_to_h264(tracks, video)
        }
        None | Some(Playing::DirectPlay | Playing::Remux { .. }) => tracks,
    }
}

/// How many channels a Remux converts the audio Track `audio` to, as AAC, if `device` can't
/// decode it, as [`converted_channels`] says.
fn aac_channels(device: &DeviceDecoding, audio: &Track) -> Option<i64> {
    if undecodable(audio, device).is_empty() {
        return None;
    }
    Some(converted_channels(device, audio))
}

/// How many channels the audio Track `audio` has once converted to AAC for `device`: as many as
/// it has, up to 5.1 if the Device plays 5.1 and stereo otherwise.
fn converted_channels(device: &DeviceDecoding, audio: &Track) -> i64 {
    let most = if device.audio_channels >= 6 { 6 } else { 2 };
    // Stereo when the file doesn't say, which every Device plays.
    channels_of(audio).unwrap_or(2).min(most)
}

/// How many channels the audio Track `audio` has, if the file says.
const fn channels_of(audio: &Track) -> Option<i64> {
    match audio.kind {
        TrackKind::Audio { channels } => channels,
        TrackKind::Video { .. } | TrackKind::Subtitle { .. } => None,
    }
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

/// `tracks` with their video converted to 8-bit H.264 as `video` says, keeping the HDR it decodes
/// unless it tone-maps it.
// ffmpeg tags the H.264 with the colours it decoded, so an HDR10 or HLG picture stays one at 8
// bits.
// ponytail: checked only for libx264; a GPU encoder that drops those tags sends an untagged
// picture the Track calls HDR.
fn converted_to_h264(mut tracks: Vec<Track>, video: &VideoConversion) -> Vec<Track> {
    for track in &mut tracks {
        if let TrackKind::Video {
            bit_depth,
            width,
            height,
            hdr,
            ..
        } = &mut track.kind
        {
            "h264".clone_into(&mut track.codec);
            *bit_depth = Some(8);
            *hdr = hdr
                .and_then(transcoded_hdr)
                .filter(|_| video.tone_map.is_none());
            if let Some((scaled_width, scaled_height)) = video.limit.and_then(|limit| limit.size) {
                (*width, *height) = (Some(scaled_width), Some(scaled_height));
            }
        }
    }
    tracks
}

/// `probe`'s container and its name in plain words, if a Device can play it at all.
fn container(probe: &Probe) -> Option<(proto::Container, &'static str)> {
    // ffprobe names WebM and Matroska alike; WebM is Matroska holding only these codecs.
    const WEBM: [&str; 6] = ["vp8", "vp9", "av1", "opus", "vorbis", "webvtt"];
    let webm = (probe.tracks.iter()).all(|track| WEBM.contains(&track.codec.as_str()));
    match probe.container.as_str() {
        "mov,mp4,m4a,3gp,3g2,mj2" => Some((proto::Container::CONTAINER_MP4, "MP4")),
        "matroska,webm" if webm => Some((proto::Container::CONTAINER_WEBM, "WebM")),
        "matroska,webm" => Some((proto::Container::CONTAINER_MATROSKA, "Matroska")),
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

/// The Tracks of `probe` a plan plays as they are in the file: its first video, and the audio
/// Track at `audio`.
fn played_tracks(probe: &Probe, audio: Option<usize>) -> Vec<Track> {
    let audio = audio.and_then(|place| probe.tracks.get(place));
    [played_video(probe), audio]
        .into_iter()
        .flatten()
        .cloned()
        .collect()
}

/// The first video Track, which a browser plays.
fn played_video(probe: &Probe) -> Option<&Track> {
    (probe.tracks.iter()).find(|track| matches!(track.kind, TrackKind::Video { .. }))
}

/// How the played video's picture is HDR, or `None` for SDR or no video.
fn played_hdr(facts: &Facts) -> Option<Hdr> {
    let TrackKind::Video { hdr, .. } = facts.probe.as_ref().and_then(played_video)?.kind else {
        return None;
    };
    hdr
}

/// How the picture a Transcode decodes from video that's `hdr` is HDR, or `None` for SDR: a Dolby
/// Vision Track's Base Layer, since re-encoding drops Dolby Vision, or else its own.
fn transcoded_hdr(hdr: Hdr) -> Option<Hdr> {
    match hdr {
        Hdr::DolbyVision { base_layer, .. } => base_layer?.hdr(),
        hdr @ (Hdr::Hdr10 | Hdr::Hlg) => Some(hdr),
    }
}

/// The place in the Version's Tracks of the audio Track the plan plays: the one chosen, unless the
/// plan dropped it, or else [`file_audio`]. A choice that isn't an audio Track, as when the
/// Version changed since the User saw it, is the same as none.
fn played_audio_place(facts: &Facts) -> Option<usize> {
    let probe = facts.probe.as_ref()?;
    let chosen = (facts.audio)
        .filter(|_| !facts.dropped.contains(&Choice::Audio))
        .filter(|&place| probe.tracks.get(place).is_some_and(is_audio));
    chosen.or_else(|| file_audio(probe))
}

/// The place in `probe`'s Tracks of the audio Track a browser plays from the file: the one the
/// file marks to play by default, or else its first.
fn file_audio(probe: &Probe) -> Option<usize> {
    let audio = || (probe.tracks.iter().enumerate()).filter(|(_, track)| is_audio(track));
    (audio().find(|(_, track)| track.default))
        .or_else(|| audio().next())
        .map(|(place, _)| place)
}

const fn is_audio(track: &Track) -> bool {
    matches!(track.kind, TrackKind::Audio { .. })
}

/// The Version's subtitle Tracks that `device` shows itself.
fn shown_subtitles(version: i64, probe: &Probe, device: &DeviceDecoding) -> Vec<Subtitle> {
    (probe.tracks.iter().enumerate())
        .filter_map(|(position, track)| {
            let format = shown_as(device, probe, track)?;
            let fonts = if format == ASS {
                probe.font_attachments.len()
            } else {
                0
            };
            Some(Subtitle {
                track: track.clone(),
                format,
                url: subtitles::url(version, position, format),
                font_urls: (0..fonts)
                    .map(|font| subtitles::font_url(version, font))
                    .collect(),
            })
        })
        .collect()
}

fn direct_play_url(version: i64) -> String {
    format!("/media/versions/{version}")
}

/// Serves `GET /media/versions/<id>` with the file of the Version `id` in `contents`, and 404 if
/// there is none, marking the caller's Direct Plays of it in `running` asked for, and the
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

    use nama_proto::nama::v1::{
        AudioCodec, Container, PlaybackReasonKind as Kind, SubtitleFormat, VideoCodec,
    };

    use super::{
        Candidates, DeviceDecoding, Facts, Fallback, Method, Network, Offers, PickedLimit, Plan,
        PlaybackLimit, Playing, Reason, Subtitle, build, default_max_transcodes, narrowed,
        on_local_network, too_busy_to_play,
    };
    use crate::{
        ffmpeg::{
            Accel, CanDo, Hardware, Remux, RemuxAudio, ToneMapper, Transcode, VideoConversion,
            VideoLimit,
        },
        server_settings::HardwareChoice,
        tracks::{BaseLayer, Hdr, Probe, Track, TrackKind},
    };

    /// How long the Version in [`facts`] is.
    const LENGTH_MS: i64 = 7_200_000;

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

    const fn dolby_vision_8(base_layer: Option<BaseLayer>) -> Hdr {
        Hdr::DolbyVision {
            profile: 8,
            base_layer,
        }
    }

    /// How the video among `tracks` is HDR.
    fn video_hdr(tracks: &[Track]) -> Option<Hdr> {
        tracks.iter().find_map(|track| match track.kind {
            TrackKind::Video { hdr, .. } => hdr,
            TrackKind::Audio { .. } | TrackKind::Subtitle { .. } => None,
        })
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
        DeviceDecoding {
            containers: vec![Container::CONTAINER_MP4, Container::CONTAINER_WEBM],
            video_codecs: vec![
                VideoCodec::VIDEO_CODEC_H264,
                VideoCodec::VIDEO_CODEC_VP8,
                VideoCodec::VIDEO_CODEC_VP9,
                VideoCodec::VIDEO_CODEC_AV1,
            ],
            audio_codecs: vec![
                AudioCodec::AUDIO_CODEC_AAC,
                AudioCodec::AUDIO_CODEC_MP3,
                AudioCodec::AUDIO_CODEC_OPUS,
                AudioCodec::AUDIO_CODEC_VORBIS,
                AudioCodec::AUDIO_CODEC_FLAC,
            ],
            hdr: false,
            hls: true,
            audio_channels: 2,
            subtitle_formats: vec![],
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
                duration_ms: Some(LENGTH_MS),
                bitrate: Some(5_000_000),
                tracks,
                font_attachments: vec![],
            }),
            device: firefox(),
            can_do: CanDo {
                runs: true,
                zscale: true,
                libx264: true,
                hardware: vec![],
            },
            failed: BTreeSet::new(),
            audio: None,
            transcodes_full: None,
            picked_limit: PickedLimit::Unpicked,
            network: Network::Local,
            subtitle: None,
            hardware_choice: HardwareChoice::Automatic,
            dropped: BTreeSet::new(),
        }
    }

    /// What `plan` Transcodes, failing the test if it doesn't Transcode.
    fn transcoded(plan: &Plan) -> Transcode {
        match &plan.playing {
            Some(Playing::Transcode { transcode, .. }) => transcode.clone(),
            other => panic!("{other:?} isn't a Transcode"),
        }
    }

    /// What `plan` Transcodes in software once its hardware fails, failing the test if it
    /// doesn't Transcode.
    fn falls_back_to(plan: &Plan) -> Option<Fallback> {
        match &plan.playing {
            Some(Playing::Transcode { fallback, .. }) => fallback.clone(),
            other => panic!("{other:?} isn't a Transcode"),
        }
    }

    /// A Remux playing the audio Track `track`, converting it to AAC with `aac_channels` if given.
    const fn remuxing(track: usize, aac_channels: Option<i64>) -> Playing {
        Playing::Remux {
            remux: Remux {
                audio: Some(RemuxAudio {
                    track,
                    aac_channels,
                }),
            },
            duration_ms: LENGTH_MS,
        }
    }

    fn subtitle() -> Track {
        track("subrip", true, TrackKind::Subtitle { image: false })
    }

    /// A reason's kind and params, as a test expects them.
    type Given<'a> = (Kind, Vec<(&'a str, &'a str)>);

    /// The kind and params of each of `plan`'s reasons, in order.
    fn given(plan: &Plan) -> Vec<Given<'_>> {
        plan.reasons.iter().map(kind_and_params).collect()
    }

    fn kind_and_params(reason: &Reason) -> Given<'_> {
        let params = (reason.params.iter())
            .map(|(key, value)| (*key, value.as_str()))
            .collect();
        (reason.kind, params)
    }

    /// The kind of each of `plan`'s reasons, in order.
    fn kinds(plan: &Plan) -> Vec<Kind> {
        plan.reasons.iter().map(|reason| reason.kind).collect()
    }

    /// `plan`'s reason of `kind`, failing the test if it has none.
    fn reason_of(plan: &Plan, kind: Kind) -> &Reason {
        (plan.reasons.iter())
            .find(|reason| reason.kind == kind)
            .unwrap_or_else(|| panic!("no {kind:?} among {:?}", kinds(plan)))
    }

    /// Whether each of `plan`'s reasons decided it, in order.
    fn decided(plan: &Plan) -> Vec<bool> {
        plan.reasons.iter().map(|reason| reason.decided).collect()
    }

    #[test]
    fn an_h264_and_aac_mp4_direct_plays_with_no_reasons() {
        let plan = build(&mut facts(Some(vec![video("h264"), audio("aac", true)])));

        assert_eq!(
            plan,
            Plan {
                version: 7,
                playing: Some(Playing::DirectPlay),
                tracks: vec![video("h264"), audio("aac", true)],
                reasons: vec![],
                subtitles: vec![],
                limit: None,
                audio: Some(1),
                burn_in: None,
                offers: Offers {
                    audio: vec![1],
                    burn_in: vec![],
                    limits: vec![PlaybackLimit::P720, PlaybackLimit::P480],
                },
            }
        );
    }

    #[test]
    fn a_version_the_device_cant_decode_cant_be_direct_played_and_each_thing_decided_it() {
        let hdr10 = Some(Hdr::Hdr10);
        let cases: [(&str, Vec<Track>, Vec<Given<'_>>); 3] = [
            (
                MKV,
                vec![video("hevc"), audio("aac", true)],
                vec![
                    (
                        Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_CONTAINER,
                        vec![("container", "CONTAINER_MATROSKA")],
                    ),
                    (
                        Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_VIDEO,
                        vec![("codec", "hevc")],
                    ),
                ],
            ),
            (
                MP4,
                vec![hdr_video("av1", hdr10), audio("aac", true)],
                vec![(
                    Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_SHOW_HDR,
                    vec![("hdrFormat", "HDR_FORMAT_HDR10")],
                )],
            ),
            (
                "avi",
                vec![video("mpeg4"), audio("pcm_s16le", true)],
                vec![
                    (
                        Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_CONTAINER,
                        vec![("container", "avi")],
                    ),
                    (
                        Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_VIDEO,
                        vec![("codec", "mpeg4")],
                    ),
                    (
                        Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_AUDIO,
                        vec![("codec", "pcm_s16le")],
                    ),
                ],
            ),
        ];
        for (container, tracks, cant) in cases {
            let plan = build(&mut in_container(container, Some(tracks)));

            assert_eq!(plan.method(), Some(Method::Transcode), "{cant:?}");
            assert_eq!(given(&plan)[..cant.len()], cant);
            assert!(
                plan.reasons[..cant.len()]
                    .iter()
                    .all(|reason| reason.decided),
                "{cant:?}"
            );
        }
    }

    #[test]
    fn a_device_decodes_what_its_codecs_and_display_say() {
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
            let mut facts = Facts {
                device,
                ..in_container(container, Some(tracks.clone()))
            };

            let plan = build(&mut facts);

            assert_eq!(plan.method(), Some(Method::DirectPlay), "{tracks:?}");
        }
    }

    #[test]
    fn only_a_transcode_helps_with_video_the_device_cant_decode() {
        let cases = [
            (MKV, vec![video("h264"), audio("aac", true)], Method::Remux),
            (
                MKV,
                vec![video("hevc"), audio("aac", true)],
                Method::Transcode,
            ),
            (MP4, vec![video("h264"), audio("dts", true)], Method::Remux),
            (
                MP4,
                vec![video("hevc"), audio("dts", true)],
                Method::Transcode,
            ),
            (
                MP4,
                vec![hdr_video("av1", Some(Hdr::Hdr10)), audio("aac", true)],
                Method::Transcode,
            ),
        ];
        for (container, tracks, method) in cases {
            let plan = build(&mut in_container(container, Some(tracks.clone())));

            assert_eq!(plan.method(), Some(method), "{tracks:?}");
            let rules_out_a_remux = kinds(&plan).iter().any(|kind| {
                matches!(
                    kind,
                    Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_VIDEO
                        | Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_SHOW_HDR
                )
            });
            assert_eq!(rules_out_a_remux, method == Method::Transcode, "{tracks:?}");
        }
    }

    #[test]
    fn a_version_whose_length_isnt_known_isnt_remuxed_or_transcoded_and_says_why() {
        let unknown_length = |container| {
            let mut facts = in_container(container, Some(vec![video("h264"), audio("aac", true)]));
            if let Some(probe) = &mut facts.probe {
                probe.duration_ms = None;
            }
            build(&mut facts)
        };

        let mkv = unknown_length(MKV);
        let mp4 = unknown_length(MP4);

        assert_eq!(mkv.method(), None);
        assert_eq!(
            kinds(&mkv),
            [
                Kind::PLAYBACK_REASON_KIND_LENGTH_UNKNOWN,
                Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_CONTAINER,
            ]
        );
        assert_eq!(decided(&mkv), [true, true], "nothing plays it");
        assert_eq!(mkv.offers.audio, [1]);
        assert_eq!(
            mp4.method(),
            Some(Method::DirectPlay),
            "Direct Play needs no length"
        );
    }

    #[test]
    fn a_remux_and_a_transcode_carry_the_versions_length() {
        let remux = build(&mut in_container(
            MKV,
            Some(vec![video("h264"), audio("aac", true)]),
        ));
        let transcode = build(&mut facts(Some(vec![video("hevc"), audio("aac", true)])));

        assert!(
            matches!(
                remux.playing,
                Some(Playing::Remux {
                    duration_ms: LENGTH_MS,
                    ..
                })
            ),
            "{:?}",
            remux.playing
        );
        assert!(
            matches!(
                transcode.playing,
                Some(Playing::Transcode {
                    duration_ms: LENGTH_MS,
                    ..
                })
            ),
            "{:?}",
            transcode.playing
        );
    }

    #[test]
    fn a_version_whose_tracks_arent_known_direct_plays_and_says_so_as_context() {
        let plan = build(&mut facts(None));

        assert_eq!(plan.method(), Some(Method::DirectPlay));
        assert_eq!(plan.tracks, []);
        assert_eq!(
            kinds(&plan),
            [
                Kind::PLAYBACK_REASON_KIND_TRACKS_UNKNOWN,
                Kind::PLAYBACK_REASON_KIND_LENGTH_UNKNOWN,
            ]
        );
        assert_eq!(decided(&plan), [false, false]);
    }

    #[test]
    fn a_version_whose_tracks_arent_known_cant_be_played_once_direct_play_failed() {
        let mut facts = Facts {
            failed: BTreeSet::from([Method::DirectPlay]),
            ..facts(None)
        };

        let plan = build(&mut facts);

        assert_eq!(plan.method(), None);
    }

    #[test]
    fn a_version_whose_tracks_arent_known_is_never_too_busy_to_play() {
        let facts = Facts {
            failed: BTreeSet::from([Method::DirectPlay, Method::Remux]),
            transcodes_full: Some(1),
            ..facts(None)
        };

        assert_eq!(too_busy_to_play(facts), None);
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

            let plan = build(&mut facts(Some(with_subtitle)));

            assert_eq!(plan.tracks, [video("h264"), played], "{tracks:?}");
        }
    }

    #[test]
    fn a_chosen_audio_track_other_than_the_files_own_is_remuxed_and_the_plan_says_so() {
        let (english, commentary) = (audio("aac", true), audio("aac", false));
        let mut facts = Facts {
            audio: Some(3),
            ..facts(Some(vec![
                video("h264"),
                english,
                subtitle(),
                commentary.clone(),
            ]))
        };

        let plan = build(&mut facts);

        assert_eq!(plan.playing, Some(remuxing(1, None)), "counting only audio");
        assert_eq!(plan.tracks, [video("h264"), commentary]);
        assert_eq!(
            kinds(&plan),
            [Kind::PLAYBACK_REASON_KIND_CHOSEN_AUDIO_NOT_FILES_OWN]
        );
        assert_eq!(decided(&plan), [true]);
    }

    #[test]
    fn choosing_the_files_own_audio_track_or_no_audio_track_plays_the_files_own() {
        let (mp3, english) = (audio("mp3", false), audio("aac", true));
        let tracks = vec![video("h264"), mp3, english.clone(), subtitle()];
        for chosen in [Some(2), Some(0), Some(3), Some(9)] {
            let mut facts = Facts {
                audio: chosen,
                ..facts(Some(tracks.clone()))
            };

            let plan = build(&mut facts);

            assert_eq!(plan.method(), Some(Method::DirectPlay), "{chosen:?}");
            assert_eq!(plan.tracks, [video("h264"), english.clone()]);
        }
    }

    #[test]
    fn a_chosen_audio_track_the_device_cant_decode_is_converted_to_aac() {
        let mut facts = Facts {
            audio: Some(2),
            ..facts(Some(vec![
                video("h264"),
                audio("aac", true),
                audio("dts", false),
            ]))
        };

        let plan = build(&mut facts);

        assert_eq!(plan.playing, Some(remuxing(1, Some(2))));
        assert_eq!(plan.tracks[1], audio("aac", false));
    }

    #[test]
    fn a_version_only_its_container_keeps_from_direct_playing_is_remuxed() {
        let plan = build(&mut in_container(
            MKV,
            Some(vec![video("h264"), audio("aac", true)]),
        ));

        assert_eq!(plan.method(), Some(Method::Remux));
        assert_eq!(
            given(&plan),
            [(
                Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_CONTAINER,
                vec![("container", "CONTAINER_MATROSKA")]
            )]
        );
        assert_eq!(decided(&plan), [true]);
    }

    #[test]
    fn each_method_that_just_failed_on_the_device_is_ruled_out_and_the_plan_says_so() {
        use Method::{DirectPlay, Remux, Transcode};
        let failed_on_device = |method| {
            (
                Kind::PLAYBACK_REASON_KIND_METHOD_FAILED_ON_DEVICE,
                vec![("method", method)],
            )
        };
        let cases = [
            (vec![], DirectPlay, vec![]),
            (
                vec![DirectPlay],
                Remux,
                vec![failed_on_device("PLAYBACK_METHOD_DIRECT_PLAY")],
            ),
            (
                vec![Remux, DirectPlay],
                Transcode,
                vec![
                    failed_on_device("PLAYBACK_METHOD_DIRECT_PLAY"),
                    failed_on_device("PLAYBACK_METHOD_REMUX"),
                ],
            ),
        ];
        for (failed, method, reasons) in cases {
            let mut facts = Facts {
                failed: BTreeSet::from_iter(failed),
                ..facts(Some(vec![video("h264"), audio("aac", true)]))
            };

            let plan = build(&mut facts);

            assert_eq!(plan.method(), Some(method));
            let failures: Vec<Given<'_>> = (given(&plan).into_iter())
                .filter(|(kind, _)| *kind == Kind::PLAYBACK_REASON_KIND_METHOD_FAILED_ON_DEVICE)
                .collect();
            assert_eq!(failures, reasons, "{method:?}");
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
                format: SubtitleFormat::SUBTITLE_FORMAT_WEBVTT,
                url: "/media/versions/7/subtitles/1".to_owned(),
                font_urls: vec![],
            },
            Subtitle {
                track: french,
                format: SubtitleFormat::SUBTITLE_FORMAT_WEBVTT,
                url: "/media/versions/7/subtitles/5".to_owned(),
                font_urls: vec![],
            },
        ];

        for (container, method) in [(MP4, Method::DirectPlay), (MKV, Method::Remux)] {
            let plan = build(&mut in_container(container, Some(tracks.clone())));

            assert_eq!(plan.method(), Some(method), "{container}");
            assert_eq!(plan.subtitles, offered, "{container}");
        }
    }

    #[test]
    fn without_ffmpeg_on_the_server_or_hls_on_the_device_a_version_isnt_remuxed() {
        let tracks = Some(vec![video("h264"), audio("aac", true)]);
        let mut no_ffmpeg = in_container(MKV, tracks.clone());
        without_ffmpeg(&mut no_ffmpeg);
        let mut no_hls = in_container(MKV, tracks);
        no_hls.device.hls = false;

        let without_ffmpeg = build(&mut no_ffmpeg);
        let without_hls = build(&mut no_hls);

        assert_eq!(without_ffmpeg.method(), None);
        assert_eq!(
            kinds(&without_ffmpeg),
            [
                Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_CONTAINER,
                Kind::PLAYBACK_REASON_KIND_SERVER_LACKS_FFMPEG,
            ]
        );
        assert_eq!(without_hls.method(), None);
        assert_eq!(
            kinds(&without_hls),
            [
                Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_CONTAINER,
                Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_PLAY_HLS,
            ]
        );
    }

    #[test]
    fn without_libx264_on_the_server_a_version_isnt_transcoded_but_still_remuxes_or_direct_plays() {
        let without_libx264 = |container, tracks| {
            let mut facts = in_container(container, Some(tracks));
            facts.can_do.libx264 = false;
            facts
        };
        let mut transcode = without_libx264(MP4, vec![video("hevc"), audio("aac", true)]);
        let mut remux = without_libx264(MKV, vec![video("h264"), audio("aac", true)]);
        let mut direct_play = without_libx264(MP4, vec![video("h264"), audio("aac", true)]);

        let transcode = build(&mut transcode);

        assert_eq!(transcode.method(), None);
        assert_eq!(
            kinds(&transcode),
            [
                Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_VIDEO,
                Kind::PLAYBACK_REASON_KIND_SERVER_CANT_ENCODE_VIDEO,
            ]
        );
        assert_eq!(build(&mut remux).method(), Some(Method::Remux));
        assert_eq!(build(&mut direct_play).method(), Some(Method::DirectPlay));
    }

    #[test]
    fn a_plan_that_doesnt_transcode_doesnt_say_where_a_transcode_runs() {
        let tracks = || Some(vec![video("h264"), audio("aac", true)]);
        let mut no_hls = in_container(MKV, tracks());
        no_hls.device.hls = false;
        let mut no_ffmpeg = in_container(MKV, tracks());
        without_ffmpeg(&mut no_ffmpeg);
        let mut no_libx264 = in_container(MKV, tracks());
        no_libx264.can_do.libx264 = false;
        let cases = [
            ("no ffmpeg", no_ffmpeg),
            ("no libx264", no_libx264),
            ("unprobed Version", facts(None)),
            ("no HLS", no_hls),
            ("a Remux", in_container(MKV, tracks())),
        ];
        for (case, mut facts) in cases {
            let plan = build(&mut facts);

            assert!(
                !kinds(&plan).iter().any(|kind| {
                    matches!(
                        kind,
                        Kind::PLAYBACK_REASON_KIND_TRANSCODES_ON_HARDWARE
                            | Kind::PLAYBACK_REASON_KIND_NO_HARDWARE_ACCELERATION_PASSED
                    )
                }),
                "{case}: {:?}",
                kinds(&plan)
            );
        }
    }

    fn rules_out_direct_play(_: &Facts, _: Option<&Candidates>) -> Vec<Reason> {
        vec![
            Reason::new(Kind::PLAYBACK_REASON_KIND_CHOSEN_AUDIO_NOT_FILES_OWN, "")
                .ruling_out([Method::DirectPlay]),
        ]
    }

    fn rules_nothing_out(_: &Facts, _: Option<&Candidates>) -> Vec<Reason> {
        vec![Reason::new(
            Kind::PLAYBACK_REASON_KIND_REMOTE_DEFAULT_LIMIT,
            "",
        )]
    }

    #[test]
    fn a_later_step_cant_allow_what_an_earlier_step_ruled_out() {
        let mut reasons = vec![];

        let allowed = narrowed(
            &facts(None),
            None,
            &[rules_out_direct_play, rules_nothing_out],
            BTreeSet::from(Method::ALL),
            &mut reasons,
        );

        assert_eq!(allowed, BTreeSet::from([Method::Remux, Method::Transcode]));
        assert_eq!(reasons.len(), 2);
    }

    #[test]
    fn audio_the_device_cant_decode_is_remuxed_to_aac_and_the_plan_says_so() {
        let dts = audio_with("dts", true, Some(6));
        let mut facts = in_container(MKV, Some(vec![video("h264"), dts.clone()]));
        facts.device.audio_channels = 6;

        let plan = build(&mut facts);

        assert_eq!(plan.method(), Some(Method::Remux));
        let aac = Track {
            codec: "aac".to_owned(),
            ..dts
        };
        assert_eq!(plan.tracks, [video("h264"), aac]);
        assert_eq!(
            given(&plan),
            [
                (
                    Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_CONTAINER,
                    vec![("container", "CONTAINER_MATROSKA")]
                ),
                (
                    Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_AUDIO,
                    vec![("codec", "dts")]
                ),
            ],
            "no reason says what a Remux makes"
        );
    }

    #[test]
    fn converted_audio_has_as_many_channels_as_the_device_plays_up_to_5_1_and_says_why_fewer() {
        let fewer = Kind::PLAYBACK_REASON_KIND_DEVICE_PLAYS_FEWER_CHANNELS;
        let unknown = Kind::PLAYBACK_REASON_KIND_DEVICE_CHANNELS_UNKNOWN;
        let capped = Kind::PLAYBACK_REASON_KIND_CONVERSION_CHANNELS_CAPPED;
        let cases = [
            (Some(6), 6, Some(6), None),
            (Some(8), 8, Some(6), Some((capped, "6"))),
            (Some(6), 2, Some(2), Some((fewer, "2"))),
            (Some(6), 4, Some(2), Some((fewer, "2"))),
            (Some(6), 0, Some(2), Some((unknown, "2"))),
            (Some(2), 6, Some(2), None),
            (Some(1), 6, Some(1), None),
            (None, 6, Some(2), None),
        ];
        for (source, plays, converted, why) in cases {
            let mut facts = in_container(
                MP4,
                Some(vec![video("h264"), audio_with("dts", true, source)]),
            );
            facts.device.audio_channels = plays;

            let plan = build(&mut facts);

            let case = format!("{source:?} on {plays}");
            assert_eq!(plan.tracks[1], audio_with("aac", true, converted), "{case}");
            let channel_reasons: Vec<(Given<'_>, bool)> = (plan.reasons.iter())
                .filter(|reason| [fewer, unknown, capped].contains(&reason.kind))
                .map(|reason| (kind_and_params(reason), reason.decided))
                .collect();
            let expected: Vec<_> = (why.into_iter())
                .map(|(kind, channels)| ((kind, vec![("channels", channels)]), true))
                .collect();
            assert_eq!(channel_reasons, expected, "{case}");
        }
    }

    #[test]
    fn video_the_device_cant_decode_is_transcoded_to_h264_and_aac_and_the_plan_says_why() {
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
        let mut facts = in_container(MKV, Some(vec![hevc, audio_with("aac", true, Some(6))]));

        let plan = build(&mut facts);

        assert_eq!(plan.method(), Some(Method::Transcode));
        assert_eq!(
            plan.tracks,
            [video("h264"), audio_with("aac", true, Some(2))]
        );
        assert_eq!(
            given(&plan),
            [
                (
                    Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_CONTAINER,
                    vec![("container", "CONTAINER_MATROSKA")]
                ),
                (
                    Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_VIDEO,
                    vec![("codec", "hevc")]
                ),
                (
                    Kind::PLAYBACK_REASON_KIND_DEVICE_PLAYS_FEWER_CHANNELS,
                    vec![("channels", "2")]
                ),
                (
                    Kind::PLAYBACK_REASON_KIND_NO_HARDWARE_ACCELERATION_PASSED,
                    vec![]
                ),
            ]
        );
        assert_eq!(decided(&plan), [true, true, true, false]);
    }

    #[test]
    fn hdr_transcoded_for_a_device_that_cant_show_it_is_tone_mapped_and_the_plan_says_so() {
        let cases = [
            (Hdr::Hdr10, "HDR_FORMAT_HDR10"),
            (Hdr::Hlg, "HDR_FORMAT_HLG"),
            (
                dolby_vision_8(Some(BaseLayer::Hdr10)),
                "HDR_FORMAT_DOLBY_VISION",
            ),
            (
                dolby_vision_8(Some(BaseLayer::Hlg)),
                "HDR_FORMAT_DOLBY_VISION",
            ),
        ];
        for (hdr, name) in cases {
            let mut facts = in_container(MP4, Some(vec![hdr_video("hevc", Some(hdr))]));

            let plan = build(&mut facts);

            assert_eq!(
                transcoded(&plan).video.tone_map,
                Some(ToneMapper::Software),
                "{name}"
            );
            assert_eq!(video_hdr(&plan.tracks), None, "{hdr:?} comes out SDR");
            let tone_mapped = reason_of(&plan, Kind::PLAYBACK_REASON_KIND_HDR_TONE_MAPPED);
            assert_eq!(
                kind_and_params(tone_mapped).1,
                [("hdrFormat", name)],
                "{name}"
            );
            assert!(tone_mapped.decided, "{name}");
        }
    }

    #[test]
    fn without_zscale_hdr_isnt_tone_mapped_and_the_plan_says_its_colours_look_washed_out() {
        let mut facts = in_container(MP4, Some(vec![hdr_video("hevc", Some(Hdr::Hdr10))]));
        facts.can_do.zscale = false;

        let plan = build(&mut facts);

        assert_eq!(transcoded(&plan).video.tone_map, None);
        let hdr10 = vec![("hdrFormat", "HDR_FORMAT_HDR10")];
        assert_eq!(
            given(&plan)[..3],
            [
                (
                    Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_VIDEO,
                    vec![("codec", "hevc")]
                ),
                (
                    Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_SHOW_HDR,
                    hdr10.clone()
                ),
                (Kind::PLAYBACK_REASON_KIND_HDR_NOT_TONE_MAPPED, hdr10),
            ]
        );
    }

    #[test]
    fn hdr_transcoded_for_a_device_that_shows_it_isnt_tone_mapped() {
        let mut facts = in_container(MP4, Some(vec![hdr_video("hevc", Some(Hdr::Hdr10))]));
        facts.device.hdr = true;

        let plan = build(&mut facts);

        assert_eq!(transcoded(&plan).video.tone_map, None);
        assert_eq!(
            kinds(&plan),
            [
                Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_VIDEO,
                Kind::PLAYBACK_REASON_KIND_NO_HARDWARE_ACCELERATION_PASSED,
            ]
        );
    }

    #[test]
    fn a_transcode_that_doesnt_tone_map_lists_the_hdr_of_what_it_decodes() {
        let cases = [
            (Hdr::Hdr10, Some(Hdr::Hdr10)),
            (Hdr::Hlg, Some(Hdr::Hlg)),
            (dolby_vision_8(Some(BaseLayer::Hdr10)), Some(Hdr::Hdr10)),
            (dolby_vision_8(Some(BaseLayer::Hlg)), Some(Hdr::Hlg)),
            (dolby_vision_8(Some(BaseLayer::Sdr)), None),
        ];
        for (hdr, played) in cases {
            let mut facts = in_container(MP4, Some(vec![hdr_video("hevc", Some(hdr))]));
            facts.device.hdr = true;

            let plan = build(&mut facts);

            assert_eq!(transcoded(&plan).video.tone_map, None, "{hdr:?}");
            assert_eq!(video_hdr(&plan.tracks), played, "{hdr:?}");
        }
    }

    #[test]
    fn dolby_vision_over_sdr_isnt_tone_mapped_for_a_device_that_cant_show_hdr() {
        let hdr = dolby_vision_8(Some(BaseLayer::Sdr));
        let mut facts = in_container(MP4, Some(vec![hdr_video("hevc", Some(hdr))]));

        let plan = build(&mut facts);

        assert_eq!(transcoded(&plan).video.tone_map, None);
        assert!(
            !kinds(&plan).iter().any(|kind| matches!(
                kind,
                Kind::PLAYBACK_REASON_KIND_HDR_TONE_MAPPED
                    | Kind::PLAYBACK_REASON_KIND_HDR_NOT_TONE_MAPPED
            )),
            "{:?}",
            kinds(&plan)
        );
        assert_eq!(video_hdr(&plan.tracks), None);
    }

    #[test]
    fn dolby_vision_without_a_base_layer_isnt_transcoded_so_a_device_that_cant_decode_it_cant_play_it()
     {
        let profile_5 = Hdr::DolbyVision {
            profile: 5,
            base_layer: None,
        };
        let mut facts = in_container(MP4, Some(vec![hdr_video("hevc", Some(profile_5))]));

        let plan = build(&mut facts);

        assert_eq!(plan.method(), None);
        assert_eq!(
            kinds(&plan),
            [
                Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_VIDEO,
                Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_SHOW_HDR,
                Kind::PLAYBACK_REASON_KIND_DOLBY_VISION_NEEDS_BASE_LAYER,
            ]
        );
        assert_eq!(decided(&plan), [true, true, true]);
    }

    #[test]
    fn a_transcode_of_a_version_without_audio_says_nothing_of_audio() {
        let mut facts = in_container(MP4, Some(vec![video("hevc")]));

        let plan = build(&mut facts);

        assert_eq!(plan.method(), Some(Method::Transcode));
        assert_eq!(
            kinds(&plan),
            [
                Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_VIDEO,
                Kind::PLAYBACK_REASON_KIND_NO_HARDWARE_ACCELERATION_PASSED,
            ]
        );
    }

    #[test]
    fn a_busy_server_remuxes_what_the_device_can_play_remuxed_and_says_so_as_context() {
        let mut facts = Facts {
            transcodes_full: Some(1),
            ..in_container(MKV, Some(vec![video("h264"), audio("aac", true)]))
        };

        let plan = build(&mut facts);

        assert_eq!(plan.method(), Some(Method::Remux));
        assert_eq!(
            given(&plan)[1],
            (
                Kind::PLAYBACK_REASON_KIND_SERVER_BUSY,
                vec![("maxTranscodes", "1")]
            )
        );
        assert_eq!(decided(&plan), [true, false], "a Remux would play anyway");
        assert_eq!(too_busy_to_play(facts), None);
    }

    #[test]
    fn a_plan_only_a_transcode_would_play_is_too_busy_to_play_while_the_server_is_busy() {
        let hevc = || Some(vec![video("hevc"), audio("aac", true)]);
        let busy = |facts| Facts {
            transcodes_full: Some(2),
            ..facts
        };
        let mut without_hls = busy(facts(hevc()));
        without_hls.device.hls = false;

        assert_eq!(build(&mut busy(facts(hevc()))).method(), None);
        assert_eq!(too_busy_to_play(busy(facts(hevc()))), Some(2));
        assert_eq!(too_busy_to_play(facts(hevc())), None, "not busy");
        assert_eq!(too_busy_to_play(without_hls), None, "nothing plays it");
    }

    #[test]
    fn audio_the_device_decodes_is_copied_into_a_remux() {
        let mut facts = in_container(MKV, Some(vec![video("h264"), audio("aac", true)]));

        let plan = build(&mut facts);

        assert_eq!(plan.playing, Some(remuxing(0, None)));
        assert_eq!(plan.tracks, [video("h264"), audio("aac", true)]);
    }

    #[test]
    fn a_playback_limit_below_the_versions_picture_transcodes_it_down_to_fit_and_says_so() {
        let mut facts = Facts {
            picked_limit: PickedLimit::Picked(Some(PlaybackLimit::P720)),
            ..facts(Some(vec![video("h264"), audio("aac", true)]))
        };

        let plan = build(&mut facts);

        assert_eq!(plan.method(), Some(Method::Transcode));
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
        let scaled_down = reason_of(&plan, Kind::PLAYBACK_REASON_KIND_PICTURE_SCALED_DOWN);
        assert_eq!(
            kind_and_params(scaled_down).1,
            [
                ("playbackLimit", "PLAYBACK_LIMIT_720P"),
                ("widthPixels", "1280"),
                ("heightPixels", "720"),
            ]
        );
        assert!(scaled_down.decided);
    }

    #[test]
    fn outside_the_local_network_the_remote_default_limits_a_device_until_its_user_picks_one() {
        let remote = |picked_limit| Facts {
            picked_limit,
            network: Network::Remote(Some(PlaybackLimit::P720)),
            ..facts(Some(vec![video("h264"), audio("aac", true)]))
        };

        let defaulted = build(&mut remote(PickedLimit::Unpicked));
        let picked = build(&mut remote(PickedLimit::Picked(None)));

        assert_eq!(defaulted.method(), Some(Method::Transcode));
        assert_eq!(defaulted.limit, Some(PlaybackLimit::P720));
        let remote_default = reason_of(&defaulted, Kind::PLAYBACK_REASON_KIND_REMOTE_DEFAULT_LIMIT);
        assert_eq!(
            kind_and_params(remote_default).1,
            [("playbackLimit", "PLAYBACK_LIMIT_720P")]
        );
        assert!(!remote_default.decided, "the limit's own reason decided it");
        assert_eq!(picked.method(), Some(Method::DirectPlay));
        assert_eq!(picked.limit, None);
        assert_eq!(picked.reasons, []);
    }

    #[test]
    fn loopback_private_link_local_and_unique_local_addresses_are_on_the_local_network() {
        let cases = [
            ("127.0.0.1", true),
            ("10.1.2.3", true),
            ("172.16.0.9", true),
            ("192.168.1.5", true),
            ("169.254.10.20", true),
            ("::1", true),
            ("fe80::1", true),
            ("fd12:3456::1", true),
            ("203.0.113.7", false),
            ("100.64.0.1", false),
            ("2001:db8::1", false),
        ];
        for (address, local) in cases {
            assert_eq!(
                on_local_network(address.parse().unwrap()),
                local,
                "{address}"
            );
        }
    }

    #[test]
    fn a_playback_limit_the_versions_picture_fits_or_original_leaves_it_direct_played() {
        let probed = Some(vec![video("h264"), audio("aac", true)]);
        let size_unknown = (
            Kind::PLAYBACK_REASON_KIND_PICTURE_SIZE_UNKNOWN,
            vec![("playbackLimit", "PLAYBACK_LIMIT_480P")],
        );
        let cases = [
            (Some(PlaybackLimit::P1080), probed.clone(), None),
            (None, probed, None),
            (Some(PlaybackLimit::P480), None, Some(size_unknown)),
        ];
        for (limit, tracks, last) in cases {
            let mut facts = Facts {
                picked_limit: PickedLimit::Picked(limit),
                ..facts(tracks)
            };

            let plan = build(&mut facts);

            assert_eq!(plan.method(), Some(Method::DirectPlay), "{limit:?}");
            assert_eq!(given(&plan).pop(), last, "{limit:?}");
        }
    }

    #[test]
    fn a_transcode_scales_a_picture_down_to_a_playback_limit_keeping_its_shape_with_even_sides() {
        let cases = [
            ((1920, 800), PlaybackLimit::P720, (1280, 532)),
            ((3840, 2160), PlaybackLimit::P480, (852, 480)),
            ((1440, 1080), PlaybackLimit::P720, (960, 720)),
            ((3840, 2160), PlaybackLimit::P1080, (1920, 1080)),
            ((1080, 1920), PlaybackLimit::P720, (720, 1280)),
        ];
        for (picture, limit, scaled) in cases {
            let plan = build(&mut with_picture_limited_to(picture, limit));

            assert_eq!(
                transcoded(&plan).video.limit,
                Some(VideoLimit {
                    size: Some(scaled),
                    bits_per_second: limit.bits_per_second,
                }),
                "{picture:?} at {limit:?}"
            );
        }
    }

    #[test]
    fn a_picture_within_a_playback_limit_on_its_short_side_is_direct_played() {
        let cases = [
            ((1920, 1080), PlaybackLimit::P1080),
            ((1280, 536), PlaybackLimit::P720),
            ((1080, 1920), PlaybackLimit::P1080),
        ];
        for (picture, limit) in cases {
            let plan = build(&mut with_picture_limited_to(picture, limit));

            assert_eq!(
                plan.playing,
                Some(Playing::DirectPlay),
                "{picture:?} at {limit:?}"
            );
        }
    }

    /// An H.264 Version with a picture `width` by `height`, whose User picked `limit`.
    fn with_picture_limited_to((width, height): (i64, i64), limit: PlaybackLimit) -> Facts {
        let picture = TrackKind::Video {
            width: Some(width),
            height: Some(height),
            frame_rate: None,
            bit_depth: None,
            hdr: None,
        };
        Facts {
            picked_limit: PickedLimit::Picked(Some(limit)),
            ..facts(Some(vec![
                track("h264", false, picture),
                audio("aac", true),
            ]))
        }
    }

    fn pgs() -> Track {
        image_subtitle("hdmv_pgs_subtitle")
    }

    fn image_subtitle(codec: &str) -> Track {
        track(codec, false, TrackKind::Subtitle { image: true })
    }

    /// A Device that draws PGS subtitles itself.
    fn drawing_pgs() -> DeviceDecoding {
        DeviceDecoding {
            subtitle_formats: vec![SubtitleFormat::SUBTITLE_FORMAT_PGS],
            ..firefox()
        }
    }

    #[test]
    fn a_chosen_subtitle_the_device_cant_draw_is_burned_in_by_a_transcode_and_the_plan_names_it() {
        let ass = track("ass", false, TrackKind::Subtitle { image: false });
        let cases = [
            (pgs(), firefox(), "PGS"),
            (image_subtitle("dvd_subtitle"), drawing_pgs(), "VobSub"),
            (image_subtitle("dvb_subtitle"), drawing_pgs(), "DVB"),
            (image_subtitle("xsub"), drawing_pgs(), "XSUB"),
            (ass, drawing_pgs(), "ASS"),
        ];
        for (chosen, device, name) in cases {
            let mut facts = Facts {
                subtitle: Some(2),
                device,
                ..facts(Some(vec![
                    video("h264"),
                    audio("aac", true),
                    chosen.clone(),
                ]))
            };

            let plan = build(&mut facts);

            assert_eq!(plan.method(), Some(Method::Transcode), "{}", chosen.codec);
            assert!(
                reason_of(&plan, Kind::PLAYBACK_REASON_KIND_SUBTITLES_BURNED_IN).decided,
                "{}",
                chosen.codec
            );
            assert_eq!(plan.burn_in, Some(2), "{name}");
            assert_eq!(plan.offers.burn_in, vec![2], "{name}");
        }
    }

    #[test]
    fn a_device_that_draws_pgs_shows_each_pgs_track_itself_even_when_the_plan_transcodes() {
        for (video_codec, method) in [("h264", Method::DirectPlay), ("hevc", Method::Transcode)] {
            let tracks = vec![video(video_codec), audio("aac", true), pgs(), subtitle()];
            let mut facts = Facts {
                device: drawing_pgs(),
                ..facts(Some(tracks))
            };

            let plan = build(&mut facts);

            assert_eq!(plan.method(), Some(method), "{video_codec}");
            assert_eq!(
                plan.subtitles,
                [
                    Subtitle {
                        track: pgs(),
                        format: SubtitleFormat::SUBTITLE_FORMAT_PGS,
                        url: "/media/versions/7/subtitles/2".to_owned(),
                        font_urls: vec![],
                    },
                    Subtitle {
                        track: subtitle(),
                        format: SubtitleFormat::SUBTITLE_FORMAT_WEBVTT,
                        url: "/media/versions/7/subtitles/3".to_owned(),
                        font_urls: vec![],
                    },
                ],
                "{video_codec}"
            );
            assert_eq!(plan.offers.burn_in, Vec::<usize>::new(), "{video_codec}");
        }
    }

    #[test]
    fn a_pgs_track_asked_to_burn_in_on_a_device_that_draws_it_burns_nothing_in() {
        let mut facts = Facts {
            subtitle: Some(2),
            device: drawing_pgs(),
            ..facts(Some(vec![video("h264"), audio("aac", true), pgs()]))
        };

        let plan = build(&mut facts);

        assert_eq!(plan.method(), Some(Method::DirectPlay));
        assert!(
            (plan.reasons.iter())
                .all(|reason| reason.kind != Kind::PLAYBACK_REASON_KIND_SUBTITLES_BURNED_IN)
        );
        assert_eq!(plan.burn_in, None);
    }

    fn ass() -> Track {
        track("ass", false, TrackKind::Subtitle { image: false })
    }

    /// A Device that draws ASS subtitles itself.
    fn drawing_ass() -> DeviceDecoding {
        DeviceDecoding {
            subtitle_formats: vec![SubtitleFormat::SUBTITLE_FORMAT_ASS],
            ..firefox()
        }
    }

    /// [`facts`] for an H.264 and AAC MP4 with an ASS Track third, whose file carries the fonts
    /// `font_attachments`, played on `device` with the Track at `subtitle` asked to burn in.
    fn with_ass(
        font_attachments: Vec<usize>,
        device: DeviceDecoding,
        subtitle: Option<usize>,
    ) -> Facts {
        let mut facts = Facts {
            device,
            subtitle,
            ..facts(Some(vec![video("h264"), audio("aac", true), ass()]))
        };
        if let Some(probe) = &mut facts.probe {
            probe.font_attachments = font_attachments;
        }
        facts
    }

    #[test]
    fn an_ass_track_in_a_file_with_fonts_is_drawn_with_them_on_a_device_that_lists_ass() {
        let mut facts = with_ass(vec![1, 3], drawing_ass(), Some(2));

        let plan = build(&mut facts);

        assert_eq!(plan.method(), Some(Method::DirectPlay));
        assert_eq!(
            plan.subtitles,
            [Subtitle {
                track: ass(),
                format: SubtitleFormat::SUBTITLE_FORMAT_ASS,
                url: "/media/versions/7/subtitles/2/ass".to_owned(),
                font_urls: vec![
                    "/media/versions/7/fonts/0".to_owned(),
                    "/media/versions/7/fonts/1".to_owned(),
                ],
            }]
        );
        assert_eq!(plan.burn_in, None);
        assert_eq!(plan.offers.burn_in, Vec::<usize>::new());
    }

    #[test]
    fn an_ass_track_in_a_file_without_fonts_is_burned_in_on_a_device_that_lists_ass() {
        let mut facts = with_ass(vec![], drawing_ass(), Some(2));

        let plan = build(&mut facts);

        assert_eq!(plan.method(), Some(Method::Transcode));
        assert!(
            reason_of(
                &plan,
                Kind::PLAYBACK_REASON_KIND_SUBTITLES_FILE_CARRIES_NO_FONTS
            )
            .decided
        );
        assert_eq!(plan.burn_in, Some(2));
        assert_eq!(plan.offers.burn_in, vec![2]);
        assert_eq!(
            plan.subtitles[0].format,
            SubtitleFormat::SUBTITLE_FORMAT_WEBVTT
        );
        assert_eq!(plan.subtitles[0].url, "/media/versions/7/subtitles/2");
    }

    #[test]
    fn an_ass_track_in_a_file_with_fonts_keeps_both_entries_on_a_device_that_doesnt_list_ass() {
        let plan = build(&mut with_ass(vec![0], drawing_pgs(), None));

        assert_eq!(
            plan.subtitles,
            [Subtitle {
                track: ass(),
                format: SubtitleFormat::SUBTITLE_FORMAT_WEBVTT,
                url: "/media/versions/7/subtitles/2".to_owned(),
                font_urls: vec![],
            }]
        );
        assert_eq!(plan.offers.burn_in, vec![2]);
    }

    #[test]
    fn automatic_transcodes_on_the_first_hardware_found_and_the_plan_says_so() {
        let mut facts = hevc_on(HardwareChoice::Automatic, vec![QSV, VAAPI], None);

        let plan = build(&mut facts);

        let on_hardware = reason_of(&plan, Kind::PLAYBACK_REASON_KIND_TRANSCODES_ON_HARDWARE);
        assert_eq!(
            kind_and_params(on_hardware).1,
            [("hardwareAcceleration", "HARDWARE_ACCELERATION_QSV")]
        );
        assert!(!on_hardware.decided);
        assert_eq!(
            transcoded(&plan).video,
            VideoConversion {
                hardware: Some(QSV),
                tone_map: None,
                limit: None,
                burn_in: None,
                in_band_headers: true,
            }
        );
    }

    #[test]
    fn a_chosen_api_is_used_if_it_was_found_and_else_transcodes_run_in_software() {
        let cases = [
            (
                HardwareChoice::Api(Accel::Vaapi),
                vec![QSV, VAAPI],
                Some(Accel::Vaapi),
                (
                    Kind::PLAYBACK_REASON_KIND_TRANSCODES_ON_HARDWARE,
                    vec![("hardwareAcceleration", "HARDWARE_ACCELERATION_VAAPI")],
                ),
            ),
            (
                HardwareChoice::Api(Accel::Nvenc),
                vec![VAAPI],
                None,
                (
                    Kind::PLAYBACK_REASON_KIND_HARDWARE_ACCELERATION_FAILED_TEST,
                    vec![("hardwareAcceleration", "HARDWARE_ACCELERATION_NVENC")],
                ),
            ),
            (
                HardwareChoice::Off,
                vec![VAAPI],
                None,
                (Kind::PLAYBACK_REASON_KIND_HARDWARE_ACCELERATION_OFF, vec![]),
            ),
            (
                HardwareChoice::Automatic,
                vec![],
                None,
                (
                    Kind::PLAYBACK_REASON_KIND_NO_HARDWARE_ACCELERATION_PASSED,
                    vec![],
                ),
            ),
        ];
        for (choice, found, on, reason) in cases {
            let mut facts = hevc_on(choice, found, None);

            let plan = build(&mut facts);

            assert_eq!(given(&plan).last(), Some(&reason), "{choice:?}");
            let accel = transcoded(&plan)
                .video
                .hardware
                .map(|hardware| hardware.accel);
            assert_eq!(accel, on, "{choice:?}");
        }
    }

    #[test]
    fn a_subtitle_choice_that_isnt_an_image_or_ass_subtitle_burns_nothing_in() {
        let teletext = track("dvb_teletext", false, TrackKind::Subtitle { image: false });
        let tracks = vec![video("h264"), audio("aac", true), teletext, subtitle()];
        for chosen in [None, Some(1), Some(2), Some(3), Some(9)] {
            let mut facts = Facts {
                subtitle: chosen,
                ..facts(Some(tracks.clone()))
            };

            let plan = build(&mut facts);

            assert_eq!(plan.method(), Some(Method::DirectPlay), "{chosen:?}");
            assert_eq!(plan.reasons, [], "{chosen:?}");
        }
    }

    const VAAPI: Hardware = Hardware {
        accel: Accel::Vaapi,
        gpu: None,
        tone_maps: false,
    };
    const QSV: Hardware = Hardware {
        accel: Accel::Qsv,
        gpu: None,
        tone_maps: true,
    };

    /// VAAPI on a dedicated AMD Radeon RX 9070 XT, then on an integrated AMD GPU, as Automatic
    /// orders them.
    #[cfg(unix)]
    fn vaapi_on_two_gpus(root: &std::path::Path) -> Vec<Hardware> {
        use crate::ffmpeg::{fake, find_gpus};

        let (sys, dri) = (root.join("sys"), root.join("dri"));
        fake::gpu(&sys, &dri, "renderD128", "0000:18:00.0", "0x1002");
        fake::pci_function(&sys, "0000:18:00.2", "0x1022");
        fake::gpu(&sys, &dri, "renderD129", "0000:03:00.0", "0x1002");
        let mesa = "VAAPI driver: Mesa Gallium driver 26.2.6 for AMD Radeon RX 9070 XT (radeonsi)";
        find_gpus(&dri, &sys.join("class/drm"))
            .into_iter()
            .map(|gpu| Hardware {
                gpu: Some(if gpu.id() == "0000:03:00.0" {
                    gpu.with_model(mesa)
                } else {
                    gpu
                }),
                ..VAAPI
            })
            .collect()
    }

    #[cfg(unix)]
    #[test]
    fn a_stored_api_runs_on_the_first_gpu_that_passed_with_it_and_the_plan_names_it() {
        let root = tempfile::tempdir().unwrap();
        let found = vaapi_on_two_gpus(root.path());
        for choice in [HardwareChoice::Automatic, HardwareChoice::Api(Accel::Vaapi)] {
            let mut facts = hevc_on(choice, found.clone(), None);

            let plan = build(&mut facts);

            assert_eq!(
                reason_of(&plan, Kind::PLAYBACK_REASON_KIND_TRANSCODES_ON_HARDWARE).text,
                "It Transcodes on the AMD Radeon RX 9070 XT with VAAPI",
                "{choice:?}"
            );
            assert_eq!(transcoded(&plan).video.hardware.as_ref(), found.first());
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_transcode_on_an_integrated_gpu_names_it_as_integrated() {
        let root = tempfile::tempdir().unwrap();
        let integrated = vaapi_on_two_gpus(root.path()).split_off(1);
        let mut facts = hevc_on(HardwareChoice::Automatic, integrated, None);

        let plan = build(&mut facts);

        assert_eq!(
            reason_of(&plan, Kind::PLAYBACK_REASON_KIND_TRANSCODES_ON_HARDWARE).text,
            "It Transcodes on the AMD integrated GPU with VAAPI"
        );
    }

    /// HEVC 4K that Firefox can't decode, on a Server whose ffmpeg found `found` and whose admin
    /// chose `choice`.
    fn hevc_on(choice: HardwareChoice, found: Vec<Hardware>, hdr: Option<Hdr>) -> Facts {
        let mut facts = Facts {
            hardware_choice: choice,
            ..in_container(MKV, Some(vec![hdr_video("hevc", hdr), audio("aac", true)]))
        };
        facts.can_do.hardware = found;
        facts
    }

    #[test]
    fn hdr_is_tone_mapped_on_the_gpu_if_it_passed_that_test_and_else_in_software() {
        let cases = [
            (QSV, Some(ToneMapper::Hardware)),
            (VAAPI, Some(ToneMapper::Software)),
        ];
        for (hardware, tone_map) in cases {
            let mut facts = hevc_on(
                HardwareChoice::Automatic,
                vec![hardware.clone()],
                Some(Hdr::Hdr10),
            );

            let plan = build(&mut facts);

            assert_eq!(transcoded(&plan).video.tone_map, tone_map, "{hardware:?}");
            let tone_mapped = reason_of(&plan, Kind::PLAYBACK_REASON_KIND_HDR_TONE_MAPPED);
            assert_eq!(
                kind_and_params(tone_mapped).1,
                [("hdrFormat", "HDR_FORMAT_HDR10")]
            );
        }
    }

    #[test]
    fn without_zscale_hdr_is_tone_mapped_by_hardware_that_can() {
        let mut facts = hevc_on(HardwareChoice::Automatic, vec![QSV], Some(Hdr::Hdr10));
        facts.can_do.zscale = false;

        let plan = build(&mut facts);

        assert_eq!(transcoded(&plan).video.tone_map, Some(ToneMapper::Hardware));
    }

    #[test]
    fn a_hardware_transcode_of_hdr_falls_back_to_tone_mapping_in_software_with_zscale() {
        let mut facts = hevc_on(HardwareChoice::Automatic, vec![QSV], Some(Hdr::Hdr10));

        let plan = build(&mut facts);

        let on_hardware = transcoded(&plan);
        let fallback = falls_back_to(&plan).unwrap();
        assert_eq!(
            fallback.transcode,
            Transcode {
                video: VideoConversion {
                    hardware: None,
                    tone_map: Some(ToneMapper::Software),
                    ..on_hardware.video
                },
                ..on_hardware
            }
        );
        let fell_back = (
            Kind::PLAYBACK_REASON_KIND_HARDWARE_FAILED_PARTWAY,
            vec![("hardwareAcceleration", "HARDWARE_ACCELERATION_QSV")],
        );
        let mut expected = given(&plan);
        expected.retain(|(kind, _)| *kind != Kind::PLAYBACK_REASON_KIND_TRANSCODES_ON_HARDWARE);
        expected.push(fell_back);
        let reasons: Vec<Given<'_>> = fallback.reasons.iter().map(kind_and_params).collect();
        assert_eq!(reasons, expected);
    }

    #[test]
    fn without_zscale_a_hardware_transcode_of_hdr_falls_back_without_tone_mapping_and_says_so() {
        let mut facts = hevc_on(HardwareChoice::Automatic, vec![QSV], Some(Hdr::Hdr10));
        facts.can_do.zscale = false;

        let plan = build(&mut facts);

        let fallback = falls_back_to(&plan).unwrap();
        assert_eq!(fallback.transcode.video.hardware, None);
        assert_eq!(fallback.transcode.video.tone_map, None);
        let fallback_kinds: Vec<Kind> = fallback.reasons.iter().map(|reason| reason.kind).collect();
        assert_eq!(
            fallback_kinds,
            [
                Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_CONTAINER,
                Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_DECODE_VIDEO,
                Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_SHOW_HDR,
                Kind::PLAYBACK_REASON_KIND_HARDWARE_FAILED_PARTWAY,
                Kind::PLAYBACK_REASON_KIND_HDR_NOT_TONE_MAPPED,
            ],
            "on hardware: {:?}",
            kinds(&plan)
        );
    }

    #[test]
    fn a_software_transcode_has_no_fallback() {
        let mut facts = hevc_on(HardwareChoice::Off, vec![QSV], Some(Hdr::Hdr10));

        let plan = build(&mut facts);

        assert_eq!(transcoded(&plan).video.hardware, None);
        assert_eq!(falls_back_to(&plan), None);
    }

    #[test]
    fn the_transcode_cap_is_unlimited_by_default_only_with_hardware_in_use() {
        assert_eq!(
            default_max_transcodes(&hevc_on(HardwareChoice::Automatic, vec![VAAPI], None)),
            u32::MAX
        );
        assert_eq!(
            default_max_transcodes(&hevc_on(HardwareChoice::Off, vec![VAAPI], None)),
            2
        );
        assert_eq!(
            default_max_transcodes(&hevc_on(HardwareChoice::Automatic, vec![], None)),
            2
        );
    }

    /// An ffprobe in `bin` that finds an H.264 video and an AAC audio Track in any file. It's a
    /// shell script.
    #[cfg(unix)]
    fn h264_aac_ffprobe(bin: &std::path::Path) -> crate::ffmpeg::Ffmpeg {
        let script = r#"#!/bin/sh
echo '{"streams": [{"codec_name": "h264", "codec_type": "video"},
                   {"codec_name": "aac", "codec_type": "audio"}],
       "format": {"format_name": "mov,mp4,m4a,3gp,3g2,mj2"}}'
"#;
        crate::ffmpeg::Ffmpeg {
            ffprobe: Ok(crate::ffmpeg::fake::script(bin, "ffprobe", script)),
            ..crate::ffmpeg::Ffmpeg::default()
        }
    }

    fn ripley_on_the_nostromo() -> crate::users::Caller {
        use crate::users::{Caller, Device, Permissions, User};

        Caller {
            user: User {
                id: 1,
                name: "Ripley".to_owned(),
                permissions: Permissions::NEW_USER,
                created_at: 0,
            },
            device: Device {
                id: 1,
                name: "Nostromo".to_owned(),
                created_at: 0,
                last_seen_at: 0,
                authenticated_at: 0,
            },
        }
    }

    // The fake ffprobe is a shell script. The Version is stored without a scan, so no background
    // pass probes it.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_version_without_tracks_is_probed_before_its_playback_plan_is_built() {
        use std::sync::Arc;

        use crate::{
            contents::{
                Contents,
                fixture::{MatchedMovie, tmdb_movie},
            },
            db::Database,
            events::EventBus,
            ffmpeg::{Abilities, Ffmpeg},
            playback::{Asked, Planned, StartedPlan, plan, running::RunningPlans, subtitles},
            plugins::ExtensionPoints,
            probing::Probing,
            server_settings::{ServerUrl, Settings},
        };

        let data = tempfile::tempdir().unwrap();
        let media = tempfile::tempdir().unwrap();
        let root = media.path().canonicalize().unwrap().join("library");
        std::fs::create_dir(&root).unwrap();
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let alien = tmdb_movie(348, "Alien");
        let version = MatchedMovie::new(&db, &root, alien).await.version;
        let probing = Probing::new(
            Arc::clone(&db),
            Arc::new(h264_aac_ffprobe(data.path())),
            subtitles::Cache::new(data.path()),
        );
        let settings = Settings::new(Arc::clone(&db), data.path().join("transcodes"));
        let ffmpeg = Arc::new(Ffmpeg::default());
        let running = RunningPlans::new(
            settings.playback(),
            Arc::clone(&ffmpeg),
            Contents::new(Arc::clone(&db)),
            probing.clone(),
            EventBus::new(db, ExtensionPoints::default(), ServerUrl::default()),
        );

        let asked = Asked {
            version,
            decoding: DeviceDecoding::default(),
            failed: BTreeSet::new(),
            audio: None,
            subtitle: None,
            limit: PickedLimit::Unpicked,
            client: std::net::Ipv4Addr::LOCALHOST.into(),
        };
        let planned = plan(
            &probing,
            &Abilities::new(ffmpeg),
            &running,
            &ripley_on_the_nostromo(),
            asked,
            settings.playback().now(),
        )
        .await
        .unwrap();

        let tracks = match planned {
            Some(Planned::Started(StartedPlan { plan, .. }) | Planned::Unplayable(plan)) => {
                plan.tracks
            }
            None => vec![],
        };
        let codecs: Vec<&str> = tracks.iter().map(|track| track.codec.as_str()).collect();
        assert_eq!(codecs, ["h264", "aac"]);
    }

    /// A 1080p H.264 Version this Device Direct Plays, with a second audio Track and a PGS one.
    fn direct_played() -> Facts {
        let pgs = track(
            "hdmv_pgs_subtitle",
            false,
            TrackKind::Subtitle { image: true },
        );
        facts(Some(vec![
            video("h264"),
            audio("aac", true),
            audio("aac", false),
            pgs,
        ]))
    }

    fn without_ffmpeg(facts: &mut Facts) {
        facts.can_do.runs = false;
    }

    /// A Dolby Vision profile 5 picture, which this Device shows, in place of the video.
    fn in_dolby_vision_5(facts: &mut Facts) {
        let profile_5 = Hdr::DolbyVision {
            profile: 5,
            base_layer: None,
        };
        if let Some(probe) = &mut facts.probe {
            probe.tracks[0] = hdr_video("h264", Some(profile_5));
        }
        facts.device.hdr = true;
    }

    fn after_a_failed_transcode(facts: &mut Facts) {
        facts.failed = BTreeSet::from([Method::Transcode]);
    }

    fn while_busy(facts: &mut Facts) {
        facts.transcodes_full = Some(2);
    }

    /// What a test changes in the facts it plans for.
    type Change = fn(&mut Facts);

    /// Each way a Transcode is ruled out, by a lasting fact or only for now, no ffmpeg first.
    const NO_TRANSCODE: [(&str, Change); 4] = [
        ("no ffmpeg", without_ffmpeg),
        ("Dolby Vision profile 5", in_dolby_vision_5),
        ("a failed Transcode", after_a_failed_transcode),
        ("a busy Server", while_busy),
    ];

    fn picking_720p(facts: &mut Facts) {
        facts.picked_limit = PickedLimit::Picked(Some(PlaybackLimit::P720));
    }

    fn remote_at_720p(facts: &mut Facts) {
        facts.network = Network::Remote(Some(PlaybackLimit::P720));
    }

    fn burning_in_pgs(facts: &mut Facts) {
        facts.subtitle = Some(3);
    }

    #[test]
    fn a_choice_only_a_transcode_honours_is_dropped_when_it_cant_transcode_and_that_decided_it() {
        let limit_dropped = || {
            (
                Kind::PLAYBACK_REASON_KIND_PLAYBACK_LIMIT_DROPPED,
                vec![("playbackLimit", "PLAYBACK_LIMIT_720P")],
            )
        };
        let choices: [(&str, Change, Given<'_>); 3] = [
            ("720p", picking_720p, limit_dropped()),
            ("the remote default", remote_at_720p, limit_dropped()),
            (
                "PGS",
                burning_in_pgs,
                (Kind::PLAYBACK_REASON_KIND_CHOSEN_SUBTITLES_DROPPED, vec![]),
            ),
        ];
        for (chosen, choose, dropped) in choices {
            for (ruled_out, rule_out) in NO_TRANSCODE {
                let mut facts = direct_played();
                choose(&mut facts);
                rule_out(&mut facts);

                let plan = build(&mut facts);

                let case = format!("{chosen} with {ruled_out}");
                assert_eq!(plan.method(), Some(Method::DirectPlay), "{case}");
                assert_eq!((plan.limit, plan.burn_in), (None, None), "{case}");
                assert_eq!(given(&plan).last(), Some(&dropped), "{case}");
                assert!(reason_of(&plan, dropped.0).decided, "{case}");
                assert_eq!(too_busy_to_play(facts), None, "{case}");
            }
        }
    }

    #[test]
    fn a_choice_only_a_transcode_honours_is_kept_while_it_can_transcode() {
        for choose in [picking_720p, remote_at_720p, burning_in_pgs] {
            let mut facts = direct_played();
            choose(&mut facts);

            let plan = build(&mut facts);

            assert_eq!(plan.method(), Some(Method::Transcode));
            assert!(facts.dropped.is_empty());
        }
    }

    /// [`direct_played`] with its second audio Track `chosen`, and chosen.
    fn choosing_audio(chosen: &Track) -> Facts {
        let mut facts = direct_played();
        if let Some(probe) = &mut facts.probe {
            probe.tracks[2] = chosen.clone();
        }
        facts.audio = Some(2);
        facts
    }

    fn after_a_failed_remux_and_transcode(facts: &mut Facts) {
        facts.failed = BTreeSet::from([Method::Remux, Method::Transcode]);
    }

    fn after_a_failed_remux_while_busy(facts: &mut Facts) {
        facts.failed = BTreeSet::from([Method::Remux]);
        while_busy(facts);
    }

    /// Commentary audio this Device decodes, and DTS, which it can't: DTS keeps it from Direct
    /// Playing only while it's chosen.
    fn commentaries() -> [Track; 2] {
        [audio("aac", false), audio("dts", false)]
    }

    #[test]
    fn a_chosen_audio_track_is_dropped_when_neither_a_remux_nor_a_transcode_is_left() {
        let cases: [(&str, Change); 3] = [
            ("no ffmpeg", without_ffmpeg),
            (
                "a failed Remux and Transcode",
                after_a_failed_remux_and_transcode,
            ),
            (
                "a failed Remux on a busy Server",
                after_a_failed_remux_while_busy,
            ),
        ];
        for chosen in commentaries() {
            for (ruled_out, rule_out) in cases {
                let mut facts = choosing_audio(&chosen);
                rule_out(&mut facts);

                let plan = build(&mut facts);

                let case = format!("{} with {ruled_out}", chosen.codec);
                assert_eq!(plan.method(), Some(Method::DirectPlay), "{case}");
                assert_eq!(plan.audio, Some(1), "{case}");
                assert_eq!(
                    kinds(&plan).last(),
                    Some(&Kind::PLAYBACK_REASON_KIND_CHOSEN_AUDIO_DROPPED),
                    "{case}"
                );
            }
        }
    }

    #[test]
    fn a_chosen_audio_track_is_remuxed_while_only_a_transcode_is_ruled_out() {
        for chosen in commentaries() {
            for (ruled_out, rule_out) in &NO_TRANSCODE[1..] {
                let mut facts = choosing_audio(&chosen);
                rule_out(&mut facts);

                let plan = build(&mut facts);

                let case = format!("{} with {ruled_out}", chosen.codec);
                assert_eq!(plan.method(), Some(Method::Remux), "{case}");
                assert_eq!(plan.audio, Some(2), "{case}");
            }
        }
    }

    fn without_hls(facts: &mut Facts) {
        facts.device.hls = false;
    }

    fn choosing_the_second_audio_track(facts: &mut Facts) {
        facts.audio = Some(2);
    }

    #[test]
    fn a_device_without_hls_that_decodes_everything_direct_plays_or_nothing_whatever_is_chosen() {
        let choices: [(&str, Change); 2] = [
            ("the second audio Track", choosing_the_second_audio_track),
            ("720p", picking_720p),
        ];
        for (chosen, choose) in choices {
            let chosen_without_hls = || {
                let mut facts = direct_played();
                choose(&mut facts);
                without_hls(&mut facts);
                facts
            };
            let mut facts = chosen_without_hls();
            let mut after_a_failed_direct_play = chosen_without_hls();
            after_a_failed_direct_play.failed = BTreeSet::from([Method::DirectPlay]);

            let played = build(&mut facts);
            let unplayable = build(&mut after_a_failed_direct_play);

            assert_eq!(played.method(), Some(Method::DirectPlay), "{chosen}");
            assert_eq!(unplayable.method(), None, "{chosen}");
            for plan in [played, unplayable] {
                let no_hls = reason_of(&plan, Kind::PLAYBACK_REASON_KIND_DEVICE_CANT_PLAY_HLS);
                assert_eq!(no_hls.decided, plan.method().is_none(), "{chosen}");
            }
        }
    }

    #[test]
    fn a_plan_offers_the_choices_its_lasting_facts_allow_whatever_is_busy_or_failed() {
        let everything = || Offers {
            audio: vec![1, 2],
            burn_in: vec![3],
            limits: vec![PlaybackLimit::P720, PlaybackLimit::P480],
        };
        let cases: [(&str, Change, Offers); 5] = [
            (
                "no ffmpeg",
                without_ffmpeg,
                Offers {
                    audio: vec![1],
                    ..Offers::default()
                },
            ),
            (
                "no HLS",
                without_hls,
                Offers {
                    audio: vec![1],
                    ..Offers::default()
                },
            ),
            (
                "Dolby Vision profile 5",
                in_dolby_vision_5,
                Offers {
                    audio: vec![1, 2],
                    ..Offers::default()
                },
            ),
            ("a failed Transcode", after_a_failed_transcode, everything()),
            ("a busy Server", while_busy, everything()),
        ];
        for (ruled_out, rule_out, offers) in cases {
            let mut facts = direct_played();
            rule_out(&mut facts);

            assert_eq!(build(&mut facts).offers, offers, "{ruled_out}");
        }
    }

    #[test]
    fn a_plan_offers_no_playback_limit_its_picture_already_fits() {
        let mut facts = facts(Some(vec![
            track(
                "h264",
                false,
                TrackKind::Video {
                    width: Some(1280),
                    height: Some(720),
                    frame_rate: None,
                    bit_depth: None,
                    hdr: None,
                },
            ),
            audio("aac", true),
        ]));

        assert_eq!(build(&mut facts).offers.limits, [PlaybackLimit::P480]);
    }
}
