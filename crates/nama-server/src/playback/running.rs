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
//! A segment asked for waits for ffmpeg to write its files. One far past what ffmpeg has written
//! restarts ffmpeg at it, and ffmpeg stops once it's [`RUN_AHEAD_MS`] past the last segment asked
//! for, to start again at the next file asked for that it hasn't written.
//!
//! A Transcode on hardware that fails carries on in software from the first file it hadn't
//! written, adds why to its plan's reasons and publishes `PlaybackChanged`.
//!
//! A plan is stopped, and its folder deleted, when its player says it stopped, [`STOP_UNUSED_AFTER`]
//! after it was last asked for, or as the Server shuts down. A Direct Play is asked for through
//! its file at `/media/versions/<id>`, which marks every Direct Play of that Version on the Device
//! asked for. [`sweep_leftovers`] deletes the folders of plans a run left behind when the next
//! starts.

use std::{
    collections::HashMap,
    fmt::Write as _,
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
use nama_proto::nama::v1::{Playback, PlaybackChanged, PlaybackEnded, PlaybackStarted};
use tokio::{process::Child, task::JoinHandle};

use super::{Method, Playing, StartedPlan, direct_play_url, method_to_proto};
use crate::{
    contents::Contents,
    events::{EventBus, event},
    ffmpeg::{
        Ffmpeg, FromCut, Hardware, Keyframe, ProbeError, Remux, ToneMapper, Transcode,
        VideoConversion, hls_init,
    },
    server_settings::PlaybackSettings,
    tracks::{Tracks, track_to_proto},
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

const CHECK_UNUSED_EVERY: Duration = Duration::from_secs(10);

/// How often a request for a file ffmpeg hasn't written yet looks for it again, and how often
/// ffmpeg's progress is looked at.
const POLL_EVERY: Duration = Duration::from_millis(100);

/// Why a plan couldn't start running.
#[derive(Debug, thiserror::Error)]
pub(crate) enum StartError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("the Version's keyframes couldn't be listed: {0}")]
    Keyframes(#[from] ProbeError),
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
    fallback: Option<Fallback>,
}

/// What a Transcode on hardware runs instead once a run there fails.
struct Fallback {
    run: Box<Run>,
    /// Why, as its plan's reasons then say.
    why: String,
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
    /// Whether `ffmpeg` has zscale, once it answered.
    zscale: tokio::sync::OnceCell<bool>,
    /// The hardware acceleration that passed its test, once tested.
    hardware: tokio::sync::OnceCell<Vec<Hardware>>,
    contents: Contents,
    tracks: Tracks,
    events: EventBus,
    next_id: AtomicU64,
    plans: Mutex<HashMap<u64, Arc<Plan>>>,
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
    fallback: Option<Fallback>,
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
    /// The first keyframe from `from` on whose file isn't written, as last looked.
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
    /// Runs plans with `ffmpeg`, writing their files to the segment folder in `settings` as it
    /// is when each starts, and publishing on `events` when each stops. Until these are dropped,
    /// stops the plans gone unused in the background.
    pub(crate) fn new(
        settings: PlaybackSettings,
        ffmpeg: Arc<Ffmpeg>,
        contents: Contents,
        tracks: Tracks,
        events: EventBus,
    ) -> Self {
        let inner = Arc::new(Inner {
            settings,
            ffmpeg,
            zscale: tokio::sync::OnceCell::new(),
            hardware: tokio::sync::OnceCell::new(),
            contents,
            tracks,
            events,
            // From 1, as the API's plan id 0 is no plan.
            next_id: AtomicU64::new(1),
            plans: Mutex::default(),
        });
        tokio::spawn(stop_unused_plans(Arc::downgrade(&inner)));
        let plans = Self { inner };
        // At startup, in the background, as it takes a few seconds; the first plan or status asked
        // for waits for it.
        let testing = plans.clone();
        tokio::spawn(async move {
            // Only for testing once; what was found is kept, and a task's output must be owned.
            let _: usize = testing.found_hardware().await.len();
        });
        plans
    }

    /// Whether plans can run at all (ADR 0026).
    pub(crate) fn can_run(&self) -> bool {
        self.inner.ffmpeg.can_remux()
    }

    /// The hardware acceleration that passed its test encode, tested the first time it's asked
    /// for, which the Server does as it starts, and kept.
    pub(crate) async fn found_hardware(&self) -> &[Hardware] {
        let ffmpeg = &self.inner.ffmpeg;
        self.inner
            .hardware
            .get_or_init(|| async {
                let found = ffmpeg.find_hardware().await;
                let names: Vec<&str> = found.iter().map(|found| found.accel.name()).collect();
                tracing::info!(hardware = ?names, "tested hardware acceleration");
                found
            })
            .await
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

    /// Whether a Transcode can tone-map HDR to SDR in software, as ffmpeg said the first time it
    /// answered.
    pub(crate) async fn can_tone_map(&self) -> bool {
        let ffmpeg = &self.inner.ffmpeg;
        // Only an answer is kept, so a check that timed out on a busy host is asked again.
        match self
            .inner
            .zscale
            .get_or_try_init(|| ffmpeg.has_zscale())
            .await
        {
            Ok(&zscale) => zscale,
            Err(err) => {
                tracing::warn!("could not ask ffmpeg whether it has zscale: {err}");
                false
            }
        }
    }

    /// Starts `plan` for `caller`'s Device, telling Observers so, and returns it with its id and
    /// where to play it from. A Remux or a Transcode runs ffmpeg over the Version's file, whose
    /// length is `duration_ms` if known. `None` if there is no such Version, or `plan` can't be
    /// played.
    pub(crate) async fn start(
        &self,
        caller: &Caller,
        plan: super::Plan,
        duration_ms: Option<i64>,
    ) -> Result<Option<StartedPlan>, StartError> {
        let Some(playing) = plan.playing else {
            return Ok(None);
        };
        let playback = playback(caller, &plan);
        let started = match playing {
            Playing::DirectPlay => {
                Some((self.direct_play(playback), direct_play_url(plan.version)))
            }
            Playing::Remux(remux) => self.remux(playback, remux, duration_ms).await?,
            Playing::Transcode(transcode) => {
                self.transcode(playback, transcode, duration_ms).await?
            }
        };
        Ok(started.map(|(id, url)| StartedPlan { id, url, plan }))
    }

    /// Starts Remuxing the Version `playback` plays, `duration_ms` long, for the Device it plays
    /// on, converting what `remux` says, and returns its id and its playlist's URL. Lists the
    /// Version's keyframes the first time and keeps them. `None` if there is no such Version.
    async fn remux(
        &self,
        playback: Playback,
        remux: Remux,
        duration_ms: Option<i64>,
    ) -> Result<Option<(u64, String)>, StartError> {
        let version = playback.version_id;
        let Some((path, _)) = self.inner.contents.version_file(version).await? else {
            return Ok(None);
        };
        let keyframes = self.keyframes(version, &path).await?;
        let timeline = Timeline::new(keyframes, duration_ms.unwrap_or_default());
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
    /// segments are [`SEGMENT_MS`] each, as it forces a keyframe at each. On hardware, it falls
    /// back to software, tone-mapping there only if [`Self::can_tone_map`]. `None` if there is no
    /// such Version.
    async fn transcode(
        &self,
        playback: Playback,
        transcode: Transcode,
        duration_ms: Option<i64>,
    ) -> Result<Option<(u64, String)>, StartError> {
        let Some((path, _)) = (self.inner.contents)
            .version_file(playback.version_id)
            .await?
        else {
            return Ok(None);
        };
        let duration_ms = duration_ms.unwrap_or_default();
        // At least one, so a Version whose length isn't known still has a segment.
        let segments = (duration_ms.max(1) + SEGMENT_MS - 1) / SEGMENT_MS;
        let cuts: Vec<i64> = (0..segments).map(|n| n * SEGMENT_MS).collect();
        let timeline = Timeline::new(cuts.clone(), duration_ms);
        let ffmpeg = Arc::clone(&self.inner.ffmpeg);
        let run = move |attempt: Transcode| -> Box<Run> {
            let (ffmpeg, path, cuts) = (Arc::clone(&ffmpeg), path.clone(), cuts.clone());
            Box::new(move |dir: &Path, from: Keyframe| {
                let cuts = cuts.get(from.n..).unwrap_or_default();
                let from = FromCut { n: from.n, cuts };
                ffmpeg.transcode(&path, &attempt, dir, from)?.spawn()
            })
        };
        let video = transcode.video;
        // Kept once answered, so this is the answer the plan was built with unless that check failed.
        let zscale =
            video.hardware.is_some() && video.tone_map.is_some() && self.can_tone_map().await;
        let fallback = video.hardware.map(|accel| {
            let software = VideoConversion {
                hardware: None,
                tone_map: (video.tone_map)
                    .filter(|_| zscale)
                    .map(|_| ToneMapper::Software),
                ..video
            };
            let untone_mapped = if software.tone_map.is_none() && video.tone_map.is_some() {
                ", without tone-mapping its HDR, as this Server's ffmpeg lacks zscale"
            } else {
                ""
            };
            Fallback {
                run: run(Transcode {
                    video: software,
                    ..transcode
                }),
                why: format!(
                    "{} failed partway, so the rest of it Transcodes in software{untone_mapped}",
                    accel.name()
                ),
            }
        });
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

    /// The Version `version`'s keyframes, as kept, or else listed from its file at `path` and
    /// kept for next time.
    async fn keyframes(&self, version: i64, path: &str) -> Result<Vec<i64>, StartError> {
        if let Some(keyframes) = self.inner.tracks.keyframes(version).await? {
            return Ok(keyframes);
        }
        let keyframes = self.inner.ffmpeg.keyframes(path).await?;
        self.inner
            .tracks
            .store_keyframes(version, &keyframes)
            .await?;
        Ok(keyframes)
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

    /// Stops every plan not asked for in the last `unused_for`, and deletes its files.
    async fn stop_unused(&self, unused_for: Duration) {
        let unused: Vec<(u64, Arc<Plan>)> = lock(&self.inner.plans)
            .extract_if(|_, plan| lock(&plan.last_used).elapsed() >= unused_for)
            .collect();
        for (id, plan) in unused {
            self.stop(id, &plan).await;
        }
    }

    /// Stops every plan and deletes its files, as the Server shuts down, so none is left in a
    /// segment folder the next run's sweep may not know of.
    pub(crate) async fn stop_all(&self) {
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
        reasons: plan.reasons.clone(),
        ..Default::default()
    }
}

fn playlist_url(plan: u64) -> String {
    format!("/media/plans/{plan}/{PLAYLIST}")
}

/// Every [`CHECK_UNUSED_EVERY`], stops the plans unused for [`STOP_UNUSED_AFTER`], until `inner`
/// is dropped.
async fn stop_unused_plans(inner: Weak<Inner>) {
    let mut every = tokio::time::interval(CHECK_UNUSED_EVERY);
    loop {
        every.tick().await;
        let Some(inner) = inner.upgrade() else {
            return;
        };
        RunningPlans { inner }.stop_unused(STOP_UNUSED_AFTER).await;
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
    /// multiple of [`SEGMENT_MS`] past the last cut's multiple. The last segment ends at
    /// `duration_ms`.
    fn new(keyframes: Vec<i64>, duration_ms: i64) -> Self {
        let mut segments = vec![];
        let (mut first_keyframe, mut start, mut cut_after) = (0, 0, SEGMENT_MS);
        for (n, &keyframe) in keyframes.iter().enumerate() {
            if keyframe >= cut_after {
                let ms = keyframe - start;
                segments.push(Segment { first_keyframe, ms });
                (first_keyframe, start) = (n, keyframe);
                cut_after += SEGMENT_MS;
            }
        }
        let ms = (duration_ms - start).max(0);
        segments.push(Segment { first_keyframe, ms });
        Self {
            keyframes,
            segments,
        }
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
            if !self.keyframe_written(keyframe, n).await? {
                return Ok(None);
            }
            segment.extend(tokio::fs::read(self.keyframe_path(keyframe)).await?);
        }
        Ok(Some(segment))
    }

    /// The init segment, once ffmpeg has written it whole, or `None` if it stopped first.
    async fn init(&self) -> io::Result<Option<Vec<u8>>> {
        loop {
            let Some((from, stopped)) = (self.ffmpeg.lock().await)
                .as_ref()
                .map(|(run, _)| (run.from, run.stopped.load(Ordering::Acquire)))
            else {
                return Ok(None);
            };
            // Read `stopped` before looking, so a file written just before ffmpeg stopped is
            // still found. A run is only started at a keyframe whose file isn't written.
            if tokio::fs::try_exists(self.keyframe_path(from)).await? {
                return tokio::fs::read(self.dir.join(hls_init(from)))
                    .await
                    .map(Some);
            }
            if stopped {
                return Ok(None);
            }
            tokio::time::sleep(POLL_EVERY).await;
        }
    }

    fn keyframe_path(&self, n: usize) -> PathBuf {
        self.dir.join(format!("{n}.m4s"))
    }

    /// Waits until ffmpeg has written keyframe `n`'s file, for segment `segment`, and says
    /// whether it did. ffmpeg writes each under another name and renames it once whole
    /// (`-hls_flags temp_file`).
    async fn keyframe_written(self: &Arc<Self>, n: usize, segment: usize) -> io::Result<bool> {
        let path = self.keyframe_path(n);
        while !tokio::fs::try_exists(&path).await? {
            if !self.will_write(n, segment).await? {
                // It may have been written just before ffmpeg stopped.
                return tokio::fs::try_exists(&path).await;
            }
            tokio::time::sleep(POLL_EVERY).await;
        }
        Ok(true)
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
    /// there has stopped, or once a later request asked for a segment more than that away, so
    /// ffmpeg isn't started over and over.
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
            if stopped && run.from == n {
                return Ok(false);
            }
            let written_to = (self.timeline).keyframe_ms(run.written_to.load(Ordering::Relaxed));
            let short = self.timeline.keyframe_ms(n).saturating_sub(written_to);
            if !stopped && run.from <= n && short <= self.wait_ahead_ms() {
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
            // An aborted `watch` never marks it, and the restart below may fail.
            run.stopped.store(true, Ordering::Release);
        }
        *ffmpeg = Some(self.run_from(n)?);
        Ok(true)
    }

    /// How it plays now: as it started, and why it fell back if it did.
    fn playback_now(&self) -> Playback {
        let mut playback = self.playback.clone();
        if let Some(Fallback { why, .. }) =
            (self.fallback.as_ref()).filter(|_| self.fell_back.load(Ordering::Acquire))
        {
            playback.reasons.push(why.clone());
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
        if failed {
            self.fall_back(&run).await;
        }
        run.stopped.store(true, Ordering::Release);
    }

    /// Once `run`, the plan's current run of ffmpeg, failed on hardware, starts its fallback from
    /// the first file it hadn't written, and tells Observers so. Only the first failure does.
    async fn fall_back(self: &Arc<Self>, run: &Arc<FfmpegRun>) {
        let Some(Fallback { why, .. }) = &self.fallback else {
            return;
        };
        // Before waiting for the lock, so a seek that restarts ffmpeg meanwhile, aborting this
        // task, restarts it in software.
        if self.fell_back.swap(true, Ordering::AcqRel) {
            return;
        }
        tracing::warn!(plan = self.id, "{why}");
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
        let mut from = run.from;
        while from < self.timeline.keyframes.len()
            && tokio::fs::try_exists(self.keyframe_path(from))
                .await
                .unwrap_or_default()
        {
            from += 1;
        }
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
        let mut written_to = run.from;
        loop {
            tokio::time::sleep(POLL_EVERY).await;
            while written_to < self.timeline.keyframes.len()
                && tokio::fs::try_exists(self.keyframe_path(written_to))
                    .await
                    .unwrap_or_default()
            {
                written_to += 1;
            }
            run.written_to.store(written_to, Ordering::Relaxed);
            let asked = (self.timeline).segment_start_ms(self.last_asked.load(Ordering::Relaxed));
            // An earlier segment asked for just after a seek mustn't stop the run it started.
            let asked = asked.max(self.timeline.keyframe_ms(run.from));
            if self.timeline.keyframe_ms(written_to).saturating_sub(asked) >= RUN_AHEAD_MS {
                return;
            }
        }
    }
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
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering::Relaxed},
        },
        time::Duration,
    };

    use futures_util::StreamExt;
    use nama_proto::nama::v1::{
        Playback, PlaybackChanged, PlaybackEnded, PlaybackMethod, PlaybackStarted,
    };

    use super::{
        Fallback, Method, POLL_EVERY, Running, RunningPlans, Runs, Timeline, lock, sweep_leftovers,
    };
    use crate::{
        contents::Contents,
        db::Database,
        events::{EventBus, Filter, event},
        ffmpeg::{Ffmpeg, Keyframe},
        plugins::Registry,
        server_settings::{HardwareChoice, PlaybackChange, PlaybackSettings, ServerUrl, Settings},
        tracks::Tracks,
    };

    fn plans_in(db: &Arc<Database>, settings: PlaybackSettings) -> RunningPlans {
        let events = EventBus::new(Arc::clone(db), Registry::new(), ServerUrl::default());
        plans_with(db, settings, &events)
    }

    fn plans_with(
        db: &Arc<Database>,
        settings: PlaybackSettings,
        events: &EventBus,
    ) -> RunningPlans {
        RunningPlans::new(
            settings,
            Arc::new(Ffmpeg::default()),
            Contents::new(Arc::clone(db)),
            Tracks::new(Arc::clone(db)),
            events.clone(),
        )
    }

    /// The playback of a Remux on the Device `device`.
    fn playback(device: i64) -> Playback {
        Playback {
            device_id: device,
            method: PlaybackMethod::PLAYBACK_METHOD_REMUX.into(),
            ..Default::default()
        }
    }

    /// The HLS files of the plan `id`, as asked for by the Device `device`.
    fn hls(plans: &RunningPlans, id: u64, device: i64) -> Arc<Running> {
        Arc::clone(plans.ask(id, device).unwrap().hls.as_ref().unwrap())
    }

    /// The Server settings in `db`, with segment files going to `default` while no folder is set.
    fn settings(db: &Arc<Database>, default: PathBuf) -> Settings {
        Settings::new(Arc::clone(db), default)
    }

    async fn database(data: &Path) -> Arc<Database> {
        Arc::new(Database::open(data).await.unwrap())
    }

    async fn running_plans(data: &Path) -> RunningPlans {
        let db = database(data).await;
        plans_in(&db, settings(&db, data.join("transcodes")).playback())
    }

    /// A Version 1000 seconds long with a keyframe every 2, so each segment is 3 keyframes long.
    fn every_two_seconds() -> Timeline {
        Timeline::new((0..500).map(|n| n * 2000).collect(), 1_000_000)
    }

    /// Starts a Remux for the Device 7 whose ffmpeg writes nothing, and returns its id with the
    /// keyframes each run of ffmpeg started at.
    async fn start_writing_nothing(plans: &RunningPlans) -> (u64, Arc<Mutex<Vec<usize>>>) {
        start_method_writing_nothing(plans, Method::Remux, every_two_seconds()).await
    }

    /// As [`start_writing_nothing`], for a plan by `method` cut as `timeline` says.
    async fn start_method_writing_nothing(
        plans: &RunningPlans,
        method: Method,
        timeline: Timeline,
    ) -> (u64, Arc<Mutex<Vec<usize>>>) {
        let starts = Arc::new(Mutex::new(vec![]));
        let started = Arc::clone(&starts);
        let run = move |_: &Path, from: Keyframe| {
            lock(&started).push(from.n);
            tokio::process::Command::new("sleep")
                .arg("600")
                .kill_on_drop(true)
                .spawn()
        };
        let id = plans
            .start_hls(
                playback(7),
                method,
                timeline,
                Runs {
                    run: Box::new(run),
                    fallback: None,
                },
            )
            .await
            .unwrap();
        (id, starts)
    }

    // The fake ffmpeg is `sleep`.
    #[cfg(unix)]
    #[tokio::test]
    async fn seeking_a_transcode_past_the_segment_ffmpeg_is_on_restarts_it_there() {
        let data = tempfile::tempdir().unwrap();
        let plans = running_plans(data.path()).await;
        let every_six_seconds = Timeline::new((0..167).map(|n| n * 6000).collect(), 1_000_000);
        let (id, starts) =
            start_method_writing_nothing(&plans, Method::Transcode, every_six_seconds).await;
        let plan = hls(&plans, id, 7);

        // A Remux would wait for this, 18 seconds on.
        let eighteen_seconds = tokio::spawn(async move { plan.segment(3).await });

        assert_eq!(until_started(&starts, 2).await, [0, 3]);
        write_keyframes(data.path(), id, 3..4);
        assert_eq!(answer(eighteen_seconds).await.unwrap().unwrap(), b"3");
    }

    /// Writes the files of `keyframes` for the plan `id` as ffmpeg would, each holding its number.
    fn write_keyframes(data: &Path, id: u64, keyframes: std::ops::Range<usize>) {
        let dir = data.join("transcodes").join(format!("nama-plan-{id}"));
        for n in keyframes {
            fs::write(dir.join(format!("{n}.m4s")), n.to_string()).unwrap();
        }
    }

    /// What `request` answers, failing the test if it doesn't within 10 seconds.
    async fn answer<T>(request: tokio::task::JoinHandle<T>) -> T {
        let answered = tokio::time::timeout(Duration::from_secs(10), request).await;
        answered.unwrap().unwrap()
    }

    /// Waits until `starts` holds `count` runs.
    async fn until_started<T: Clone>(starts: &Mutex<Vec<T>>, count: usize) -> Vec<T> {
        let started = async {
            while lock(starts).len() < count {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        };
        tokio::time::timeout(Duration::from_secs(10), started)
            .await
            .unwrap();
        lock(starts).clone()
    }

    // The fake ffmpeg is `sleep`.
    #[cfg(unix)]
    #[tokio::test]
    async fn seeking_past_what_ffmpeg_has_written_restarts_it_at_the_segment_asked_for() {
        let data = tempfile::tempdir().unwrap();
        let plans = running_plans(data.path()).await;
        let (id, starts) = start_writing_nothing(&plans).await;
        let plan = hls(&plans, id, 7);

        let eighty_percent = tokio::spawn(async move { plan.segment(133).await });

        assert_eq!(until_started(&starts, 2).await, [0, 399]);
        write_keyframes(data.path(), id, 399..402);
        let segment = answer(eighty_percent).await.unwrap().unwrap();
        assert_eq!(segment, b"399400401");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_request_left_waiting_by_a_later_seek_gives_up_rather_than_restart_ffmpeg() {
        let data = tempfile::tempdir().unwrap();
        let plans = running_plans(data.path()).await;
        let (id, starts) = start_writing_nothing(&plans).await;
        let plan = hls(&plans, id, 7);
        let first = Arc::clone(&plan);
        let eighty_percent = tokio::spawn(async move { first.segment(133).await });
        until_started(&starts, 2).await;

        let ten_percent = tokio::spawn(async move { plan.segment(10).await });

        assert_eq!(until_started(&starts, 3).await, [0, 399, 30]);
        assert_eq!(answer(eighty_percent).await.unwrap(), None);
        tokio::time::sleep(POLL_EVERY * 3).await;
        assert_eq!(*lock(&starts), [0, 399, 30]);
        write_keyframes(data.path(), id, 30..33);
        assert_eq!(answer(ten_percent).await.unwrap().unwrap(), b"303132");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn neighbouring_segments_asked_for_at_once_after_a_seek_are_both_served() {
        let data = tempfile::tempdir().unwrap();
        let plans = running_plans(data.path()).await;
        let (id, starts) = start_writing_nothing(&plans).await;
        let plan = hls(&plans, id, 7);
        let (first, second) = (Arc::clone(&plan), plan);

        let both =
            tokio::spawn(async move { tokio::join!(first.segment(133), second.segment(134)) });

        until_started(&starts, 2).await;
        tokio::time::sleep(POLL_EVERY * 3).await;
        // Whichever restarted ffmpeg first, it ends up at the earlier one.
        assert_eq!(lock(&starts).last(), Some(&399));
        write_keyframes(data.path(), id, 399..405);
        let (seeked_to, after) = answer(both).await;
        assert_eq!(seeked_to.unwrap().unwrap(), b"399400401");
        assert_eq!(after.unwrap().unwrap(), b"402403404");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_restart_that_fails_leaves_the_next_request_to_start_ffmpeg_again() {
        let data = tempfile::tempdir().unwrap();
        let plans = running_plans(data.path()).await;
        let runs = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&runs);
        let second_fails = move |_: &Path, _: Keyframe| match counted.fetch_add(1, Relaxed) {
            1 => Err(std::io::Error::other("too many open files")),
            _ => tokio::process::Command::new("sleep")
                .arg("600")
                .kill_on_drop(true)
                .spawn(),
        };
        let id = plans
            .start_hls(
                playback(7),
                Method::Remux,
                every_two_seconds(),
                Runs {
                    run: Box::new(second_fails),
                    fallback: None,
                },
            )
            .await
            .unwrap();
        let plan = hls(&plans, id, 7);
        plan.segment(133).await.unwrap_err();

        let near_the_start = tokio::spawn(async move { plan.segment(1).await });

        tokio::time::sleep(POLL_EVERY * 3).await;
        assert_eq!(runs.load(Relaxed), 3);
        write_keyframes(data.path(), id, 3..6);
        assert_eq!(answer(near_the_start).await.unwrap().unwrap(), b"345");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_segment_made_earlier_asked_for_after_a_seek_leaves_ffmpeg_running_there() {
        let data = tempfile::tempdir().unwrap();
        let plans = running_plans(data.path()).await;
        let (id, starts) = start_writing_nothing(&plans).await;
        let plan = hls(&plans, id, 7);
        write_keyframes(data.path(), id, 0..30);
        let first = Arc::clone(&plan);
        let eighty_percent = tokio::spawn(async move { first.segment(133).await });
        until_started(&starts, 2).await;

        plan.segment(2).await.unwrap().unwrap();

        tokio::time::sleep(POLL_EVERY * 3).await;
        write_keyframes(data.path(), id, 399..402);
        assert_eq!(answer(eighty_percent).await.unwrap().unwrap(), b"399400401");
        assert_eq!(*lock(&starts), [0, 399]);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn segments_already_made_are_served_without_restarting_ffmpeg() {
        let data = tempfile::tempdir().unwrap();
        let plans = running_plans(data.path()).await;
        let (id, starts) = start_writing_nothing(&plans).await;
        let plan = hls(&plans, id, 7);
        write_keyframes(data.path(), id, 0..30);
        write_keyframes(data.path(), id, 300..330);

        let back = plan.segment(2).await.unwrap().unwrap();
        let later = plan.segment(101).await.unwrap().unwrap();

        assert_eq!(back, b"678");
        assert_eq!(later, b"303304305");
        assert_eq!(*lock(&starts), [0]);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn ffmpeg_two_minutes_past_the_last_segment_asked_for_stops_and_playing_on_restarts_it() {
        let data = tempfile::tempdir().unwrap();
        let plans = running_plans(data.path()).await;
        let (id, starts) = start_writing_nothing(&plans).await;
        let plan = hls(&plans, id, 7);
        write_keyframes(data.path(), id, 0..60);
        plan.segment(0).await.unwrap().unwrap();
        // Long enough for ffmpeg's progress to be looked at.
        tokio::time::sleep(POLL_EVERY * 3).await;

        // Waited for rather than restarted at, were ffmpeg still running.
        let next = tokio::spawn(async move { plan.segment(20).await });

        assert_eq!(until_started(&starts, 2).await, [0, 60]);
        write_keyframes(data.path(), id, 60..63);
        assert_eq!(answer(next).await.unwrap().unwrap(), b"606162");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn ffmpeg_short_of_two_minutes_past_the_last_segment_asked_for_keeps_running() {
        let data = tempfile::tempdir().unwrap();
        let plans = running_plans(data.path()).await;
        let (id, starts) = start_writing_nothing(&plans).await;
        let plan = hls(&plans, id, 7);
        // To 118 seconds.
        write_keyframes(data.path(), id, 0..59);
        plan.segment(0).await.unwrap().unwrap();
        tokio::time::sleep(POLL_EVERY * 3).await;

        let next = tokio::spawn(async move { plan.segment(20).await });

        tokio::time::sleep(POLL_EVERY * 3).await;
        write_keyframes(data.path(), id, 59..63);
        assert_eq!(answer(next).await.unwrap().unwrap(), b"606162");
        assert_eq!(*lock(&starts), [0]);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn the_init_segment_is_served_once_ffmpeg_has_written_its_first_file() {
        let data = tempfile::tempdir().unwrap();
        let plans = running_plans(data.path()).await;
        let (id, _) = start_writing_nothing(&plans).await;
        let plan = hls(&plans, id, 7);
        let dir = data
            .path()
            .join("transcodes")
            .join(format!("nama-plan-{id}"));
        // ffmpeg makes it empty as it starts, and fills it in as the first file ends.
        fs::write(dir.join("init-0.mp4"), "").unwrap();

        let init = tokio::spawn(async move { plan.init().await });

        tokio::time::sleep(POLL_EVERY * 3).await;
        assert!(!init.is_finished());
        fs::write(dir.join("init-0.mp4"), "init").unwrap();
        write_keyframes(data.path(), id, 0..1);
        assert_eq!(init.await.unwrap().unwrap().unwrap(), b"init");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_running_plan_is_marked_with_its_method_and_found_only_by_its_device() {
        let data = tempfile::tempdir().unwrap();
        let plans = running_plans(data.path()).await;

        let (id, _) = start_writing_nothing(&plans).await;

        assert_eq!(hls(&plans, id, 7).method, Method::Remux);
        assert!(plans.ask(id, 8).is_none());
    }

    // The fake ffmpeg is `sleep`.
    #[cfg(unix)]
    #[tokio::test]
    async fn another_devices_transcodes_fill_the_cap_until_one_stops_and_remuxes_never_do() {
        let data = tempfile::tempdir().unwrap();
        let plans = running_plans(data.path()).await;
        let transcode = || {
            let timeline = every_two_seconds();
            start_method_writing_nothing(&plans, Method::Transcode, timeline)
        };
        let (first, _) = transcode().await;
        start_writing_nothing(&plans).await;
        assert_eq!(plans.transcodes_full(8, 2), None, "a Remux doesn't count");
        transcode().await;

        let full = plans.transcodes_full(8, 2);
        let for_its_own_device = plans.transcodes_full(7, 2);
        plans.stop_for(first, 7).await;

        assert_eq!(full, Some(2), "out of the box");
        assert_eq!(
            for_its_own_device, None,
            "the one its new plan replaces doesn't count"
        );
        assert_eq!(plans.transcodes_full(8, 2), None);
    }

    // The fake ffmpeg is `sleep`.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_device_leaves_only_one_of_its_own_transcodes_out_of_the_count() {
        let data = tempfile::tempdir().unwrap();
        let plans = running_plans(data.path()).await;
        for _ in 0..3 {
            start_method_writing_nothing(&plans, Method::Transcode, every_two_seconds()).await;
        }

        assert_eq!(plans.transcodes_full(7, 2), Some(2));
    }

    // The fake ffmpegs are `false` and `sleep`.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_transcode_whose_hardware_fails_carries_on_in_software_and_says_so() {
        let data = tempfile::tempdir().unwrap();
        let db = database(data.path()).await;
        let events = EventBus::new(Arc::clone(&db), Registry::new(), ServerUrl::default());
        let plans = plans_with(
            &db,
            settings(&db, data.path().join("transcodes")).playback(),
            &events,
        );
        let mut subscription = Box::pin(events.subscribe(Filter::default()));
        let starts = Arc::new(Mutex::new(vec![]));
        let (on_hardware, in_software) = (Arc::clone(&starts), Arc::clone(&starts));
        let hardware = move |_: &Path, from: Keyframe| {
            lock(&on_hardware).push(("hardware", from.n));
            tokio::process::Command::new("false").spawn()
        };
        let software = move |_: &Path, from: Keyframe| {
            lock(&in_software).push(("software", from.n));
            tokio::process::Command::new("sleep")
                .arg("600")
                .kill_on_drop(true)
                .spawn()
        };
        let reason = "VAAPI failed partway, so the rest of it Transcodes in software";
        let id = plans
            .start_hls(
                playback(7),
                Method::Transcode,
                every_two_seconds(),
                Runs {
                    run: Box::new(hardware),
                    fallback: Some(Fallback {
                        run: Box::new(software),
                        why: reason.to_owned(),
                    }),
                },
            )
            .await
            .unwrap();
        let plan = hls(&plans, id, 7);

        let later = tokio::spawn(async move { plan.segment(1).await });

        until_started(&starts, 2).await;
        tokio::time::sleep(POLL_EVERY * 3).await;
        assert_eq!(*lock(&starts), [("hardware", 0), ("software", 0)]);
        write_keyframes(data.path(), id, 0..6);
        assert_eq!(answer(later).await.unwrap().unwrap(), b"345");
        let changed = Playback {
            reasons: vec![reason.to_owned()],
            ..playback(7)
        };
        subscription.next().await.unwrap();
        let next = subscription.next().await.unwrap();
        assert_eq!(
            next.message,
            Some(
                event(PlaybackChanged {
                    playback: changed.clone().into(),
                    ..Default::default()
                })
                .into()
            )
        );
        plans.stop_for(id, 7).await;
        let ended = subscription.next().await.unwrap();
        assert_eq!(
            ended.message,
            Some(
                event(PlaybackEnded {
                    playback: changed.into(),
                    ..Default::default()
                })
                .into()
            ),
            "ends as it played"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn transcodes_are_full_once_as_many_as_the_cap_run() {
        let data = tempfile::tempdir().unwrap();
        let db = database(data.path()).await;
        let plans = plans_in(
            &db,
            settings(&db, data.path().join("transcodes")).playback(),
        );
        for _ in 0..2 {
            start_method_writing_nothing(&plans, Method::Transcode, every_two_seconds()).await;
        }

        assert_eq!(plans.transcodes_full(8, u32::MAX), None);
        assert_eq!(plans.transcodes_full(8, 3), None);
        assert_eq!(plans.transcodes_full(8, 2), Some(2));
    }

    #[test]
    fn segments_are_cut_at_the_first_keyframe_past_each_six_seconds() {
        let every_two_and_a_half_seconds: Vec<i64> = (0..12).map(|n| n * 2500).collect();

        let timeline = Timeline::new(every_two_and_a_half_seconds, 30_020);

        let segment_ms: Vec<i64> = timeline.segments.iter().map(|segment| segment.ms).collect();
        assert_eq!(segment_ms, [7500, 5000, 7500, 5000, 5020]);
        assert_eq!(timeline.keyframes_of(1), 3..5);
        assert_eq!(timeline.keyframes_of(4), 10..12);
    }

    /// Starts a plan for the Device `device` whose ffmpeg is `sleep`, and returns its id.
    async fn start_sleeping(plans: &RunningPlans, device: i64) -> u64 {
        plans
            .start_hls(
                playback(device),
                Method::Remux,
                every_two_seconds(),
                Runs {
                    run: Box::new(|_: &Path, _: Keyframe| {
                        tokio::process::Command::new("sleep")
                            .arg("60")
                            .kill_on_drop(true)
                            .spawn()
                    }),
                    fallback: None,
                },
            )
            .await
            .unwrap()
    }

    // The stand-in ffmpeg is `sleep`.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_plan_unused_for_long_enough_is_stopped_and_its_files_deleted() {
        let data = tempfile::tempdir().unwrap();
        let db = database(data.path()).await;
        let plans = plans_in(
            &db,
            settings(&db, data.path().join("transcodes")).playback(),
        );
        let device = 7;
        let (used, unused) = (
            start_sleeping(&plans, device).await,
            start_sleeping(&plans, device).await,
        );
        let dir = |id| hls(&plans, id, device).dir.clone();
        let (used_dir, unused_dir) = (dir(used), dir(unused));
        let plan = hls(&plans, unused, device);
        tokio::time::sleep(Duration::from_millis(100)).await;
        plans.ask(used, device).unwrap();

        plans.stop_unused(Duration::from_millis(50)).await;

        assert!(plans.ask(unused, device).is_none());
        assert!(plan.closed.load(std::sync::atomic::Ordering::Acquire));
        assert!(!unused_dir.exists());
        assert!(plans.ask(used, device).is_some());
        assert!(used_dir.exists());
    }

    // The stand-in ffmpeg is `sleep`.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_plan_gone_unused_tells_its_playback_ended() {
        let data = tempfile::tempdir().unwrap();
        let db = database(data.path()).await;
        let events = EventBus::new(Arc::clone(&db), Registry::new(), ServerUrl::default());
        let plans = plans_with(
            &db,
            settings(&db, data.path().join("transcodes")).playback(),
            &events,
        );
        let mut subscription = Box::pin(events.subscribe(Filter::default()));
        start_sleeping(&plans, 7).await;

        plans.stop_unused(Duration::ZERO).await;

        let started = event(PlaybackStarted {
            playback: playback(7).into(),
            ..Default::default()
        });
        let ended = event(PlaybackEnded {
            playback: playback(7).into(),
            ..Default::default()
        });
        for told in [started, ended] {
            let next = subscription.next().await.unwrap();
            assert_eq!(next.message, Some(told.into()));
        }
    }

    // The stand-in ffmpeg is `sleep`.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_plan_its_device_stops_ends_at_once_and_another_device_cant_stop_it() {
        let data = tempfile::tempdir().unwrap();
        let db = database(data.path()).await;
        let events = EventBus::new(Arc::clone(&db), Registry::new(), ServerUrl::default());
        let plans = plans_with(
            &db,
            settings(&db, data.path().join("transcodes")).playback(),
            &events,
        );
        let (device, another) = (7, 8);
        let id = start_sleeping(&plans, device).await;
        let dir = hls(&plans, id, device).dir.clone();
        let mut subscription = Box::pin(events.subscribe(Filter::default()));

        plans.stop_for(id, another).await;
        assert!(plans.ask(id, device).is_some());
        plans.stop_for(id, device).await;

        assert!(plans.ask(id, device).is_none());
        assert!(!dir.exists());
        let ended = event(PlaybackEnded {
            playback: playback(device).into(),
            ..Default::default()
        });
        let next = subscription.next().await.unwrap();
        assert_eq!(next.message, Some(ended.into()));
    }

    #[tokio::test]
    async fn a_direct_play_is_kept_while_its_device_asks_for_its_version_and_then_ends() {
        let data = tempfile::tempdir().unwrap();
        let db = database(data.path()).await;
        let plans = plans_in(
            &db,
            settings(&db, data.path().join("transcodes")).playback(),
        );
        let (version, device, another) = (4, 7, 8);
        let direct_play = |on| Playback {
            device_id: on,
            version_id: version,
            method: PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY.into(),
            ..Default::default()
        };
        let (asked, unasked) = (
            plans.direct_play(direct_play(device)),
            plans.direct_play(direct_play(another)),
        );
        tokio::time::sleep(Duration::from_millis(100)).await;

        plans.ask_direct_plays(version, device);
        plans.stop_unused(Duration::from_millis(50)).await;

        assert!(plans.ask(asked, device).is_some());
        assert!(plans.ask(unasked, another).is_none());
    }

    // The stand-in ffmpeg is `sleep`.
    #[cfg(unix)]
    #[tokio::test]
    async fn stopping_all_plans_deletes_every_plans_files() {
        let data = tempfile::tempdir().unwrap();
        let db = database(data.path()).await;
        let plans = plans_in(
            &db,
            settings(&db, data.path().join("transcodes")).playback(),
        );
        let id = start_sleeping(&plans, 7).await;
        let dir = hls(&plans, id, 7).dir.clone();

        plans.stop_all().await;

        assert!(plans.ask(id, 7).is_none());
        assert!(!dir.exists());
    }

    #[tokio::test]
    async fn a_new_plan_writes_to_the_segment_folder_set_since_the_last() {
        let data = tempfile::tempdir().unwrap();
        let db = database(data.path()).await;
        let settings = settings(&db, data.path().join("transcodes"));
        let plans = plans_in(&db, settings.playback());
        let before = start_sleeping(&plans, 7).await;
        let tmpfs = data.path().join("tmpfs");
        settings
            .change_playback(PlaybackChange {
                segment_folder: tmpfs.to_str().unwrap(),
                max_transcodes: None,
                hardware: HardwareChoice::Automatic,
                remote_limit: None,
            })
            .await
            .unwrap();

        let after = start_sleeping(&plans, 7).await;

        assert!(
            hls(&plans, before, 7)
                .dir
                .starts_with(data.path().join("transcodes"))
        );
        assert!(hls(&plans, after, 7).dir.starts_with(&tmpfs));
    }

    #[tokio::test]
    async fn the_sweep_deletes_plan_folders_in_the_set_and_default_folders_and_nothing_else() {
        let data = tempfile::tempdir().unwrap();
        let (default, set) = (data.path().join("transcodes"), data.path().join("tmpfs"));
        let leftovers = [default.join("nama-plan-0"), set.join("nama-plan-3")];
        for leftover in &leftovers {
            fs::create_dir_all(leftover).unwrap();
            fs::write(leftover.join("0.m4s"), "segment").unwrap();
        }
        let others = set.join("someone else's");
        fs::create_dir_all(&others).unwrap();
        // A file, which deleting as a folder fails, so the sweep must go on past it.
        fs::write(default.join("nama-plan-not-a-folder"), "").unwrap();
        let db = database(data.path()).await;
        let settings = settings(&db, default);
        settings
            .change_playback(PlaybackChange {
                segment_folder: set.to_str().unwrap(),
                max_transcodes: None,
                hardware: HardwareChoice::Automatic,
                remote_limit: None,
            })
            .await
            .unwrap();

        sweep_leftovers(&settings.playback()).await;

        for leftover in leftovers {
            assert!(!leftover.exists(), "{}", leftover.display());
        }
        assert!(others.exists());
    }
}
