//! Searching every Library's Matched titles.

use std::collections::HashSet;

use nama_plugin_api::metadata::{ExternalId, ExternalIds};

use super::{Contents, ImageRef, ItemKind, ids_from_json, ids_sql, primary_id_in, show_poster};

/// What [`Contents::search`] found in every Library, each kind best fit first, and each Movie,
/// Show or Episode once per Library that has it.
#[derive(Debug, Default)]
pub(crate) struct Found {
    pub(crate) movies: Vec<FoundMovie>,
    /// Found by their own titles.
    pub(crate) shows: Vec<FoundShow>,
    /// Found by their own titles, not their Shows'.
    pub(crate) episodes: Vec<FoundEpisode>,
}

/// A Movie a search found, with the metadata the Match of its best-fitting Item stores.
#[derive(Debug)]
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
#[derive(Debug)]
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

impl Contents {
    /// The Matched Movies, Shows and Episodes of every Library whose titles have each of the
    /// words of `query`, ignoring case, accents and the marks that join words. A year after
    /// other words keeps only those of that year, or with it in their title.
    pub(crate) async fn search(&self, query: &str) -> Result<Found, sqlx::Error> {
        let mut found = Found::default();
        let Some(words) = SearchWords::of(query) else {
            return Ok(found);
        };
        for row in self.search_rows(&words.in_column("title")).await? {
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
                // Skipped as `Contents::shows` skips it, so browsing can open what's found.
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
        found.shows = found_shows(self.search_rows(&words.in_column("show_title")).await?);
        // A Movie or Episode has a row per Item; the first fits best.
        let mut seen = HashSet::new();
        found
            .movies
            .retain(|movie| seen.insert((movie.library, movie.id.clone())));
        seen.clear();
        found
            .episodes
            .retain(|episode| seen.insert((episode.library, episode.id.clone())));
        Ok(found)
    }

    /// The Matched Items whose `title_search` row fits the FTS5 query `fts_query`, best fit first.
    async fn search_rows(&self, fts_query: &str) -> Result<Vec<SearchRow>, sqlx::Error> {
        macro_rules! select {
            ($ids:literal) => {
                sqlx::query_as!(
                    SearchRow,
                    r#"SELECT library_files.library_id AS library,
                              library_items.kind AS "kind: ItemKind", library_items.season,
                              library_items.episode, "#
                        + $ids
                        + r#"'title') AS "ids?: String", "#
                        + $ids
                        + r#"'show') AS "show_ids?: String", item_matches.title AS "title!",
                              item_matches.show_title, item_matches.release_date,
                              item_matches.poster, item_matches.poster_plugin,
                              item_matches.show_poster, item_matches.show_poster_plugin,
                              item_matches.metadata_version, libraries.title_sources
                       FROM title_search
                       JOIN item_matches ON item_matches.item_id = title_search.rowid
                       JOIN library_items ON library_items.id = item_matches.item_id
                       JOIN library_files ON library_files.id = library_items.file_id
                       JOIN libraries ON libraries.id = library_files.library_id
                       WHERE title_search MATCH ?
                       ORDER BY title_search.rank"#,
                    fts_query
                )
            };
        }
        ids_sql!(select).fetch_all(self.db.reader()).await
    }
}

/// An Item a search found, as [`Contents::search_rows`] reads it.
struct SearchRow {
    library: i64,
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
}

impl SearchRow {
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
}

/// The Shows that `rows`, best fit first, are Episodes of: each once per Library, in the place of
/// its first row.
fn found_shows(rows: Vec<SearchRow>) -> Vec<FoundShow> {
    let rows: Vec<(ExternalId, SearchRow)> = (rows.into_iter())
        .filter_map(|row| Some((row.show_primary_id()?, row)))
        .collect();
    let mut seen = HashSet::new();
    let mut shows = Vec::new();
    for (id, row) in &rows {
        let Some(title) = &row.show_title else {
            continue;
        };
        if !seen.insert((row.library, id)) {
            continue;
        }
        // Every Episode of a Show fits its title alike, so its rows hold all its Seasons, unless
        // the search ends in a year: that keeps only the Episodes aired in it.
        // ponytail: a year can pick another Season's Poster than the Library grid's; search the
        // Show's rows again without it if that's ever seen.
        // ponytail: rescans every row per Show found; group rows by Show once searches get slow.
        let seasons = (rows.iter())
            .filter(|(other_id, other)| other.library == row.library && other_id == id)
            .filter_map(|(_, other)| {
                let poster = ImageRef::stored(other.poster.clone(), other.poster_plugin.clone());
                Some((other.season?, poster))
            });
        shows.push(FoundShow {
            library: row.library,
            id: id.clone(),
            ids: row.show_ids(),
            title: title.clone(),
            poster: show_poster(
                ImageRef::stored(row.show_poster.clone(), row.show_poster_plugin.clone()),
                row.metadata_version,
                seasons,
            ),
        });
    }
    shows
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
