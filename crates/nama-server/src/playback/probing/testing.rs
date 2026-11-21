//! An in-memory stand-in for ffprobe, which Probing's unit tests read files with. A test says in
//! Rust what a probe or a listing of a file answers, rather than write a program that poses as
//! ffprobe, or has it forward to the real ffprobe while counting what it reads.

use std::{
    collections::VecDeque,
    fmt, fs, future,
    sync::{Arc, Mutex},
    time::Duration,
};

use futures_util::future::BoxFuture;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use super::{
    ProbeError,
    ffprobe::{Ffprobe, Reader, Starts},
    lock,
};
use crate::{
    ffmpeg::{Ffmpeg, Program, RunError},
    playback::tracks::{ListedKeyframes, Probe, Track, TrackKind},
};

/// What a probe of the file at a path answers.
type Answer = dyn Fn(&str) -> Result<Probe, ProbeError> + Send + Sync;

/// What a whole-file listing of the keyframes of the file at a path answers.
type Listing = dyn Fn(&str) -> Result<ListedKeyframes, ProbeError> + Send + Sync;

/// Answers each probe and listing as the test says, or forwards it to ffprobe, holds it until the
/// test releases it if told to, and counts the probes and listings it started and the most reads
/// running at once.
pub(super) struct FakeReader {
    answer: Mutex<Box<Answer>>,
    listing: Mutex<Box<Listing>>,
    /// Reads with ffprobe instead of answering, for tests that need a real file's index read.
    forward: Option<Ffprobe>,
    can_read: bool,
    seen: Arc<watch::Sender<Seen>>,
    hold: Mutex<Hold>,
}

/// What each read waits on before it answers.
struct Hold {
    /// Already cancelled unless reads are held; each held read waits on a child of it.
    all: CancellationToken,
    /// What each read held waits on, earliest first, so [`FakeReader::release_one`] frees them in
    /// the order they started.
    reads: VecDeque<CancellationToken>,
}

#[derive(Default)]
struct Seen {
    probes: usize,
    listings: usize,
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
        Arc::new(Self::new(true, answer))
    }

    /// Reads each file with `ffmpeg`'s ffprobe, counting what it reads.
    pub(super) fn forwarding_to(ffmpeg: Ffmpeg) -> Arc<Self> {
        let ffprobe = Ffprobe(Arc::new(ffmpeg));
        let can_read = ffprobe.can_read();
        Arc::new(Self {
            forward: Some(ffprobe),
            ..Self::new(can_read, |_| panic!("it probes with ffprobe"))
        })
    }

    /// Reads each file as [`by_size`] does.
    pub(super) fn by_size() -> Arc<Self> {
        Self::answering(|path| Ok(by_size(path)))
    }

    /// Can't read files at all, as on a Server without ffprobe (ADR 0026).
    pub(super) fn unable() -> Arc<Self> {
        Arc::new(Self::new(false, |_| {
            Err(ProbeError::Run(RunError::Unavailable(Program::Ffprobe)))
        }))
    }

    fn new(
        can_read: bool,
        answer: impl Fn(&str) -> Result<Probe, ProbeError> + Send + Sync + 'static,
    ) -> Self {
        let all = CancellationToken::new();
        all.cancel();
        Self {
            answer: Mutex::new(Box::new(answer)),
            listing: Mutex::new(Box::new(|_| {
                panic!("the test didn't say what a listing answers")
            })),
            forward: None,
            can_read,
            seen: Arc::default(),
            hold: Mutex::new(Hold {
                all,
                reads: VecDeque::new(),
            }),
        }
    }

    /// Answers the probes it starts from now on with `answer`.
    pub(super) fn answer(
        &self,
        answer: impl Fn(&str) -> Result<Probe, ProbeError> + Send + Sync + 'static,
    ) {
        *lock(&self.answer) = Box::new(answer);
    }

    /// Answers the listings it starts from now on with `listing`.
    pub(super) fn list(
        &self,
        listing: impl Fn(&str) -> Result<ListedKeyframes, ProbeError> + Send + Sync + 'static,
    ) {
        *lock(&self.listing) = Box::new(listing);
    }

    /// Keeps the reads it starts from now on pending until [`Self::release`].
    pub(super) fn hold(&self) {
        let mut hold = lock(&self.hold);
        // A token still pending is kept, so `release` frees the reads held before this too.
        if hold.all.is_cancelled() {
            hold.all = CancellationToken::new();
        }
    }

    /// Lets every held read finish, and holds no more.
    pub(super) fn release(&self) {
        let mut hold = lock(&self.hold);
        hold.all.cancel();
        hold.reads.clear();
    }

    /// Lets the earliest read still held finish, and keeps holding the rest and those to come.
    /// Panics if no read is held.
    pub(super) fn release_one(&self) {
        let read = lock(&self.hold).reads.pop_front();
        read.expect("no read is held").cancel();
    }

    /// How many probes it started.
    pub(super) fn probes(&self) -> usize {
        self.seen.borrow().probes
    }

    /// How many listings it started.
    pub(super) fn listings(&self) -> usize {
        self.seen.borrow().listings
    }

    /// The most reads it saw running at once.
    pub(super) fn most_at_once(&self) -> usize {
        self.seen.borrow().most_at_once
    }

    /// Waits until it started `count` probes, failing the test after 10 seconds.
    pub(super) async fn until_started(&self, count: usize) {
        self.until(|seen| seen.probes >= count).await;
    }

    /// Waits until it started `count` listings, failing the test after 10 seconds.
    pub(super) async fn until_listed(&self, count: usize) {
        self.until(|seen| seen.listings >= count).await;
    }

    async fn until(&self, done: impl Fn(&Seen) -> bool) {
        let mut seen = self.seen.subscribe();
        tokio::time::timeout(Duration::from_secs(10), seen.wait_for(done))
            .await
            .unwrap()
            .unwrap();
    }

    /// `answered`, counted as started in the count `started` picks and as running until it ends
    /// or is dropped, and held while reads are.
    fn read<T: Send + 'static>(
        &self,
        started: fn(&mut Seen) -> &mut usize,
        answered: BoxFuture<'static, Result<T, ProbeError>>,
    ) -> BoxFuture<'static, Result<T, ProbeError>> {
        self.seen.send_modify(|seen| {
            *started(seen) += 1;
            seen.running += 1;
            seen.most_at_once = seen.most_at_once.max(seen.running);
        });
        let running = Running(Arc::clone(&self.seen));
        let released = {
            let mut hold = lock(&self.hold);
            let released = hold.all.child_token();
            if !released.is_cancelled() {
                hold.reads.push_back(released.clone());
            }
            released
        };
        Box::pin(async move {
            // Moved in, so the read stops counting as running only once Probing drops it.
            let _running = running;
            released.cancelled().await;
            answered.await
        })
    }
}

impl fmt::Debug for FakeReader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FakeReader")
            .field("probes", &self.probes())
            .field("listings", &self.listings())
            .finish_non_exhaustive()
    }
}

impl Reader for FakeReader {
    fn can_read(&self) -> bool {
        self.can_read
    }

    fn probe(&self, path: String) -> BoxFuture<'static, Result<(Probe, Starts), ProbeError>> {
        let answered = if let Some(ffprobe) = &self.forward {
            ffprobe.probe(path)
        } else {
            let starts = Starts {
                file_ms: 0,
                video_ms: None,
            };
            let answered = lock(&self.answer)(&path).map(|probe| (probe, starts));
            Box::pin(future::ready(answered))
        };
        self.read(|seen| &mut seen.probes, answered)
    }

    fn keyframes(&self, path: String) -> BoxFuture<'static, Result<ListedKeyframes, ProbeError>> {
        let answered = if let Some(ffprobe) = &self.forward {
            ffprobe.keyframes(path)
        } else {
            Box::pin(future::ready(lock(&self.listing)(&path)))
        };
        self.read(|seen| &mut seen.listings, answered)
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
