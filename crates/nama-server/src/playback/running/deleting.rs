//! Versions being deleted: their plans end first, and each response serving their file is cut
//! off until they're gone.

use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use super::{Plan, RunningPlans, lock};

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

impl RunningPlans {
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
        for (_, plan) in ending {
            self.stop(&plan).await;
        }
        Deleting {
            plans: self.clone(),
            versions: versions.to_vec(),
        }
    }

    /// Whether the Version `version` is being deleted.
    pub(super) fn deleting(&self, version: i64) -> bool {
        (lock(&self.inner.served).get(&version)).is_some_and(CancellationToken::is_cancelled)
    }

    /// Cancelled once the Version `version` is being deleted, to cut off a response serving its
    /// file.
    pub(in crate::playback) fn served(&self, version: i64) -> CancellationToken {
        let mut served = lock(&self.inner.served);
        served.entry(version).or_default().child_token()
    }
}
