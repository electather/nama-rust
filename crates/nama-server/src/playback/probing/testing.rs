//! An in-memory stand-in for ffprobe, which Probing's unit tests read files with. A test says in
//! Rust what a probe of a file answers, rather than write a program that poses as ffprobe.

use std::{
    fmt, fs,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use futures_util::future::BoxFuture;

use super::{
    ProbeError,
    ffprobe::{Reader, Starts},
    lock,
};
use crate::playback::tracks::{ListedKeyframes, Probe, Track, TrackKind};

/// What a probe of the file at a path answers.
type Answer = dyn Fn(&str) -> Result<Probe, ProbeError> + Send + Sync;

/// Answers each probe as the test says, and counts the probes it started.
pub(super) struct FakeReader {
    answer: Mutex<Box<Answer>>,
    probes: AtomicUsize,
}

impl FakeReader {
    /// Answers a probe of each path with what `answer` returns for it.
    pub(super) fn answering(
        answer: impl Fn(&str) -> Result<Probe, ProbeError> + Send + Sync + 'static,
    ) -> Arc<Self> {
        Arc::new(Self {
            answer: Mutex::new(Box::new(answer)),
            probes: AtomicUsize::new(0),
        })
    }

    /// Reads each file as [`by_size`] does.
    pub(super) fn by_size() -> Arc<Self> {
        Self::answering(|path| Ok(by_size(path)))
    }

    /// Answers the probes it starts from now on with `answer`.
    pub(super) fn answer(
        &self,
        answer: impl Fn(&str) -> Result<Probe, ProbeError> + Send + Sync + 'static,
    ) {
        *lock(&self.answer) = Box::new(answer);
    }

    /// How many probes it started.
    pub(super) fn probes(&self) -> usize {
        self.probes.load(Ordering::Relaxed)
    }
}

impl fmt::Debug for FakeReader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FakeReader")
            .field("probes", &self.probes)
            .finish_non_exhaustive()
    }
}

impl Reader for FakeReader {
    fn can_read(&self) -> bool {
        true
    }

    fn probe(&self, path: String) -> BoxFuture<'static, Result<(Probe, Starts), ProbeError>> {
        self.probes.fetch_add(1, Ordering::Relaxed);
        let starts = Starts {
            file_ms: 0,
            video_ms: None,
        };
        let answered = lock(&self.answer)(&path).map(|probe| (probe, starts));
        Box::pin(std::future::ready(answered))
    }

    fn keyframes(&self, _: String) -> BoxFuture<'static, Result<ListedKeyframes, ProbeError>> {
        panic!("no test lists keyframes with the in-memory reader yet")
    }
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
