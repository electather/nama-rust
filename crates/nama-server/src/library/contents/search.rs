//! Searching every Library's Matched titles.

use nama_plugin_api::metadata::{ExternalId, ExternalIds};
use nama_proto::nama::v1::PageResponse;

use super::{Contents, ImageRef, ItemKind, ids_from_json, ids_sql, primary_id_in, show_poster};
use crate::page::Page;

/// A page of what [`Contents::search`] found in every Library, each kind best fit first, and each
/// Movie, Show or Episode once per Library that has it.
#[derive(Debug, Default)]
pub(crate) struct Found {
    pub(crate) movies: Vec<FoundMovie>,
    /// Found by their own titles.
    pub(crate) shows: Vec<FoundShow>,
    /// Found by their own titles, not their Shows'.
    pub(crate) episodes: Vec<FoundEpisode>,
}

/// A Movie a search found, with the metadata the Match of its best-fitting Item stores.
#[derive(Debug, Clone)]
pub(crate) struct FoundMovie {
    pub(crate) library: i64,
    /// As browsing identifies it: see [`primary_id`](super::primary_id).
    pub(crate) id: ExternalId,
    /// Every id its Match gives it.
    pub(crate) ids: ExternalIds,
    pub(crate) title: String,
    pub(crate) release_date: Option<String>,
    pub(crate) poster: Option<ImageRef>,
}

/// A Show a search found, with the metadata the Match of its best-fitting Episode stores.
#[derive(Debug, Clone)]
pub(crate) struct FoundShow {
    pub(crate) library: i64,
    /// As browsing identifies it: see [`primary_id`](super::primary_id).
    pub(crate) id: ExternalId,
    /// Every id the Match of its first Episode found gives it.
    pub(crate) ids: ExternalIds,
    pub(crate) title: String,
    /// As [`show_poster`] picks it, from the Match and the Seasons of the Show's Episodes the
    /// search found.
    pub(crate) poster: Option<ImageRef>,
}

/// An Episode a search found, with what browsing needs to find it: its Show and Season.
#[derive(Debug)]
pub(crate) struct FoundEpisode {
    pub(crate) library: i64,
    /// As browsing identifies it: see [`primary_id`](super::primary_id).
    pub(crate) id: ExternalId,
    /// Its Show's id, as browsing identifies it.
    pub(crate) show: ExternalId,
    pub(crate) show_title: String,
    pub(crate) season: i64,
    /// The episode number within its Season.
    pub(crate) number: i64,
    pub(crate) title: String,
}

/// Where a found title sits in a search's order, tie-broken by its best-fitting Item's id: its
/// FTS5 rank's bits, since JSON doesn't round-trip every `f64`, and whether it was found as a Show.
/// A rank weighs a word by how many titles have it, so a Scan between two pages can move titles
/// across the token: ADR 0055's keyset is only as stable as the order it pages.
type Place = (u64, bool);

impl Contents {
    /// A `page` of the Matched Movies, Shows and Episodes of every Library whose titles have each
    /// of the words of `query`, ignoring case, accents and the marks that join words. A year after
    /// other words keeps only those of that year, or with it in their title. Only the first page
    /// counts them all.
    pub(crate) async fn search(
        &self,
        query: &str,
        page: &Page<Place>,
    ) -> Result<(Found, PageResponse), sqlx::Error> {
        let mut rows = match SearchWords::of(query) {
            Some(words) => self.search_rows(&words, page).await?,
            None => Vec::new(),
        };
        let total = rows.first().map_or(0, |row| row.total);
        let mut paging = page.finish(&mut rows, SearchRow::place);
        if page.is_first() {
            paging.total_size = Some(i32::try_from(total).unwrap_or(i32::MAX));
        }
        Ok((found(rows), paging))
    }

    /// A page of the titles whose `title_search` rows fit `words`, best fit first: each Movie,
    /// Show and Episode once per Library, by the row of its best-fitting Item.
    #[expect(
        clippy::too_many_lines,
        reason = "one query, which sqlx reads only as one literal"
    )]
    async fn search_rows(
        &self,
        words: &SearchWords<'_>,
        page: &Page<Place>,
    ) -> Result<Vec<SearchRow>, sqlx::Error> {
        let (titles, shows) = (words.in_column("title"), words.in_column("show_title"));
        let (rank, by_show, id) = match page.after {
            Some(((rank, by_show), id)) => (Some(f64::from_bits(rank)), Some(by_show), Some(id)),
            None => (None, None, None),
        };
        let limit = page.limit();
        // A title is grouped by the id browsing knows it by, picked in SQL as `primary_id_in`
        // picks it, so the keyset and the count see each title once. SQLite can't order a
        // subquery by an outer column, so a window picks it.
        // ponytail: each page groups every hit, since a title's Items can be anywhere in FTS5's
        // order; the `LIMIT` caps what is read back. Store a title row to search if that's slow.
        macro_rules! select {
            ($ids:literal) => {
                sqlx::query_as!(
                    SearchRow,
                    r#"WITH hits (by_show, item_id, rank) AS (
                           SELECT 0, rowid, rank FROM title_search WHERE title_search MATCH ?1
                           UNION ALL
                           SELECT 1, rowid, rank FROM title_search WHERE title_search MATCH ?2
                       ),
                       ids AS (
                           SELECT hits.by_show, hits.item_id, hits.rank,
                                  library_files.library_id AS library,
                                  item_match_ids.namespace || '-' || item_match_ids.value
                                      AS title_key,
                                  row_number() OVER (
                                      PARTITION BY hits.by_show, hits.item_id
                                      ORDER BY instr(',' || libraries.title_sources || ',',
                                                     ',' || item_match_ids.namespace || ',') = 0,
                                               instr(',' || libraries.title_sources || ',',
                                                     ',' || item_match_ids.namespace || ','),
                                               item_match_ids.namespace
                                  ) AS preference
                           FROM hits
                           JOIN item_match_ids ON item_match_ids.item_id = hits.item_id
                                AND item_match_ids.scope = iif(hits.by_show, 'show', 'title')
                           JOIN library_items ON library_items.id = hits.item_id
                           JOIN library_files ON library_files.id = library_items.file_id
                           JOIN libraries ON libraries.id = library_files.library_id
                           -- An Episode is skipped as `Contents::shows` skips it, so browsing
                           -- can open what's found.
                           WHERE hits.by_show OR library_items.kind = 'movie'
                              OR (library_items.season IS NOT NULL
                                  AND library_items.episode IS NOT NULL
                                  AND EXISTS (SELECT 1 FROM item_match_ids AS show_ids
                                              WHERE show_ids.item_id = hits.item_id
                                                AND show_ids.scope = 'show'))
                       ),
                       grouped AS (
                           SELECT *, row_number() OVER (
                               PARTITION BY by_show, library, title_key ORDER BY rank, item_id
                           ) AS nth
                           FROM ids WHERE preference = 1
                       ),
                       titles AS (
                           SELECT by_show, item_id, rank, library, title_key,
                                  count(*) OVER () AS total
                           FROM grouped WHERE nth = 1
                       ),
                       page AS (
                           SELECT * FROM titles
                           WHERE ?3 IS NULL OR (rank, by_show, item_id) > (?3, ?4, ?5)
                           ORDER BY rank, by_show, item_id
                           LIMIT ?6
                       )
                       SELECT page.library AS "library!", page.by_show AS "by_show!: bool",
                              page.item_id AS "item_id!", page.rank AS "rank!: f64",
                              page.total AS "total!: i64",
                              library_items.kind AS "kind: ItemKind", library_items.season,
                              library_items.episode, "#
                        + $ids
                        + r#"'title') AS "ids?: String", "#
                        + $ids
                        + r#"'show') AS "show_ids?: String", item_matches.title AS "title!",
                              item_matches.show_title, item_matches.release_date,
                              item_matches.poster, item_matches.poster_plugin,
                              item_matches.show_poster, item_matches.show_poster_plugin,
                              item_matches.metadata_version, libraries.title_sources,
                              iif(page.by_show, (
                                  SELECT json_group_array(json_array(
                                      library_items.season, item_matches.poster,
                                      item_matches.poster_plugin))
                                  FROM grouped
                                  JOIN item_matches ON item_matches.item_id = grouped.item_id
                                  JOIN library_items ON library_items.id = grouped.item_id
                                  WHERE grouped.by_show AND grouped.library = page.library
                                    AND grouped.title_key = page.title_key
                              )) AS "seasons?: String"
                       FROM page
                       JOIN item_matches ON item_matches.item_id = page.item_id
                       JOIN library_items ON library_items.id = page.item_id
                       JOIN libraries ON libraries.id = page.library
                       ORDER BY page.rank, page.by_show, page.item_id"#,
                    titles,
                    shows,
                    rank,
                    by_show,
                    id,
                    limit
                )
            };
        }
        ids_sql!(select).fetch_all(self.db.reader()).await
    }
}

/// A title a search found, as [`Contents::search_rows`] reads it from its best-fitting Item.
struct SearchRow {
    library: i64,
    /// Whether it is the Show the Item is an Episode of, found by the Show's title.
    by_show: bool,
    item_id: i64,
    rank: f64,
    /// How many titles the whole search found.
    total: i64,
    kind: ItemKind,
    /// Set for an Episode.
    season: Option<i64>,
    /// Set for an Episode.
    episode: Option<i64>,
    /// As [`ids_sql`] gives them.
    ids: Option<String>,
    /// As [`ids_sql`] gives them.
    show_ids: Option<String>,
    title: String,
    show_title: Option<String>,
    release_date: Option<String>,
    poster: Option<String>,
    poster_plugin: Option<String>,
    show_poster: Option<String>,
    show_poster_plugin: Option<String>,
    metadata_version: i64,
    /// The Library's, as `libraries` stores them.
    title_sources: String,
    /// For a Show, a JSON array of the season number, poster and poster's Plugin of each of its
    /// Episodes the search found.
    seasons: Option<String>,
}

impl SearchRow {
    fn place(&self) -> (Place, i64) {
        ((self.rank.to_bits(), self.by_show), self.item_id)
    }

    fn ids(&self) -> ExternalIds {
        ids_from_json(self.ids.as_deref())
    }

    fn show_ids(&self) -> ExternalIds {
        ids_from_json(self.show_ids.as_deref())
    }

    /// As browsing identifies the title: see [`primary_id`](super::primary_id).
    fn primary_id(&self) -> Option<ExternalId> {
        primary_id_in(&self.ids(), &self.title_sources)
    }

    /// As browsing identifies the Show: see [`primary_id`](super::primary_id).
    fn show_primary_id(&self) -> Option<ExternalId> {
        primary_id_in(&self.show_ids(), &self.title_sources)
    }

    /// The Seasons of the Show's Episodes the search found, with their posters, in any order.
    fn seasons(&self) -> impl Iterator<Item = (i64, Option<ImageRef>)> + use<> {
        let seasons: Vec<(Option<i64>, Option<String>, Option<String>)> = (self.seasons)
            .as_deref()
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default();
        (seasons.into_iter()).filter_map(|(season, poster, plugin)| {
            Some((season?, ImageRef::stored(poster, plugin)))
        })
    }
}

/// What the search `rows`, best fit first, found.
fn found(rows: Vec<SearchRow>) -> Found {
    let mut found = Found::default();
    for row in rows {
        if row.by_show {
            found.shows.extend(found_show(row));
            continue;
        }
        let Some(id) = row.primary_id() else {
            continue;
        };
        match (row.kind, row.season, row.episode, row.show_primary_id()) {
            (ItemKind::Episode, Some(season), Some(number), Some(show)) => {
                found.episodes.push(FoundEpisode {
                    library: row.library,
                    id,
                    show,
                    show_title: row.show_title.unwrap_or_default(),
                    season,
                    number,
                    title: row.title,
                });
            }
            (ItemKind::Episode, ..) => {}
            (ItemKind::Movie, ..) => found.movies.push(FoundMovie {
                library: row.library,
                id,
                ids: row.ids(),
                title: row.title,
                release_date: row.release_date,
                poster: ImageRef::stored(row.poster, row.poster_plugin),
            }),
        }
    }
    found
}

/// The Show that `row`, found by the Show's title, is an Episode of.
fn found_show(row: SearchRow) -> Option<FoundShow> {
    let id = row.show_primary_id()?;
    let ids = row.show_ids();
    // Every Episode of a Show fits its title alike, so its rows hold all its Seasons, unless
    // the search ends in a year: that keeps only the Episodes aired in it.
    // ponytail: a year can pick another Season's Poster than the Library grid's; search the
    // Show's rows again without it if that's ever seen.
    let seasons = row.seasons();
    let own = ImageRef::stored(row.show_poster, row.show_poster_plugin);
    Some(FoundShow {
        library: row.library,
        id,
        ids,
        title: row.show_title?,
        poster: show_poster(own, row.metadata_version, seasons),
    })
}

/// A search query's words, and the year it ends with, if any.
#[derive(Debug, PartialEq, Eq)]
struct SearchWords<'a> {
    words: Vec<&'a str>,
    year: Option<&'a str>,
}

impl<'a> SearchWords<'a> {
    /// `query`'s runs of letters and digits, or `None` if it has none. A last word of four digits
    /// after others is the year.
    fn of(query: &'a str) -> Option<Self> {
        let mut words: Vec<&str> = query
            .split(|c: char| !c.is_alphanumeric())
            .filter(|word| !word.is_empty())
            .collect();
        let year = match words.as_slice() {
            [_, .., last] if last.len() == 4 && last.bytes().all(|b| b.is_ascii_digit()) => {
                words.pop()
            }
            _ => None,
        };
        (!words.is_empty()).then_some(Self { words, year })
    }

    /// The FTS5 query for `title_search` rows whose `column` has a word starting with each word,
    /// so a client can search as the User types, and of the year, if any.
    fn in_column(&self, column: &str) -> String {
        // Every word is letters and digits, so quoting it keeps FTS5 from reading it as `AND`,
        // `NOT` or the like.
        let words: Vec<String> = self
            .words
            .iter()
            .map(|word| format!("\"{word}\"*"))
            .collect();
        let words = format!("{column} : ({})", words.join(" "));
        match self.year {
            Some(year) => format!("{words} AND (year : \"{year}\" OR {column} : \"{year}\")"),
            None => words,
        }
    }
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
