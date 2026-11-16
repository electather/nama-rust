//! What the Server's ffmpeg can do, asked once and remembered.

use std::sync::Arc;

use tokio_util::{sync::CancellationToken, task::TaskTracker};

use super::{Ffmpeg, Gpu, Hardware, ToneMapper};

/// What the Server's ffmpeg can do, as [`Abilities`] hands it back.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each is a separate thing ffmpeg can do, not a state"
)]
pub(crate) struct CanDo {
    /// Whether the Server has ffmpeg and ffprobe, which every method but Direct Play needs.
    pub(crate) runs: bool,
    /// Whether ffmpeg has zscale, which a Transcode tone-maps HDR to SDR with in software.
    pub(crate) zscale: bool,
    /// Whether ffmpeg has libx264, which a Transcode encodes its video with in software.
    pub(crate) libx264: bool,
    /// The APIs on GPUs that passed their test, in the order Automatic picks.
    pub(crate) hardware: Vec<Hardware>,
    /// The GPUs the Server found at startup, whether or not anything passed on them.
    pub(crate) gpus: Vec<Gpu>,
}

impl CanDo {
    /// Whether a Transcode runs at all: it needs ffmpeg and ffprobe, and libx264 even on hardware.
    pub(crate) const fn transcodes(&self) -> bool {
        // ponytail: a Transcode on hardware needs no libx264, but its fallback to software does;
        // allow one once a failed hardware run can stop instead of falling back, and change
        // `server_ffmpeg`'s reason with it.
        self.runs && self.libx264
    }

    /// The hardware Automatic picks, if any passed its test.
    pub(crate) fn automatic(&self) -> Option<&Hardware> {
        self.hardware.first()
    }

    /// What tone-maps HDR to SDR in a Transcode on `gpu`, or in software for `None`: the GPU if
    /// it passed tone-mapping, or else zscale if ffmpeg has it.
    pub(crate) const fn tone_mapper(&self, gpu: Option<&Hardware>) -> Option<ToneMapper> {
        match gpu {
            Some(Hardware {
                tone_maps: true, ..
            }) => Some(ToneMapper::Hardware),
            _ if self.zscale => Some(ToneMapper::Software),
            _ => None,
        }
    }
}

/// Asks the Server's ffmpeg what it can do and remembers each answer, so planning and the
/// Playback status read the same [`CanDo`]. The only asker: see `ffmpeg/AGENTS.md`, "What
/// ffmpeg can do is asked once".
#[derive(Clone)]
pub(crate) struct Abilities {
    inner: Arc<AbilitiesInner>,
}

struct AbilitiesInner {
    ffmpeg: Arc<Ffmpeg>,
    zscale: tokio::sync::OnceCell<bool>,
    libx264: tokio::sync::OnceCell<bool>,
    hardware: tokio::sync::OnceCell<Vec<Hardware>>,
    /// The hardware test, so shutting down can wait for it.
    tasks: TaskTracker,
    /// Cancelled when the Server shuts down.
    cancel: CancellationToken,
}

impl Abilities {
    /// Starts testing `ffmpeg`'s hardware in the background, as it takes a few seconds; the first
    /// [`Self::can_do`] waits for it, and asks again if ffmpeg couldn't list its encoders.
    pub(crate) fn new(ffmpeg: Arc<Ffmpeg>) -> Self {
        let abilities = Self {
            inner: Arc::new(AbilitiesInner {
                ffmpeg,
                zscale: tokio::sync::OnceCell::new(),
                libx264: tokio::sync::OnceCell::new(),
                hardware: tokio::sync::OnceCell::new(),
                tasks: TaskTracker::new(),
                cancel: CancellationToken::new(),
            }),
        };
        // Holds nothing but ffmpeg, so the Server shutting down mid-test still closes the
        // database cleanly (#554).
        let testing = abilities.clone();
        abilities.inner.tasks.spawn(async move {
            // Only for testing early; a task's output must be owned.
            let _: Option<usize> = (testing.inner.cancel)
                .run_until_cancelled(async { testing.hardware().await.len() })
                .await;
        });
        abilities
    }

    /// Cancels the hardware test if it still runs, killing its test encode, and waits for it to
    /// stop.
    pub(crate) async fn shutdown(&self) {
        self.inner.cancel.cancel();
        self.inner.tasks.close();
        self.inner.tasks.wait().await;
    }

    /// What ffmpeg can do. An answer ffmpeg couldn't give, such as on a busy host whose listing
    /// timed out, counts as `false` or no hardware and is asked again next time.
    pub(crate) async fn can_do(&self) -> CanDo {
        let ffmpeg = &self.inner.ffmpeg;
        // Together, so a busy host waits out one listing's timeout, not two.
        let (zscale, libx264, hardware) = tokio::join!(
            answered_once(&self.inner.zscale, "zscale", ffmpeg.has_zscale()),
            answered_once(&self.inner.libx264, "libx264", ffmpeg.has_libx264()),
            self.hardware(),
        );
        CanDo {
            runs: ffmpeg.can_remux(),
            zscale,
            libx264,
            hardware: hardware.to_vec(),
            gpus: ffmpeg.gpus.clone(),
        }
    }

    /// The hardware acceleration that passed its test encode, tested the first time it's asked
    /// for and kept, or none while ffmpeg couldn't list its encoders, which is asked again next
    /// time.
    async fn hardware(&self) -> &[Hardware] {
        let ffmpeg = &self.inner.ffmpeg;
        let tested = self.inner.hardware.get_or_try_init(|| async {
            let found = ffmpeg.find_hardware().await?;
            let names: Vec<String> = found.iter().map(Hardware::name).collect();
            tracing::info!(hardware = ?names, "tested hardware acceleration");
            Ok::<_, String>(found)
        });
        // As `answered_once`: a listing that timed out on a busy host isn't kept as no hardware.
        match tested.await {
            Ok(found) => found,
            Err(err) => {
                tracing::warn!("could not ask ffmpeg for its encoders: {err}");
                &[]
            }
        }
    }
}

/// Whether ffmpeg has `feature`, as `cell` keeps from `ask`'s first answer, or `false` while it
/// can't say.
async fn answered_once(
    cell: &tokio::sync::OnceCell<bool>,
    feature: &str,
    ask: impl Future<Output = Result<bool, String>>,
) -> bool {
    // Only an answer is kept, so a check that timed out on a busy host is asked again.
    match cell.get_or_try_init(|| ask).await {
        Ok(&has) => has,
        Err(err) => {
            tracing::warn!("could not ask ffmpeg whether it has {feature}: {err}");
            false
        }
    }
}
