//! A title's ids as fixed TMDB, IMDb and TVDB fields, as Contents stores and reads them.

use nama_plugin_api::metadata::{ExternalId, ExternalIds, IMDB, TMDB, TVDB};

use super::pins::Pin;

impl Pin {
    /// The pin stored in a `match_pins` row, if there is one, holding the id in `ids`.
    pub(super) fn from_row(pinned: bool, ids: &ExternalIds) -> Option<Self> {
        if !pinned {
            return None;
        }
        Some(primary_id(ids).map_or(Self::Unmatched, Self::Title))
    }

    pub(super) fn columns(&self) -> ExternalIds {
        match self {
            Self::Title(id) => ExternalIds::from_iter([id.clone()]),
            Self::Unmatched => ExternalIds::default(),
        }
    }
}

/// The one of `ids` that identifies a title: its TMDB id if it has one, or else its IMDb or TVDB
/// id. `None` if it has none.
pub(super) fn primary_id(ids: &ExternalIds) -> Option<ExternalId> {
    [TMDB, IMDB, TVDB]
        .into_iter()
        .find_map(|namespace| ids.get(namespace))
}

/// The ID Hint stored as `text`, which is how [`ExternalId`] displays.
pub(crate) fn parse_hint(text: &str) -> Option<ExternalId> {
    let id: ExternalId = text.parse().ok()?;
    // A number is rebuilt from its value, so `tmdb-0348` reads as `tmdb-348`.
    match id.namespace() {
        TMDB => id.value().parse().ok().map(ExternalId::tmdb),
        TVDB => id.value().parse().ok().map(ExternalId::tvdb),
        IMDB => Some(id),
        _ => None,
    }
}

#[cfg(test)]
#[path = "ids_tests.rs"]
mod tests;
