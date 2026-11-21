//! An in-memory stand-in for ffprobe, which Probing's unit tests read files with. A test says in
//! Rust what a probe of a file answers, rather than write a program that poses as ffprobe.

use std::{
    fmt, fs,
    sync::{Arc, Mutex},
    time::Duration,
};

use futures_util::future::BoxFuture;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use super::{
    ProbeError,
    ffprobe::{Reader, Starts},
    lock,
};
use crate::{
    ffmpeg::{Program, RunError},
    playback::tracks::{ListedKeyframes, Probe, Track, TrackKind},
};

/// What a probe of the file at a path answers.
type Answer = dyn Fn(&str) -> Result<Probe, ProbeError> + Send + Sync;

/// Answers each probe as the test says, holds it until the test releases it if told to, and
/// counts the probes it started and the most running at once.
pub(super) struct FakeReader {
    answer: Mutex<Box<Answer>>,
    can_read: bool,
    seen: Arc<watch::Sender<Seen>>,
    /// What each read waits on before it answers; already cancelled unless reads are held.
    released: Mutex<CancellationToken>,
}

#[derive(Default)]
struct Seen {
    probes: usize,
    running: usize,
    most_at_once: usize,
}

/// Counts its read as running until it ends or is dropped.
struct Running(Arc<watch::Sender<Seen>>);

impl Drop for Running {
    fn drop(&mut self) {
        self.0.send_modify(|seen| seen.running -= 1);
    }
}

impl FakeReader {
    /// Answers a probe of each path with what `answer` returns for it.
    pub(super) fn answering(
        answer: impl Fn(&str) -> Result<Probe, ProbeError> + Send + Sync + 'static,
    ) -> Arc<Self> {
        Self::new(true, answer)
    }

    /// Reads each file as [`by_size`] does.
    pub(super) fn by_size() -> Arc<Self> {
        Self::answering(|path| Ok(by_size(path)))
    }

    /// Can't read files at all, as on a Server without ffprobe (ADR 0026).
    pub(super) fn unable() -> Arc<Self> {
        Self::new(false, |_| {
            Err(ProbeError::Run(RunError::Unavailable(Program::Ffprobe)))
        })
    }

    fn new(
        can_read: bool,
        answer: impl Fn(&str) -> Result<Probe, ProbeError> + Send + Sync + 'static,
    ) -> Arc<Self> {
        let released = CancellationToken::new();
        released.cancel();
        Arc::new(Self {
            answer: Mutex::new(Box::new(answer)),
            can_read,
            seen: Arc::default(),
            released: Mutex::new(released),
        })
    }

    /// Answers the probes it starts from now on with `answer`.
    pub(super) fn answer(
        &self,
        answer: impl Fn(&str) -> Result<Probe, ProbeError> + Send + Sync + 'static,
    ) {
        *lock(&self.answer) = Box::new(answer);
    }

    /// Keeps the reads it starts from now on pending until [`Self::release`].
    pub(super) fn hold(&self) {
        let mut released = lock(&self.released);
        // A token still pending is kept, so `release` frees the reads held before this too.
        if released.is_cancelled() {
            *released = CancellationToken::new();
        }
    }

    /// Lets every held read finish, and holds no more.
    pub(super) fn release(&self) {
        lock(&self.released).cancel();
    }

    /// How many probes it started.
    pub(super) fn probes(&self) -> usize {
        self.seen.borrow().probes
    }

    /// The most reads it saw running at once.
    pub(super) fn most_at_once(&self) -> usize {
        self.seen.borrow().most_at_once
    }

    /// Waits until it started `count` probes, failing the test after 10 seconds.
    pub(super) async fn until_started(&self, count: usize) {
        let mut seen = self.seen.subscribe();
        let started = seen.wait_for(|seen| seen.probes >= count);
        tokio::time::timeout(Duration::from_secs(10), started)
            .await
            .unwrap()
            .unwrap();
    }
}

impl fmt::Debug for FakeReader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FakeReader")
            .field("probes", &self.probes())
            .finish_non_exhaustive()
    }
}

impl Reader for FakeReader {
    fn can_read(&self) -> bool {
        self.can_read
    }

    fn probe(&self, path: String) -> BoxFuture<'static, Result<(Probe, Starts), ProbeError>> {
        self.seen.send_modify(|seen| {
            seen.probes += 1;
            seen.running += 1;
            seen.most_at_once = seen.most_at_once.max(seen.running);
        });
        let running = Running(Arc::clone(&self.seen));
        let released = lock(&self.released).clone();
        let starts = Starts {
            file_ms: 0,
            video_ms: None,
        };
        let answered = lock(&self.answer)(&path).map(|probe| (probe, starts));
        Box::pin(async move {
            // Moved in, so the read stops counting as running only once Probing drops it.
            let _running = running;
            released.cancelled().await;
            answered
        })
    }

    fn keyframes(&self, _: String) -> BoxFuture<'static, Result<ListedKeyframes, ProbeError>> {
        panic!("no test lists keyframes with the in-memory reader yet")
    }
}

/// What ffprobe failing on a file answers.
pub(super) fn failed() -> ProbeError {
    ProbeError::Run(RunError::Failed(Program::Ffprobe, String::new()))
}

/// The file at `path` as one audio Track with as many channels as it has bytes, so a changed file
/// reads differently.
pub(super) fn by_size(path: &str) -> Probe {
    let bytes = fs::metadata(path).unwrap().len();
    one_audio_track(i64::try_from(bytes).unwrap())
}

/// A Matroska file a second long holding one audio Track with `channels`.
pub(super) fn one_audio_track(channels: i64) -> Probe {
    Probe {
        container: "matroska,webm".to_owned(),
        duration_ms: Some(1000),
        bitrate: None,
        tracks: vec![Track {
            codec: "ac3".to_owned(),
            language: None,
            title: None,
            default: false,
            forced: false,
            sdh: false,
            kind: TrackKind::Audio {
                channels: Some(channels),
            },
        }],
        font_attachments: vec![],
        chapters: vec![],
    }
}
