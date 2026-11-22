//! `list_version_titles` and `list_devices`: what a Playback event's Version and Device are, which
//! an Observer of plays, such as the Hub Plugin's, records them by (ADR 0093).

use std::sync::Arc;

use nama_plugin_api::{
    automation::KernelRefused,
    contract::{Device, Devices, DevicesAsked, TitleRefKind, VersionTitles, VersionTitlesAsked},
};

use super::{Caller, title_ref};
use crate::library::{Contents, TitlesIn};

impl Caller {
    pub(super) async fn list_version_titles(
        self,
        asked: VersionTitlesAsked,
    ) -> Result<VersionTitles, KernelRefused> {
        let contents = Contents::new(Arc::clone(&self.automation.db));
        let read = |err| self.database_refused("reading the Version's titles", &err);
        let version = asked.version_id;
        let Some(library) = contents.library_of_version(version).await.map_err(read)? else {
            return Ok(VersionTitles::default());
        };
        let (kind, ids) = match contents.titles_in(version).await.map_err(read)? {
            TitlesIn::NoSuchVersion => return Ok(VersionTitles::default()),
            TitlesIn::Movie(movie) => (TitleRefKind::TITLE_REF_KIND_MOVIE, Vec::from_iter(movie)),
            TitlesIn::Episodes(episodes) => (
                TitleRefKind::TITLE_REF_KIND_EPISODE,
                episodes.into_iter().flatten().collect(),
            ),
        };
        let titles = (ids.iter())
            .map(|(id, _)| title_ref(library, kind, id))
            .collect();
        Ok(VersionTitles {
            titles,
            ..VersionTitles::default()
        })
    }

    pub(super) async fn list_devices(self, asked: DevicesAsked) -> Result<Devices, KernelRefused> {
        let found = (self.automation.users.device_kinds(&asked.device_ids))
            .await
            .map_err(|err| self.database_refused("reading the Devices", &err))?;
        Ok(Devices {
            devices: (found.into_iter())
                .map(|(id, user_id, kind)| Device {
                    id,
                    user_id,
                    kind: kind.into(),
                    ..Device::default()
                })
                .collect(),
            ..Devices::default()
        })
    }
}

#[cfg(test)]
#[path = "playing_tests.rs"]
mod tests;
