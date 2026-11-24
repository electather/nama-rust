//! Playable plans, from when they're planned until they stop, and the ffmpeg of those that run it:
//! Remuxes and Transcodes, played over HLS with fMP4 segments. Each plan's start and end are
//! published as `PlaybackStarted` and `PlaybackEnded`.
//!
//! A plan belongs to the Device it was planned for, and only that Device is served its files, at
//! `/media/plans/<id>/`: the playlist `index.m3u8`, then `init.mp4` and the segments `<n>.m4s`.
//! The playlist lists the whole Version up front, cut at keyframes where it will be cut, so HLS
//! time is the file's time and Watch State needs no conversion. ffmpeg writes a file per keyframe
//! to `nama-plan-<id>/` in the segment folder, and the plan joins those starting from each listed
//! keyframe up to the next into that keyframe's file, by the times ffmpeg's own playlist gives, as
//! the Version's index may list only some (ADR 0066). A segment joins its keyframes' files. A
//! Remux keeps the Version's own keyframes; a Transcode forces one every [`SEGMENT_MS`], so each
//! of its segments is one file.
//!
//! A segment asked for waits for ffmpeg to write its files, and one whose files have gone since
//! waits for them to be written again. One far past what ffmpeg has written restarts ffmpeg at it,
//! and ffmpeg stops once it's [`RUN_AHEAD_MS`] past the last segment asked for, to start again at
//! the next file asked for that it hasn't written. Every [`TIDY_EVERY`], files ending further
//! before the last segment asked for than the Playback settings keep for skipping back are
//! deleted, and so are the files earlier runs wrote themselves and, unless ffmpeg is writing
//! them, files starting [`RUN_AHEAD_MS`] or more past that segment. A plan's folder so holds at
//! most the time kept behind plus [`RUN_AHEAD_MS`] (ADR 0062); a file asked for again is written
//! again.
//!
//! A plan whose first run stops before writing anything is never kept or told of. A Transcode on
//! hardware that fails carries on in software from the first file it hadn't written and takes its
//! software fallback's reasons, which `PlaybackChanged`, or `PlaybackStarted` if earlier, says.
//!
//! A plan is stopped, and its folder deleted, when its player says it stopped, [`STOP_UNUSED_AFTER`]
//! after it was last asked for, or as the Server shuts down. A Direct Play is asked for through
//! its file at `/media/versions/<id>`, which marks every Direct Play of that Version on the Device
//! asked for as the request arrives and again with each part of the file sent, so one being
//! downloaded in a single request isn't stopped. [`sweep_leftovers`] deletes the folders of plans
//! a run left behind when the next starts.
//!
//! Deleting a Version ends its plans first and cuts off each response serving its file
//! ([`RunningPlans::end_for_deleting`]): a file open for reading can still be read once it's
//! deleted. An Admin stopping a plan likewise cuts off its Device's responses serving its Version's
//! file until that Device plans it again ([`RunningPlans::stop_by_admin`]), so a Direct Play stops
//! too.
//!
//! [`SEGMENT_MS`]: timeline::SEGMENT_MS

mod plays;
mod process;
mod run;
mod serving;
mod sweeping;
#[cfg(test)]
mod testing;
mod timeline;

use std::{
    collections::HashMap,
    fmt, io,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use futures_util::{Stream, StreamExt as _};
use nama_proto::nama::v1::{
    AdminStop, Playback, PlaybackChanged, PlaybackEnded, PlaybackReason, PlaybackStarted,
};
use tokio_util::{sync::CancellationToken, task::TaskTracker};

pub(crate) use plays::{MessageTooLong, PlayNow, ReportedProgress, STOP_MESSAGE_LENGTH};
use run::{Fallback, Running, Runs, Start};
pub(crate) use serving::router;
pub(crate) use sweeping::sweep_leftovers;
use sweeping::tidy_plans;
use timeline::Timeline;

use super::{
    Method, Playing, StartedPlan, direct_play_url, method_to_proto,
    probing::{self, ProbeError, Probing},
    reason_to_proto, subtitles,
    tracks::track_to_proto,
};
use crate::{
    events::{EventBus, event},
    ffmpeg::{
        BurnIn, Ffmpeg, FromCut, Hardware, Keyframe, Remux, TextSubtitle, Transcode,
        sidecar_charset,
    },
    library::Contents,
    server_settings::PlaybackSettings,
    users::Caller,
};

const PLAYLIST: &str = "index.m3u8";
const INIT: &str = "init.mp4";

/// How far past the start of the last segment asked for ffmpeg writes before it stops, in
/// milliseconds.
const RUN_AHEAD_MS: i64 = 120_000;

/// How far past what a Remux's ffmpeg has written a file asked for can be and still be waited
/// for, in milliseconds. ffmpeg restarts at one further on, since a Remux writes this much in a
/// second or two but a seek far ahead would wait for everything before it.
const REMUX_WAIT_AHEAD_MS: i64 = 30_000;

/// What a plan's folder in the segment folder is named, before its id. The sweep deletes only
/// folders named so, since the admin may set a segment folder that holds other files.
const PLAN_FOLDER: &str = "nama-plan-";

/// How long after a plan was last asked for it's stopped and its files deleted, so a closed tab's
/// plan goes within about a minute.
const STOP_UNUSED_AFTER: Duration = Duration::from_secs(60);

const TIDY_EVERY: Duration = Duration::from_secs(10);

/// How often a request for a file ffmpeg hasn't written yet looks for it again, and how often
/// ffmpeg's progress is looked at.
const POLL_EVERY: Duration = Duration::from_millis(100);

/// Why a plan couldn't start running.
#[derive(Debug, thiserror::Error)]
pub(crate) enum StartError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("the Version's keyframes couldn't be listed: {0}")]
    Keyframes(Arc<ProbeError>),
    #[error("ffmpeg couldn't be started: {0}")]
    Run(#[from] io::Error),
    #[error("ffmpeg stopped before writing a {0:?}'s first file")]
    FailedOnServer(Method),
    /// The plan's folder couldn't be created in the segment folder, `folder`.
    #[error("the plan's folder couldn't be created in {}: {source}", folder.display())]
    SegmentFolder { folder: PathBuf, source: io::Error },
    /// Only a Transcode would play it, and the Server runs as many as it may at once, this many.
    #[error("the Server is running as many Transcodes as it may at once, {0}")]
    Busy(u32),
}

/// What a Transcode on `hardware` runs instead once a run there fails, and its plan's reasons from
/// then on, which say why.
struct FallbackRun {
    start: Box<Start>,
    reasons: Vec<PlaybackReason>,
    hardware: Hardware,
}

/// What a Transcode on hardware runs on until it falls back to software, and its plan's reasons
/// from then on.
struct OnHardware {
    hardware: Hardware,
    fallback_reasons: Vec<PlaybackReason>,
}

/// The Transcodes running, as a Transcode cap counts them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct RunningTranscodes {
    pub(super) all: u32,
    /// Those that work the CPU hard, which the default cap counts.
    pub(super) cpu_bound: u32,
}

/// The playable plans, by id.
#[derive(Clone)]
pub(crate) struct RunningPlans {
    inner: Arc<Inner>,
}

struct Inner {
    /// For the segment folder each plan's files go in, in a folder named by its id.
    settings: PlaybackSettings,
    ffmpeg: Arc<Ffmpeg>,
    contents: Contents,
    probing: Probing,
    subtitles: subtitles::Cache,
    events: EventBus,
    next_id: AtomicU64,
    plans: Mutex<HashMap<u64, Arc<Plan>>>,
    /// [`tidy_plans`], so shutting down can wait for it.
    tasks: TaskTracker,
    /// Cancelled when the Server shuts down.
    cancel: CancellationToken,
    /// By Version, cancelled while it's deleted, which cuts off each response serving its file.
    // ponytail: one kept per Version, and in `stopped` per Version and Device, served since the
    // Server started; drop each once nothing serves it if that ever adds up.
    served: Mutex<HashMap<i64, CancellationToken>>,
    /// By Version and Device, cancelled once an Admin stops that Device's last plan of it, which
    /// cuts off each response serving the file to it until the Device plans it again.
    stopped: Mutex<HashMap<(i64, i64), CancellationToken>>,
}

impl fmt::Debug for RunningPlans {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RunningPlans").finish_non_exhaustive()
    }
}

/// Versions being deleted, each of whose files is cut off as soon as it's served until this is
/// dropped, once they're gone from disk and the Library.
#[must_use = "the Versions are served again once it's dropped"]
pub(crate) struct Deleting {
    plans: RunningPlans,
    versions: Vec<i64>,
}

impl Drop for Deleting {
    fn drop(&mut self) {
        let mut served = lock(&self.plans.inner.served);
        for version in &self.versions {
            served.remove(version);
        }
    }
}

/// One playable plan.
struct Plan {
    /// What it plays, for whom, and on which Device, which alone is served its files, as it
    /// started.
    playback: Playback,
    /// Its HLS files and the ffmpeg writing them; `None` for a Direct Play, which runs nothing.
    hls: Option<Arc<Running>>,
    on_hardware: Option<OnHardware>,
    /// When the plan was last asked for anything, which keeps it another [`STOP_UNUSED_AFTER`].
    last_used: Mutex<Instant>,
    progress: Mutex<Option<ReportedProgress>>,
}

impl RunningPlans {
    /// Runs plans with `ffmpeg`, cutting a Remux at the keyframes `probing` lists and burning in a
    /// text subtitle from `subtitles` once it's there, writing their files to the segment folder
    /// in `settings` as it is when each starts, and publishing on `events` when each stops.
    /// Until these are dropped or shut down, stops the plans gone unused in the background.
    #[expect(
        clippy::too_many_arguments,
        reason = "each is a separate handle plans run on"
    )]
    pub(crate) fn new(
        settings: PlaybackSettings,
        ffmpeg: Arc<Ffmpeg>,
        contents: Contents,
        probing: Probing,
        subtitles: subtitles::Cache,
        events: EventBus,
    ) -> Self {
        let inner = Arc::new(Inner {
            settings,
            ffmpeg,
            contents,
            probing,
            subtitles,
            events,
            // From 1, as the API's plan id 0 is no plan.
            next_id: AtomicU64::new(1),
            plans: Mutex::default(),
            tasks: TaskTracker::new(),
            cancel: CancellationToken::new(),
            served: Mutex::default(),
            stopped: Mutex::default(),
        });
        inner
            .tasks
            .spawn(tidy_plans(Arc::downgrade(&inner), inner.cancel.clone()));
        Self { inner }
    }

    /// The Transcodes running, every Device's own included: a player stops its old plan before
    /// asking for a new one.
    // ponytail: the plan isn't kept until after probing, the folder, spawning ffmpeg and up to
    // `START_WAIT` waiting for its first file, so Devices planning in that window can pass the cap
    // together until Transcodes end; reserve room under the plans' lock if that ever matters.
    pub(super) fn running_transcodes(&self) -> RunningTranscodes {
        let plans = lock(&self.inner.plans);
        let transcodes: Vec<_> = (plans.values())
            .filter_map(|plan| {
                plan.hls
                    .as_ref()
                    .filter(|hls| hls.method() == Method::Transcode)
            })
            .collect();
        let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        RunningTranscodes {
            all: count(transcodes.len()),
            cpu_bound: count(transcodes.iter().filter(|hls| hls.cpu_bound()).count()),
        }
    }

    /// Starts `plan` for `caller`'s Device, telling Observers so, and returns it with its id and
    /// where to play it from. A Remux or a Transcode runs ffmpeg over the Version's file, from the
    /// segment playing at `start_ms`. `None` if there is no such Version, it's being deleted, or
    /// `plan` can't be played.
    pub(crate) async fn start(
        &self,
        caller: &Caller,
        plan: super::Plan,
        start_ms: i64,
    ) -> Result<Option<StartedPlan>, StartError> {
        let Some(playing) = plan
            .playing
            .clone()
            .filter(|_| !self.deleting(plan.version))
        else {
            return Ok(None);
        };
        let playback = playback(caller, &plan);
        let started = match playing {
            Playing::DirectPlay => {
                Some((self.direct_play(playback), direct_play_url(plan.version)))
            }
            Playing::Remux { remux, duration_ms } => {
                self.remux(playback, remux, duration_ms, start_ms).await?
            }
            Playing::Transcode {
                transcode,
                duration_ms,
                fallback,
            } => {
                self.transcode(playback, transcode, duration_ms, fallback, start_ms)
                    .await?
            }
        };
        Ok(started.map(|(id, url)| StartedPlan { id, url, plan }))
    }

    /// Starts Remuxing the Version `playback` plays, `duration_ms` long, for the Device it plays
    /// on, converting what `remux` says, from the segment playing at `start_ms`, and returns its
    /// id and its playlist's URL. `None` if there is no such Version.
    async fn remux(
        &self,
        playback: Playback,
        remux: Remux,
        duration_ms: i64,
        start_ms: i64,
    ) -> Result<Option<(u64, String)>, StartError> {
        let version = playback.version_id;
        let Some((path, _)) = self.inner.contents.version_file(version).await? else {
            return Ok(None);
        };
        let listed = match self.inner.probing.keyframes(version).await {
            Ok(listed) => listed,
            Err(probing::Error::Probe(err)) => return Err(StartError::Keyframes(err)),
            Err(probing::Error::NoSuchVersion(_)) => return Ok(None),
            Err(probing::Error::Database(err)) => return Err(err.into()),
        };
        let timeline = Timeline::new(listed.keyframes_ms, duration_ms);
        let ffmpeg = Arc::clone(&self.inner.ffmpeg);
        let file_start_ms = listed.file_start_ms;
        let start = move |dir: &Path, from, stop| {
            let command = ffmpeg.remux(&path, &remux, dir, from, file_start_ms)?;
            command.spawn().map(|child| process::run(child, stop))
        };
        let runs = Runs {
            method: Method::Remux,
            start: Box::new(start),
            cpu_bound: false,
            fallback: None,
        };
        let id = self
            .start_hls(playback, timeline, start_ms, runs, None)
            .await?;
        Ok(Some((id, playlist_url(id))))
    }

    /// Starts Transcoding the Version `playback` plays, `duration_ms` long, for the Device it
    /// plays on, converting what `transcode` says, from the segment playing at `start_ms`, and
    /// returns its id and its playlist's URL. Its segments are [`SEGMENT_MS`] each, as it forces
    /// a keyframe at each. Once a run on hardware fails, it goes on with `fallback`. `None` if
    /// there is no such Version.
    ///
    /// [`SEGMENT_MS`]: timeline::SEGMENT_MS
    #[expect(
        clippy::too_many_arguments,
        reason = "`Playing::Transcode`'s fields as `start` unpacks them, and where playing starts"
    )]
    async fn transcode(
        &self,
        playback: Playback,
        transcode: Transcode,
        duration_ms: i64,
        fallback: Option<super::Fallback>,
        start_ms: i64,
    ) -> Result<Option<(u64, String)>, StartError> {
        let Some((path, file)) = (self.inner.contents)
            .version_file(playback.version_id)
            .await?
        else {
            return Ok(None);
        };
        let transcode = in_sidecar_charset(transcode).await;
        let fallback = match fallback {
            Some(fallback) => Some(super::Fallback {
                transcode: in_sidecar_charset(fallback.transcode).await,
                ..fallback
            }),
            None => None,
        };
        let timeline = Timeline::transcoded(duration_ms);
        let cuts = timeline.keyframes.clone();
        let ffmpeg = Arc::clone(&self.inner.ffmpeg);
        let subtitles = self.inner.subtitles.clone();
        let start = move |attempt: Transcode| -> Box<Start> {
            let (ffmpeg, subtitles) = (Arc::clone(&ffmpeg), subtitles.clone());
            let (path, cuts) = (path.clone(), cuts.clone());
            Box::new(move |dir: &Path, from: Keyframe, stop| {
                let cuts = cuts.get(from.n..).unwrap_or_default();
                let from = FromCut { n: from.n, cuts };
                // Asked each run, so a seek once the cache is filled skips reading the whole file.
                let attempt = subtitles.burning_in(file, &attempt);
                let child = ffmpeg.transcode(&path, &attempt, dir, from)?.spawn()?;
                Ok(process::run(child, stop))
            })
        };
        // A plan has a fallback only when it Transcodes on hardware.
        let fallback = fallback.zip(transcode.video.hardware.clone()).map(
            |(
                super::Fallback {
                    transcode: software,
                    reasons,
                },
                hardware,
            )| FallbackRun {
                start: start(software),
                reasons: reasons.iter().map(reason_to_proto).collect(),
                hardware,
            },
        );
        let runs = Runs {
            method: Method::Transcode,
            cpu_bound: transcode.video.cpu_bound(),
            start: start(transcode),
            fallback: None,
        };
        let id = (self.start_hls(playback, timeline, start_ms, runs, fallback)).await?;
        Ok(Some((id, playlist_url(id))))
    }

    /// Keeps a plan playing `playback` that runs nothing, a Direct Play, and returns its id.
    fn direct_play(&self, mut playback: Playback) -> u64 {
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        playback.plan_id = id;
        self.insert(id, playback, None, None);
        id
    }

    /// Starts a plan playing `playback`, cut as `timeline` says, whose files `runs` starts
    /// ffmpeg writing from the segment playing at `start_ms`, going on with `fallback` once a run
    /// on hardware fails, and returns the plan's id once ffmpeg wrote its first file or is still
    /// going after a wait. [`StartError::FailedOnServer`], its folder deleted, if it stopped first.
    #[expect(
        clippy::too_many_arguments,
        reason = "each is a different part of the plan, and `start_ms` only says where its first \
                  run starts"
    )]
    async fn start_hls(
        &self,
        mut playback: Playback,
        timeline: Timeline,
        start_ms: i64,
        mut runs: Runs,
        fallback: Option<FallbackRun>,
    ) -> Result<u64, StartError> {
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        playback.plan_id = id;
        let folder = self.inner.settings.path();
        let dir = folder.join(format!("{PLAN_FOLDER}{id}"));
        // Ids start again with each run, and the sweep at startup misses a segment folder an
        // earlier run that didn't shut down cleanly had set but this one doesn't, so its segments
        // may be there.
        let made = match tokio::fs::remove_dir_all(&dir).await {
            Err(err) if err.kind() != io::ErrorKind::NotFound => Err(err),
            _ => tokio::fs::create_dir_all(&dir).await,
        };
        made.map_err(|source| StartError::SegmentFolder { folder, source })?;
        let on_hardware = fallback.as_ref().map(|fallback| OnHardware {
            hardware: fallback.hardware.clone(),
            fallback_reasons: fallback.reasons.clone(),
        });
        runs.fallback = fallback.map(|FallbackRun { start, reasons, .. }| Fallback {
            start,
            on_fall_back: self.telling_fall_back(
                id,
                Playback {
                    reasons,
                    ..playback.clone()
                },
            ),
        });
        let keep_behind_ms = self.inner.settings.keep_behind_ms();
        let started = Running::start(id, timeline, dir.clone(), start_ms, runs, keep_behind_ms);
        let running = match started.await {
            Ok(running) => running,
            Err(err) => {
                // No `Running` was built, so its `stop` won't delete the folder.
                if let Err(delete) = tokio::fs::remove_dir_all(&dir).await {
                    tracing::warn!(plan = id, "could not delete {}: {delete}", dir.display());
                }
                return Err(StartError::Run(err));
            }
        };
        if running.failed_at_start().await {
            running.stop().await;
            return Err(StartError::FailedOnServer(running.method()));
        }
        self.insert(id, playback, Some(running), on_hardware);
        Ok(id)
    }

    /// What the plan `id` calls as it falls back to software, playing as `changed` from then: it
    /// publishes `PlaybackChanged` once the plan is kept, as `PlaybackStarted` says it before.
    fn telling_fall_back(&self, id: u64, changed: Playback) -> Box<dyn Fn() + Send + Sync> {
        let inner = Arc::downgrade(&self.inner);
        let changed = event(PlaybackChanged {
            playback: changed.into(),
            ..Default::default()
        });
        Box::new(move || {
            let Some(inner) = inner.upgrade() else { return };
            // Under the lock `insert` publishes under, so it can't come before `PlaybackStarted`.
            // ponytail: a fallback landing just as the plan is kept is told by both; tell it once
            // if an Observer ever minds.
            let plans = lock(&inner.plans);
            if plans.contains_key(&id) {
                inner.events.publish(vec![changed.clone()]);
            }
        })
    }

    /// Adds the plan `id` playing `playback`, with `hls` if it runs ffmpeg and `on_hardware` if
    /// it Transcodes on hardware, and publishes its `PlaybackStarted`, as it plays now.
    fn insert(
        &self,
        id: u64,
        playback: Playback,
        hls: Option<Arc<Running>>,
        on_hardware: Option<OnHardware>,
    ) {
        let to_device = (playback.version_id, playback.device_id);
        let plan = Plan {
            playback,
            hls,
            on_hardware,
            last_used: Mutex::new(Instant::now()),
            progress: Mutex::default(),
        };
        // `plans` before `stopped`, as in `stop_by_admin`, so a stop can't land between lifting it
        // and adding the plan.
        let mut plans = lock(&self.inner.plans);
        let mut stopped = lock(&self.inner.stopped);
        // Planning it again lifts an Admin's stop: a stop isn't a ban. A token not cancelled stays,
        // since responses hold its children and a later stop must still reach them.
        if stopped
            .get(&to_device)
            .is_some_and(CancellationToken::is_cancelled)
        {
            stopped.remove(&to_device);
        }
        let started = PlaybackStarted {
            playback: plan.playback_now().into(),
            ..Default::default()
        };
        plans.insert(id, Arc::new(plan));
        self.inner.events.publish(vec![event(started)]);
        drop((stopped, plans));
    }

    /// The plan `id`, if it's running for the Device `device`, marked asked for now, which keeps
    /// it running another [`STOP_UNUSED_AFTER`].
    fn ask(&self, id: u64, device: i64) -> Option<Arc<Plan>> {
        // Marked under the lock `stop_unused` takes plans out under, so a plan found here isn't
        // stopped while it's served.
        let plans = lock(&self.inner.plans);
        let plan = (plans.get(&id)).filter(|plan| plan.playback.device_id == device)?;
        *lock(&plan.last_used) = Instant::now();
        Some(Arc::clone(plan))
    }

    /// Marks the Device `device`'s Direct Plays of the Version `version` asked for now, which
    /// keeps them another [`STOP_UNUSED_AFTER`], and says whether it has any.
    pub(crate) fn ask_direct_plays(&self, version: i64, device: i64) -> bool {
        let plans = lock(&self.inner.plans);
        let direct_plays = plans.values().filter(|plan| {
            plan.hls.is_none()
                && plan.playback.version_id == version
                && plan.playback.device_id == device
        });
        let mut any = false;
        for plan in direct_plays {
            *lock(&plan.last_used) = Instant::now();
            any = true;
        }
        any
    }

    /// `file`, the parts of the Version `version`'s file being sent to the Device `device`, which
    /// marks that Device's Direct Plays of it asked for with each part, so they're kept while it's
    /// sent. With no Device it marks nothing.
    pub(super) fn ask_direct_plays_while_sending<S: Stream>(
        &self,
        version: i64,
        device: Option<i64>,
        file: S,
    ) -> impl Stream<Item = S::Item> + use<S> {
        let plans = self.clone();
        file.inspect(move |_| {
            if let Some(device) = device {
                plans.ask_direct_plays(version, device);
            }
        })
    }

    /// Stops the plan `id`, if it's the Device `device`'s, and deletes its files.
    pub(crate) async fn stop_for(&self, id: u64, device: i64) {
        let plan = {
            let mut plans = lock(&self.inner.plans);
            let theirs = (plans.get(&id)).is_some_and(|plan| plan.playback.device_id == device);
            if theirs { plans.remove(&id) } else { None }
        };
        if let Some(plan) = plan {
            self.stop(&plan).await;
        }
    }

    /// How many plans play one of `versions`.
    pub(crate) fn playing(&self, versions: &[i64]) -> usize {
        (lock(&self.inner.plans).values())
            .filter(|plan| versions.contains(&plan.playback.version_id))
            .count()
    }

    /// Stops every plan playing one of `versions`, whose files are about to be deleted, and cuts
    /// off each response serving one of them, until the [`Deleting`] it returns is dropped.
    // ponytail: a Remux or Transcode already starting as this begins can still start on a file
    // about to go, and play on until it's unused; check again as it's kept if that ever shows.
    pub(crate) async fn end_for_deleting(&self, versions: &[i64]) -> Deleting {
        {
            let mut served = lock(&self.inner.served);
            for &version in versions {
                served.entry(version).or_default().cancel();
            }
        }
        let ending: Vec<(u64, Arc<Plan>)> = lock(&self.inner.plans)
            .extract_if(|_, plan| versions.contains(&plan.playback.version_id))
            .collect();
        for (_, plan) in ending {
            self.stop(&plan).await;
        }
        Deleting {
            plans: self.clone(),
            versions: versions.to_vec(),
        }
    }

    /// Whether the Version `version` is being deleted.
    fn deleting(&self, version: i64) -> bool {
        (lock(&self.inner.served).get(&version)).is_some_and(CancellationToken::is_cancelled)
    }

    /// Cancelled once the Version `version` is being deleted, to cut off a response serving its
    /// file.
    pub(crate) fn served(&self, version: i64) -> CancellationToken {
        let mut served = lock(&self.inner.served);
        served.entry(version).or_default().child_token()
    }

    /// Cancelled once an Admin stops the Device `device`'s last plan of the Version `version`, to
    /// cut off a response serving its file, and at once while that stop holds. Never cancelled
    /// without a Device.
    pub(crate) fn stopped(&self, version: i64, device: Option<i64>) -> CancellationToken {
        let Some(device) = device else {
            return CancellationToken::new();
        };
        let mut stopped = lock(&self.inner.stopped);
        stopped.entry((version, device)).or_default().child_token()
    }

    /// Stops every plan not asked for in the last `unused_for`, and deletes its files.
    async fn stop_unused(&self, unused_for: Duration) {
        let unused: Vec<(u64, Arc<Plan>)> = lock(&self.inner.plans)
            .extract_if(|_, plan| lock(&plan.last_used).elapsed() >= unused_for)
            .collect();
        for (_, plan) in unused {
            self.stop(&plan).await;
        }
    }

    /// Deletes each running plan's files from well behind or far ahead of playback, as
    /// [`Running::delete_unkept`] says.
    async fn delete_unkept_files(&self) {
        let running: Vec<(u64, Arc<Running>)> = (lock(&self.inner.plans).iter())
            .filter_map(|(&id, plan)| Some((id, plan.hls.clone()?)))
            .collect();
        let keep_behind_ms = self.inner.settings.keep_behind_ms();
        for (id, hls) in running {
            if let Err(err) = hls.delete_unkept(keep_behind_ms).await {
                tracing::warn!(
                    plan = id,
                    "could not delete files behind or ahead of playback: {err}"
                );
            }
        }
    }

    /// As the Server shuts down, stops its background tasks and waits for them, so none holds
    /// the database still, then stops every plan and deletes its files, so none is left in a
    /// segment folder the next run's sweep may not know of.
    pub(crate) async fn shutdown(&self) {
        self.inner.cancel.cancel();
        self.inner.tasks.close();
        self.inner.tasks.wait().await;
        let plans: Vec<Arc<Plan>> = lock(&self.inner.plans)
            .drain()
            .map(|(_, plan)| plan)
            .collect();
        for plan in plans {
            self.stop(&plan).await;
        }
    }

    /// Stops `plan`'s ffmpeg, if it runs one, deletes its files and publishes that its playback
    /// ended.
    async fn stop(&self, plan: &Plan) {
        self.stop_telling(plan, None).await;
    }

    /// As [`Self::stop`], publishing `admin_stop` too if an Admin stopped it.
    async fn stop_telling(&self, plan: &Plan, admin_stop: Option<AdminStop>) {
        if let Some(hls) = &plan.hls {
            hls.stop().await;
        }
        let ended = PlaybackEnded {
            playback: plan.playback_now().into(),
            admin_stop: admin_stop.into(),
            ..Default::default()
        };
        self.inner.events.publish(vec![event(ended)]);
    }
}

impl Plan {
    /// How it plays now: as it started, with its fallback's reasons if it fell back.
    fn playback_now(&self) -> Playback {
        let mut playback = self.playback.clone();
        if let Some(on_hardware) = &self.on_hardware
            && self.fell_back()
        {
            playback.reasons.clone_from(&on_hardware.fallback_reasons);
        }
        playback
    }

    /// The hardware it Transcodes on now; `None` in software, or for a Direct Play or a Remux.
    fn hardware_now(&self) -> Option<&Hardware> {
        (self.on_hardware.as_ref())
            .filter(|_| !self.fell_back())
            .map(|on_hardware| &on_hardware.hardware)
    }

    fn fell_back(&self) -> bool {
        self.hls.as_ref().is_some_and(|hls| hls.fell_back())
    }
}

/// `transcode`, reading the sidecar it burns in, if it burns one in, in the charset it's in.
/// Found here, once for the plan, rather than as each run starts, which each seek does.
async fn in_sidecar_charset(mut transcode: Transcode) -> Transcode {
    if let Some(BurnIn::Text {
        subtitle: TextSubtitle::Sidecar { path, charset },
        ..
    }) = &mut transcode.video.burn_in
    {
        *charset = sidecar_charset(path).await;
    }
    transcode
}

/// What Observers are told of `caller` playing by `plan`.
fn playback(caller: &Caller, plan: &super::Plan) -> Playback {
    Playback {
        user_id: caller.user.id,
        device_id: caller.device.id,
        version_id: plan.version,
        method: method_to_proto(plan.method()).into(),
        tracks: plan.tracks.iter().map(track_to_proto).collect(),
        reasons: plan.reasons.iter().map(reason_to_proto).collect(),
        ..Default::default()
    }
}

fn playlist_url(plan: u64) -> String {
    format!("/media/plans/{plan}/{PLAYLIST}")
}

/// Locks `mutex`, which no code panics while holding.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
#[path = "running/running_tests.rs"]
mod tests;
