//! The one derivation of a Library's titles, as SQL fragments its title reads build on, so the
//! rules for which Items a Library lists and how they group into titles live here alone.
//!
//! Each fragment is a macro, since sqlx reads a query from literals only. It hands `$query!` its
//! `$arg`s, then itself as one parenthesised group of `+`-joined literals, which `$query` splices
//! into its own `sqlx::query!` after a `+`, so the query stays checked at compile time.
//! `$query` may be a path, such as `$crate::…`, so a fragment can build on another a reader
//! didn't import.

/// Hands `$query!` the start of a subquery giving the preferred id, in its text form, of an
/// Item's ids in one scope of `item_match_ids`: the first in a namespace of its Library's
/// `title_sources`, or else the first by namespace. The caller closes it with the scope, as in
/// `'title')`, and needs `item_matches` and `libraries` in scope. `primary_id_agrees_with_sql`
/// keeps it picking as [`primary_id`](super::primary_id) does.
macro_rules! listed_id_sql {
    ($($query:tt)::+ $(, $arg:tt)*) => {
        $($query)::+!(
            $($arg,)*
            // The lowest of each id's place in the Library's order, padded so text compares it
            // as a number, unordered ones last in namespace order; the place is cut off after.
            // The commas around both sides keep one namespace from matching a prefix of another.
            "(SELECT substr(MIN(printf('%05d', coalesce(nullif(instr(
                         ',' || libraries.title_sources || ',', ',' || candidate.namespace || ','),
                         0), 99999)) || candidate.namespace || '-' || candidate.value), 6)
              FROM item_match_ids AS candidate
              WHERE candidate.item_id = item_matches.item_id AND candidate.scope = "
        )
    };
}
pub(super) use listed_id_sql;

/// Hands `$query!` a subquery with one row per Item a Library lists: `library_id`, `item_id`,
/// `kind`, `path`, `season`, `episode`, `added_at`, `own_id` (its preferred `title` id) and
/// `listed_id` (the preferred id of the title it is listed under: a Movie's own, an Episode's
/// Show's). It keeps an Item whose Match is Matched and has both ids, and for an Episode a season
/// and an episode number. With no window or aggregate, SQLite flattens it into a reader's join.
macro_rules! matched_items {
    ([$($query:tt)::+ $(, $arg:tt)*], $listed_id:literal) => {
        $($query)::+!(
            $($arg,)*
            ("(SELECT library_files.library_id, library_items.id AS item_id, library_items.kind,
                      library_files.path, library_items.season, library_items.episode,
                      library_items.added_at, "
             + $listed_id
             + "'title') AS own_id, "
             + $listed_id
             + "IIF(library_items.kind = 'movie', 'title', 'show')) AS listed_id
               FROM item_matches
               JOIN library_items ON library_items.id = item_matches.item_id
               JOIN library_files ON library_files.id = library_items.file_id
               JOIN libraries ON libraries.id = library_files.library_id
               WHERE item_matches.state = 'matched'
                 AND library_items.kind = IIF(libraries.kind = 'movies', 'movie', 'episode')
                 AND own_id IS NOT NULL AND listed_id IS NOT NULL
                 AND (library_items.kind = 'movie'
                      OR (library_items.season IS NOT NULL
                          AND library_items.episode IS NOT NULL)))")
        )
    };
    ($($query:tt)::+ $(, $arg:tt)*) => {
        $crate::library::contents::listed_id_sql!(
            $crate::library::contents::matched_items,
            [$($query)::+ $(, $arg)*]
        )
    };
}
pub(super) use matched_items;

/// Hands `$query!` [`matched_items`]' subquery with, on each row, what belongs to its title, the
/// rows sharing a `library_id` and `listed_id`: `nth`, its place in the title by season, episode,
/// then path, 1 being the Item whose Match's metadata the title shows; `title_item`, the title's
/// lowest Item id, which breaks ties in keyset paging (ADR 0055); and `first_added_at`,
/// `newest_added_at` and `added_at`, the title's added time: a Movie's first Version's, a Show's
/// newest Episode's. The Item's own `added_at` is replaced by the title's.
// ponytail: whole-Library read. Grouping a 25,000-Episode Library takes tens of milliseconds;
// store a row per title, kept current on Match writes, title source reorders (ADR 0044) and
// deletes, if that grows, and move every reader of this with it.
macro_rules! library_titles {
    ([$($query:tt)::+ $(, $arg:tt)*], ($($matched:tt)+)) => {
        $($query)::+!(
            $($arg,)*
            ("(SELECT library_id, item_id, kind, path, season, episode, own_id, listed_id,
                      ROW_NUMBER() OVER title AS nth, MIN(item_id) OVER title AS title_item,
                      MIN(added_at) OVER title AS first_added_at,
                      MAX(added_at) OVER title AS newest_added_at,
                      IIF(kind = 'movie', MIN(added_at) OVER title, MAX(added_at) OVER title)
                          AS added_at
               FROM "
             + $($matched)+
             + "
               WINDOW title AS (PARTITION BY library_id, listed_id
                                ORDER BY season, episode, path
                                ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING))")
        )
    };
    ($($query:tt)::+ $(, $arg:tt)*) => {
        $crate::library::contents::matched_items!(
            $crate::library::contents::library_titles,
            [$($query)::+ $(, $arg)*]
        )
    };
}
pub(super) use library_titles;

#[cfg(test)]
#[path = "titles_sql_tests.rs"]
mod tests;
