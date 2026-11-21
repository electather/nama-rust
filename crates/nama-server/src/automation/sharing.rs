//! `list_titles_sharing`: the Movies or Shows of every Library sharing Persons or genres with a
//! title, which a Similar source narrows the Server down with.

use std::sync::Arc;

use buffa::EnumValue;
use nama_plugin_api::{
    automation::{KernelRefused, SharingTitle, SharingTitles, TitlesSharingAsked},
    contract::TitleKind as AskedKind,
    metadata::ExternalId,
};

use super::{Caller, title_id};
use crate::library::{Contents, TitleKind, title_roles};

/// The most titles one answer holds, which a `limit` of 0 asks for.
const MAX_SHARING: u32 = 1000;

impl Caller {
    pub(super) async fn list_titles_sharing(
        self,
        asked: TitlesSharingAsked,
    ) -> Result<SharingTitles, KernelRefused> {
        let kind = match asked.kind {
            EnumValue::Known(AskedKind::TITLE_KIND_MOVIE) => TitleKind::Movie,
            EnumValue::Known(AskedKind::TITLE_KIND_SHOW) => TitleKind::Show,
            EnumValue::Known(AskedKind::TITLE_KIND_UNSPECIFIED) | EnumValue::Unknown(_) => {
                return Err(KernelRefused(
                    "the kind of title must be Movie or Show".to_owned(),
                ));
            }
        };
        // A malformed id is no Person's a title source gave, so it can share nothing.
        let people: Vec<ExternalId> = (asked.people.iter())
            .filter_map(|id| ExternalId::new(&id.namespace, id.value.clone()).ok())
            .collect();
        let limit = match asked.limit {
            0 => MAX_SHARING,
            limit => limit.min(MAX_SHARING),
        };
        let found = Contents::new(Arc::clone(&self.automation.db))
            .titles_sharing(kind, &people, &asked.genres, limit)
            .await
            .map_err(|err| self.database_refused("reading the Libraries' titles", &err))?;
        let titles = (found.into_iter())
            .map(|title| SharingTitle {
                ids: title.ids.iter().map(|id| title_id(&id)).collect(),
                genres: title.genres,
                roles: title_roles(&title.roles),
                ..SharingTitle::default()
            })
            .collect();
        Ok(SharingTitles {
            titles,
            ..SharingTitles::default()
        })
    }
}

#[cfg(test)]
#[path = "sharing_tests.rs"]
mod tests;
