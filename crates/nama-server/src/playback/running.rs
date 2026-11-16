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
//!
//! [`SEGMENT_MS`]: timeline::SEGMENT_MS

mod run;
mod serving;
mod sweeping;
mod timeline;

use std::{
    collections::HashMap,
    fmt, io,
    path::Path,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use nama_proto::nama::v1::{Playback, PlaybackEnded, PlaybackStarted};
use tokio_util::{sync::CancellationToken, task::TaskTracker};

use run::{FallbackRun, Run, Running, Runs};
pub(crate) use serving::router;
pub(crate) use sweeping::sweep_leftovers;
use sweeping::tidy_plans;
use timeline::Timeline;

use super::{
    Method, Playing, StartedPlan, direct_play_url, method_to_proto,
    probing::{self, ProbeError, Probing},
    reason_to_proto,
    tracks::track_to_proto,
};
use crate::{
    events::{EventBus, event},
    ffmpeg::{Ffmpeg, FromCut, Keyframe, Remux, Transcode},
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
    /// Only a Transcode would play it, and the Server runs as many as it may at once, this many.
    #[error("the Server is running as many Transcodes as it may at once, {0}")]
    Busy(u32),
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

    /// The Transcodes running, leaving out one of `device`'s own: the plan it asks for replaces
    /// the one it plays, which it stops only once the new one plays.
    // ponytail: the plan isn't kept until after probing, the folder and spawning ffmpeg, so every
    // Device planning in that window finds room and they can pass the cap together until
    // Transcodes end; reserve room under the plans' lock if that ever matters.
    pub(super) fn running_transcodes(&self, device: i64) -> RunningTranscodes {
        let plans = lock(&self.inner.plans);
        let (own, others): (Vec<_>, Vec<_>) = (plans.values())
            .filter_map(|plan| {
                let hls = (plan.hls.as_ref()).filter(|hls| hls.method == Method::Transcode)?;
                Some((plan.playback.device_id, hls.cpu_bound()))
            })
            .partition(|(id, _)| *id == device);
        let cpu_bound =
            |transcodes: &[(i64, bool)]| transcodes.iter().filter(|(_, cpu)| *cpu).count();
        let leaving_one_own_out = |of_own: usize, of_others: usize| {
            u32::try_from(of_others + of_own.saturating_sub(1)).unwrap_or(u32::MAX)
        };
        RunningTranscodes {
            all: leaving_one_own_out(own.len(), others.len()),
            cpu_bound: leaving_one_own_out(cpu_bound(&own), cpu_bound(&others)),
        }
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
            cpu_bound: false,
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
    ///
    /// [`SEGMENT_MS`]: timeline::SEGMENT_MS
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
            cpu_bound: transcode.video.cpu_bound(),
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
        Runs {
            run,
            fallback,
            cpu_bound,
        }: Runs,
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
            started_cpu_bound: cpu_bound,
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

/// Locks `mutex`, which no code panics while holding.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
#[path = "running_tests.rs"]
mod tests;
