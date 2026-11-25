//! Plans: the one path that plans a Version for a Device and keeps the Tracks a User picks
//! (`AGENTS.md`).

use std::{collections::BTreeSet, net::IpAddr, sync::Arc};

use nama_proto::nama::v1::{AudioCodec, Container, VideoCodec};

use super::{
    AskedAudio, Description, DeviceDecoding, Facts, Method, Network, PickedLimit, Plan, Planned,
    PreviewFrames, Probing, RunningPlans, StartError, StartedPlan, SubtitlePreference, Track,
    TrackKind, TrackPreferenceError, TrackPreferences, audio_asked, build,
    codecs::is_audio,
    direct_play::sees_version,
    on_local_network, playable_versions,
    probing::{self, ProbeError},
    subtitle_offsets, too_busy_to_play,
    tracks::segments_of,
};
use crate::{
    db::{Database, detached},
    diagnostics::{self, Usage},
    ffmpeg::{Abilities, RunError},
    server_settings::{PlaybackLimit, PlaybackSettings},
    users::{Caller, remote_cap},
};

/// Plans a Version for a Device, starting it if it plays, and keeps the Tracks a User picks.
#[derive(Clone)]
pub(crate) struct Plans {
    probing: Probing,
    abilities: Abilities,
    running: RunningPlans,
    settings: PlaybackSettings,
    preferences: TrackPreferences,
    preview_frames: PreviewFrames,
    db: Arc<Database>,
}

/// What a request for a plan asks, never a Preference already read.
#[derive(Debug)]
pub(crate) struct Asked {
    /// The Version to play, by id.
    pub(crate) version: i64,
    pub(crate) decoding: DeviceDecoding,
    /// The methods that just failed to play it on the Device.
    pub(crate) failed: BTreeSet<Method>,
    /// The Recipes of the plans that just failed to play it on the Device, as they named them.
    pub(crate) failed_recipes: BTreeSet<String>,
    /// The audio Track chosen, by its place in the Version's Tracks, or `None` for the Server's
    /// choice by the User's Audio Preference.
    pub(crate) audio: Option<usize>,
    pub(crate) subtitle: AskedSubtitle,
    pub(crate) limit: PickedLimit,
    /// Where playing starts, in milliseconds. A Remux or a Transcode starts ffmpeg at the segment
    /// playing there.
    pub(crate) start_ms: i64,
    /// The Device's address, which says whether it's on the local network.
    pub(crate) client: IpAddr,
}

impl Asked {
    /// Whether this is a player's first ask for the Version, so a play starting: that leaves every
    /// choice to the Server, while asking again, for another Track or after a failure, carries
    /// what the last plan played or what failed.
    fn is_first(&self) -> bool {
        self.audio.is_none()
            && matches!(self.subtitle, AskedSubtitle::Preferred)
            && self.failed.is_empty()
            && self.failed_recipes.is_empty()
    }
}

/// What a request for a plan asks of subtitles.
#[derive(Debug)]
pub(crate) enum AskedSubtitle {
    /// The subtitle Track chosen to burn into the picture, by its place in the Version's Tracks.
    BurnIn(usize),
    /// Subtitles turned off: none burned in, and none for the Server to choose.
    None,
    /// The Server's choice, by the User's Subtitle Preference for what the Version is of, or else
    /// their own, if they have one (ADR 0035).
    Preferred,
}

/// The Track a User picked while playing, by its place in the Version's Tracks.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Pick {
    /// A subtitle, or `None` for subtitles turned off.
    Subtitle(Option<usize>),
    /// Audio, which is never off.
    Audio(usize),
}

/// Why a Track pick or Subtitle Offset couldn't be kept.
#[derive(Debug, thiserror::Error)]
pub(crate) enum PickError {
    #[error("there is no such Version")]
    NoSuchVersion,
    #[error("the Version has no Track of the kind picked at that place")]
    TrackNotFound,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl From<TrackPreferenceError> for PickError {
    fn from(err: TrackPreferenceError) -> Self {
        match err {
            TrackPreferenceError::NoSuchVersion => Self::NoSuchVersion,
            TrackPreferenceError::Database(err) => Self::Database(err),
        }
    }
}

impl Plans {
    #[expect(
        clippy::too_many_arguments,
        reason = "each is a handle planning reads; a struct bundling them would exist only for \
                  this call"
    )]
    pub(crate) fn new(
        probing: Probing,
        abilities: Abilities,
        running: RunningPlans,
        settings: PlaybackSettings,
        preview_frames: PreviewFrames,
        preferences: TrackPreferences,
        db: Arc<Database>,
    ) -> Self {
        Self {
            probing,
            abilities,
            running,
            settings,
            preferences,
            preview_frames,
            db,
        }
    }

    /// Plans what `asked` asks on `caller`'s Device, by the User's Preferences where it leaves the
    /// choice to the Server, and starts it if it plays. It probes the Version first unless its
    /// Tracks are current. A Remux whose keyframes can't be listed, or a plan whose ffmpeg stops
    /// before writing anything, is planned again without its method, until one plays or none is
    /// left. `None` if there is no such Version, or it is in a Library the User doesn't see, and
    /// [`StartError::Busy`] if only a Transcode would play it and the Server is running as many as
    /// it may.
    pub(crate) async fn plan(
        &self,
        caller: &Caller,
        asked: Asked,
    ) -> Result<Option<Planned>, StartError> {
        let (user, version, start_ms) = (caller.user.id, asked.version, asked.start_ms);
        let first = asked.is_first();
        if !sees_version(&self.db, user, version).await? {
            return Ok(None);
        }
        // Read before anything starts, so a failed read leaves no Remux or Transcode running that
        // no Device was told about.
        let segments = segments_of(&self.db, version).await?;
        let preview_frames = self.preview_frames.of(version).await?;
        let subtitle_offsets = subtitle_offsets::of(&self.db, user, version).await?;
        let Some(facts) = self.facts(user, asked).await? else {
            return Ok(None);
        };
        let mut planned = self.start_first_playing(caller, facts, start_ms).await?;
        if let Some(Planned::Started(StartedPlan { plan, .. }) | Planned::Unplayable { plan, .. }) =
            &mut planned
        {
            plan.segments = segments;
            plan.preview_frames = preview_frames;
            plan.subtitle_offsets = subtitle_offsets;
        }
        if first && matches!(planned, Some(Planned::Started(_))) {
            diagnostics::record(Usage::PlaybackStarted);
        }
        Ok(planned)
    }

    /// The plan of a Transcode of the Version `version` into a file `caller`'s Device keeps to
    /// play offline, asked from `client`, fitting `limit`, or kept to their cap for remote
    /// playback if lower: H.264 and stereo AAC, which every Device plays, with no subtitle burned
    /// in. Nothing is started. `None` as [`Self::plan`] says; a plan that can't be played says why
    /// in its reasons.
    pub(crate) async fn plan_download(
        &self,
        caller: &Caller,
        version: i64,
        limit: PlaybackLimit,
        client: IpAddr,
    ) -> Result<Option<Plan>, StartError> {
        let user = caller.user.id;
        if !sees_version(&self.db, user, version).await? {
            return Ok(None);
        }
        let asked = Asked {
            version,
            decoding: DeviceDecoding {
                containers: vec![Container::CONTAINER_MP4],
                video_codecs: vec![VideoCodec::VIDEO_CODEC_H264],
                audio_codecs: vec![AudioCodec::AUDIO_CODEC_AAC],
                hls: true,
                audio_channels: 2,
                ..DeviceDecoding::default()
            },
            // Ruled out as if they'd failed, so only a Transcode, which makes a file, is left.
            failed: BTreeSet::from([Method::DirectPlay, Method::Remux]),
            failed_recipes: BTreeSet::new(),
            audio: None,
            subtitle: AskedSubtitle::None,
            limit: PickedLimit::Picked(Some(limit)),
            start_ms: 0,
            client,
        };
        let Some(mut facts) = self.facts(user, asked).await? else {
            return Ok(None);
        };
        Ok(Some(build(&mut facts)))
    }

    /// Stops the plan `plan` if the Device `device` owns it.
    pub(crate) async fn stop(&self, plan: u64, device: i64) {
        self.running.stop_for(plan, device).await;
    }

    /// Keeps `pick`, made by the User `user` in the Version `version`, as their Preference for the
    /// Show or Movie it is of, probing the Version first if its Tracks aren't current. A Version of
    /// an Unmatched Item keeps none.
    pub(crate) async fn keep_pick(
        &self,
        user: i64,
        version: i64,
        pick: Pick,
    ) -> Result<(), PickError> {
        // An Unmatched Item keeps nothing, as it keeps no Watch State (ADR 0013): its Tracks aren't
        // read, so the pick never fails, whatever it names.
        let Some(kept_for) = self.preferences.kept_for(version).await? else {
            return Ok(());
        };
        let preferences = &self.preferences;
        match pick {
            Pick::Subtitle(None) => {
                (preferences.keep_subtitle(user, kept_for, SubtitlePreference::Off)).await?;
            }
            Pick::Subtitle(Some(place)) => {
                let picked = Description::of(&self.track_at(version, place, is_subtitle).await?);
                let like = SubtitlePreference::Like(picked);
                preferences.keep_subtitle(user, kept_for, like).await?;
            }
            Pick::Audio(place) => {
                let picked = Description::of(&self.track_at(version, place, is_audio).await?);
                preferences.keep_audio(user, kept_for, picked).await?;
            }
        }
        Ok(())
    }

    /// Sets the User `user`'s Subtitle Offset of the subtitle Track at `place` in the Version
    /// `version`'s Tracks to `offset_ms`, probing them first if they aren't current.
    pub(crate) async fn keep_offset(
        &self,
        user: i64,
        version: i64,
        place: usize,
        offset_ms: i32,
    ) -> Result<(), PickError> {
        let track = self.track_at(version, place, is_subtitle).await?;
        subtitle_offsets::set(&self.db, user, version, (place, &track), offset_ms).await?;
        Ok(())
    }

    /// What a plan of what `asked` asks for the User `user` is based on, probing the Version
    /// unless its Tracks are current. `None` if there is no such Version.
    async fn facts(&self, user: i64, asked: Asked) -> Result<Option<Facts>, StartError> {
        let Asked {
            version,
            decoding,
            failed,
            failed_recipes,
            audio,
            subtitle,
            limit,
            client,
            ..
        } = asked;
        let (subtitle, subtitle_preference) = match subtitle {
            AskedSubtitle::BurnIn(place) => (Some(place), None),
            AskedSubtitle::None => (None, None),
            AskedSubtitle::Preferred => (None, self.preferences.subtitle(user, version).await?),
        };
        let audio = match audio {
            Some(place) => AskedAudio::Chosen(place),
            None => AskedAudio::Preferred(self.preferences.audio(user, version).await?),
        };
        let probe = match self.probing.current(version).await {
            Ok(probe) => Some(probe),
            Err(probing::Error::Probe(err)) => {
                if !matches!(*err, ProbeError::Run(RunError::Unavailable(_))) {
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
        let settings = self.settings.now();
        let network = if on_local_network(client) {
            Network::Local
        } else {
            Network::Remote {
                default: settings.remote_limit,
                cap: remote_cap(&self.db, user).await?,
            }
        };
        Ok(Some(Facts {
            version,
            audio: audio_asked(audio, probe.as_ref(), &decoding),
            probe,
            device: decoding,
            can_do: self.abilities.can_do().await,
            failed,
            failed_on_server: BTreeSet::new(),
            failed_recipes,
            keyframes_unlisted: false,
            running_transcodes: self.running.running_transcodes(),
            max_transcodes: settings.max_transcodes,
            picked_limit: limit,
            network,
            subtitle,
            shown_subtitle: None,
            subtitle_preference,
            hardware_choice: settings.hardware,
            dropped: BTreeSet::new(),
        }))
    }

    /// Starts the plan for `facts`, as [`Self::plan`] says, planning again without a method that
    /// failed on the Server.
    async fn start_first_playing(
        &self,
        caller: &Caller,
        mut facts: Facts,
        start_ms: i64,
    ) -> Result<Option<Planned>, StartError> {
        let version = facts.version;
        loop {
            let mut built = facts.clone();
            let plan = build(&mut built);
            let recipe = plan.recipe();
            match self.start_built(caller, built, plan, start_ms).await {
                Err(StartError::Keyframes(err)) => {
                    tracing::warn!(
                        version,
                        "could not list a Version's keyframes, so planning it without a Remux: \
                         {err}"
                    );
                    facts.keyframes_unlisted = true;
                }
                Err(StartError::FailedOnServer(_)) => {
                    facts.failed_on_server.insert(recipe);
                }
                planned => return planned,
            }
            // The pass may have waited seconds, so the Transcode limit is checked again (#827).
            facts.running_transcodes = self.running.running_transcodes();
        }
    }

    /// Starts `plan`, built from `facts`, if it plays, or else lists the other Versions that would
    /// play.
    async fn start_built(
        &self,
        caller: &Caller,
        facts: Facts,
        plan: Plan,
        start_ms: i64,
    ) -> Result<Option<Planned>, StartError> {
        if plan.playing.is_none() {
            if let Some(max) = too_busy_to_play(facts.clone()) {
                return Err(StartError::Busy(max));
            }
            let playable_versions = playable_versions(&self.probing, &facts).await?;
            return Ok(Some(Planned::Unplayable {
                plan,
                playable_versions,
            }));
        }
        let (running, caller) = (self.running.clone(), caller.clone());
        // A Device giving up mid-wait mustn't leave ffmpeg and its folder with no plan (ADR 0029).
        let started = detached(async move { running.start(&caller, plan, start_ms).await }).await?;
        Ok(started.map(Planned::Started))
    }

    /// The Track at `place` in the Version `version`'s Tracks, probing them first if they aren't
    /// current, if `is_kind` says it is of the kind picked.
    async fn track_at(
        &self,
        version: i64,
        place: usize,
        is_kind: fn(&Track) -> bool,
    ) -> Result<Track, PickError> {
        let probe = match self.probing.current(version).await {
            Ok(probe) => probe,
            // Its Tracks aren't known, so no plan offered one to pick.
            Err(probing::Error::Probe(_)) => return Err(PickError::TrackNotFound),
            Err(probing::Error::NoSuchVersion(_)) => return Err(PickError::NoSuchVersion),
            Err(probing::Error::Database(err)) => return Err(err.into()),
        };
        (probe.tracks.into_iter().nth(place))
            .filter(is_kind)
            .ok_or(PickError::TrackNotFound)
    }
}

const fn is_subtitle(track: &Track) -> bool {
    matches!(track.kind, TrackKind::Subtitle { .. })
}

#[cfg(test)]
#[path = "plans_tests.rs"]
mod tests;
