//! What an Admin sees of the plans playing now, whether any is, and stopping any of them.

use std::{sync::Arc, time::Duration};

use nama_proto::nama::v1::{AdminStop, Playback};
use tokio_util::sync::CancellationToken;

use super::{Plan, RunningPlans, lock};
use crate::{db, ffmpeg::Hardware, signed_urls::Viewer};

/// How often [`RunningPlans::until_playing`] checks again.
const PLAYING_CHECK: Duration = Duration::from_secs(1);

/// The most characters an Admin's message to a viewer whose play they stop may have.
pub(crate) const STOP_MESSAGE_LENGTH: usize = 200;

/// An Admin's message to a viewer was over [`STOP_MESSAGE_LENGTH`] characters.
#[derive(Debug)]
pub(crate) struct MessageTooLong;

/// How far a plan's Device last said it was in its Version, and how long that is, in
/// milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReportedProgress {
    pub(crate) position_ms: u64,
    pub(crate) duration_ms: u64,
}

/// A plan playing now, as an Admin sees it.
#[derive(Debug, PartialEq)]
pub(crate) struct PlayNow {
    /// How it plays now, as its latest Playback event told it.
    pub(crate) playback: Playback,
    /// What it plays, as people name it; `None` if its Version is gone.
    pub(crate) title: Option<String>,
    /// `None` until its Device says.
    pub(crate) progress: Option<ReportedProgress>,
    /// The hardware it Transcodes on now; `None` in software, or for a Direct Play or a Remux.
    pub(crate) hardware: Option<Hardware>,
}

impl RunningPlans {
    /// Stops the plan `id` whatever its Device, as an Admin asked, publishing its end with their
    /// `message` to the viewer. Unless its User has another plan of its Version on that Device,
    /// also cuts off each response serving that Version's file to them there until they plan it
    /// again, so a Direct Play stops too. A plan already stopped is left alone.
    ///
    /// # Errors
    ///
    /// [`MessageTooLong`] for a message over [`STOP_MESSAGE_LENGTH`] characters, and nothing stops.
    pub(crate) async fn stop_by_admin(
        &self,
        id: u64,
        message: String,
    ) -> Result<(), MessageTooLong> {
        if message.chars().count() > STOP_MESSAGE_LENGTH {
            return Err(MessageTooLong);
        }
        let plan = {
            // Held while cancelling, so `insert` can't add a plan of this Version for the Viewer in
            // between and have it cut off.
            let mut plans = lock(&self.inner.plans);
            let Some(plan) = plans.remove(&id) else {
                return Ok(());
            };
            if !plans.values().any(|other| plays_alike(other, &plan)) {
                lock(&self.inner.stopped)
                    .entry(version_for_viewer(&plan))
                    .or_default()
                    .cancel();
            }
            plan
        };
        let admin_stop = AdminStop {
            message,
            ..Default::default()
        };
        self.stop_telling(&plan, Some(admin_stop)).await;
        Ok(())
    }

    /// Cancelled once an Admin stops `viewer`'s last plan of the Version `version`, to cut off a
    /// response serving its file, and at once while that stop holds; never without a Viewer.
    pub(crate) fn stopped(&self, version: i64, viewer: Option<Viewer>) -> CancellationToken {
        let Some(viewer) = viewer else {
            return CancellationToken::new();
        };
        let mut stopped = lock(&self.inner.stopped);
        stopped.entry((version, viewer)).or_default().child_token()
    }

    /// Whether any plan is playing, a Direct Play included.
    fn any_playing(&self) -> bool {
        !lock(&self.inner.plans).is_empty()
    }

    /// Returns once whether any plan plays is `playing`, for background work that waits it out.
    pub(in crate::playback) async fn until_playing(&self, playing: bool) {
        while self.any_playing() != playing {
            tokio::time::sleep(PLAYING_CHECK).await;
        }
    }

    /// Every plan playing now, oldest first, with its title from the Library.
    pub(crate) async fn plays(&self) -> Result<Vec<PlayNow>, sqlx::Error> {
        let mut running: Vec<Arc<Plan>> = lock(&self.inner.plans).values().cloned().collect();
        running.sort_unstable_by_key(|plan| plan.playback.plan_id);
        let mut plays = Vec::with_capacity(running.len());
        for plan in running {
            let (playback, progress) = (plan.playback_now(), *lock(&plan.progress));
            let title = (self.inner.contents)
                .version_title(playback.version_id)
                .await?;
            plays.push(PlayNow {
                playback,
                title,
                progress,
                hardware: plan.hardware_now().cloned(),
            });
        }
        Ok(plays)
    }

    /// Keeps `progress`, which `viewer` reported through the Version `version`, beside their newest
    /// plan of that Version, for [`Self::plays`], and marks it asked for now, which keeps a paused
    /// player's plan another [`STOP_UNUSED_AFTER`](super::STOP_UNUSED_AFTER). An older one is most
    /// likely a play left without a stop, such as a reloaded tab's, so it keeps what it last had.
    pub(crate) fn report_progress(&self, version: i64, viewer: Viewer, progress: ReportedProgress) {
        let plans = lock(&self.inner.plans);
        let newest = (plans.values())
            .filter(|plan| version_for_viewer(plan) == (version, viewer))
            .max_by_key(|plan| plan.playback.plan_id);
        if let Some(plan) = newest {
            *lock(&plan.progress) = Some(progress);
            *lock(&plan.last_used) = db::instant_now();
        }
    }
}

/// Whether `a` and `b` play the same Version for the same User on the same Device.
fn plays_alike(a: &Plan, b: &Plan) -> bool {
    version_for_viewer(a) == version_for_viewer(b)
}

/// The Version `plan` plays and who it plays for.
fn version_for_viewer(plan: &Plan) -> (i64, Viewer) {
    (plan.playback.version_id, plan.viewer())
}

#[cfg(test)]
#[path = "plays_tests.rs"]
mod tests;
