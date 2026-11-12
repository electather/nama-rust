//! A title's ids as fixed TMDB, IMDb and TVDB fields, as Contents stores and reads them.

use nama_plugin_api::metadata::{ExternalId, ExternalIds};

use super::Pin;

impl Pin {
    /// The pin stored in a `match_pins` row, if there is one, holding the id in `ids`.
    pub(super) fn from_row(pinned: bool, ids: ExternalIds) -> Option<Self> {
        if !pinned {
            return None;
        }
        Some(primary_id(ids).map_or(Self::Unmatched, Self::Title))
    }

    pub(super) fn columns(&self) -> ExternalIds {
        let mut ids = ExternalIds::default();
        match self {
            Self::Title(ExternalId::Tmdb(id)) => ids.tmdb = Some(*id),
            Self::Title(ExternalId::Imdb(id)) => ids.imdb = Some(id.clone()),
            Self::Title(ExternalId::Tvdb(id)) => ids.tvdb = Some(*id),
            Self::Unmatched => {}
        }
        ids
    }
}

/// The one of `ids` that identifies a title: its TMDB id if it has one, or else its IMDb or TVDB
/// id. `None` if it has none.
pub(super) fn primary_id(ids: ExternalIds) -> Option<ExternalId> {
    ids.tmdb
        .map(ExternalId::Tmdb)
        .or_else(|| ids.imdb.map(ExternalId::Imdb))
        .or_else(|| ids.tvdb.map(ExternalId::Tvdb))
}

/// The ID Hint stored as `text`, which is how [`ExternalId`] displays.
pub(crate) fn parse_hint(text: &str) -> Option<ExternalId> {
    match text.split_once('-')? {
        ("tmdb", id) => id.parse().ok().map(ExternalId::Tmdb),
        ("tvdb", id) => id.parse().ok().map(ExternalId::Tvdb),
        ("imdb", id) => Some(ExternalId::Imdb(id.to_owned())),
        _ => None,
    }
}

#[cfg(test)]
#[path = "ids_tests.rs"]
mod tests;
