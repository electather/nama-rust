//! A page of a Library's Movies or Shows, filtered and sorted in SQL (ADR 0055).

use nama_plugin_api::metadata::ExternalId;
use nama_proto::nama::v1::PageResponse;

use super::{Contents, library_titles, parse_hint};
use crate::page::Page;

/// What a page of a Library's Movies or Shows is sorted by, tie-broken by the title's first Item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TitleSort {
    /// By sort title (`sort_titles.rs`).
    Title,
    /// By when it was added, as `library_titles` has it.
    Added,
    /// By release date, or first air date, those without one first.
    Released,
}

/// Which titles a page keeps, by a User's Watch State of them: a Show's rolled up from its
/// Episodes'.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Watched {
    /// None started.
    NotStarted,
    /// Some started or played, not all played.
    Started,
    /// All played.
    Played,
}

/// Which of a Library's Movies or Shows a page lists, and in which order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Listing {
    /// Those with any of these genres, or all if empty.
    pub(crate) genres: Vec<String>,
    /// Those released, or first aired, in this year or later.
    pub(crate) min_year: Option<u32>,
    /// Those released, or first aired, in this year or earlier.
    pub(crate) max_year: Option<u32>,
    pub(crate) watched: Option<Watched>,
    pub(crate) sort: TitleSort,
    pub(crate) descending: bool,
}

impl Contents {
    /// A page of the Movies of the Movies Library `library`, or the Shows of the Shows Library,
    /// that `listing` keeps, in its order, each by the id it is listed under, and what the page
    /// says of the rest; only the first page counts them. `started` is the User's Watch States of
    /// the Movies, or Episodes, they've started or played: each by an id in its text form, and
    /// whether it is played.
    #[expect(clippy::too_many_lines, reason = "one query, whose SQL is most of it")]
    pub(crate) async fn title_page(
        &self,
        library: i64,
        listing: &Listing,
        started: &[(String, bool)],
        page: &Page<String>,
    ) -> Result<(Vec<ExternalId>, PageResponse), sqlx::Error> {
        let started =
            serde_json::to_string(started).map_err(|err| sqlx::Error::Encode(err.into()))?;
        let sort = match listing.sort {
            TitleSort::Title => "title",
            TitleSort::Added => "added",
            TitleSort::Released => "released",
        };
        let genres = (!listing.genres.is_empty())
            .then(|| serde_json::Value::from(listing.genres.clone()).to_string());
        let watched = listing.watched.map(|watched| match watched {
            Watched::NotStarted => "not_started",
            Watched::Started => "started",
            Watched::Played => "played",
        });
        let (after_key, after_id) = page.after.clone().unzip();
        let limit = page.limit();
        // ponytail: whole-Library read, grouped by `library_titles`.
        macro_rules! select {
            (($($titles:tt)+)) => {
                sqlx::query!(
                    r#"WITH started (title_id, played) AS MATERIALIZED (
                           SELECT json_extract(value, '$[0]'), json_extract(value, '$[1]')
                           FROM json_each(?2)),
                       items AS (
                           SELECT library_titles.*,
                                  (SELECT MAX(started.played) FROM item_match_ids AS own
                                   JOIN started ON started.title_id = own.namespace || '-' || own.value
                                   WHERE own.item_id = library_titles.item_id
                                     AND own.scope = 'title') AS played
                           FROM "#
                        + $($titles)+
                        + r#" AS library_titles
                           WHERE library_titles.library_id = ?1),
                       titles AS (
                           SELECT item_id, listed_id, title_item, kind, added_at, nth,
                                  CASE WHEN TOTAL(played) OVER title = COUNT(*) OVER title
                                           THEN 'played'
                                       WHEN COUNT(played) OVER title > 0 THEN 'started'
                                       ELSE 'not_started' END AS watched
                           FROM items
                           WINDOW title AS (PARTITION BY listed_id)),
                       listed AS (
                           SELECT listed_id, title_item AS id,
                                  -- Text for every sort, so one token type holds any key; the
                                  -- padding orders times as numbers.
                                  CASE ?3 WHEN 'added' THEN printf('%020d', added_at)
                                          WHEN 'released' THEN coalesce(released, '')
                                          ELSE coalesce(sort_title, '') END AS sort_key
                           FROM (SELECT titles.*, item_matches.sort_title, item_matches.genres,
                                        IIF(titles.kind = 'movie', item_matches.release_date,
                                            item_matches.show_first_air_date) AS released
                                 FROM titles
                                 JOIN item_matches ON item_matches.item_id = titles.item_id
                                 WHERE titles.nth = 1) AS shown
                           WHERE (?4 IS NULL
                                  OR EXISTS (SELECT 1 FROM json_each(shown.genres) AS genre
                                             WHERE genre.value IN (SELECT value
                                                                   FROM json_each(?4))))
                             AND (?5 IS NULL OR CAST(substr(released, 1, 4) AS INTEGER) >= ?5)
                             AND (?6 IS NULL OR CAST(substr(released, 1, 4) AS INTEGER) <= ?6)
                             AND (?7 IS NULL OR watched = ?7))
                       SELECT listed_id AS "listed_id!: String", id AS "id!: i64",
                              sort_key AS "sort_key!: String", COUNT(*) OVER () AS "total!: i64"
                       FROM listed
                       WHERE ?9 IS NULL
                          OR (?8 AND (sort_key, id) < (?9, ?10))
                          OR (NOT ?8 AND (sort_key, id) > (?9, ?10))
                       ORDER BY IIF(?8, sort_key, NULL) DESC, IIF(?8, id, NULL) DESC, sort_key, id
                       LIMIT ?11"#,
                    library,
                    started,
                    sort,
                    genres,
                    listing.min_year,
                    listing.max_year,
                    watched,
                    listing.descending,
                    after_key,
                    after_id,
                    limit,
                )
            };
        }
        let mut rows = library_titles!(select).fetch_all(self.db.reader()).await?;
        // Every row holds the count of those the page's WHERE lets through: on the first page,
        // every title the listing keeps.
        let total = rows.first().map_or(0, |row| row.total);
        let mut paged = page.finish(&mut rows, |row| (row.sort_key.clone(), row.id));
        if page.is_first() {
            paged.total_size = Some(i32::try_from(total).unwrap_or(i32::MAX));
        }
        let ids = rows.iter().filter_map(|row| parse_hint(&row.listed_id));
        Ok((ids.collect(), paged))
    }
}

impl Contents {
    /// The genres of the titles the Library `library` lists, by name: those of the Match each
    /// title shows the metadata of, so every genre keeps a title in a page filtered by it.
    pub(crate) async fn genres(&self, library: i64) -> Result<Vec<String>, sqlx::Error> {
        // ponytail: whole-Library read, grouped by `library_titles`.
        macro_rules! select {
            (($($titles:tt)+)) => {
                sqlx::query_scalar!(
                    r#"SELECT DISTINCT CAST(genre.value AS TEXT) AS "genre!: String"
                       FROM "#
                        + $($titles)+
                        + r#" AS library_titles
                       JOIN item_matches ON item_matches.item_id = library_titles.item_id
                       JOIN json_each(item_matches.genres) AS genre
                       WHERE library_titles.library_id = ? AND library_titles.nth = 1
                       ORDER BY genre.value"#,
                    library
                )
            };
        }
        library_titles!(select).fetch_all(self.db.reader()).await
    }
}

#[cfg(test)]
#[path = "pages_tests.rs"]
mod tests;
