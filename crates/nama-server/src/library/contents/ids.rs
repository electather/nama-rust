//! A title's ids as Contents stores and reads them: a Match's in `item_match_ids`, a Pin's as its
//! text form in `match_pins.pinned_id`.

use std::collections::HashMap;

use nama_plugin_api::metadata::{ExternalId, ExternalIds, IMDB, TMDB, TVDB};

use super::pins::Pin;

impl Pin {
    /// The pin stored in a `match_pins` row, if there is one, holding the id `id` in its text
    /// form. An id that doesn't parse is no pin, as a bad stored ID Hint is no hint.
    pub(super) fn from_row(pinned: bool, id: Option<&str>) -> Option<Self> {
        if !pinned {
            return None;
        }
        match id {
            None => Some(Self::Unmatched),
            Some(id) => parse_hint(id).map(Self::Title),
        }
    }
}

/// The one of `ids` that identifies a title: its TMDB id if it has one, or else its IMDb, then
/// its TVDB id, or else the first of any other in namespace order. `None` if it has none. This
/// order stands until per-Library source order exists.
pub(crate) fn primary_id(ids: &ExternalIds) -> Option<ExternalId> {
    [TMDB, IMDB, TVDB]
        .into_iter()
        .find_map(|namespace| ids.get(namespace))
        .or_else(|| ids.iter().next())
}

/// Hands `$query!` the start of a subquery giving an Item's ids in one scope of `item_match_ids`
/// as a JSON object of namespace to value; the caller closes it with the scope, as in `'title')`.
/// [`ids_from_json`] reads it. A macro because sqlx reads a query from literals only.
macro_rules! ids_sql {
    ($query:ident) => {
        $query!(
            "(SELECT json_group_object(namespace, value) FROM item_match_ids
              WHERE item_match_ids.item_id = item_matches.item_id AND scope = "
        )
    };
}
pub(super) use ids_sql;

/// The ids an [`ids_sql`] subquery returned. A pair that isn't a valid id is skipped.
pub(super) fn ids_from_json(json: Option<&str>) -> ExternalIds {
    let pairs: HashMap<String, String> = json
        .and_then(|json| serde_json::from_str(json).ok())
        .unwrap_or_default();
    (pairs.into_iter())
        .filter_map(|(namespace, value)| ExternalId::new(&namespace, value).ok())
        .collect()
}

/// The ID Hint stored as `text`, which is how [`ExternalId`] displays: any well-formed id.
pub(crate) fn parse_hint(text: &str) -> Option<ExternalId> {
    text.parse().ok()
}

#[cfg(test)]
#[path = "ids_tests.rs"]
mod tests;
