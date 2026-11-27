//! An in-memory stand-in for ffmpeg's runs, which the unit tests of running plans share, and
//! running plans for tests elsewhere. A test plays ffmpeg's part by writing a run's files itself.

use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use nama_proto::nama::v1::{Playback, PlaybackMethod};
use tokio::sync::{oneshot, watch};
use tokio_util::sync::CancellationToken;

use super::{
    Contents, EventBus, Ffmpeg, Method, Probing, RunningPlans, lock,
    run::{Converts, Outcome, Run, Runner, Runs, Start},
    subtitles,
    timeline::Timeline,
};
use crate::{
    db::Database,
    ffmpeg::Keyframe,
    playback::{Offers, Plan, Playing, ProbePasses, SubtitleOffsets, WholeFileReads},
    server_settings::PlaybackSettings,
    signed_urls::Viewer,
};

/// Runs a test starts, ends and watches being dropped, as a plan starts them.
#[derive(Clone, Default)]
pub(in crate::playback) struct FakeRuns {
    shared: Arc<Shared>,
}

#[derive(Default)]
struct Shared {
    seen: watch::Sender<Seen>,
    /// Each run's end, by the order it started in, until the test ends it.
    ends: Mutex<Vec<Option<oneshot::Sender<Outcome>>>>,
    /// Whether the next start fails.
    fail_next: AtomicBool,
    /// What the runs that fail as soon as they start convert.
    failing: Mutex<Vec<Converted>>,
}

#[derive(Default)]
struct Seen {
    /// The keyframe each start was asked to start at, by number, the failed ones too.
    from: Vec<usize>,
    /// What each start through [`FakeRuns::runner`] was asked to convert, in order.
    converted: Vec<Converted>,
    /// Whether each run started, in order, was dropped.
    dropped: Vec<bool>,
}

/// Marks its run dropped when it's dropped.
struct MarksDropped {
    shared: Arc<Shared>,
    run: usize,
}

impl Drop for MarksDropped {
    fn drop(&mut self) {
        self.shared
            .seen
            .send_modify(|seen| seen.dropped[self.run] = true);
    }
}

/// What a run converts, as a test tells runs apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::playback) enum Converted {
    Remux,
    TranscodeOnHardware,
    TranscodeInSoftware,
}

impl Converted {
    fn of(converts: Converts<'_>) -> Self {
        match converts {
            Converts::Remux { .. } => Self::Remux,
            Converts::Transcode { attempt, .. } if attempt.video.hardware.is_some() => {
                Self::TranscodeOnHardware
            }
            Converts::Transcode { .. } => Self::TranscodeInSoftware,
        }
    }
}

impl FakeRuns {
    /// What starts these runs: each runs until the test ends it, it's stopped or it's dropped.
    pub(super) fn start(&self) -> Box<Start> {
        let runs = self.clone();
        Box::new(move |_: &Path, from: Keyframe, stop: CancellationToken| {
            Ok(runs.run(from, stop)?.1)
        })
    }

    /// These runs as running plans start them, noting what each converts.
    fn runner(&self) -> Arc<Runner> {
        let runs = self.clone();
        Arc::new(
            move |_: &str, converts: Converts<'_>, _: &Path, from: Keyframe, stop| {
                let converted = Converted::of(converts);
                (runs.shared.seen).send_modify(|seen| seen.converted.push(converted));
                let (n, run) = runs.run(from, stop)?;
                if lock(&runs.shared.failing).contains(&converted) {
                    runs.fail(n, "Conversion failed!");
                }
                Ok(run)
            },
        )
    }

    /// Starts a run from `from`, and returns it with its place in the order runs started in.
    fn run(&self, from: Keyframe, stop: CancellationToken) -> std::io::Result<(usize, Run)> {
        let shared = Arc::clone(&self.shared);
        shared.seen.send_modify(|seen| seen.from.push(from.n));
        if shared.fail_next.swap(false, Ordering::Relaxed) {
            return Err(std::io::Error::other("too many open files"));
        }
        let (end, ended) = oneshot::channel();
        let run = {
            // Under the lock, so each run's end and drop mark share its place.
            let mut ends = lock(&shared.ends);
            ends.push(Some(end));
            shared.seen.send_modify(|seen| seen.dropped.push(false));
            ends.len() - 1
        };
        let marks = MarksDropped { shared, run };
        Ok((
            run,
            Box::pin(async move {
                let _marks = marks;
                tokio::select! {
                    // An end gone untold doesn't match, so it runs until it's stopped or dropped.
                    Ok(outcome) = ended => outcome,
                    () = stop.cancelled() => Outcome::Stopped,
                }
            }),
        ))
    }

    /// Makes every run converting as `converted` that starts from now on fail at once, as an
    /// ffmpeg that can't convert the file does.
    pub(in crate::playback) fn fail_at_once(&self, converted: Converted) {
        lock(&self.shared.failing).push(converted);
    }

    /// What each start was asked to convert, in order.
    pub(in crate::playback) fn converted(&self) -> Vec<Converted> {
        self.shared.seen.borrow().converted.clone()
    }

    /// Makes the next start fail, as one failing to spawn ffmpeg does.
    pub(super) fn fail_next_start(&self) {
        self.shared.fail_next.store(true, Ordering::Relaxed);
    }

    /// The keyframe each start was asked to start at, by number.
    pub(super) fn started(&self) -> Vec<usize> {
        self.shared.seen.borrow().from.clone()
    }

    /// Waits until `count` starts were asked for, failing the test after 10 seconds, and returns
    /// where each was asked to start.
    pub(in crate::playback) async fn until_started(&self, count: usize) -> Vec<usize> {
        self.until(|seen| seen.from.len() >= count).await;
        self.started()
    }

    /// Waits until run `run`, by the order runs started in, was dropped, failing the test after
    /// 10 seconds.
    pub(super) async fn until_dropped(&self, run: usize) {
        self.until(|seen| seen.dropped.get(run) == Some(&true))
            .await;
    }

    /// Whether run `run`, by the order runs started in, was dropped.
    pub(in crate::playback) fn dropped(&self, run: usize) -> bool {
        self.shared.seen.borrow().dropped.get(run) == Some(&true)
    }

    /// Ends run `run`, by the order runs started in, with `outcome`.
    pub(super) fn end(&self, run: usize, outcome: Outcome) {
        let end = lock(&self.shared.ends)[run].take().unwrap();
        // A run already dropped has no one to tell.
        let _: Result<(), Outcome> = end.send(outcome);
    }

    /// Ends run `run` failing with `stderr`, as ffmpeg exiting with status 1 does.
    pub(super) fn fail(&self, run: usize, stderr: &str) {
        self.end(
            run,
            Outcome::Failed {
                status: "exit status: 1".to_owned(),
                stderr: stderr.to_owned(),
            },
        );
    }

    async fn until(&self, done: impl FnMut(&Seen) -> bool) {
        let mut seen = self.shared.seen.subscribe();
        let waited = tokio::time::timeout(Duration::from_secs(10), seen.wait_for(done)).await;
        waited.unwrap().unwrap();
    }
}

/// A Version 1000 seconds long with a keyframe every 2, so each segment is 3 keyframes long.
pub(super) fn every_two_seconds() -> Timeline {
    Timeline::new((0..500).map(|n| n * 2000).collect(), 1_000_000)
}

/// A plan that Direct Plays the Version `version`, planned on the local network.
pub(in crate::playback) fn direct_play_of(version: i64) -> Plan {
    Plan {
        version,
        playing: Some(Playing::DirectPlay),
        tracks: vec![],
        reasons: vec![],
        subtitles: vec![],
        limit: None,
        audio: None,
        burn_in: None,
        starting_subtitle: None,
        offers: Offers::default(),
        chapters: vec![],
        segments: vec![],
        preview_frames: None,
        subtitle_offsets: SubtitleOffsets::default(),
    }
}

impl RunningPlans {
    /// How many cut-offs it keeps, by Version and by Version and Viewer.
    pub(in crate::playback) fn cut_offs_kept(&self) -> usize {
        lock(&self.inner.served).len() + lock(&self.inner.stopped).len()
    }

    /// Starts a Remux of the Version `version` for `viewer` whose runs write nothing, on paused
    /// time so the wait for its first file ends at once, and returns its id.
    pub(in crate::playback) async fn start_faked_remux(&self, viewer: Viewer, version: i64) -> u64 {
        let playback = Playback {
            user_id: viewer.user,
            device_id: viewer.device,
            version_id: version,
            method: PlaybackMethod::PLAYBACK_METHOD_REMUX.into(),
            ..Default::default()
        };
        let runs = Runs {
            method: Method::Remux,
            start: FakeRuns::default().start(),
            cpu_bound: false,
            fallback: None,
        };
        tokio::time::pause();
        let started = (self.start_hls(playback, every_two_seconds(), 0, runs, None)).await;
        tokio::time::resume();
        started.unwrap()
    }

    /// Plans on `db` publishing on `events`, with the default playback settings and their files
    /// under `folder`, whose runs write nothing, for a test that only asks what plays and ends it.
    pub(crate) fn for_tests(db: &Arc<Database>, events: EventBus, folder: &Path) -> Self {
        let settings =
            crate::server_settings::Settings::new(Arc::clone(db), folder.join("transcodes"));
        let probing = Probing::new(
            Arc::clone(db),
            Arc::new(Ffmpeg::default()),
            ProbePasses::default(),
            WholeFileReads::default(),
        );
        Self::on_fake_runs(
            &FakeRuns::default(),
            settings.playback(),
            probing,
            db,
            events,
            folder,
        )
    }

    /// Plans on `db` publishing on `events` with `settings`, cutting a Remux at the keyframes
    /// `probing` lists, whose runs are `runs`, and burning in subtitles cached under `folder`.
    #[expect(
        clippy::too_many_arguments,
        reason = "each is a separate handle plans run on, shared with the test"
    )]
    pub(in crate::playback) fn on_fake_runs(
        runs: &FakeRuns,
        settings: PlaybackSettings,
        probing: Probing,
        db: &Arc<Database>,
        events: EventBus,
        folder: &Path,
    ) -> Self {
        let subtitles = subtitles::Cache::new(folder);
        let contents = Contents::new(Arc::clone(db));
        Self::with_runner(
            settings,
            runs.runner(),
            contents,
            probing,
            subtitles,
            events,
        )
    }
}
