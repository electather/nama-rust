//! The one read of a Version's Tracks (see the area's `AGENTS.md`): the file's own, which a
//! Device names by their place, then its sidecars.

use std::{collections::HashMap, sync::Arc};

use super::{Probe, Sidecars, Tracks};
use crate::{db::Database, library::Stored};

/// Reads the Tracks of Versions as stored now, sidecars included.
#[derive(Debug, Clone)]
pub(crate) struct StoredTracks {
    tracks: Tracks,
    sidecars: Sidecars,
}

impl StoredTracks {
    /// Reads `db` through the owners of the probe and sidecar tables.
    pub(crate) fn new(db: Arc<Database>) -> Self {
        Self {
            tracks: Tracks::new(Arc::clone(&db)),
            sidecars: Sidecars::new(db),
        }
    }

    /// What probing read from each of `files`, by Version id, with the sidecars stored beside it
    /// after the file's own Tracks in path order. A file probed only as it was before it changed
    /// has no entry, as [`Tracks::of`] leaves it out.
    pub(crate) async fn of(&self, files: &[Stored]) -> Result<HashMap<i64, Probe>, sqlx::Error> {
        let mut probes = self.tracks.of(files).await?;
        (self.sidecars)
            .append_to(probes.iter_mut().map(|(&id, probe)| (id, probe)))
            .await?;
        Ok(probes)
    }
}

#[cfg(test)]
#[path = "stored_tracks_tests.rs"]
mod tests;
