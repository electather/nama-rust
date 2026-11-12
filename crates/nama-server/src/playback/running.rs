//! Playable plans, from when they're planned until they stop, and the ffmpeg of those that run it:
//! Remuxes and Transcodes, played over HLS with fMP4 segments. Each plan's start and end are
//! published as `PlaybackStarted` and `PlaybackEnded`.
//!
//! A plan belongs to the Device it was planned for, and only that Device is served its files, at
//! `/media/plans/<id>/`: the playlist `index.m3u8`, then `init.mp4` and the segments `<n>.m4s`.
//! The playlist lists the whole Version up front, cut at keyframes where it will be cut, so HLS
//! time is the file's time and Watch State needs no conversion. ffmpeg writes a file per keyframe
//! to `nama-plan-<id>/` in the segment folder, and a segment joins its keyframes' files. A Remux
//! keeps the Version's own keyframes; a Transcode forces one every [`SEGMENT_MS`], so each of its
//! segments is one file.
//!
//! A segment asked for waits for ffmpeg to write its files, and one whose files have gone since
//! waits for them to be written again. One far past what ffmpeg has written restarts ffmpeg at it,
//! and ffmpeg stops once it's [`RUN_AHEAD_MS`] past the last segment asked for, to start again at
//! the next file asked for that it hasn't written. Every [`TIDY_EVERY`], files ending further
//! before the last segment asked for than the Playback settings keep for skipping back are
//! deleted, as are earlier runs' init files, so a plan's folder stops growing; one asked for again
//! is written again.
//!
//! A Transcode on hardware that fails carries on in software from the first file it hadn't
//! written, takes its software fallback's reasons and publishes `PlaybackChanged`.
//!
//! A plan is stopped, and its folder deleted, when its player says it stopped, [`STOP_UNUSED_AFTER`]
//! after it was last asked for, or as the Server shuts down. A Direct Play is asked for through
//! its file at `/media/versions/<id>`, which marks every Direct Play of that Version on the Device
//! asked for. [`sweep_leftovers`] deletes the folders of plans a run left behind when the next
//! starts.
//!
//! Deleting a Version ends its plans first and cuts off each response serving its file
//! ([`RunningPlans::end_for_deleting`]): a file open for reading can still be read once it's
//! deleted.

use std::{
    collections::HashMap,
    fmt::{self, Write as _},
    io,
    ops::Range,
    path::{Path, PathBuf},
    process::Output,
    sync::{
        Arc, Mutex, PoisonError, Weak,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use axum::{
    Router,
    extract::{Path as UrlPath, Request, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use nama_proto::nama::v1::{
    Playback, PlaybackChanged, PlaybackEnded, PlaybackReason, PlaybackStarted,
};
use tokio::{process::Child, task::JoinHandle};
use tokio_util::{sync::CancellationToken, task::TaskTracker};

use super::{Method, Playing, StartedPlan, direct_play_url, method_to_proto, reason_to_proto};
use crate::{
    contents::Contents,
    events::{EventBus, event},
    ffmpeg::{Ffmpeg, FromCut, Keyframe, Remux, Transcode, hls_init, hls_init_from},
    probing::{self, ProbeError, Probing},
    server_settings::PlaybackSettings,
    tracks::track_to_proto,
    users::Caller,
};

const PLAYLIST: &str = "index.m3u8";
const INIT: &str = "init.mp4";

/// How long segments aim to be, in milliseconds.
const SEGMENT_MS: i64 = 6000;

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
    /// Only a Transcode would play it, and the Server runs as many as it may at once, this many.
    #[error("the Server is running as many Transcodes as it may at once, {0}")]
    Busy(u32),
}

/// Starts ffmpeg writing a plan's files, from a keyframe on, to the folder it's given.
type Run = dyn Fn(&Path, Keyframe) -> io::Result<Child> + Send + Sync;

/// What starts ffmpeg writing a plan's files: `run`, or once a run on hardware fails, `fallback`.
struct Runs {
    run: Box<Run>,
    fallback: Option<FallbackRun>,
}

/// What a Transcode on hardware runs instead once a run there fails.
struct FallbackRun {
    run: Box<Run>,
    /// Its plan's reasons from then on, which say why.
    reasons: Vec<PlaybackReason>,
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
    events: EventBus,
    next_id: AtomicU64,
    plans: Mutex<HashMap<u64, Arc<Plan>>>,
    /// [`tidy_plans`], so shutting down can wait for it.
    tasks: TaskTracker,
    /// Cancelled when the Server shuts down.
    cancel: CancellationToken,
    /// By Version, cancelled while it's deleted, which cuts off each response serving its file.
    // ponytail: one kept per Version served since the Server started; drop each once nothing
    // serves it if that ever adds up.
    served: Mutex<HashMap<i64, CancellationToken>>,
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
    /// When the plan was last asked for anything, which keeps it another [`STOP_UNUSED_AFTER`].
    last_used: Mutex<Instant>,
}

/// A plan's HLS files, and the ffmpeg writing them.
struct Running {
    id: u64,
    /// How far ahead a request waits for ffmpeg, and so the Transcode cap leaves Remuxes out.
    method: Method,
    timeline: Timeline,
    dir: PathBuf,
    run: Box<Run>,
    /// The last segment asked for, by number.
    last_asked: AtomicUsize,
    /// Whether the plan was stopped, so no ffmpeg is started again for requests still waiting.
    closed: AtomicBool,
    /// For a Transcode on hardware, what it runs once a run there fails.
    fallback: Option<FallbackRun>,
    /// Whether a run on hardware failed, so `fallback` runs from then on.
    fell_back: AtomicBool,
    /// Tells Observers how the plan plays now, once it fell back.
    events: EventBus,
    /// As it started, for telling Observers how it plays once it fell back.
    playback: Playback,
    /// The run of ffmpeg writing its files, and its task, which stops ffmpeg when aborted.
    ffmpeg: tokio::sync::Mutex<Option<(Arc<FfmpegRun>, JoinHandle<()>)>>,
}

/// One run of ffmpeg, from one keyframe on.
struct FfmpegRun {
    /// The keyframe it started at, by number.
    from: usize,
    /// The first keyframe from `from` on it hadn't written, as last looked. A file behind it that's
    /// gone since doesn't move it back.
    written_to: AtomicUsize,
    /// Whether ffmpeg has stopped, so a file not written yet never will be by this run.
    stopped: AtomicBool,
}

/// Where a Version's keyframes fall, and the segments its playlist lists, made of them.
struct Timeline {
    /// When each keyframe falls, in milliseconds, in order.
    keyframes: Vec<i64>,
    segments: Vec<Segment>,
}

/// One segment a playlist lists.
struct Segment {
    /// The keyframe it starts at, by number.
    first_keyframe: usize,
    /// How long it lasts, in milliseconds.
    ms: i64,
}

impl RunningPlans {
    /// Runs plans with `ffmpeg`, cutting a Remux at the keyframes `probing` lists, writing their
    /// files to the segment folder in `settings` as it is when each starts, and publishing on
    /// `events` when each stops. Until these are dropped or shut down, stops the plans gone unused
    /// in the background.
    pub(crate) fn new(
        settings: PlaybackSettings,
        ffmpeg: Arc<Ffmpeg>,
        contents: Contents,
        probing: Probing,
        events: EventBus,
    ) -> Self {
        let inner = Arc::new(Inner {
            settings,
            ffmpeg,
            contents,
            probing,
            events,
            // From 1, as the API's plan id 0 is no plan.
            next_id: AtomicU64::new(1),
            plans: Mutex::default(),
            tasks: TaskTracker::new(),
            cancel: CancellationToken::new(),
            served: Mutex::default(),
        });
        inner
            .tasks
            .spawn(tidy_plans(Arc::downgrade(&inner), inner.cancel.clone()));
        Self { inner }
    }

    /// The most Transcodes the Server may run at once, `max`, if that many already run, leaving
    /// out one of `device`'s own: the plan it asks for replaces the one it plays, which it stops
    /// only once the new one plays.
    // ponytail: the plan isn't kept until after probing, the folder and spawning ffmpeg, so every
    // Device planning in that window finds room and they can pass the cap together until
    // Transcodes end; reserve room under the plans' lock if that ever matters.
    pub(crate) fn transcodes_full(&self, device: i64, max: u32) -> Option<u32> {
        let plans = lock(&self.inner.plans);
        let (own, others): (Vec<_>, Vec<_>) = (plans.values())
            .filter(|plan| (plan.hls.as_ref()).is_some_and(|hls| hls.method == Method::Transcode))
            .partition(|plan| plan.playback.device_id == device);
        let running = others.len() + own.len().saturating_sub(1);
        (u32::try_from(running).unwrap_or(u32::MAX) >= max).then_some(max)
    }

    /// Starts `plan` for `caller`'s Device, telling Observers so, and returns it with its id and
    /// where to play it from. A Remux or a Transcode runs ffmpeg over the Version's file. `None`
    /// if there is no such Version, it's being deleted, or `plan` can't be played.
    pub(crate) async fn start(
        &self,
        caller: &Caller,
        plan: super::Plan,
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
                self.remux(playback, remux, duration_ms).await?
            }
            Playing::Transcode {
                transcode,
                duration_ms,
                fallback,
            } => {
                self.transcode(playback, transcode, duration_ms, fallback)
                    .await?
            }
        };
        Ok(started.map(|(id, url)| StartedPlan { id, url, plan }))
    }

    /// Starts Remuxing the Version `playback` plays, `duration_ms` long, for the Device it plays
    /// on, converting what `remux` says, and returns its id and its playlist's URL. `None` if
    /// there is no such Version.
    async fn remux(
        &self,
        playback: Playback,
        remux: Remux,
        duration_ms: i64,
    ) -> Result<Option<(u64, String)>, StartError> {
        let version = playback.version_id;
        let Some((path, _)) = self.inner.contents.version_file(version).await? else {
            return Ok(None);
        };
        let keyframes = match self.inner.probing.keyframes(version).await {
            Ok(keyframes) => keyframes,
            Err(probing::Error::Probe(err)) => return Err(StartError::Keyframes(err)),
            Err(probing::Error::NoSuchVersion(_)) => return Ok(None),
            Err(probing::Error::Database(err)) => return Err(err.into()),
        };
        let timeline = Timeline::new(keyframes, duration_ms);
        let ffmpeg = Arc::clone(&self.inner.ffmpeg);
        let run = move |dir: &Path, from| ffmpeg.remux(&path, &remux, dir, from)?.spawn();
        let runs = Runs {
            run: Box::new(run),
            fallback: None,
        };
        let id = self
            .start_hls(playback, Method::Remux, timeline, runs)
            .await?;
        Ok(Some((id, playlist_url(id))))
    }

    /// Starts Transcoding the Version `playback` plays, `duration_ms` long, for the Device it
    /// plays on, converting what `transcode` says, and returns its id and its playlist's URL. Its
    /// segments are [`SEGMENT_MS`] each, as it forces a keyframe at each. Once a run on hardware
    /// fails, it goes on with `fallback`. `None` if there is no such Version.
    async fn transcode(
        &self,
        playback: Playback,
        transcode: Transcode,
        duration_ms: i64,
        fallback: Option<super::Fallback>,
    ) -> Result<Option<(u64, String)>, StartError> {
        let Some((path, _)) = (self.inner.contents)
            .version_file(playback.version_id)
            .await?
        else {
            return Ok(None);
        };
        let timeline = Timeline::transcoded(duration_ms);
        let cuts = timeline.keyframes.clone();
        let ffmpeg = Arc::clone(&self.inner.ffmpeg);
        let run = move |attempt: Transcode| -> Box<Run> {
            let (ffmpeg, path, cuts) = (Arc::clone(&ffmpeg), path.clone(), cuts.clone());
            Box::new(move |dir: &Path, from: Keyframe| {
                let cuts = cuts.get(from.n..).unwrap_or_default();
                let from = FromCut { n: from.n, cuts };
                ffmpeg.transcode(&path, &attempt, dir, from)?.spawn()
            })
        };
        let fallback = fallback.map(
            |super::Fallback {
                 transcode: software,
                 reasons,
             }| FallbackRun {
                run: run(software),
                reasons: reasons.iter().map(reason_to_proto).collect(),
            },
        );
        let runs = Runs {
            run: run(transcode),
            fallback,
        };
        let id = self
            .start_hls(playback, Method::Transcode, timeline, runs)
            .await?;
        Ok(Some((id, playlist_url(id))))
    }

    /// Keeps a plan playing `playback` that runs nothing, a Direct Play, and returns its id.
    fn direct_play(&self, playback: Playback) -> u64 {
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        self.insert(id, playback, None);
        id
    }

    /// Starts a plan playing `playback`, cut as `timeline` says, whose files `runs` starts
    /// ffmpeg writing, and returns the plan's id.
    async fn start_hls(
        &self,
        playback: Playback,
        method: Method,
        timeline: Timeline,
        Runs { run, fallback }: Runs,
    ) -> io::Result<u64> {
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let dir = self
            .inner
            .settings
            .path()
            .join(format!("{PLAN_FOLDER}{id}"));
        // Ids start again with each run, and the sweep at startup misses a segment folder an
        // earlier run that didn't shut down cleanly had set but this one doesn't, so its segments
        // may be there.
        match tokio::fs::remove_dir_all(&dir).await {
            Err(err) if err.kind() != io::ErrorKind::NotFound => return Err(err),
            _ => {}
        }
        tokio::fs::create_dir_all(&dir).await?;
        let running = Arc::new(Running {
            id,
            method,
            timeline,
            dir,
            run,
            last_asked: AtomicUsize::new(0),
            closed: AtomicBool::new(false),
            fallback,
            fell_back: AtomicBool::new(false),
            events: self.inner.events.clone(),
            playback: playback.clone(),
            ffmpeg: tokio::sync::Mutex::default(),
        });
        *running.ffmpeg.lock().await = Some(running.run_from(0)?);
        self.insert(id, playback, Some(running));
        Ok(id)
    }

    /// Adds the plan `id` playing `playback`, with `hls` if it runs ffmpeg, and publishes its
    /// `PlaybackStarted`.
    fn insert(&self, id: u64, playback: Playback, hls: Option<Arc<Running>>) {
        let started = PlaybackStarted {
            playback: playback.clone().into(),
            ..Default::default()
        };
        let plan = Plan {
            playback,
            hls,
            last_used: Mutex::new(Instant::now()),
        };
        lock(&self.inner.plans).insert(id, Arc::new(plan));
        self.inner.events.publish(vec![event(started)]);
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
    /// keeps them another [`STOP_UNUSED_AFTER`].
    pub(crate) fn ask_direct_plays(&self, version: i64, device: i64) {
        let plans = lock(&self.inner.plans);
        let direct_plays = plans.values().filter(|plan| {
            plan.hls.is_none()
                && plan.playback.version_id == version
                && plan.playback.device_id == device
        });
        for plan in direct_plays {
            *lock(&plan.last_used) = Instant::now();
        }
    }

    /// Stops the plan `id`, if it's the Device `device`'s, and deletes its files.
    pub(crate) async fn stop_for(&self, id: u64, device: i64) {
        let plan = {
            let mut plans = lock(&self.inner.plans);
            let theirs = (plans.get(&id)).is_some_and(|plan| plan.playback.device_id == device);
            if theirs { plans.remove(&id) } else { None }
        };
        if let Some(plan) = plan {
            self.stop(id, &plan).await;
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
        for (id, plan) in ending {
            self.stop(id, &plan).await;
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

    /// Stops every plan not asked for in the last `unused_for`, and deletes its files.
    async fn stop_unused(&self, unused_for: Duration) {
        let unused: Vec<(u64, Arc<Plan>)> = lock(&self.inner.plans)
            .extract_if(|_, plan| lock(&plan.last_used).elapsed() >= unused_for)
            .collect();
        for (id, plan) in unused {
            self.stop(id, &plan).await;
        }
    }

    /// Deletes each running plan's files from well behind playback, as [`Running::delete_behind`]
    /// says.
    async fn delete_behind_playback(&self) {
        let running: Vec<Arc<Running>> = (lock(&self.inner.plans).values())
            .filter_map(|plan| plan.hls.clone())
            .collect();
        let keep_behind_ms = self.inner.settings.keep_behind_ms();
        for hls in running {
            if let Err(err) = hls.delete_behind(keep_behind_ms).await {
                tracing::warn!(
                    plan = hls.id,
                    "could not delete files behind playback: {err}"
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
        let plans: Vec<(u64, Arc<Plan>)> = lock(&self.inner.plans).drain().collect();
        for (id, plan) in plans {
            self.stop(id, &plan).await;
        }
    }

    /// Stops the plan `id`'s ffmpeg, if it runs one, deletes its files and publishes that its
    /// playback ended.
    async fn stop(&self, id: u64, plan: &Plan) {
        if let Some(hls) = &plan.hls {
            hls.stop(id).await;
        }
        let playback = match &plan.hls {
            Some(hls) => hls.playback_now(),
            None => plan.playback.clone(),
        };
        let ended = PlaybackEnded {
            playback: playback.into(),
            ..Default::default()
        };
        self.inner.events.publish(vec![event(ended)]);
    }
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

/// Every [`TIDY_EVERY`], stops the plans unused for [`STOP_UNUSED_AFTER`] and deletes the running
/// ones' files from well behind playback, until `inner` is dropped or `cancel` is cancelled.
async fn tidy_plans(inner: Weak<Inner>, cancel: CancellationToken) {
    // Not at once, so a test's plans aren't tidied at whatever point its task first runs.
    let first = tokio::time::Instant::now() + TIDY_EVERY;
    let mut every = tokio::time::interval_at(first, TIDY_EVERY);
    loop {
        // Only between rounds: a plan a round took out isn't in `shutdown`'s drain, so ending
        // mid-round would leave its ffmpeg and files behind.
        if cancel.run_until_cancelled(every.tick()).await.is_none() {
            return;
        }
        let Some(inner) = inner.upgrade() else {
            return;
        };
        let plans = RunningPlans { inner };
        plans.stop_unused(STOP_UNUSED_AFTER).await;
        plans.delete_behind_playback().await;
    }
}

/// Deletes the plan folders an earlier run left in the segment folder in `settings` and the
/// default. Runs before any plan starts, whose folder it would delete too.
pub(crate) async fn sweep_leftovers(settings: &PlaybackSettings) {
    for path in settings.leftover_paths() {
        if let Err(err) = sweep(&path).await {
            tracing::warn!(
                "could not delete the segment files left in {}: {err}",
                path.display()
            );
        }
    }
}

/// Deletes the plan folders in `path`, going on past one it can't delete.
async fn sweep(path: &Path) -> io::Result<()> {
    let mut entries = match tokio::fs::read_dir(path).await {
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        entries => entries?,
    };
    while let Some(entry) = entries.next_entry().await? {
        if entry.file_name().to_string_lossy().starts_with(PLAN_FOLDER)
            && let Err(err) = tokio::fs::remove_dir_all(entry.path()).await
        {
            tracing::warn!("could not delete {}: {err}", entry.path().display());
        }
    }
    Ok(())
}

impl Timeline {
    /// A video with `keyframes`, in milliseconds, cut at the first keyframe at or after each
    /// multiple of [`SEGMENT_MS`] past the last cut's multiple. No cut falls at or after
    /// `duration_ms`, where the last segment ends, so none lasts 0 ms.
    fn new(keyframes: Vec<i64>, duration_ms: i64) -> Self {
        let mut segments = vec![];
        let (mut first_keyframe, mut start, mut cut_after) = (0, 0, SEGMENT_MS);
        for (n, &keyframe) in keyframes.iter().enumerate() {
            if keyframe >= cut_after && keyframe < duration_ms {
                let ms = keyframe - start;
                segments.push(Segment { first_keyframe, ms });
                (first_keyframe, start) = (n, keyframe);
                cut_after += SEGMENT_MS;
            }
        }
        let ms = duration_ms - start;
        segments.push(Segment { first_keyframe, ms });
        Self {
            keyframes,
            segments,
        }
    }

    /// A Transcode of a video `duration_ms` long, with a keyframe forced at each multiple of
    /// [`SEGMENT_MS`] before its end, so each is a segment's first.
    fn transcoded(duration_ms: i64) -> Self {
        let segments = (duration_ms + SEGMENT_MS - 1) / SEGMENT_MS;
        Self::new((0..segments).map(|n| n * SEGMENT_MS).collect(), duration_ms)
    }

    /// The keyframes whose files segment `n` joins, by number.
    fn keyframes_of(&self, n: usize) -> Range<usize> {
        let first = self
            .segments
            .get(n)
            .map_or(0, |segment| segment.first_keyframe);
        let next = self.segments.get(n + 1);
        first..next.map_or(self.keyframes.len(), |next| next.first_keyframe)
    }

    /// When keyframe `n` falls, in milliseconds, or the end of time for one past the last.
    fn keyframe_ms(&self, n: usize) -> i64 {
        self.keyframes.get(n).copied().unwrap_or(i64::MAX)
    }

    /// When segment `n` starts, in milliseconds.
    fn segment_start_ms(&self, n: usize) -> i64 {
        self.keyframe_ms(self.keyframes_of(n).start)
    }

    /// Keyframe `n`, for ffmpeg to start at.
    fn keyframe(&self, n: usize) -> Keyframe {
        let previous_ms = n
            .checked_sub(1)
            .and_then(|previous| self.keyframes.get(previous));
        Keyframe {
            n,
            ms: self.keyframes.get(n).copied().unwrap_or_default(),
            previous_ms: previous_ms.copied(),
        }
    }

    /// The HLS playlist listing segments `0.m4s`, `1.m4s` and so on.
    fn playlist(&self) -> String {
        let longest = self.segments.iter().map(|segment| segment.ms).max();
        let mut playlist = format!(
            "#EXTM3U\n#EXT-X-VERSION:7\n#EXT-X-TARGETDURATION:{}\n#EXT-X-PLAYLIST-TYPE:VOD\n\
             #EXT-X-MEDIA-SEQUENCE:0\n#EXT-X-INDEPENDENT-SEGMENTS\n#EXT-X-MAP:URI=\"{INIT}\"\n",
            // Whole seconds, rounded up, as HLS asks.
            (longest.unwrap_or_default() + 999) / 1000,
        );
        for (n, Segment { ms, .. }) in self.segments.iter().enumerate() {
            // Writing to a `String` never fails.
            let _ = write!(
                playlist,
                "#EXTINF:{}.{:03},\n{n}.m4s\n",
                ms / 1000,
                ms % 1000
            );
        }
        playlist.push_str("#EXT-X-ENDLIST\n");
        playlist
    }
}

impl Running {
    /// Stops the plan `id`'s ffmpeg and deletes its files.
    async fn stop(&self, id: u64) {
        // Under the lock a restart takes, so none starts ffmpeg after this.
        let mut ffmpeg = self.ffmpeg.lock().await;
        self.closed.store(true, Ordering::Release);
        if let Some((run, task)) = ffmpeg.take() {
            task.abort();
            // Awaited so ffmpeg is dropped, which kills it, before its folder is deleted.
            let _: Result<(), _> = task.await;
            run.stopped.store(true, Ordering::Release);
        }
        drop(ffmpeg);
        if let Err(err) = tokio::fs::remove_dir_all(&self.dir).await {
            tracing::warn!(plan = id, "could not delete {}: {err}", self.dir.display());
        }
    }

    /// Deletes the files of keyframes ending more than `keep_behind_ms` before the start of the
    /// last segment asked for, and the init files of every run but the current one. Files ahead
    /// of it, and those the current run may still be writing, are kept.
    async fn delete_behind(&self, keep_behind_ms: i64) -> io::Result<()> {
        // Under the lock a restart takes, so the current run is the one whose init is kept.
        let ffmpeg = self.ffmpeg.lock().await;
        // `stop` takes the run under it, so a stopped plan has none.
        let Some((run, _)) = ffmpeg.as_ref() else {
            return Ok(());
        };
        // A running run's walk in `note_written` stops at the first file gone, so what it may
        // write after it last looked is kept.
        let still_writing_from = if run.stopped.load(Ordering::Acquire) {
            usize::MAX
        } else {
            self.note_written(run).await
        };
        let asked = (self.timeline).segment_start_ms(self.last_asked.load(Ordering::Relaxed));
        let keep_from_ms = asked - keep_behind_ms;
        let mut entries = tokio::fs::read_dir(&self.dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let behind = match keyframe_number(&name) {
                Some(n) => {
                    n < still_writing_from && self.timeline.keyframe_ms(n + 1) < keep_from_ms
                }
                None => hls_init_from(&name).is_some_and(|from| from != run.from),
            };
            // Going on past one, which Windows refuses while a request reads it.
            if behind && let Err(err) = tokio::fs::remove_file(entry.path()).await {
                tracing::warn!(
                    plan = self.id,
                    "could not delete {}: {err}",
                    entry.path().display()
                );
            }
        }
        Ok(())
    }

    /// The number of the segment a request for `asked` gets, if the playlist lists it.
    fn segment_number(&self, asked: &str) -> Option<usize> {
        let n: usize = asked.strip_suffix(".m4s")?.parse().ok()?;
        (n < self.timeline.segments.len()).then_some(n)
    }

    /// Segment `n`, its keyframes' files joined, once ffmpeg has written them, or `None` if it
    /// stopped first.
    // ponytail: a segment is read whole into memory; stream it if big Versions strain memory.
    async fn segment(self: &Arc<Self>, n: usize) -> io::Result<Option<Vec<u8>>> {
        self.last_asked.store(n, Ordering::Relaxed);
        let mut segment = vec![];
        for keyframe in self.timeline.keyframes_of(n) {
            let Some(file) = self.keyframe_file(keyframe, n).await? else {
                return Ok(None);
            };
            segment.extend(file);
        }
        Ok(Some(segment))
    }

    /// Keyframe `n`'s file, for segment `segment`, once ffmpeg has written it, or `None` if it
    /// stopped first. One gone, even between being found written and read, is waited for as one
    /// not written yet. ffmpeg writes each under another name and renames it once whole
    /// (`-hls_flags temp_file`).
    async fn keyframe_file(
        self: &Arc<Self>,
        n: usize,
        segment: usize,
    ) -> io::Result<Option<Vec<u8>>> {
        loop {
            if let Some(file) = self.read_keyframe(n).await? {
                return Ok(Some(file));
            }
            if !self.will_write(n, segment).await? {
                // It may have been written just before ffmpeg stopped.
                return self.read_keyframe(n).await;
            }
            tokio::time::sleep(POLL_EVERY).await;
        }
    }

    /// Keyframe `n`'s file, or `None` if it isn't there.
    async fn read_keyframe(&self, n: usize) -> io::Result<Option<Vec<u8>>> {
        match tokio::fs::read(self.keyframe_path(n)).await {
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
            read => read.map(Some),
        }
    }

    /// The init segment, once the current run of ffmpeg has written it whole, as it has once it
    /// wrote its first file, or `None` if it stopped first.
    async fn init(&self) -> io::Result<Option<Vec<u8>>> {
        loop {
            let ffmpeg = self.ffmpeg.lock().await;
            let Some((run, _)) = ffmpeg.as_ref() else {
                return Ok(None);
            };
            // `stopped` first, as a run notes what it wrote last before it's marked so.
            let stopped = run.stopped.load(Ordering::Acquire);
            // Not whether its first file is there, which may be gone since.
            if run.written_to.load(Ordering::Relaxed) > run.from {
                // Read under the lock, as once a seek replaces this run a tick deletes its init.
                return tokio::fs::read(self.dir.join(hls_init(run.from)))
                    .await
                    .map(Some);
            }
            drop(ffmpeg);
            if stopped {
                return Ok(None);
            }
            tokio::time::sleep(POLL_EVERY).await;
        }
    }

    fn keyframe_path(&self, n: usize) -> PathBuf {
        self.dir.join(format!("{n}.m4s"))
    }

    /// How far past what ffmpeg has written a file asked for can be and still be waited for, in
    /// milliseconds. A software Transcode writes about as fast as it plays, so it waits only for
    /// the segment ffmpeg is on and the next.
    const fn wait_ahead_ms(&self) -> i64 {
        match self.method {
            Method::Remux | Method::DirectPlay => REMUX_WAIT_AHEAD_MS,
            Method::Transcode => SEGMENT_MS,
        }
    }

    /// Whether ffmpeg is on its way to keyframe `n`'s file, for segment `segment`, restarting it
    /// there unless it's at most [`Self::wait_ahead_ms`] short of it. `false` once a run started
    /// there has stopped without writing it, or once a later request asked for a segment more
    /// than that away, so ffmpeg isn't started over and over.
    async fn will_write(self: &Arc<Self>, n: usize, segment: usize) -> io::Result<bool> {
        let mut ffmpeg = self.ffmpeg.lock().await;
        if self.closed.load(Ordering::Acquire) {
            return Ok(false);
        }
        // Under the lock, since a run started at a written file would write its init again.
        if tokio::fs::try_exists(self.keyframe_path(n)).await? {
            return Ok(true);
        }
        if let Some((run, task)) = &*ffmpeg {
            let stopped = run.stopped.load(Ordering::Acquire);
            let written_to = run.written_to.load(Ordering::Relaxed);
            if stopped && run.from == n && written_to == n {
                return Ok(false);
            }
            // One it wrote that's gone since is restarted at, as it won't write it again.
            let short = (self.timeline.keyframe_ms(n))
                .saturating_sub(self.timeline.keyframe_ms(written_to));
            if !stopped && written_to <= n && short <= self.wait_ahead_ms() {
                return Ok(true);
            }
            // A player fetching the next segment alongside this one mustn't leave it behind.
            let last_asked = self.last_asked.load(Ordering::Relaxed);
            let apart = (self.timeline.segment_start_ms(last_asked))
                .abs_diff(self.timeline.segment_start_ms(segment));
            if apart > self.wait_ahead_ms().unsigned_abs() {
                return Ok(false);
            }
            task.abort();
            // An aborted `watch` never notes what it wrote last or marks it stopped, and the
            // restart below may fail, leaving it for `init`.
            self.note_written(run).await;
            run.stopped.store(true, Ordering::Release);
        }
        *ffmpeg = Some(self.run_from(n)?);
        Ok(true)
    }

    /// How it plays now: as it started, with its fallback's reasons if it fell back.
    fn playback_now(&self) -> Playback {
        let mut playback = self.playback.clone();
        if let Some(FallbackRun { reasons, .. }) =
            (self.fallback.as_ref()).filter(|_| self.fell_back.load(Ordering::Acquire))
        {
            playback.reasons.clone_from(reasons);
        }
        playback
    }

    /// Starts ffmpeg writing this plan's files from keyframe `n` on.
    fn run_from(self: &Arc<Self>, n: usize) -> io::Result<(Arc<FfmpegRun>, JoinHandle<()>)> {
        let start = match &self.fallback {
            Some(fallback) if self.fell_back.load(Ordering::Acquire) => &fallback.run,
            _ => &self.run,
        };
        let ffmpeg = start(&self.dir, self.timeline.keyframe(n))?;
        let run = Arc::new(FfmpegRun {
            from: n,
            written_to: AtomicUsize::new(n),
            stopped: AtomicBool::new(false),
        });
        let task = tokio::spawn(Arc::clone(self).watch(ffmpeg, Arc::clone(&run)));
        Ok((run, task))
    }

    /// Waits for `ffmpeg`, doing the run `run`, to stop, or stops it once it has written far
    /// enough ahead, then logs why if it failed and marks it stopped.
    async fn watch(self: Arc<Self>, ffmpeg: Child, run: Arc<FfmpegRun>) {
        let failed = tokio::select! {
            output = ffmpeg.wait_with_output() => log_failure(output, self.id),
            // Dropping the child stops ffmpeg.
            () = self.written_far_ahead(&run) => false,
        };
        // What it wrote since last looked, for its fallback, `will_write` and `init`, which reads
        // it once marked stopped, so before.
        self.note_written(&run).await;
        if failed {
            self.fall_back(&run).await;
        }
        run.stopped.store(true, Ordering::Release);
    }

    /// Once `run`, the plan's current run of ffmpeg, failed on hardware, starts its fallback from
    /// the first file it hadn't written, and tells Observers so. Only the first failure does.
    async fn fall_back(self: &Arc<Self>, run: &Arc<FfmpegRun>) {
        if self.fallback.is_none() {
            return;
        }
        // Before waiting for the lock, so a seek that restarts ffmpeg meanwhile, aborting this
        // task, restarts it in software.
        if self.fell_back.swap(true, Ordering::AcqRel) {
            return;
        }
        tracing::warn!(
            plan = self.id,
            "hardware acceleration failed partway, so the rest Transcodes in software"
        );
        let changed = PlaybackChanged {
            playback: self.playback_now().into(),
            ..Default::default()
        };
        self.events.publish(vec![event(changed)]);
        // Under the lock a restart takes, so it doesn't race a seek's.
        let mut ffmpeg = self.ffmpeg.lock().await;
        let current = (ffmpeg.as_ref()).is_some_and(|(current, _)| Arc::ptr_eq(current, run));
        if !current || self.closed.load(Ordering::Acquire) {
            return;
        }
        let from = run.written_to.load(Ordering::Relaxed);
        if from < self.timeline.keyframes.len() {
            run.stopped.store(true, Ordering::Release);
            match self.run_from(from) {
                // Replaces this task's own handle, which leaves it to finish.
                Ok(fallback) => *ffmpeg = Some(fallback),
                Err(err) => tracing::warn!(plan = self.id, "could not start ffmpeg again: {err}"),
            }
        }
    }

    /// Returns once `run` has written [`RUN_AHEAD_MS`] past the start of the last segment asked
    /// for, or of where it started if later, keeping track of how far it has written meanwhile.
    async fn written_far_ahead(&self, run: &FfmpegRun) {
        loop {
            tokio::time::sleep(POLL_EVERY).await;
            let written_to = self.note_written(run).await;
            let asked = (self.timeline).segment_start_ms(self.last_asked.load(Ordering::Relaxed));
            // An earlier segment asked for just after a seek mustn't stop the run it started.
            let asked = asked.max(self.timeline.keyframe_ms(run.from));
            if self.timeline.keyframe_ms(written_to).saturating_sub(asked) >= RUN_AHEAD_MS {
                return;
            }
        }
    }

    /// Notes in `run` how far it has written, and returns it. Walks on from where it last looked,
    /// not from where it started, whose files may be gone since.
    // A file deleted before it's looked at stops the walk there, so `delete_behind` keeps those.
    async fn note_written(&self, run: &FfmpegRun) -> usize {
        let mut written_to = run.written_to.load(Ordering::Relaxed);
        while written_to < self.timeline.keyframes.len()
            && tokio::fs::try_exists(self.keyframe_path(written_to))
                .await
                .unwrap_or_default()
        {
            written_to += 1;
        }
        // The most, as the tick may walk alongside `watch` and delete what the slower walk is on.
        run.written_to
            .fetch_max(written_to, Ordering::Relaxed)
            .max(written_to)
    }
}

/// The keyframe whose file is named `name`, if it's one.
fn keyframe_number(name: &str) -> Option<usize> {
    name.strip_suffix(".m4s")?.parse().ok()
}

/// Logs why ffmpeg, running for the plan `plan`, failed, if it did, and says whether it did.
fn log_failure(output: io::Result<Output>, plan: u64) -> bool {
    match output {
        Ok(Output { status, .. }) if status.success() => false,
        Ok(Output { status, stderr, .. }) => {
            let stderr = String::from_utf8_lossy(&stderr);
            tracing::warn!(plan, "ffmpeg failed ({status}): {}", stderr.trim());
            true
        }
        Err(err) => {
            tracing::warn!(plan, "could not wait for ffmpeg: {err}");
            false
        }
    }
}

/// Serves `GET /media/plans/<id>/<file>`: the playlist and segments of the plan `id`, to the
/// Device it belongs to, and 404 to any other. Whether the caller has a session is up to the
/// layers the router is put behind.
pub(crate) fn router(plans: RunningPlans) -> Router {
    Router::new()
        .route("/media/plans/{id}/{file}", get(serve))
        .with_state(plans)
}

async fn serve(
    State(plans): State<RunningPlans>,
    UrlPath((id, file)): UrlPath<(u64, String)>,
    request: Request,
) -> Response {
    let device = request
        .extensions()
        .get::<Caller>()
        .map(|caller| caller.device.id);
    let Some(plan) = device.and_then(|device| plans.ask(id, device)) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(plan) = &plan.hls else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if file == PLAYLIST {
        let content_type = [(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")];
        return (content_type, plan.timeline.playlist()).into_response();
    }
    let written = if file == INIT {
        plan.init().await
    } else if let Some(n) = plan.segment_number(&file) {
        plan.segment(n).await
    } else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match written {
        Ok(Some(body)) => ([(header::CONTENT_TYPE, "video/mp4")], body).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            tracing::error!(plan = id, "could not serve {file}: {err}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Locks `mutex`, which no code panics while holding.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
#[path = "running_tests.rs"]
mod tests;
