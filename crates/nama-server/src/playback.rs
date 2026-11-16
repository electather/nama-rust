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

mod choices;
mod codecs;
mod direct_play;
mod limits;
mod probing;
mod running;
mod steps;
mod subtitles;
mod track_preferences;
mod tracks;

pub(crate) use direct_play::router;
pub(crate) use probing::{Error as ProbingError, Probing};
pub(crate) use running::{RunningPlans, StartError, router as running_router, sweep_leftovers};
pub(crate) use subtitles::Cache as SubtitleCache;
pub(crate) use track_preferences::{
    Description, Error as TrackPreferenceError, Preferred, SubtitlePreference, TrackPreferences,
    is_language,
};
pub(crate) use tracks::{Probe, Track, TrackKind, Tracks, track_to_proto};

use std::{collections::BTreeSet, net::IpAddr, num::NonZeroU32};

use nama_proto::nama::v1 as proto;
use proto::PlaybackReasonKind as Kind;

use crate::{
    ffmpeg::{Abilities, Accel, CanDo, Remux, Transcode},
    server_settings::{HardwareChoice, PlaybackLimit, PlaybackValues},
    users::Caller,
};
use probing::ProbeError;

use choices::{
    audio_asked, burns_in, chosen_audio, chosen_subtitle, preferred_subtitles, shown_subtitles,
};
use codecs::{file_audio, is_audio, played_audio_place, played_by, played_tracks};
use direct_play::direct_play_url;
use limits::{fitted, limit_in_effect, network, on_local_network, picture, playback_limit};
use steps::{
    converting, default_max_transcodes, device_decoding, failed_on_device, fallback, remux,
    server_busy, server_ffmpeg, too_busy_to_play, transcode, version_length, version_tracks,
};

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
    /// The place in the Version's Tracks of the subtitle Track playback starts with, if any: the
    /// one it burns in, or else a text one the Server chose for the Device to show.
    pub(crate) starting_subtitle: Option<usize>,
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
    /// Its Track's place in the Version's Tracks.
    pub(crate) place: usize,
    pub(crate) format: proto::SubtitleFormat,
    /// Where it's served in `format`, on this Server.
    pub(crate) url: String,
    /// Where the fonts its file carries are served, for one drawn as ASS; empty for any other.
    pub(crate) font_urls: Vec<String>,
}

/// What a Device says it can decode, as it found out just now. The Server never stores it.
#[derive(Debug, Default, Clone)]
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
    pub(crate) audio: AskedAudio,
    pub(crate) subtitle: AskedSubtitle,
    pub(crate) limit: PickedLimit,
    /// The Device's address, which says whether it's on the local network.
    pub(crate) client: IpAddr,
}

/// What a request for a plan asks of audio.
#[derive(Debug)]
pub(crate) enum AskedAudio {
    /// The audio Track chosen, by its place in the Version's Tracks.
    Chosen(usize),
    /// The Server's choice, by the User's Audio Preference for what the Version is of, or else
    /// their own, if they have one, or else the file's own (ADR 0035).
    Preferred(Option<Preferred<Description>>),
}

/// What a request for a plan asks of subtitles.
#[derive(Debug)]
pub(crate) enum AskedSubtitle {
    /// The subtitle Track chosen to burn into the picture, by its place in the Version's Tracks.
    BurnIn(usize),
    /// None burned in, and none for the Server to choose.
    None,
    /// The Server's choice, by the User's Subtitle Preference for what the Version is of, or else
    /// their own, if they have one (ADR 0035).
    Preferred(Option<Preferred<SubtitlePreference>>),
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
    let audio = audio_asked(audio, probe.as_ref(), &decoding);
    let (subtitle, subtitle_preference) = match subtitle {
        AskedSubtitle::BurnIn(place) => (Some(place), None),
        AskedSubtitle::None => (None, None),
        AskedSubtitle::Preferred(preferred) => (None, preferred),
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
        shown_subtitle: None,
        subtitle_preference,
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
#[derive(Clone)]
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
    /// The audio Track the User chose, or the Server chose by their Audio Preference, by its place
    /// in the Version's Tracks, if one was.
    audio: Option<usize>,
    /// The most Transcodes the Server may run at once, if other Devices already run that many.
    transcodes_full: Option<u32>,
    picked_limit: PickedLimit,
    network: Network,
    /// The subtitle Track the User chose to burn into the picture, or the Server chose by their
    /// Subtitle Preference, by its place in the Version's Tracks, if one was.
    subtitle: Option<usize>,
    /// The text subtitle Track the Server chose by the User's Subtitle Preference for the Device
    /// to show itself, by its place in the Version's Tracks, if it chose one.
    shown_subtitle: Option<usize>,
    /// The User's Subtitle Preference the Server chooses `subtitle` or `shown_subtitle` by, which
    /// [`build`] matches once it knows the audio the plan plays.
    subtitle_preference: Option<Preferred<SubtitlePreference>>,
    /// What the admin chose for Transcodes to run on.
    hardware_choice: HardwareChoice,
    /// The User's choices the plan doesn't honour, as [`build`] found.
    dropped: BTreeSet<Choice>,
}

/// A step gives the reasons it found, and the plan may be what none of them rules out.
type Step = fn(&Facts, Option<&Candidates>) -> Vec<Reason>;

/// What a Remux and a Transcode of a probed Version would each convert, worked out before the
/// steps, which read them, so a plan's reasons and what it plays can't disagree (the area's
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

/// The steps for the User's choices, each dropped where it leaves no method and the plan plays
/// without it.
const CHOICE_STEPS: [(Choice, Step); 4] = [
    (Choice::Audio, chosen_audio),
    (Choice::Limit, network),
    (Choice::Limit, playback_limit),
    (Choice::Subtitle, chosen_subtitle),
];

/// The plan for `facts` that every step allows. A choice that would leave no method is added to
/// `facts.dropped` and the steps run again without it, since what the earlier steps see, such as
/// the audio the Device must decode, may change with it. A choice is kept if the plan doesn't play
/// without it either, so an unplayable plan says what was asked.
fn build(facts: &mut Facts) -> Plan {
    // Matched here, not before, since a dropped audio choice changes the audio it's matched over.
    if let Some(preferred) = &facts.subtitle_preference {
        (facts.subtitle, facts.shown_subtitle) = preferred_subtitles(facts, preferred);
    }
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
        if allowed.is_disjoint(&allows)
            && allows != BTreeSet::from(Method::ALL)
            && let Some(plan) = plan_without(choice, facts)
        {
            return plan;
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
    // A burn-in still in effect rules out all but a Transcode, so it plays by one or not at all.
    let burned_in = facts.subtitle.filter(|_| {
        candidates.is_some_and(|candidates| candidates.transcode.video.burn_in.is_some())
    });
    let tracks = (facts.probe.as_ref())
        .map_or_default(|probe| played_by(playing.as_ref(), played_tracks(probe, audio)));
    Plan {
        version: facts.version,
        playing,
        tracks,
        reasons,
        subtitles: facts
            .probe
            .as_ref()
            .map_or_default(|probe| shown_subtitles(facts.version, probe, &facts.device)),
        limit: limit_in_effect(facts),
        audio,
        burn_in: burned_in,
        starting_subtitle: burned_in.or(facts.shown_subtitle),
        offers: offers(facts, &lasting),
    }
}

/// The plan for `facts` with `choice` dropped, if it plays, and only then drops it from `facts`.
fn plan_without(choice: Choice, facts: &mut Facts) -> Option<Plan> {
    let mut without = facts.clone();
    without.dropped.insert(choice);
    let plan = build(&mut without);
    plan.playing.is_some().then(|| {
        *facts = without;
        plan
    })
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

#[cfg(test)]
#[path = "playback/playback_tests.rs"]
mod tests;
