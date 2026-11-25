use std::{io, path::Path, sync::Weak};

use tokio_util::sync::CancellationToken;

use super::{
    Arc, Duration, Inner, PLAN_FOLDER, Plan, RunningPlans, STOP_UNUSED_AFTER, TIDY_EVERY, lock,
};
use crate::{db, server_settings::PlaybackSettings};

/// Every [`TIDY_EVERY`], stops the plans unused for [`STOP_UNUSED_AFTER`] and deletes the running
/// ones' files from well behind or far ahead of playback, until `inner` is dropped or `cancel` is
/// cancelled.
pub(super) async fn tidy_plans(inner: Weak<Inner>, cancel: CancellationToken) {
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
        plans.delete_unkept_files().await;
    }
}

impl RunningPlans {
    /// Whether the plan `id` runs for the Device `device`, without marking it asked for. One
    /// unused for [`STOP_UNUSED_AFTER`] counts as stopped though [`tidy_plans`] hasn't got to it.
    pub(crate) fn runs_for(&self, id: u64, device: i64) -> bool {
        let now = db::instant_now();
        (lock(&self.inner.plans).get(&id)).is_some_and(|plan| {
            plan.playback.device_id == device
                && now.duration_since(*lock(&plan.last_used)) < STOP_UNUSED_AFTER
        })
    }

    /// Stops every plan not asked for in the last `unused_for`, and deletes its files.
    pub(super) async fn stop_unused(&self, unused_for: Duration) {
        let now = db::instant_now();
        let unused: Vec<(u64, Arc<Plan>)> = lock(&self.inner.plans)
            .extract_if(|_, plan| now.duration_since(*lock(&plan.last_used)) >= unused_for)
            .collect();
        for (_, plan) in unused {
            self.stop(&plan).await;
        }
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
