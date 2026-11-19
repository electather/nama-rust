//! What an Admin sees of the plans playing now, and stopping any of them.

use std::sync::Arc;

use nama_proto::nama::v1::{AdminStop, Playback};

use super::{Plan, RunningPlans, lock};
use crate::ffmpeg::Hardware;

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
    /// Stops the plan `id` whatever its Device, as an Admin asked, publishing its end with
    /// `admin_stop`. Unless the Device has another plan of its Version, also cuts off each response
    /// serving that Version's file to it until it plans it again, so a Direct Play stops too. A
    /// plan already stopped is left alone.
    pub(crate) async fn stop_by_admin(&self, id: u64, admin_stop: AdminStop) {
        let plan = {
            // Held while cancelling, so `insert` can't add a plan of this Version for the Device in
            // between and have it cut off.
            let mut plans = lock(&self.inner.plans);
            let Some(plan) = plans.remove(&id) else {
                return;
            };
            if !plans.values().any(|other| plays_alike(other, &plan)) {
                let to_device = (plan.playback.version_id, plan.playback.device_id);
                lock(&self.inner.stopped)
                    .entry(to_device)
                    .or_default()
                    .cancel();
            }
            plan
        };
        self.stop_telling(&plan, Some(admin_stop)).await;
    }

    /// Every plan playing now, oldest first, with its title from the Library.
    pub(crate) async fn plays(&self) -> Result<Vec<PlayNow>, sqlx::Error> {
        let mut running: Vec<Arc<Plan>> = lock(&self.inner.plans).values().cloned().collect();
        running.sort_unstable_by_key(|plan| plan.playback.plan_id);
        let mut plays = Vec::with_capacity(running.len());
        for plan in running {
            let playback = plan.playback_now();
            plays.push(PlayNow {
                title: self
                    .inner
                    .contents
                    .version_title(playback.version_id)
                    .await?,
                progress: *lock(&plan.progress),
                hardware: plan.hardware_now().cloned(),
                playback,
            });
        }
        Ok(plays)
    }

    /// Keeps `progress`, which the Device `device` reported through the Version `version`, beside
    /// each of its plans of it, for [`Self::plays`].
    pub(crate) fn report_progress(&self, version: i64, device: i64, progress: ReportedProgress) {
        let plans = lock(&self.inner.plans);
        let reported = plans.values().filter(|plan| {
            plan.playback.version_id == version && plan.playback.device_id == device
        });
        for plan in reported {
            *lock(&plan.progress) = Some(progress);
        }
    }
}

/// Whether `a` and `b` play the same Version on the same Device.
fn plays_alike(a: &Plan, b: &Plan) -> bool {
    (a.playback.version_id, a.playback.device_id) == (b.playback.version_id, b.playback.device_id)
}
