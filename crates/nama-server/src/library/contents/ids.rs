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

/// The namespaces a title's id is preferred in where no Library says: TMDB's, then IMDb's,
/// then TVDB's. A Request, which belongs to no Library, keeps a title under it.
pub(crate) const DEFAULT_ORDER: [&str; 3] = [TMDB, IMDB, TVDB];

/// The one of `ids` that identifies a title: the first in a namespace of `order`, a Library's
/// title sources or [`DEFAULT_ORDER`], or else the first in namespace order. `None` if it has
/// none.
pub(crate) fn primary_id(ids: &ExternalIds, order: &[impl AsRef<str>]) -> Option<ExternalId> {
    (order.iter())
        .find_map(|namespace| ids.get(namespace.as_ref()))
        .or_else(|| ids.iter().next())
}

/// [`primary_id`] in the order a Library stores as `libraries.title_sources`, comma-separated.
pub(super) fn primary_id_in(ids: &ExternalIds, title_sources: &str) -> Option<ExternalId> {
    primary_id(ids, &title_sources.split(',').collect::<Vec<_>>())
}

/// Hands `$query!` the start of a subquery giving an Item's ids in one scope of `item_match_ids`
/// as a JSON object of namespace to value; the caller closes it with the scope, as in `'title')`.
/// [`ids_from_json`] reads it. A macro because sqlx reads a query from literals only; `$arg`s go
/// to `$query!` first, so another such macro can hand on its literal too.
macro_rules! ids_sql {
    ($query:ident $(, $arg:literal)*) => {
        $query!(
            $($arg,)*
            "(SELECT json_group_object(namespace, value) FROM item_match_ids
              WHERE item_match_ids.item_id = item_matches.item_id AND scope = "
        )
    };
}
pub(super) use ids_sql;

/// Hands `$query!` a subquery giving the id, in its text form, of the title an Item of a Library
/// is listed under: a Movie's own, an Episode's Show's. It picks as [`primary_id_in`] does, and
/// needs `item_matches`, `library_items` and `libraries` in scope. `$arg`s go first, as for
/// [`ids_sql`].
macro_rules! listed_id_sql {
    ($query:ident $(, $arg:literal)*) => {
        $query!(
            $($arg,)*
            // The lowest of each id's place in the Library's order, padded so text compares it
            // as a number, unordered ones last in namespace order; the place is cut off after.
            "(SELECT substr(MIN(printf('%05d', coalesce(nullif(instr(
                         ',' || libraries.title_sources || ',', ',' || listed.namespace || ','), 0),
                         99999)) || listed.namespace || '-' || listed.value), 6)
              FROM item_match_ids AS listed
              WHERE listed.item_id = item_matches.item_id
                AND listed.scope = IIF(library_items.kind = 'movie', 'title', 'show'))"
        )
    };
}
pub(super) use listed_id_sql;

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
