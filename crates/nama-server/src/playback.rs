//! Playback Plans: how the Server plays one Version on one Device, and why (ADR 0027).
//!
//! [`plan`] gathers what a plan is based on, probing the Version first if its Tracks aren't
//! current, and runs the steps over it in order. Each step says which methods it allows and why,
//! and the plan keeps only what every step allows, so no step can widen what an earlier one ruled
//! out. The steps for the User's choices run last, and a choice no method is left for is dropped
//! instead (ADR 0031).
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

use std::{collections::BTreeSet, net::IpAddr, num::NonZeroU32, sync::Arc};

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
    ffmpeg::{
        Abilities, BurnIn, CanDo, Ffmpeg, Hardware, Remux, RemuxAudio, ToneMapper, Transcode,
        TranscodeAudio, VideoConversion, VideoLimit,
    },
    probing::{self, ProbeError, Probing},
    server_settings::{HardwareChoice, PlaybackValues},
    track_preferences::{SubtitlePreference, same_language},
    tracks::{Hdr, Probe, Track, TrackKind, Tracks},
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
    /// Why, one for each step, in the order they ran.
    pub(crate) reasons: Vec<String>,
    /// The text subtitles a player can turn on without a new plan.
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
    pub(crate) const fn method(&self) -> Option<Method> {
        match self.playing {
            None => None,
            Some(Playing::DirectPlay) => Some(Method::DirectPlay),
            Some(Playing::Remux { .. }) => Some(Method::Remux),
            Some(Playing::Transcode { .. }) => Some(Method::Transcode),
        }
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

/// What a Transcode on hardware goes on with in software once the hardware fails partway.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Fallback {
    pub(crate) transcode: Transcode,
    /// Why, as its plan's reasons then say.
    pub(crate) why: String,
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

/// A text subtitle a player shows as a WebVTT `<track>`.
#[derive(Debug, PartialEq)]
pub(crate) struct Subtitle {
    pub(crate) track: Track,
    /// Its Track's place in the Version's Tracks.
    pub(crate) place: usize,
    /// Where it's served as WebVTT, on this Server.
    pub(crate) url: String,
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
    pub(crate) subtitle: AskedSubtitle,
    pub(crate) limit: PickedLimit,
    /// The Device's address, which says whether it's on the local network.
    pub(crate) client: IpAddr,
}

/// What a request for a plan asks of subtitles.
#[derive(Debug)]
pub(crate) enum AskedSubtitle {
    /// The subtitle Track chosen to burn into the picture, by its place in the Version's Tracks.
    BurnIn(usize),
    /// None burned in, and none for the Server to choose.
    None,
    /// The Server's choice, by the User's Subtitle Preference for what the Version is of, if they
    /// have one (ADR 0035).
    Preferred(Option<SubtitlePreference>),
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
    let (subtitle, shown_subtitle) = subtitles_asked(subtitle, probe.as_ref());
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
        shown_subtitle,
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

/// The subtitle Track `asked` asks to burn in and the one it asks the Device to show, each by its
/// place in `probe`'s Tracks: a Track the Server chose by the User's Subtitle Preference is shown
/// by the Device if it can, since that is no input to the plan (ADR 0033), and burned in if not.
fn subtitles_asked(asked: AskedSubtitle, probe: Option<&Probe>) -> (Option<usize>, Option<usize>) {
    match asked {
        AskedSubtitle::BurnIn(place) => (Some(place), None),
        AskedSubtitle::None | AskedSubtitle::Preferred(None) => (None, None),
        AskedSubtitle::Preferred(Some(preference)) => {
            let tracks = probe.map_or(&[][..], |probe| &probe.tracks);
            let place = preferred_subtitle(&preference, tracks);
            match place.and_then(|place| tracks.get(place)) {
                Some(track) if track.is_text_subtitle() => (None, place),
                _ => (place, None),
            }
        }
    }
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
    /// The subtitle Track the User chose to burn into the picture, or the Server chose by their
    /// Subtitle Preference, by its place in the Version's Tracks, if one was.
    subtitle: Option<usize>,
    /// The text subtitle Track the Server chose by the User's Subtitle Preference for the Device
    /// to show itself, by its place in the Version's Tracks, if it chose one.
    shown_subtitle: Option<usize>,
    /// What the admin chose for Transcodes to run on.
    hardware_choice: HardwareChoice,
    /// The User's choices the plan doesn't honour, as [`build`] found.
    dropped: BTreeSet<Choice>,
}

/// What one step lets a plan be, and why.
struct Narrowing {
    allows: BTreeSet<Method>,
    reason: String,
}

type Step = fn(&Facts, Option<&Candidates>) -> Narrowing;

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

/// What tone-maps HDR to SDR in the Transcode among `candidates`, if anything does.
fn candidate_tone_mapper(candidates: Option<&Candidates>) -> Option<ToneMapper> {
    candidates?.transcode.video.tone_map
}

// The steps run in ADR 0027's order, split by ADR 0031. A new input is a new step in one of these.
// The Server's encoders aren't one: they rule nothing out, so `build` gives their reason between
// the lasting and the now steps, and only while a Transcode is allowed (see `LASTING_STEPS`).

/// The steps for what the Version, Device and Server allow that lasts, so what they allow is what
/// the User's choices can ever have. [`server_encoders`] speaks right after them, and only while
/// they allow a Transcode, since saying where one runs would contradict a step that ruled it out.
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
    if lasting.contains(&Method::Transcode) {
        reasons.push(server_encoders(facts, candidates));
    }
    let mut allowed = narrowed(facts, candidates, &NOW_STEPS, lasting.clone(), &mut reasons);
    for (choice, step) in CHOICE_STEPS {
        let Narrowing { allows, reason } = step(facts, candidates);
        // A step allowing every method has no choice in effect to drop.
        if allowed.is_disjoint(&allows) && allows != BTreeSet::from(Method::ALL) {
            facts.dropped.insert(choice);
            return build(facts);
        }
        allowed.retain(|method| allows.contains(method));
        reasons.push(reason);
    }
    // Never `None` once a Remux or Transcode is allowed: `version_length` rules both out first and
    // says why.
    let duration_ms = facts.probe.as_ref().and_then(|probe| probe.duration_ms);
    let playing = allowed.first().and_then(|method| match method {
        Method::DirectPlay => Some(Playing::DirectPlay),
        Method::Remux => Some(Playing::Remux {
            remux: candidates?.remux,
            duration_ms: duration_ms?,
        }),
        Method::Transcode => Some(Playing::Transcode {
            transcode: candidates?.transcode,
            duration_ms: duration_ms?,
            fallback: fallback(facts, candidates?.transcode),
        }),
    });
    let audio = candidates.and_then(|candidates| candidates.audio);
    let burned_in = facts.subtitle.filter(|_| {
        matches!(
            &playing,
            Some(Playing::Transcode { transcode, .. }) if transcode.video.burn_in.is_some()
        )
    });
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
            .map(|probe| text_subtitles(facts.version, probe))
            .unwrap_or_default(),
        limit: limit_in_effect(facts),
        audio,
        burn_in: burned_in,
        starting_subtitle: burned_in.or(facts.shown_subtitle),
        offers: offers(facts, &lasting),
    }
}

/// What `allowed` leaves once each of `steps` narrows it, saying why in `reasons`.
fn narrowed(
    facts: &Facts,
    candidates: Option<&Candidates>,
    steps: &[Step],
    mut allowed: BTreeSet<Method>,
    reasons: &mut Vec<String>,
) -> BTreeSet<Method> {
    for step in steps {
        let Narrowing { allows, reason } = step(facts, candidates);
        allowed.retain(|method| allows.contains(method));
        reasons.push(reason);
    }
    allowed
}

/// What a plan for `facts` can ever honour, with only the methods in `lasting` allowed.
fn offers(facts: &Facts, lasting: &BTreeSet<Method>) -> Offers {
    let Some(probe) = &facts.probe else {
        return Offers::default();
    };
    let transcodes = lasting.contains(&Method::Transcode);
    let places = |keep: fn(&Track) -> bool| {
        (probe.tracks.iter().enumerate())
            .filter(move |(_, track)| keep(track))
            .map(|(place, _)| place)
    };
    // Only the file's own audio plays without a Remux or a Transcode.
    let audio = if transcodes || lasting.contains(&Method::Remux) {
        places(is_audio).collect()
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
        burn_in: places(burns_in).collect(),
        limits: (PlaybackLimit::ALL.into_iter())
            .filter(|&limit| picture.and_then(|picture| fitted(picture, limit)).is_some())
            .collect(),
    }
}

/// The step for the Version's own Tracks. None needs changing to play yet, so once they're known
/// it allows every method, and before then only Direct Play, since a Remux or a Transcode needs
/// them.
fn version_tracks(facts: &Facts, _: Option<&Candidates>) -> Narrowing {
    let (allows, reason) = if facts.probe.is_some() {
        (BTreeSet::from(Method::ALL), "Its Tracks need no change")
    } else {
        (
            BTreeSet::from([Method::DirectPlay]),
            "Its Tracks aren't known, so they're taken to need no change, and it can only be \
             Direct Played",
        )
    };
    Narrowing {
        allows,
        reason: reason.to_owned(),
    }
}

/// The step for the Version's length: without it only Direct Play, since the HLS playlist of a
/// Remux or a Transcode lists how long each segment is.
fn version_length(facts: &Facts, _: Option<&Candidates>) -> Narrowing {
    let known = facts
        .probe
        .as_ref()
        .is_some_and(|probe| probe.duration_ms.is_some());
    let (allows, reason) = if known {
        (BTreeSet::from(Method::ALL), "Its length is known")
    } else {
        (
            BTreeSet::from([Method::DirectPlay]),
            "Its length isn't known, which a Remux's or a Transcode's playlist needs, so it can \
             only be Direct Played",
        )
    };
    Narrowing {
        allows,
        reason: reason.to_owned(),
    }
}

/// The step for what the Device decodes: Direct Play only if it decodes the Version's container
/// and the Tracks the plan plays, and its display shows their HDR, a Remux only if it decodes the
/// video, since a Remux converts only the audio, and neither a Remux nor a Transcode unless it
/// plays HLS.
fn device_decoding(facts: &Facts, candidates: Option<&Candidates>) -> Narrowing {
    let (Some(probe), Some(candidates)) = (&facts.probe, candidates) else {
        // `version_tracks` already leaves only Direct Play, so this step needn't rule anything out.
        return Narrowing {
            allows: BTreeSet::from(Method::ALL),
            reason: "Its Tracks aren't known, so this Device is taken to decode them".to_owned(),
        };
    };
    let cant: Vec<String> = undecodable_container(probe, &facts.device)
        .into_iter()
        .chain(undecodable_tracks(probe, candidates.audio, &facts.device))
        .collect();
    let Some(cant) = joined(&cant, "or") else {
        let reason = "This Device decodes its container and Tracks";
        return if facts.device.hls {
            Narrowing {
                allows: BTreeSet::from(Method::ALL),
                reason: reason.to_owned(),
            }
        } else {
            Narrowing {
                allows: BTreeSet::from([Method::DirectPlay]),
                reason: format!("{reason}, but it can't play HLS, which anything else needs"),
            }
        };
    };
    let reason = format!("This Device can't decode its {cant}, so it can't be Direct Played");
    if !facts.device.hls {
        return Narrowing {
            allows: BTreeSet::new(),
            reason: format!("{reason}, and it can't play HLS, which anything else needs"),
        };
    }
    let played_channels = (candidates.audio)
        .and_then(|place| probe.tracks.get(place))
        .and_then(channels_of);
    let decodes_video =
        played_video(probe).is_none_or(|video| undecodable(video, &facts.device).is_empty());
    if !decodes_video {
        return Narrowing {
            allows: BTreeSet::from([Method::Transcode]),
            reason: format!(
                "{reason} or Remuxed, and a Transcode converts it to {}",
                transcode_in_words(&facts.device, played_channels, &candidates.transcode)
            ),
        };
    }
    let reason = match candidates.remux.audio.and_then(|audio| audio.aac_channels) {
        Some(channels) => format!(
            "{reason}, and a Remux converts that audio to {}",
            aac_in_words(&facts.device, played_channels, channels, Method::Remux)
        ),
        None => reason,
    };
    Narrowing {
        allows: BTreeSet::from([Method::Remux, Method::Transcode]),
        reason,
    }
}

/// The step for the Server's ffmpeg: without it, only Direct Play (ADR 0026); without libx264, no
/// Transcode; and otherwise a Transcode tone-maps HDR the Device can't show to SDR, if it can.
fn server_ffmpeg(facts: &Facts, candidates: Option<&Candidates>) -> Narrowing {
    if !facts.can_do.runs {
        return Narrowing {
            allows: BTreeSet::from([Method::DirectPlay]),
            reason: "This Server lacks ffmpeg or ffprobe, so it can only Direct Play".to_owned(),
        };
    }
    if !facts.can_do.transcodes() {
        return Narrowing {
            allows: BTreeSet::from([Method::DirectPlay, Method::Remux]),
            reason: "This Server's ffmpeg lacks libx264, so it can't Transcode".to_owned(),
        };
    }
    let reason = match played_hdr(facts) {
        // Its picture is in Dolby Vision's own colours, which only a Dolby Vision player knows.
        Some(Hdr::DolbyVision {
            base_layer: None, ..
        }) => {
            return Narrowing {
                allows: BTreeSet::from([Method::DirectPlay, Method::Remux]),
                reason: "This Server has ffmpeg, but it can't Transcode a Dolby Vision picture \
                         with no Base Layer, as in profile 5"
                    .to_owned(),
            };
        }
        Some(hdr) if candidate_tone_mapper(candidates).is_some() => format!(
            "This Server has ffmpeg, which tone-maps its {} picture to SDR in a Transcode",
            hdr_name(hdr)
        ),
        Some(hdr) if !facts.device.hdr && transcoded_hdr(hdr).is_some() => format!(
            "This Server's ffmpeg lacks zscale, so a Transcode can't tone-map its {} picture to \
             SDR and its colours look washed out",
            hdr_name(hdr)
        ),
        _ => "This Server has ffmpeg".to_owned(),
    };
    Narrowing {
        allows: BTreeSet::from(Method::ALL),
        reason,
    }
}

/// What `transcode` goes on with in software if it runs on hardware: the same conversion,
/// tone-mapping only if ffmpeg has zscale.
fn fallback(facts: &Facts, transcode: Transcode) -> Option<Fallback> {
    let accel = transcode.video.hardware?;
    let tone_map = (transcode.video.tone_map).and_then(|_| facts.can_do.tone_mapper(None));
    let without_tone_mapping = if tone_map.is_none() && transcode.video.tone_map.is_some() {
        ", without tone-mapping its HDR, as this Server's ffmpeg lacks zscale"
    } else {
        ""
    };
    Some(Fallback {
        transcode: Transcode {
            video: VideoConversion {
                hardware: None,
                tone_map,
                ..transcode.video
            },
            ..transcode
        },
        why: format!(
            "{} failed partway, so the rest of it Transcodes in software{without_tone_mapping}",
            accel.name()
        ),
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
    facts.can_do.tone_mapper(gpu)
}

/// The reason for the Server's encoders: what a Transcode decodes, scales, tone-maps and encodes
/// on, which the admin chooses. It rules nothing out, so it isn't a [`Step`].
// ponytail: Dolby Vision profile 5 stays out of reach: none of these APIs' ffmpeg filters reshape
// its colours, which only libplacebo does.
fn server_encoders(facts: &Facts, candidates: Option<&Candidates>) -> String {
    match transcodes_on(facts) {
        Err(why) => format!("{why}, so a Transcode runs in software"),
        Ok(Hardware { accel, .. }) => {
            let on = format!("A Transcode runs on the GPU with {}", accel.name());
            match (candidate_tone_mapper(candidates), played_hdr(facts)) {
                (Some(ToneMapper::Hardware), Some(hdr)) => {
                    format!("{on}, which tone-maps its {} picture to SDR", hdr_name(hdr))
                }
                (Some(ToneMapper::Software), Some(hdr)) => format!(
                    "{on}, and tone-maps its {} picture to SDR in software",
                    hdr_name(hdr)
                ),
                _ => on,
            }
        }
    }
}

/// The hardware a Transcode runs on, as the admin chose, or why it runs in software.
fn transcodes_on(facts: &Facts) -> Result<Hardware, String> {
    let found = |accel| (facts.can_do.hardware.iter()).find(|found| found.accel == accel);
    match facts.hardware_choice {
        HardwareChoice::Off => Err("Hardware acceleration is off".to_owned()),
        HardwareChoice::Automatic => facts.can_do.automatic().ok_or_else(|| {
            "No hardware acceleration passed its test when this Server started".to_owned()
        }),
        HardwareChoice::Api(accel) => found(accel).copied().ok_or_else(|| {
            format!(
                "{} didn't pass its test when this Server started",
                accel.name()
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
    let hardware = transcodes_on(facts).ok().map(|hardware| hardware.accel);
    VideoConversion {
        hardware,
        tone_map: tone_mapper(facts),
        limit: video_limit(facts),
        burn_in: burn_in(facts),
        // Hardware may fail, and software carry on after its init segment.
        in_band_headers: hardware.is_some(),
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
fn chosen_audio(facts: &Facts, candidates: Option<&Candidates>) -> Narrowing {
    if facts.dropped.contains(&Choice::Audio) {
        return Narrowing {
            allows: BTreeSet::from(Method::ALL),
            reason: "The audio Track chosen needs a Remux or a Transcode, and neither can play it \
                     here, so it's dropped for the file's own"
                .to_owned(),
        };
    }
    match (&facts.probe, candidates) {
        (Some(probe), Some(candidates)) if candidates.audio != file_audio(probe) => Narrowing {
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

/// The step for how busy the Server is: no Transcode while other Devices run as many as it may.
fn server_busy(facts: &Facts, _: Option<&Candidates>) -> Narrowing {
    match facts.transcodes_full {
        None => Narrowing {
            allows: BTreeSet::from(Method::ALL),
            reason: "This Server isn't running as many Transcodes as it may at once".to_owned(),
        },
        Some(max) => Narrowing {
            allows: BTreeSet::from([Method::DirectPlay, Method::Remux]),
            reason: format!(
                "This Server is busy, running as many Transcodes as it may at once ({max}), so \
                 it can't Transcode it"
            ),
        },
    }
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
fn failed_on_device(facts: &Facts, _: Option<&Candidates>) -> Narrowing {
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

/// The step for where the Device is: it allows every method, and says whether the default
/// Playback Limit for remote playback chose the limit the next step keeps to.
fn network(facts: &Facts, _: Option<&Candidates>) -> Narrowing {
    let reason = match (facts.network, facts.picked_limit) {
        (Network::Local, _) => "This Device is on the local network".to_owned(),
        (Network::Remote(default), PickedLimit::Unpicked) => format!(
            "This Device is outside the local network, so the default Playback Limit for remote \
             playback, {}, applies",
            default.map_or("Original", PlaybackLimit::name)
        ),
        (Network::Remote(_), PickedLimit::Picked(_)) => {
            "This Device is outside the local network, but its User picked a Playback Limit in \
             the player, which applies instead of the default for remote playback"
                .to_owned()
        }
    };
    Narrowing {
        allows: BTreeSet::from(Method::ALL),
        reason,
    }
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
fn playback_limit(facts: &Facts, candidates: Option<&Candidates>) -> Narrowing {
    if let Some(PlaybackLimit { name, .. }) = limit_asked(facts)
        && facts.dropped.contains(&Choice::Limit)
    {
        return Narrowing {
            allows: BTreeSet::from(Method::ALL),
            reason: format!(
                "The {name} Playback Limit needs a Transcode, which can't play it here, so it's \
                 dropped for Original"
            ),
        };
    }
    let Some(PlaybackLimit { name, .. }) = limit_in_effect(facts) else {
        return Narrowing {
            allows: BTreeSet::from(Method::ALL),
            reason: "The Playback Limit is Original".to_owned(),
        };
    };
    let scaled = candidates.and_then(|candidates| candidates.transcode.video.limit?.size);
    if let Some((width, height)) = scaled {
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

/// The step for the subtitle Track the User chose to burn in: only a Transcode draws it into the
/// picture.
fn chosen_subtitle(facts: &Facts, candidates: Option<&Candidates>) -> Narrowing {
    if facts.dropped.contains(&Choice::Subtitle) {
        return Narrowing {
            allows: BTreeSet::from(Method::ALL),
            reason: "The subtitles chosen need a Transcode to burn them in, which can't play it \
                     here, so they're dropped"
                .to_owned(),
        };
    }
    if candidates.is_some_and(|candidates| candidates.transcode.video.burn_in.is_some()) {
        return Narrowing {
            allows: BTreeSet::from([Method::Transcode]),
            reason: "The subtitles chosen are burned into its picture, which only a Transcode does"
                .to_owned(),
        };
    }
    Narrowing {
        allows: BTreeSet::from(Method::ALL),
        reason: "No subtitles are burned into its picture".to_owned(),
    }
}

/// How a Transcode burns in the subtitle Track the User chose, unless the plan dropped it, if it's
/// one that [`burns_in`]. Any other choice, as when the Version changed since the User saw it, is
/// the same as none.
fn burn_in(facts: &Facts) -> Option<BurnIn> {
    let probe = facts.probe.as_ref()?;
    let place = facts
        .subtitle
        .filter(|_| !facts.dropped.contains(&Choice::Subtitle))?;
    let track = probe.tracks.get(place).filter(|track| burns_in(track))?;
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

/// The place in `tracks` of the subtitle Track `preference` starts playback with, `None` for off or
/// when none matches. A Track in its language and as forced matches; among those, the same SDH
/// wins, then the same title, then one the Device shows itself over one that needs Burn-in, then
/// the first.
fn preferred_subtitle(preference: &SubtitlePreference, tracks: &[Track]) -> Option<usize> {
    let SubtitlePreference::Like(wanted) = preference else {
        return None;
    };
    let title = |title: Option<&str>| title.map(str::to_lowercase);
    (tracks.iter().enumerate())
        .filter(|(_, track)| track.is_text_subtitle() || burns_in(track))
        .filter(|(_, track)| {
            track.forced == wanted.forced
                && same_language(track.language.as_deref(), wanted.language.as_deref())
        })
        // `min_by_key` keeps the first of equals.
        .min_by_key(|(_, track)| {
            (
                track.sdh != wanted.sdh,
                title(track.title.as_deref()) != title(wanted.title.as_deref()),
                !track.is_text_subtitle(),
            )
        })
        .map(|(place, _)| place)
}

/// Whether `track` is a subtitle a browser can't show as it should, so a Transcode burns it in:
/// an image one, such as PGS, or ASS or SSA, with its styling.
fn burns_in(track: &Track) -> bool {
    match track.kind {
        TrackKind::Subtitle { image } => image || track.is_ass_subtitle(),
        TrackKind::Video { .. } | TrackKind::Audio { .. } => false,
    }
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

/// `probe`'s container in plain words, such as "Matroska container", if `device` can't decode it.
fn undecodable_container(probe: &Probe, device: &DeviceDecoding) -> Option<String> {
    match container(probe) {
        Some((container, _)) if device.containers.contains(&container) => None,
        Some((_, name)) => Some(format!("{name} container")),
        None => Some(format!("{} container", probe.container)),
    }
}

/// What of the Tracks of `probe` a plan playing the audio Track at `audio` plays `device` can't
/// decode, in plain words, such as "HEVC video".
fn undecodable_tracks(probe: &Probe, audio: Option<usize>, device: &DeviceDecoding) -> Vec<String> {
    (played_tracks(probe, audio).iter())
        .flat_map(|track| undecodable(track, device))
        .collect()
}

/// What of `track` `device` can't decode, in plain words, such as "HEVC video" and "HDR10 picture".
fn undecodable(track: &Track, device: &DeviceDecoding) -> Vec<String> {
    let mut cant = vec![];
    let (codec, hdr) = match track.kind {
        TrackKind::Video { hdr, .. } => {
            let codec = undecodable_codec(&track.codec, &VIDEO_CODECS, &device.video_codecs);
            (codec.map(|name| format!("{name} video")), hdr)
        }
        TrackKind::Audio { .. } => {
            let codec = undecodable_codec(&track.codec, &AUDIO_CODECS, &device.audio_codecs);
            (codec.map(|name| format!("{name} audio")), None)
        }
        TrackKind::Subtitle { .. } => (Some(format!("{} subtitle", track.codec)), None),
    };
    cant.extend(codec);
    if let Some(hdr) = hdr.filter(|_| !device.hdr) {
        cant.push(format!("{} picture", hdr_name(hdr)));
    }
    cant
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

/// What `transcode` of the Version makes for `device`, in plain words, such as "H.264 and stereo
/// AAC", from audio with `played_channels`.
fn transcode_in_words(
    device: &DeviceDecoding,
    played_channels: Option<i64>,
    transcode: &Transcode,
) -> String {
    let audio = transcode.audio.map(|audio| {
        let aac = aac_in_words(
            device,
            played_channels,
            audio.aac_channels,
            Method::Transcode,
        );
        format!(" and {aac}")
    });
    format!("H.264{}", audio.unwrap_or_default())
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

/// AAC with `channels` channels in plain words, such as "5.1 AAC", saying why when that's fewer
/// than the `played_channels` the audio has, which `method` converts it to for `device`.
fn aac_in_words(
    device: &DeviceDecoding,
    played_channels: Option<i64>,
    channels: i64,
    method: Method,
) -> String {
    let name = match channels {
        1 => "mono".to_owned(),
        2 => "stereo".to_owned(),
        6 => "5.1".to_owned(),
        n => format!("{n}-channel"),
    };
    let why = if channels >= played_channels.unwrap_or_default() {
        ""
    } else if device.audio_channels >= 6 {
        &format!(", the most a {} makes", method.name())
    } else if device.audio_channels == 0 {
        ", as this Device doesn't say it plays more"
    } else {
        ", as this Device doesn't play 5.1"
    };
    format!("{name} AAC{why}")
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

/// `items` as "a, b <conjunction> c", or `None` if there are none.
fn joined(items: &[String], conjunction: &str) -> Option<String> {
    let (last, rest) = items.split_last()?;
    Some(if rest.is_empty() {
        last.clone()
    } else {
        format!("{} {conjunction} {last}", rest.join(", "))
    })
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

/// The Version's text subtitle Tracks.
fn text_subtitles(version: i64, probe: &Probe) -> Vec<Subtitle> {
    (probe.tracks.iter().enumerate())
        .filter(|(_, track)| track.is_text_subtitle())
        .map(|(place, track)| Subtitle {
            track: track.clone(),
            place,
            url: subtitles::url(version, place),
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

    use nama_proto::nama::v1::{AudioCodec, Container, VideoCodec};

    use super::{
        AskedSubtitle, Candidates, DeviceDecoding, Facts, Fallback, Method, Narrowing, Network,
        Offers, PickedLimit, Plan, PlaybackLimit, Playing, Subtitle, build, default_max_transcodes,
        narrowed, on_local_network, preferred_subtitle, subtitles_asked, too_busy_to_play,
    };
    use crate::{
        ffmpeg::{
            Accel, CanDo, Hardware, Remux, RemuxAudio, ToneMapper, Transcode, VideoConversion,
            VideoLimit,
        },
        server_settings::HardwareChoice,
        track_preferences::{Description, SubtitlePreference},
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
            shown_subtitle: None,
            hardware_choice: HardwareChoice::Automatic,
            dropped: BTreeSet::new(),
        }
    }

    /// What `plan` Transcodes, failing the test if it doesn't Transcode.
    fn transcoded(plan: &Plan) -> Transcode {
        match plan.playing {
            Some(Playing::Transcode { transcode, .. }) => transcode,
            ref other => panic!("{other:?} isn't a Transcode"),
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

    #[test]
    fn an_h264_and_aac_mp4_direct_plays_because_its_tracks_need_no_change() {
        let plan = build(&mut facts(Some(vec![video("h264"), audio("aac", true)])));

        assert_eq!(
            plan,
            Plan {
                version: 7,
                playing: Some(Playing::DirectPlay),
                tracks: vec![video("h264"), audio("aac", true)],
                reasons: vec![
                    "Its Tracks need no change".to_owned(),
                    "Its length is known".to_owned(),
                    "This Device decodes its container and Tracks".to_owned(),
                    "This Server has ffmpeg".to_owned(),
                    "No hardware acceleration passed its test when this Server started, so a \
                     Transcode runs in software"
                        .to_owned(),
                    "Nothing has failed to play it on this Device".to_owned(),
                    "This Server isn't running as many Transcodes as it may at once".to_owned(),
                    "It plays the file's own audio Track".to_owned(),
                    "This Device is on the local network".to_owned(),
                    "The Playback Limit is Original".to_owned(),
                    "No subtitles are burned into its picture".to_owned(),
                ],
                subtitles: vec![],
                limit: None,
                audio: Some(1),
                burn_in: None,
                starting_subtitle: None,
                offers: Offers {
                    audio: vec![1],
                    burn_in: vec![],
                    limits: vec![PlaybackLimit::P720, PlaybackLimit::P480],
                },
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
            let plan = build(&mut in_container(container, Some(tracks)));

            assert_eq!(plan.method(), Some(Method::Transcode), "{cant}");
            let cant_direct_play =
                format!("This Device can't decode its {cant}, so it can't be Direct Played");
            assert!(
                plan.reasons[2].starts_with(&cant_direct_play),
                "{}",
                plan.reasons[2]
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
            assert_eq!(
                plan.reasons[2].contains("can't be Direct Played or Remuxed"),
                method == Method::Transcode,
                "{}",
                plan.reasons[2]
            );
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
            mkv.reasons[1],
            "Its length isn't known, which a Remux's or a Transcode's playlist needs, so it can \
             only be Direct Played"
        );
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
    fn a_version_whose_tracks_arent_known_direct_plays_and_says_so() {
        let plan = build(&mut facts(None));

        assert_eq!(plan.method(), Some(Method::DirectPlay));
        assert_eq!(plan.tracks, []);
        assert_eq!(
            plan.reasons,
            [
                "Its Tracks aren't known, so they're taken to need no change, and it can only \
                 be Direct Played",
                "Its length isn't known, which a Remux's or a Transcode's playlist needs, so it \
                 can only be Direct Played",
                "Its Tracks aren't known, so this Device is taken to decode them",
                "This Server has ffmpeg",
                "Nothing has failed to play it on this Device",
                "This Server isn't running as many Transcodes as it may at once",
                "It plays the file's own audio Track",
                "This Device is on the local network",
                "The Playback Limit is Original",
                "No subtitles are burned into its picture",
            ]
        );
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
            plan.reasons[7],
            "The audio Track chosen isn't the one browsers play from its file, so it can't be \
             Direct Played"
        );
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
            plan.reasons,
            [
                "Its Tracks need no change",
                "Its length is known",
                "This Device can't decode its Matroska container, so it can't be Direct Played",
                "This Server has ffmpeg",
                "No hardware acceleration passed its test when this Server started, so a \
                 Transcode runs in software",
                "Nothing has failed to play it on this Device",
                "This Server isn't running as many Transcodes as it may at once",
                "It plays the file's own audio Track",
                "This Device is on the local network",
                "The Playback Limit is Original",
                "No subtitles are burned into its picture",
            ]
        );
    }

    #[test]
    fn each_method_that_just_failed_on_the_device_is_ruled_out_and_the_plan_says_so() {
        use Method::{DirectPlay, Remux, Transcode};
        let cases = [
            (
                vec![],
                DirectPlay,
                "Nothing has failed to play it on this Device",
            ),
            (vec![DirectPlay], Remux, "Direct Play failed on this Device"),
            (
                vec![Remux, DirectPlay],
                Transcode,
                "Direct Play and Remux failed on this Device",
            ),
        ];
        for (failed, method, reason) in cases {
            let mut facts = Facts {
                failed: BTreeSet::from_iter(failed),
                ..facts(Some(vec![video("h264"), audio("aac", true)]))
            };

            let plan = build(&mut facts);

            assert_eq!(plan.method(), Some(method), "{reason}");
            assert_eq!(plan.reasons[5], reason);
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
                place: 1,
                url: "/media/versions/7/subtitles/1".to_owned(),
            },
            Subtitle {
                track: french,
                place: 5,
                url: "/media/versions/7/subtitles/5".to_owned(),
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
            without_ffmpeg.reasons[3],
            "This Server lacks ffmpeg or ffprobe, so it can only Direct Play"
        );
        assert_eq!(without_hls.method(), None);
        assert_eq!(
            without_hls.reasons[2],
            "This Device can't decode its Matroska container, so it can't be Direct Played, \
             and it can't play HLS, which anything else needs"
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
            transcode.reasons[3],
            "This Server's ffmpeg lacks libx264, so it can't Transcode"
        );
        assert_eq!(build(&mut remux).method(), Some(Method::Remux));
        assert_eq!(build(&mut direct_play).method(), Some(Method::DirectPlay));
    }

    #[test]
    fn once_a_lasting_step_rules_out_a_transcode_its_plan_doesnt_say_where_one_runs() {
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
        ];
        for (case, mut facts) in cases {
            let plan = build(&mut facts);

            assert!(
                !plan
                    .reasons
                    .iter()
                    .any(|reason| { reason.contains("Transcode runs") }),
                "{case}: {:?}",
                plan.reasons
            );
        }
    }

    fn rules_out_direct_play(_: &Facts, _: Option<&Candidates>) -> Narrowing {
        Narrowing {
            allows: BTreeSet::from([Method::Remux, Method::Transcode]),
            reason: "ruled out Direct Play".to_owned(),
        }
    }

    fn allows_every_method(_: &Facts, _: Option<&Candidates>) -> Narrowing {
        Narrowing {
            allows: BTreeSet::from(Method::ALL),
            reason: "allowed every method".to_owned(),
        }
    }

    #[test]
    fn a_later_step_cant_allow_what_an_earlier_step_ruled_out() {
        let mut reasons = vec![];

        let allowed = narrowed(
            &facts(None),
            None,
            &[rules_out_direct_play, allows_every_method],
            BTreeSet::from(Method::ALL),
            &mut reasons,
        );

        assert_eq!(allowed, BTreeSet::from([Method::Remux, Method::Transcode]));
        assert_eq!(reasons, ["ruled out Direct Play", "allowed every method"]);
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
            plan.reasons[2],
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

            let plan = build(&mut facts);

            assert_eq!(
                plan.tracks[1],
                audio_with("aac", true, converted),
                "{source:?} on {plays}"
            );
            assert_eq!(
                plan.reasons[2],
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
        let mut facts = in_container(MKV, Some(vec![hevc, audio_with("aac", true, Some(6))]));

        let plan = build(&mut facts);

        assert_eq!(plan.method(), Some(Method::Transcode));
        assert_eq!(
            plan.tracks,
            [video("h264"), audio_with("aac", true, Some(2))]
        );
        assert_eq!(
            plan.reasons[2],
            "This Device can't decode its Matroska container or HEVC video, so it can't be \
             Direct Played or Remuxed, and a Transcode converts it to H.264 and stereo AAC, as \
             this Device doesn't play 5.1"
        );
    }

    #[test]
    fn hdr_transcoded_for_a_device_that_cant_show_it_is_tone_mapped_and_the_plan_says_so() {
        let cases = [
            (Hdr::Hdr10, "HDR10"),
            (Hdr::Hlg, "HLG"),
            (dolby_vision_8(Some(BaseLayer::Hdr10)), "Dolby Vision"),
            (dolby_vision_8(Some(BaseLayer::Hlg)), "Dolby Vision"),
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
            assert_eq!(
                plan.reasons[3],
                format!(
                    "This Server has ffmpeg, which tone-maps its {name} picture to SDR in a Transcode"
                )
            );
        }
    }

    #[test]
    fn without_zscale_hdr_isnt_tone_mapped_and_the_plan_says_its_colours_look_washed_out() {
        let mut facts = in_container(MP4, Some(vec![hdr_video("hevc", Some(Hdr::Hdr10))]));
        facts.can_do.zscale = false;

        let plan = build(&mut facts);

        assert_eq!(transcoded(&plan).video.tone_map, None);
        assert_eq!(
            plan.reasons[2],
            "This Device can't decode its HEVC video or HDR10 picture, so it can't be Direct \
             Played or Remuxed, and a Transcode converts it to H.264"
        );
        assert_eq!(
            plan.reasons[3],
            "This Server's ffmpeg lacks zscale, so a Transcode can't tone-map its HDR10 picture \
             to SDR and its colours look washed out"
        );
    }

    #[test]
    fn hdr_transcoded_for_a_device_that_shows_it_isnt_tone_mapped() {
        let mut facts = in_container(MP4, Some(vec![hdr_video("hevc", Some(Hdr::Hdr10))]));
        facts.device.hdr = true;

        let plan = build(&mut facts);

        assert_eq!(transcoded(&plan).video.tone_map, None);
        assert_eq!(plan.reasons[3], "This Server has ffmpeg");
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
        assert_eq!(plan.reasons[3], "This Server has ffmpeg");
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
            plan.reasons[3],
            "This Server has ffmpeg, but it can't Transcode a Dolby Vision picture with no Base \
             Layer, as in profile 5"
        );
    }

    #[test]
    fn a_transcode_of_a_version_without_audio_says_only_h264() {
        let mut facts = in_container(MP4, Some(vec![video("hevc")]));

        let plan = build(&mut facts);

        assert_eq!(plan.method(), Some(Method::Transcode));
        assert_eq!(
            plan.reasons[2],
            "This Device can't decode its HEVC video, so it can't be Direct Played or Remuxed, \
             and a Transcode converts it to H.264"
        );
    }

    #[test]
    fn a_busy_server_remuxes_what_the_device_can_play_remuxed_and_the_plan_says_so() {
        let mut facts = Facts {
            transcodes_full: Some(1),
            ..in_container(MKV, Some(vec![video("h264"), audio("aac", true)]))
        };

        let plan = build(&mut facts);

        assert_eq!(plan.method(), Some(Method::Remux));
        assert_eq!(
            plan.reasons[6],
            "This Server is busy, running as many Transcodes as it may at once (1), so it can't \
             Transcode it"
        );
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
        assert_eq!(
            plan.reasons[9],
            "Its picture is larger than the 720p Playback Limit, so a Transcode scales it down to \
             1280×720"
        );
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
        assert_eq!(
            defaulted.reasons[8],
            "This Device is outside the local network, so the default Playback Limit for remote \
             playback, 720p, applies"
        );
        assert_eq!(picked.method(), Some(Method::DirectPlay));
        assert_eq!(picked.limit, None);
        assert_eq!(
            picked.reasons[8],
            "This Device is outside the local network, but its User picked a Playback Limit in \
             the player, which applies instead of the default for remote playback"
        );
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
            let mut facts = Facts {
                picked_limit: PickedLimit::Picked(limit),
                ..facts(tracks)
            };

            let plan = build(&mut facts);

            assert_eq!(plan.method(), Some(Method::DirectPlay), "{reason}");
            // The reasons end with the four choice steps', so the Playback Limit's is second last.
            assert_eq!(plan.reasons[plan.reasons.len() - 2], reason, "{reason}");
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

    #[test]
    fn a_chosen_image_or_text_subtitle_is_burned_in_by_a_transcode_and_the_plan_says_so() {
        let pgs = track(
            "hdmv_pgs_subtitle",
            false,
            TrackKind::Subtitle { image: true },
        );
        let ass = track("ass", false, TrackKind::Subtitle { image: false });
        for chosen in [pgs, ass] {
            let mut facts = Facts {
                subtitle: Some(2),
                ..facts(Some(vec![
                    video("h264"),
                    audio("aac", true),
                    chosen.clone(),
                ]))
            };

            let plan = build(&mut facts);

            assert_eq!(plan.method(), Some(Method::Transcode), "{}", chosen.codec);
            assert_eq!(
                plan.reasons[10],
                "The subtitles chosen are burned into its picture, which only a Transcode does"
            );
            assert_eq!(plan.burn_in, Some(2), "{}", chosen.codec);
        }
    }

    #[test]
    fn automatic_transcodes_on_the_first_hardware_found_and_the_plan_says_so() {
        let mut facts = hevc_on(HardwareChoice::Automatic, vec![QSV, VAAPI], None);

        let plan = build(&mut facts);

        assert_eq!(
            plan.reasons[4],
            "A Transcode runs on the GPU with Intel QSV"
        );
        assert_eq!(
            transcoded(&plan).video,
            VideoConversion {
                hardware: Some(Accel::Qsv),
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
                "A Transcode runs on the GPU with VAAPI",
            ),
            (
                HardwareChoice::Api(Accel::Nvenc),
                vec![VAAPI],
                None,
                "NVIDIA NVENC didn't pass its test when this Server started, so a Transcode runs \
                 in software",
            ),
            (
                HardwareChoice::Off,
                vec![VAAPI],
                None,
                "Hardware acceleration is off, so a Transcode runs in software",
            ),
            (
                HardwareChoice::Automatic,
                vec![],
                None,
                "No hardware acceleration passed its test when this Server started, so a \
                 Transcode runs in software",
            ),
        ];
        for (choice, found, on, reason) in cases {
            let mut facts = hevc_on(choice, found, None);

            let plan = build(&mut facts);

            assert_eq!(plan.reasons[4], reason);
            assert_eq!(transcoded(&plan).video.hardware, on, "{reason}");
        }
    }

    /// A subtitle Track of `codec` in `language`, forced or not, SDH or not, and titled `title`.
    fn subtitle_in(
        codec: &str,
        language: Option<&str>,
        (forced, sdh): (bool, bool),
        title: Option<&str>,
    ) -> Track {
        let image = codec == "hdmv_pgs_subtitle";
        Track {
            language: language.map(str::to_owned),
            title: title.map(str::to_owned),
            forced,
            sdh,
            ..track(codec, false, TrackKind::Subtitle { image })
        }
    }

    /// A Subtitle Preference for a Track like `track`.
    fn like(track: &Track) -> SubtitlePreference {
        SubtitlePreference::Like(Description::of(track))
    }

    const NOT_FORCED_OR_SDH: (bool, bool) = (false, false);

    #[test]
    fn a_preferred_subtitle_must_be_in_its_language_and_as_forced() {
        let english = subtitle_in("subrip", Some("eng"), NOT_FORCED_OR_SDH, None);
        let tracks = [
            video("h264"),
            subtitle_in("subrip", Some("eng"), (true, false), None),
            subtitle_in("subrip", Some("ger"), NOT_FORCED_OR_SDH, None),
            subtitle_in("subrip", Some("en"), NOT_FORCED_OR_SDH, None),
            subtitle_in("subrip", None, NOT_FORCED_OR_SDH, None),
        ];

        assert_eq!(preferred_subtitle(&like(&english), &tracks), Some(3));
        let unlabelled = subtitle_in("subrip", None, NOT_FORCED_OR_SDH, None);
        assert_eq!(preferred_subtitle(&like(&unlabelled), &tracks), Some(4));
        let french = subtitle_in("subrip", Some("fre"), NOT_FORCED_OR_SDH, None);
        assert_eq!(preferred_subtitle(&like(&french), &tracks), None);
        assert_eq!(preferred_subtitle(&SubtitlePreference::Off, &tracks), None);
    }

    #[test]
    fn a_preferred_subtitle_ranks_sdh_then_title_then_one_the_device_shows_then_the_first() {
        let wanted = subtitle_in("subrip", Some("eng"), NOT_FORCED_OR_SDH, Some("Full"));
        let cases = [
            (
                "the same SDH beats the same title",
                vec![
                    subtitle_in("subrip", Some("eng"), (false, true), Some("Full")),
                    subtitle_in("subrip", Some("eng"), NOT_FORCED_OR_SDH, Some("Signs")),
                ],
                Some(1),
            ),
            (
                "the same title, in any case, beats one the Device shows",
                vec![
                    subtitle_in("subrip", Some("eng"), NOT_FORCED_OR_SDH, None),
                    subtitle_in(
                        "hdmv_pgs_subtitle",
                        Some("eng"),
                        NOT_FORCED_OR_SDH,
                        Some("FULL"),
                    ),
                ],
                Some(1),
            ),
            (
                "one the Device shows beats one that needs Burn-in",
                vec![
                    subtitle_in(
                        "hdmv_pgs_subtitle",
                        Some("eng"),
                        NOT_FORCED_OR_SDH,
                        Some("Full"),
                    ),
                    subtitle_in("subrip", Some("eng"), NOT_FORCED_OR_SDH, Some("Full")),
                ],
                Some(1),
            ),
            (
                "a tie goes to the first",
                vec![
                    subtitle_in("subrip", Some("eng"), NOT_FORCED_OR_SDH, Some("Full")),
                    subtitle_in("subrip", Some("eng"), NOT_FORCED_OR_SDH, Some("Full")),
                ],
                Some(0),
            ),
            (
                "one neither shown nor burned in never matches",
                vec![subtitle_in(
                    "dvb_teletext",
                    Some("eng"),
                    NOT_FORCED_OR_SDH,
                    Some("Full"),
                )],
                None,
            ),
        ];

        for (case, tracks, place) in cases {
            let preferred = preferred_subtitle(&like(&wanted), &tracks);
            assert_eq!(preferred, place, "{case}");
        }
    }

    #[test]
    fn a_preferred_text_subtitle_is_shown_by_the_device_and_the_plan_starts_with_it() {
        let english = subtitle_in("subrip", Some("eng"), NOT_FORCED_OR_SDH, None);
        let tracks = vec![video("h264"), audio("aac", true), english.clone()];
        let mut facts = facts(Some(tracks));
        let asked = AskedSubtitle::Preferred(Some(like(&english)));
        (facts.subtitle, facts.shown_subtitle) = subtitles_asked(asked, facts.probe.as_ref());

        let plan = build(&mut facts);

        assert_eq!(plan.method(), Some(Method::DirectPlay));
        assert_eq!((plan.burn_in, plan.starting_subtitle), (None, Some(2)));
    }

    #[test]
    fn a_preferred_image_subtitle_is_burned_in_or_dropped_as_a_chosen_one_is() {
        let pgs = subtitle_in("hdmv_pgs_subtitle", Some("eng"), NOT_FORCED_OR_SDH, None);
        let tracks = vec![video("h264"), audio("aac", true), pgs.clone()];
        let preferred = || {
            let mut facts = facts(Some(tracks.clone()));
            let asked = AskedSubtitle::Preferred(Some(like(&pgs)));
            (facts.subtitle, facts.shown_subtitle) = subtitles_asked(asked, facts.probe.as_ref());
            facts
        };

        let burned_in = build(&mut preferred());
        let mut no_ffmpeg = preferred();
        without_ffmpeg(&mut no_ffmpeg);
        let dropped = build(&mut no_ffmpeg);

        assert_eq!(burned_in.method(), Some(Method::Transcode));
        assert_eq!(
            (burned_in.burn_in, burned_in.starting_subtitle),
            (Some(2), Some(2))
        );
        assert_eq!(dropped.method(), Some(Method::DirectPlay));
        assert_eq!((dropped.burn_in, dropped.starting_subtitle), (None, None));
        assert_eq!(
            dropped.reasons.last().map(String::as_str),
            Some(
                "The subtitles chosen need a Transcode to burn them in, which can't play it here, \
                 so they're dropped"
            )
        );
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
            assert_eq!(plan.reasons[10], "No subtitles are burned into its picture");
        }
    }

    const VAAPI: Hardware = Hardware {
        accel: Accel::Vaapi,
        tone_maps: false,
    };
    const QSV: Hardware = Hardware {
        accel: Accel::Qsv,
        tone_maps: true,
    };

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
            (
                QSV,
                Some(ToneMapper::Hardware),
                "A Transcode runs on the GPU with Intel QSV, which tone-maps its HDR10 picture to \
                 SDR",
            ),
            (
                VAAPI,
                Some(ToneMapper::Software),
                "A Transcode runs on the GPU with VAAPI, and tone-maps its HDR10 picture to SDR \
                 in software",
            ),
        ];
        for (hardware, tone_map, reason) in cases {
            let mut facts = hevc_on(HardwareChoice::Automatic, vec![hardware], Some(Hdr::Hdr10));

            let plan = build(&mut facts);

            assert_eq!(plan.reasons[4], reason);
            assert_eq!(transcoded(&plan).video.tone_map, tone_map, "{reason}");
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
        assert_eq!(
            falls_back_to(&plan),
            Some(Fallback {
                transcode: Transcode {
                    video: VideoConversion {
                        hardware: None,
                        tone_map: Some(ToneMapper::Software),
                        ..on_hardware.video
                    },
                    ..on_hardware
                },
                why: "Intel QSV failed partway, so the rest of it Transcodes in software"
                    .to_owned(),
            })
        );
    }

    #[test]
    fn without_zscale_a_hardware_transcode_of_hdr_falls_back_without_tone_mapping_and_says_so() {
        let mut facts = hevc_on(HardwareChoice::Automatic, vec![QSV], Some(Hdr::Hdr10));
        facts.can_do.zscale = false;

        let plan = build(&mut facts);

        let fallback = falls_back_to(&plan).unwrap();
        assert_eq!(fallback.transcode.video.hardware, None);
        assert_eq!(fallback.transcode.video.tone_map, None);
        assert_eq!(
            fallback.why,
            "Intel QSV failed partway, so the rest of it Transcodes in software, without \
             tone-mapping its HDR, as this Server's ffmpeg lacks zscale"
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

    /// Stores a file under `root` in the Library `library` in `contents` without probing it, and
    /// Matches it to Alien, which makes it a Version. Returns its id.
    async fn unprobed_version(
        contents: &crate::contents::Contents,
        library: i64,
        root: &std::path::Path,
    ) -> i64 {
        use nama_plugin_api::{PluginId, metadata::ExternalIds};

        use crate::contents::{Change, Decided, Item, Metadata};

        let file = root.join("Alien (1979)").join("Alien (1979).mp4");
        let alien = Change::Store {
            path: file.to_str().unwrap().to_owned(),
            size: 10,
            modified_ns: 0,
            items: vec![Item::Movie {
                title: "Alien".to_owned(),
                year: Some(1979),
                hint: None,
            }],
        };
        contents.apply(library, vec![alien]).await.unwrap();
        let matched = Metadata {
            plugin: PluginId::from_static("tmdb"),
            ids: ExternalIds {
                tmdb: Some(348),
                ..ExternalIds::default()
            },
            show_ids: ExternalIds::default(),
            title: "Alien".to_owned(),
            original_title: None,
            show_title: None,
            overview: None,
            release_date: None,
            runtime_minutes: None,
            poster: None,
            backdrop: None,
        };
        let (item, _, _) = contents.undecided(library).await.unwrap().remove(0);
        contents
            .record(vec![(item, Decided::Matched(Box::new(matched)))])
            .await
            .unwrap();
        contents.every_stored().await.unwrap()[0].id
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
            contents::Contents,
            db::Database,
            events::EventBus,
            ffmpeg::{Abilities, Ffmpeg},
            libraries::{Kind, Libraries},
            playback::{
                Asked, AskedSubtitle, Planned, StartedPlan, plan, running::RunningPlans, subtitles,
            },
            plugins::ExtensionPoints,
            probing::Probing,
            server_settings::{ServerUrl, Settings},
        };

        let data = tempfile::tempdir().unwrap();
        let media = tempfile::tempdir().unwrap();
        let root = media.path().canonicalize().unwrap().join("library");
        std::fs::create_dir(&root).unwrap();
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let library = Libraries::new(Arc::clone(&db))
            .create(
                "Movies",
                Kind::Movies,
                vec![root.to_str().unwrap().to_owned()],
            )
            .await
            .unwrap();
        let contents = Contents::new(Arc::clone(&db));
        let version = unprobed_version(&contents, library.id, &root).await;
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
            contents,
            probing.clone(),
            EventBus::new(db, ExtensionPoints::default(), ServerUrl::default()),
        );

        let asked = Asked {
            version,
            decoding: DeviceDecoding::default(),
            failed: BTreeSet::new(),
            audio: None,
            subtitle: AskedSubtitle::None,
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
    fn a_choice_only_a_transcode_honours_is_dropped_when_it_cant_transcode_and_the_plan_says_why() {
        const LIMIT_DROPPED: &str = "The 720p Playback Limit needs a Transcode, which can't play it here, so it's \
             dropped for Original";
        // How far from the end of the reasons each choice's step gives its own, since the steps
        // before them give more or fewer reasons depending on what ruled a Transcode out.
        let choices: [(&str, Change, usize, &str); 3] = [
            ("720p", picking_720p, 2, LIMIT_DROPPED),
            ("the remote default", remote_at_720p, 2, LIMIT_DROPPED),
            (
                "PGS",
                burning_in_pgs,
                1,
                "The subtitles chosen need a Transcode to burn them in, which can't play it here, \
                 so they're dropped",
            ),
        ];
        for (chosen, choose, from_end, reason) in choices {
            for (ruled_out, rule_out) in NO_TRANSCODE {
                let mut facts = direct_played();
                choose(&mut facts);
                rule_out(&mut facts);

                let plan = build(&mut facts);

                let case = format!("{chosen} with {ruled_out}");
                assert_eq!(plan.method(), Some(Method::DirectPlay), "{case}");
                assert_eq!((plan.limit, plan.burn_in), (None, None), "{case}");
                assert_eq!(
                    plan.reasons[plan.reasons.len() - from_end],
                    reason,
                    "{case}"
                );
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
                // The chosen audio's step is the first of the four choice steps, which end the reasons.
                assert_eq!(
                    plan.reasons[plan.reasons.len() - 4],
                    "The audio Track chosen needs a Remux or a Transcode, and neither can play it \
                     here, so it's dropped for the file's own",
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
                assert_eq!(
                    plan.reasons[2],
                    "This Device decodes its container and Tracks, but it can't play HLS, which \
                     anything else needs",
                    "{chosen}"
                );
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
