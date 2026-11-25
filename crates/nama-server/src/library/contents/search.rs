//! Searching every Library's Matched titles.

use std::cmp::{Ordering, Reverse};

use nama_plugin_api::metadata::{ExternalId, ExternalIds};

use super::{
    Contents, ImageRef, ItemKind, edit_distance::edit_distance, ids_from_json, ids_sql,
    matched_items, parse_hint, show_poster, sort_titles::folded,
};
use crate::library::TitleKind;

/// How many of the best fits by FTS5's rank [`Contents::search`] orders by [`Fit`].
// ponytail: a better fit ranked below these by bm25 is missed; raise it if one ever is.
const CANDIDATES: i64 = 500;

/// With fewer rows than this found by [`Contents::search_rows`], [`Contents::search`] corrects
/// the query's typos and searches again, as Silo's search does (#1459).
const FEW_FOUND: usize = 5;

/// A Movie, Show or Episode [`Contents::search`] found, once per Library that has it.
#[derive(Debug)]
pub(crate) enum Found {
    Movie(FoundMovie),
    /// Found by its own title.
    Show(FoundShow),
    /// Found by its own title, not its Show's.
    Episode(FoundEpisode),
}

impl Found {
    /// The Library it was found in.
    pub(crate) fn library(&self) -> i64 {
        match self {
            Self::Movie(FoundMovie { library, .. })
            | Self::Show(FoundShow { library, .. })
            | Self::Episode(FoundEpisode { library, .. }) => *library,
        }
    }

    /// Its Content Rating, an Episode's its Show's.
    pub(crate) fn content_rating(&self) -> Option<&str> {
        match self {
            Self::Movie(FoundMovie { content_rating, .. })
            | Self::Show(FoundShow { content_rating, .. })
            | Self::Episode(FoundEpisode { content_rating, .. }) => content_rating.as_deref(),
        }
    }
}

/// A Movie a search found, with the metadata the Match of its best-fitting Item stores.
#[derive(Debug, Clone)]
pub(crate) struct FoundMovie {
    pub(crate) library: i64,
    /// Its preferred id, its `own_id` in `matched_items`.
    pub(crate) id: ExternalId,
    /// Every id its Match gives it.
    pub(crate) ids: ExternalIds,
    pub(crate) title: String,
    pub(crate) release_date: Option<String>,
    pub(crate) poster: Option<ImageRef>,
    pub(crate) content_rating: Option<String>,
}

/// A Show a search found, with the metadata the Match of its best-fitting Episode stores.
#[derive(Debug, Clone)]
pub(crate) struct FoundShow {
    pub(crate) library: i64,
    /// Its preferred id, its `listed_id` in `matched_items`.
    pub(crate) id: ExternalId,
    /// Every id the Match of its first Episode found gives it.
    pub(crate) ids: ExternalIds,
    pub(crate) title: String,
    pub(crate) first_air_date: Option<String>,
    /// As [`show_poster`] picks it, from the Match and the Seasons of the Show's Episodes the
    /// search found.
    pub(crate) poster: Option<ImageRef>,
    pub(crate) content_rating: Option<String>,
}

/// An Episode a search found, with what browsing needs to find it: its Show and Season.
#[derive(Debug)]
pub(crate) struct FoundEpisode {
    pub(crate) library: i64,
    /// Its preferred id, its `own_id` in `matched_items`.
    pub(crate) id: ExternalId,
    /// Its Show's preferred id, its `listed_id` in `matched_items`.
    pub(crate) show: ExternalId,
    /// Every id its Match gives its Show.
    pub(crate) show_ids: ExternalIds,
    pub(crate) show_title: String,
    /// As [`show_poster`] picks it, from the Match and the Episode's Season.
    pub(crate) show_poster: Option<ImageRef>,
    pub(crate) season: i64,
    /// The episode number within its Season.
    pub(crate) number: i64,
    pub(crate) title: String,
    pub(crate) still: Option<ImageRef>,
    /// Its Show's.
    pub(crate) content_rating: Option<String>,
}

impl Contents {
    /// The best `limit` Matched Movies, Shows and Episodes that `seen` keeps whose
    /// titles have each of the words of `query`, ignoring case, accents and the marks that join
    /// words, of `kind` only if it's set, a Show's with its Episodes. A year after other words
    /// keeps only those of that year, or with it in their title. Ordered as ADR 0080 says: Movies
    /// and Shows before Episodes, then by [`Fit`], then the year asked for, FTS5's rank, and the
    /// shorter title. With fewer than [`FEW_FOUND`] found, the titles a search with its typos
    /// corrected finds come after all those.
    pub(crate) async fn search(
        &self,
        query: &str,
        kind: Option<TitleKind>,
        limit: usize,
        seen: impl Fn(&Found) -> bool,
    ) -> Result<Vec<Found>, sqlx::Error> {
        let Some(words) = SearchWords::of(query) else {
            return Ok(Vec::new());
        };
        let rows = self.search_rows(&words, kind).await?;
        let few_found = rows.len() < FEW_FOUND;
        let mut found = ranked(rows, &words);
        // Only a word no indexed word starts with is corrected, and it finds nothing, so a
        // corrected search adds to an empty batch: nothing it finds is already in it.
        if few_found && words.correctable() {
            let vocab = self.indexed_words().await?;
            if let Some(corrected) = words.corrected(&vocab) {
                let corrected_rows = self.search_rows(&corrected, kind).await?;
                found.extend(ranked(corrected_rows, &corrected));
            }
        }
        // ponytail: filtered after the SQL keeps the best `CANDIDATES` of every Library, so titles
        // a User doesn't see can crowd out their own; filter in SQL if that's ever seen.
        found.retain(seen);
        found.truncate(limit);
        Ok(found)
    }

    /// Every word `title_search` holds, from `title_search_vocab`.
    async fn indexed_words(&self) -> Result<Vec<IndexedWord>, sqlx::Error> {
        // ponytail: reads every word the Libraries' titles hold; read only those of a length
        // within reach if a big Library's searches get slow.
        // The casts keep sqlx's compile-time check from crashing on an `fts5vocab` column.
        sqlx::query_as!(
            IndexedWord,
            r#"SELECT CAST(term AS TEXT) AS "word!: String", CAST(doc AS INTEGER) AS "titles!: i64"
               FROM title_search_vocab"#
        )
        .fetch_all(self.db.reader())
        .await
    }

    /// The best [`CANDIDATES`] titles whose `title_search` rows fit `words`, of `kind` only if
    /// it's set, by FTS5's rank: each Movie, Show and Episode once per Library, by the row of its
    /// best-fitting Item.
    #[expect(clippy::too_many_lines, reason = "one query, whose SQL is most of it")]
    async fn search_rows(
        &self,
        words: &SearchWords<'_>,
        kind: Option<TitleKind>,
    ) -> Result<Vec<SearchRow>, sqlx::Error> {
        let titles = words.in_columns("{title original_title}");
        let shows = words.in_columns("show_title");
        let movies_wanted = kind != Some(TitleKind::Show);
        let shows_wanted = kind != Some(TitleKind::Movie);
        // A hit is grouped by the id browsing knows its title by, which `matched_items` picks,
        // so each title is read once.
        // ponytail: every search groups every hit, since a title's Items can be anywhere in
        // FTS5's order; the `LIMIT` caps what is read back.
        macro_rules! select {
            ($ids:literal, ($($matched:tt)+)) => {
                sqlx::query_as!(
                    SearchRow,
                    r#"WITH hits (by_show, item_id, rank) AS (
                           SELECT 0, rowid, rank FROM title_search WHERE title_search MATCH ?1
                           UNION ALL
                           SELECT 1, rowid, rank FROM title_search WHERE title_search MATCH ?2
                       ),
                       titled AS (
                           SELECT hits.by_show, hits.item_id, hits.rank,
                                  matched_items.library_id AS library, matched_items.own_id,
                                  matched_items.listed_id,
                                  iif(hits.by_show, matched_items.listed_id, matched_items.own_id)
                                      AS title_key
                           FROM hits
                           JOIN "#
                        + $($matched)+
                        + r#" AS matched_items ON matched_items.item_id = hits.item_id
                           WHERE iif(matched_items.kind = 'movie', ?3, ?4)
                             -- A Movie's `listed_id` is its own, so a Show title its Match
                             -- holds mustn't make it a Show.
                             AND (NOT hits.by_show OR matched_items.kind = 'episode')
                       ),
                       grouped AS (
                           SELECT *, row_number() OVER (
                               PARTITION BY by_show, library, title_key ORDER BY rank, item_id
                           ) AS nth
                           FROM titled
                       ),
                       best AS (
                           SELECT by_show, item_id, rank, library, title_key, own_id, listed_id
                           FROM grouped WHERE nth = 1
                           ORDER BY rank, by_show, item_id
                           LIMIT ?5
                       )
                       SELECT best.library AS "library!", best.by_show AS "by_show!: bool",
                              best.rank AS "rank!: f64", best.own_id AS "own_id!: String",
                              best.listed_id AS "listed_id!: String",
                              library_items.kind AS "kind: ItemKind", library_items.season,
                              library_items.episode, "#
                        + $ids
                        + r#"'title') AS "ids?: String", "#
                        + $ids
                        + r#"'show') AS "show_ids?: String", item_matches.title AS "title!",
                              item_matches.original_title, item_matches.show_title,
                              item_matches.release_date, item_matches.show_first_air_date,
                              plugin, still,
                              item_matches.poster, item_matches.poster_plugin,
                              item_matches.show_poster, item_matches.show_poster_plugin,
                              item_matches.metadata_version, item_matches.content_rating,
                              iif(best.by_show, (
                                  SELECT json_group_array(json_array(
                                      library_items.season, item_matches.poster,
                                      item_matches.poster_plugin))
                                  FROM grouped
                                  JOIN item_matches ON item_matches.item_id = grouped.item_id
                                  JOIN library_items ON library_items.id = grouped.item_id
                                  WHERE grouped.by_show AND grouped.library = best.library
                                    AND grouped.title_key = best.title_key
                              )) AS "seasons?: String"
                       FROM best
                       JOIN item_matches ON item_matches.item_id = best.item_id
                       JOIN library_items ON library_items.id = best.item_id
                       ORDER BY best.rank, best.by_show, best.item_id"#,
                    titles,
                    shows,
                    movies_wanted,
                    shows_wanted,
                    CANDIDATES
                )
            };
        }
        ids_sql!(matched_items, select)
            .fetch_all(self.db.reader())
            .await
    }
}

/// The titles `rows` found, ordered as [`Contents::search`] says, but those browsing couldn't
/// open.
fn ranked(rows: Vec<SearchRow>, words: &SearchWords<'_>) -> Vec<Found> {
    let mut ranked: Vec<(Rank, Found)> = (rows.into_iter())
        .filter_map(|row| {
            let rank = Rank::of(&row, words);
            Some((rank, found(row)?))
        })
        .collect();
    ranked.sort_by(|(a, _), (b, _)| a.order(b));
    ranked.into_iter().map(|(_, found)| found).collect()
}

/// A word `title_search` holds, as `title_search_vocab` lists it.
struct IndexedWord {
    /// Lowercase and without accents.
    word: String,
    /// How many of `title_search`'s rows hold it.
    titles: i64,
}

/// How many edits [`SearchWords::corrected`] may make to `word`, by how many letters it has.
fn edits_allowed(word: &str) -> usize {
    match word.chars().filter(|c| c.is_alphabetic()).count() {
        0..=3 => 0,
        4..=7 => 1,
        _ => 2,
    }
}

/// The word in `vocab` nearest `word`, as [`SearchWords::corrected`] picks it, unless one starts
/// with `word` or none is within [`edits_allowed`].
fn nearest<'v>(word: &str, vocab: &'v [IndexedWord]) -> Option<&'v str> {
    let allowed = edits_allowed(word);
    if allowed == 0 || vocab.iter().any(|indexed| indexed.word.starts_with(word)) {
        return None;
    }
    (vocab.iter())
        .map(|indexed| {
            let distance = edit_distance(word, &indexed.word);
            (distance, Reverse(indexed.titles), indexed.word.as_str())
        })
        .filter(|&(distance, ..)| distance <= allowed)
        .min()
        .map(|(.., word)| word)
}

/// A title a search found, as [`Contents::search_rows`] reads it from its best-fitting Item.
struct SearchRow {
    library: i64,
    /// Whether it is the Show the Item is an Episode of, found by the Show's title.
    by_show: bool,
    rank: f64,
    /// Its Item's preferred `title` id, as `matched_items` picks it.
    own_id: String,
    /// The preferred id of the title its Item is listed under, as `matched_items` picks it.
    listed_id: String,
    kind: ItemKind,
    /// Set for an Episode.
    season: Option<i64>,
    /// An Episode's Show's.
    content_rating: Option<String>,
    /// Set for an Episode.
    episode: Option<i64>,
    /// As [`ids_sql`] gives them.
    ids: Option<String>,
    /// As [`ids_sql`] gives them.
    show_ids: Option<String>,
    title: String,
    /// The title where it was made, which a search finds it by too.
    original_title: Option<String>,
    show_title: Option<String>,
    release_date: Option<String>,
    show_first_air_date: Option<String>,
    /// The Plugin whose title source made the Match, which the Still's path is for.
    plugin: Option<String>,
    still: Option<String>,
    poster: Option<String>,
    poster_plugin: Option<String>,
    show_poster: Option<String>,
    show_poster_plugin: Option<String>,
    metadata_version: i64,
    /// For a Show, a JSON array of the season number, poster and poster's Plugin of each of its
    /// Episodes the search found.
    seasons: Option<String>,
}

impl SearchRow {
    fn ids(&self) -> ExternalIds {
        ids_from_json(self.ids.as_deref())
    }

    fn show_ids(&self) -> ExternalIds {
        ids_from_json(self.show_ids.as_deref())
    }

    /// As browsing identifies the title.
    fn primary_id(&self) -> Option<ExternalId> {
        parse_hint(&self.own_id)
    }

    /// As browsing identifies the title the Item is listed under: an Episode's Show.
    fn show_primary_id(&self) -> Option<ExternalId> {
        parse_hint(&self.listed_id)
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

/// The Movie, Show or Episode `row` found, unless browsing couldn't open it.
fn found(row: SearchRow) -> Option<Found> {
    if row.by_show {
        return found_show(row).map(Found::Show);
    }
    let id = row.primary_id()?;
    match row.kind {
        ItemKind::Episode => found_episode(row, id).map(Found::Episode),
        ItemKind::Movie => Some(Found::Movie(FoundMovie {
            library: row.library,
            id,
            ids: row.ids(),
            title: row.title,
            release_date: row.release_date,
            poster: ImageRef::stored(row.poster, row.poster_plugin),
            content_rating: row.content_rating,
        })),
    }
}

/// The Episode `id` that `row` found by its own title, unless browsing couldn't open it.
fn found_episode(row: SearchRow, id: ExternalId) -> Option<FoundEpisode> {
    let (season, number, show) = (row.season?, row.episode?, row.show_primary_id()?);
    let show_ids = row.show_ids();
    // ponytail: a Match stored before Matches had a Show's own Poster stands for it with this
    // Episode's Season's, where the Show's grid takes its first Season's.
    let season_poster = ImageRef::stored(row.poster, row.poster_plugin);
    let own = ImageRef::stored(row.show_poster, row.show_poster_plugin);
    Some(FoundEpisode {
        library: row.library,
        id,
        show,
        show_ids,
        show_title: row.show_title.unwrap_or_default(),
        show_poster: show_poster(own, row.metadata_version, [(season, season_poster)]),
        season,
        number,
        title: row.title,
        still: ImageRef::stored(row.still, row.plugin),
        content_rating: row.content_rating,
    })
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
        first_air_date: row.show_first_air_date,
        poster: show_poster(own, row.metadata_version, seasons),
        content_rating: row.content_rating,
    })
}

/// How well `title`, of `year`, fits a search for `query`, in the bands [`Contents::search`]
/// orders by after the kind of title: lower is better. All alike for a query without words.
pub(crate) fn match_bands(query: &str, title: &str, year: Option<&str>) -> impl Ord + use<> {
    SearchWords::of(query).map(|words| words.bands(title, year))
}

/// Whether `name` has a word starting with each word of `query`, ignoring case, accents and the
/// marks that join words, as a search finds a title. A query with no words, such as "!!!", fits
/// none.
pub(crate) fn has_word_starts(query: &str, name: &str) -> bool {
    let query = title_words(query);
    if query.is_empty() {
        return false;
    }
    let joined = joined(name);
    [name, joined.as_str()].into_iter().any(|variant| {
        let words = title_words(variant);
        (query.iter()).all(|asked| words.iter().any(|word| word.starts_with(asked.as_str())))
    })
}

/// `title` without the marks `title_search_rows` joins words by: keep the two alike.
fn joined(title: &str) -> String {
    title.replace(['-', '\'', '’', '.', '·'], "")
}

/// A search query's words, and the year it ends with, if any.
#[derive(Debug, PartialEq, Eq)]
struct SearchWords<'a> {
    words: Vec<&'a str>,
    year: Option<&'a str>,
    /// The words, then the year, lowercase and without accents, as [`Fit`] compares them.
    compared: Vec<String>,
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
        (!words.is_empty()).then(|| Self::new(words, year))
    }

    fn new(words: Vec<&'a str>, year: Option<&'a str>) -> Self {
        let compared = words.iter().chain(&year).map(|word| folded(word)).collect();
        Self {
            words,
            year,
            compared,
        }
    }

    /// The words, lowercase and without accents, without the year.
    fn compared_words(&self) -> &[String] {
        self.compared.get(..self.words.len()).unwrap_or_default()
    }

    /// Whether some word is long enough for [`Self::corrected`] to correct.
    fn correctable(&self) -> bool {
        (self.compared_words().iter()).any(|word| edits_allowed(word) > 0)
    }

    /// These words with each of 4 or more letters that no word in `vocab` starts with replaced
    /// by the nearest one, by [`edit_distance`] within [`edits_allowed`], then by the most
    /// titles holding it, then alphabetically; `None` if no word is replaced.
    fn corrected<'s>(&'s self, vocab: &'s [IndexedWord]) -> Option<SearchWords<'s>> {
        let words = self.compared_words();
        let corrections: Vec<Option<&str>> =
            words.iter().map(|word| nearest(word, vocab)).collect();
        if corrections.iter().all(Option::is_none) {
            return None;
        }
        let corrected = (words.iter().zip(corrections))
            .map(|(word, correction)| correction.unwrap_or(word))
            .collect();
        Some(SearchWords::new(corrected, self.year))
    }

    /// How `title`, of `year`, fits these words, and whether the year they end with isn't its.
    fn bands(&self, title: &str, year: Option<&str>) -> (Fit, bool) {
        (
            Fit::of(self, title, year),
            self.year.is_some() && self.year != year,
        )
    }

    /// The FTS5 query for `title_search` rows whose `columns`, an FTS5 column filter, have a word
    /// starting with each word, so a client can search as the User types, and of the year, if any.
    fn in_columns(&self, columns: &str) -> String {
        // Every word is letters and digits, so quoting it keeps FTS5 from reading it as `AND`,
        // `NOT` or the like.
        let words: Vec<String> = self
            .words
            .iter()
            .map(|word| format!("\"{word}\"*"))
            .collect();
        let words = format!("{columns} : ({})", words.join(" "));
        match self.year {
            Some(year) => format!("{words} AND (year : \"{year}\" OR {columns} : \"{year}\")"),
            None => words,
        }
    }
}

/// Where a found title sits in a search's order, best first (ADR 0080).
#[derive(Debug)]
struct Rank {
    episode: bool,
    fit: Fit,
    /// Whether the query ends in a year that isn't the title's.
    other_year: bool,
    /// FTS5's bm25, lower for a better fit.
    bm25: f64,
    title_length: usize,
}

impl Rank {
    fn of(row: &SearchRow, words: &SearchWords<'_>) -> Self {
        // `item_matches` holds no Show's original title, so a Show is graded by its title alone.
        let (title, original_title) = match (row.by_show, &row.show_title) {
            (true, Some(show_title)) => (show_title, None),
            _ => (&row.title, row.original_title.as_deref()),
        };
        let year = row.release_date.as_deref().and_then(|date| date.get(..4));
        let (fit, other_year) = words.bands(title, year);
        let fit = original_title.map_or(fit, |original| fit.min(Fit::of(words, original, year)));
        Self {
            episode: !row.by_show && row.kind == ItemKind::Episode,
            fit,
            other_year,
            bm25: row.rank,
            title_length: title.chars().count(),
        }
    }

    fn order(&self, other: &Self) -> Ordering {
        let bands = |rank: &Self| (rank.episode, rank.fit, rank.other_year);
        (bands(self).cmp(&bands(other)))
            .then(self.bm25.total_cmp(&other.bm25))
            .then(self.title_length.cmp(&other.title_length))
    }
}

/// How well a found title fits a query's words, ignoring case, accents and punctuation, best
/// first. Every title found has a word starting with each of them, so the worst is that.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Fit {
    /// The query is the title, or the title and its year.
    Exact,
    /// The title starts with the query, the year aside.
    Prefix,
    /// The title has each of the query's words whole, the year aside.
    AllWords,
    WordStarts,
}

impl Fit {
    /// How `title`, of `year`, fits `words`: its best fit with or without the marks that join
    /// words, as `title_search` holds it both ways.
    fn of(words: &SearchWords<'_>, title: &str, year: Option<&str>) -> Self {
        let query = &words.compared;
        // The year is the year band's to rank, past an exact title.
        let without_year = words.compared_words();
        let fit = |variant: &str| {
            let variant = title_words(variant);
            let with_year = year.map(|year| [&variant[..], &[year.to_owned()]].concat());
            if variant == *query || with_year.is_some_and(|with_year| with_year == *query) {
                Self::Exact
            } else if starts_with(&variant, without_year) {
                Self::Prefix
            } else if without_year.iter().all(|word| variant.contains(word)) {
                Self::AllWords
            } else {
                Self::WordStarts
            }
        };
        fit(title).min(fit(&joined(title)))
    }
}

/// `title`'s words, lowercase and without accents.
fn title_words(title: &str) -> Vec<String> {
    (folded(title).split(|c: char| !c.is_alphanumeric()))
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Whether `title` starts with `query`, whose last word may be only the start of one.
fn starts_with(title: &[String], query: &[String]) -> bool {
    let Some((last, whole)) = query.split_last() else {
        return false;
    };
    title.starts_with(whole)
        && (title.get(whole.len())).is_some_and(|word| word.starts_with(last.as_str()))
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
