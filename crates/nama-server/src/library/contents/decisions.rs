//! What Matching decided about each Item, and storing it.

use std::{borrow::Cow, collections::HashMap};

use nama_plugin_api::{
    PluginId,
    metadata::{ExternalId, ExternalIds},
};
use nama_proto::nama::v1::{Event, PageResponse, SortDirection, UnmatchedItemSort};
use sqlx::SqliteConnection;

use super::{
    Contents, Item, ItemKind, ItemRow, Pin, air_dates, ids_from_json, ids_sql, parse_hint,
    primary_id_in, scores, sort_titles::sort_title,
};
use crate::{
    db::now,
    library::title_source::{Candidate, Season},
    library::versions::METADATA_VERSION,
    page::Page,
    requests::{make_available, make_seasons_available},
};

#[derive(Debug)]
pub(crate) struct Unmatched {
    pub(crate) id: i64,
    /// The Version that holds it.
    pub(crate) version_id: i64,
    pub(crate) path: String,
    pub(crate) item: Item,
    /// `None` while Matching hasn't decided, or for an Unmatched reason not stored.
    pub(crate) reason: Option<UnmatchedReason>,
    /// Whether Matching hasn't decided about it yet, so the next scan tries it again.
    pub(crate) undecided: bool,
    /// Whether an admin pinned it Unmatched with `ClearMatch`. Read from the pin, not stored as
    /// a reason, since `clear_match` stores none.
    pub(crate) cleared: bool,
}

/// Where an Unmatched Item sits in either order [`Contents::unmatched`] lists in: when a scan
/// found it, and its file's path, which is empty when listing by when found (ADR 0069).
pub(crate) type UnmatchedPlace = (i64, String);

/// Hands `$query!` the SQL for the Items of the Library `?1` that need a Match, one row each in
/// the shape of [`UnmatchedRow`], ending in a `WHERE` that more conditions can follow with `AND`.
macro_rules! unmatched_sql {
    ($query:ident) => {
        $query!(
            r#"SELECT library_items.id AS "id!", library_files.id AS "version_id!",
                      library_files.path, library_items.added_at, library_items.kind AS "kind: ItemKind",
                      library_items.title, library_items.year, library_items.hint,
                      library_items.season, library_items.episode,
                      item_matches.unmatched_reason AS "unmatched_reason?: ReasonColumn",
                      item_matches.tied_titles AS "tied_titles?",
                      item_matches.item_id IS NULL AS "undecided!: bool",
                      EXISTS (SELECT 1 FROM match_pins
                              WHERE match_pins.library_id = library_files.library_id
                                AND match_pins.kind = library_items.kind
                                AND match_pins.title = library_items.title
                                AND match_pins.year IS library_items.year
                                AND match_pins.hint IS library_items.hint
                                AND match_pins.pinned_id IS NULL) AS "cleared!: bool"
               FROM library_items
               JOIN library_files ON library_files.id = library_items.file_id
               LEFT JOIN item_matches ON item_matches.item_id = library_items.id
               WHERE library_files.library_id = ?1
                 AND item_matches.state IS NOT 'matched'"#
        )
    };
}

/// A row of [`Contents::unmatched`], before it is read into an [`Unmatched`].
struct UnmatchedRow {
    id: i64,
    version_id: i64,
    path: String,
    added_at: i64,
    kind: ItemKind,
    title: String,
    year: Option<i64>,
    hint: Option<String>,
    season: Option<i64>,
    episode: Option<i64>,
    unmatched_reason: Option<ReasonColumn>,
    tied_titles: Option<String>,
    undecided: bool,
    cleared: bool,
}

impl UnmatchedRow {
    /// Where the row sits in the list, as its page token carries it: the path only when listing
    /// `by_found` doesn't, since a token is readable and a User who may not see paths pages by when
    /// found (ADR 0069).
    fn place(&self, by_found: bool) -> (UnmatchedPlace, i64) {
        let path = if by_found {
            String::new()
        } else {
            self.path.clone()
        };
        ((self.added_at, path), self.id)
    }

    /// The row read in full, `None` if its Item no longer reads as one, or an error if its
    /// stored reason doesn't.
    fn into_unmatched(self) -> Option<Result<Unmatched, sqlx::Error>> {
        let reason = self
            .unmatched_reason
            .map(|column| column.reason(self.tied_titles.as_deref()))
            .transpose();
        let item = ItemRow {
            kind: self.kind,
            title: Cow::Owned(self.title),
            year: self.year,
            hint: self.hint,
            season: self.season,
            episode: self.episode,
        }
        .into_item()?;
        Some(reason.map(|reason| Unmatched {
            undecided: self.undecided,
            id: self.id,
            version_id: self.version_id,
            path: self.path,
            item,
            reason,
            cleared: self.cleared,
        }))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Decided {
    Matched(Box<Metadata>),
    Unmatched(Option<UnmatchedReason>),
}

/// Why automatic Matching left an Item Unmatched, which says how an admin fixes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UnmatchedReason {
    /// More than one title fits about as well: these, best fit first.
    Ambiguous(Vec<TiedTitle>),
    /// No title the search found fits well enough.
    NoTitleFits,
    /// The Item's ID Hint names no title the title source has.
    UnknownIdHint,
    /// The Episode's Show, found by Matching or picked by hand, has no such Season or Episode.
    NotInShow,
}

/// A title that fit an Ambiguous Item about as well as the others.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TiedTitle {
    pub(crate) id: ExternalId,
    pub(crate) title: String,
    pub(crate) original_title: Option<String>,
    pub(crate) year: Option<i32>,
}

impl From<Candidate> for TiedTitle {
    fn from(candidate: Candidate) -> Self {
        Self {
            id: candidate.id,
            title: candidate.title,
            original_title: candidate.original_title,
            year: candidate.year,
        }
    }
}

/// What Matching decided about some Items, as one write stores it.
#[derive(Debug, Default)]
pub(crate) struct Decisions {
    /// Each Item, with what was decided about it.
    pub(crate) items: Vec<(i64, Decided)>,
    /// The Seasons fetched with their Show, as the Plugin's title source gave them, with every
    /// id of their Show. Their Requests follow how much of them a Library has, and their
    /// Episodes' air dates are stored.
    pub(crate) seasons: Vec<(PluginId, ExternalIds, Season)>,
}

impl From<Vec<(i64, Decided)>> for Decisions {
    fn from(items: Vec<(i64, Decided)>) -> Self {
        Self {
            items,
            seasons: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Metadata {
    /// The Plugin whose title source made the Match.
    pub(crate) plugin: PluginId,
    pub(crate) ids: ExternalIds,
    /// For an Episode, its Show's.
    pub(crate) show_ids: ExternalIds,
    pub(crate) title: String,
    pub(crate) original_title: Option<String>,
    pub(crate) show_title: Option<String>,
    pub(crate) overview: Option<String>,
    pub(crate) release_date: Option<String>,
    pub(crate) runtime_minutes: Option<u32>,
    pub(crate) poster: Option<ImageRef>,
    pub(crate) backdrop: Option<ImageRef>,
    /// The Backdrop with its title written on it, in the language artwork is picked in; for an
    /// Episode, its Show's, as is `backdrop`.
    pub(crate) titled_backdrop: Option<ImageRef>,
    /// The Title Logo; for an Episode, its Show's, as are `genres` and `content_rating`.
    pub(crate) logo: Option<ImageRef>,
    /// For an Episode, its Still, from the Match's [`plugin`](Self::plugin).
    pub(crate) still: Option<String>,
    pub(crate) genres: Vec<String>,
    pub(crate) content_rating: Option<String>,
    /// For an Episode, its Show's own Poster, as `poster` holds its Season's.
    pub(crate) show_poster: Option<ImageRef>,
    pub(crate) show_overview: Option<String>,
    pub(crate) show_first_air_date: Option<String>,
    /// The title's Score, stored once per title apart from the Match; for an Episode, its Show's,
    /// as is `original_language`.
    pub(crate) score: Option<f64>,
    pub(crate) original_language: Option<String>,
}

/// An image a Match stores: a path the title source of `plugin` serves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImageRef {
    /// The Plugin whose title source's `image` serves the path.
    pub plugin: PluginId,
    /// The path that source gave.
    pub path: String,
}

impl ImageRef {
    /// The image at `path`, if any, that `plugin`'s title source serves.
    pub(crate) fn of(plugin: &PluginId, path: Option<&String>) -> Option<Self> {
        let path = path?.clone();
        let plugin = plugin.clone();
        Some(Self { plugin, path })
    }

    /// An image's `path` column with its Plugin's, which the write that stores them pairs.
    pub(super) fn stored(path: Option<String>, plugin: Option<String>) -> Option<Self> {
        let (path, plugin) = path.zip(plugin)?;
        let plugin = plugin.into();
        Some(Self { plugin, path })
    }
}

impl Contents {
    /// The Items of the Library `library` that Matching hasn't decided about, with what an admin
    /// pinned for their names, in the order they were stored.
    pub(crate) async fn undecided(
        &self,
        library: i64,
    ) -> Result<Vec<(i64, Item, Option<Pin>)>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT library_items.id AS "id!", library_items.kind AS "kind: ItemKind",
                      library_items.title, library_items.year, library_items.hint,
                      library_items.season, library_items.episode,
                      match_pins.pinned_at IS NOT NULL AS "pinned!: bool",
                      match_pins.pinned_id
               FROM library_items
               JOIN library_files ON library_files.id = library_items.file_id
               LEFT JOIN match_pins ON match_pins.library_id = library_files.library_id
                                   AND match_pins.kind = library_items.kind
                                   AND match_pins.title = library_items.title
                                   AND match_pins.year IS library_items.year
                                   AND match_pins.hint IS library_items.hint
               WHERE library_files.library_id = ?
                 AND NOT EXISTS (SELECT 1 FROM item_matches
                                 WHERE item_matches.item_id = library_items.id)
               ORDER BY library_items.id"#,
            library
        )
        .fetch_all(self.db.reader())
        .await?;
        Ok(rows
            .into_iter()
            .filter_map(|row| {
                let item = ItemRow {
                    kind: row.kind,
                    title: Cow::Owned(row.title),
                    year: row.year,
                    hint: row.hint,
                    season: row.season,
                    episode: row.episode,
                }
                .into_item()?;
                Some((
                    row.id,
                    item,
                    Pin::from_row(row.pinned, row.pinned_id.as_deref()),
                ))
            })
            .collect())
    }

    /// The path of the file each Item of the Library `library` that Matching hasn't decided
    /// about is in, as stored: absolute, under one of the Library's folders.
    pub(crate) async fn paths_of_undecided(
        &self,
        library: i64,
    ) -> Result<HashMap<i64, String>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT library_items.id AS "id!", library_files.path
               FROM library_items
               JOIN library_files ON library_files.id = library_items.file_id
               WHERE library_files.library_id = ?
                 AND NOT EXISTS (SELECT 1 FROM item_matches
                                 WHERE item_matches.item_id = library_items.id)"#,
            library
        )
        .fetch_all(self.db.reader())
        .await?;
        Ok(rows.into_iter().map(|row| (row.id, row.path)).collect())
    }

    /// The Matched Items of the Library `library` stored at a metadata version older than
    /// [`METADATA_VERSION`], in the order they were stored, each with the Plugin that Matched it
    /// and its title's id: a Movie's, or an Episode's Show's (see
    /// [`primary_id`](super::primary_id)).
    pub(crate) async fn outdated(
        &self,
        library: i64,
    ) -> Result<Vec<(i64, Item, PluginId, ExternalId)>, sqlx::Error> {
        macro_rules! select {
            ($ids:literal) => {
                sqlx::query!(
                    r#"SELECT library_items.id AS "id!", library_items.kind AS "kind: ItemKind",
                              library_items.title, library_items.year, library_items.hint,
                              library_items.season, library_items.episode,
                              item_matches.plugin AS "plugin!", "#
                        + $ids
                        + r#"IIF(library_items.kind = 'movie', 'title', 'show'))
                                AS "ids?: String", libraries.title_sources
                       FROM item_matches
                       JOIN library_items ON library_items.id = item_matches.item_id
                       JOIN library_files ON library_files.id = library_items.file_id
                       JOIN libraries ON libraries.id = library_files.library_id
                       WHERE library_files.library_id = ?
                         AND item_matches.state = 'matched'
                         AND item_matches.metadata_version < ?
                       ORDER BY library_items.id"#,
                    library,
                    METADATA_VERSION
                )
            };
        }
        let rows = ids_sql!(select).fetch_all(self.db.reader()).await?;
        Ok(rows
            .into_iter()
            .filter_map(|row| {
                let title = primary_id_in(&ids_from_json(row.ids.as_deref()), &row.title_sources)?;
                let item = ItemRow {
                    kind: row.kind,
                    title: Cow::Owned(row.title),
                    year: row.year,
                    hint: row.hint,
                    season: row.season,
                    episode: row.episode,
                }
                .into_item()?;
                Some((row.id, item, PluginId::from(row.plugin), title))
            })
            .collect())
    }

    /// Stores what was `decided` about each Item, as of now, skipping Items that no longer exist.
    /// Returns the events for the Requests the Matches make Available or Partially Available.
    pub(crate) async fn record(
        &self,
        decided: impl Into<Decisions>,
    ) -> Result<Vec<Event>, sqlx::Error> {
        let decided = decided.into();
        let now = now();
        self.db
            .write(move |tx| Box::pin(insert_decisions(tx, decided, now)))
            .await
    }

    /// The page `page` of the Items of the Library `library` that need a Match, sorted by
    /// `order`'s key in its direction, an unspecified one running ascending: those Matching left
    /// Unmatched or hasn't decided about, including those pinned Unmatched. Only the first page
    /// counts them all.
    pub(crate) async fn unmatched(
        &self,
        library: i64,
        order: (UnmatchedItemSort, SortDirection),
        page: &Page<UnmatchedPlace>,
    ) -> Result<(Vec<Unmatched>, PageResponse), sqlx::Error> {
        let (sort, direction) = order;
        let descending = direction == SortDirection::SORT_DIRECTION_DESCENDING;
        let (after_place, after_id) = page.after.clone().unzip();
        let (added_at, path) = after_place.unzip();
        let limit = page.limit();
        // Each query runs either way: descending, its `DESC` terms order the rows and the plain
        // ones only tie; ascending, its `DESC` terms are all NULL, so the plain ones order them.
        macro_rules! by_path {
            ($unmatched:literal) => {
                sqlx::query_as!(
                    UnmatchedRow,
                    $unmatched
                        + " AND (?2 IS NULL
                                OR CASE WHEN ?5
                                   THEN (library_files.path, library_items.id) < (?2, ?3)
                                   ELSE (library_files.path, library_items.id) > (?2, ?3) END)
                           ORDER BY CASE WHEN ?5 THEN library_files.path END DESC,
                                    library_files.path,
                                    CASE WHEN ?5 THEN library_items.id END DESC,
                                    library_items.id
                           LIMIT ?4",
                    library,
                    path,
                    after_id,
                    limit,
                    descending,
                )
            };
        }
        macro_rules! by_found {
            ($unmatched:literal) => {
                sqlx::query_as!(
                    UnmatchedRow,
                    $unmatched
                        + " AND (?2 IS NULL
                                OR CASE WHEN ?5
                                   THEN (library_items.added_at, library_items.id) < (?2, ?3)
                                   ELSE (library_items.added_at, library_items.id) > (?2, ?3) END)
                           ORDER BY CASE WHEN ?5 THEN library_items.added_at END DESC,
                                    library_items.added_at,
                                    CASE WHEN ?5 THEN library_items.id END DESC,
                                    library_items.id
                           LIMIT ?4",
                    library,
                    added_at,
                    after_id,
                    limit,
                    descending,
                )
            };
        }
        macro_rules! count {
            ($unmatched:literal) => {
                sqlx::query_scalar!(
                    r#"SELECT COUNT(*) AS "count!: u32" FROM ("# + $unmatched + ")",
                    library
                )
            };
        }
        // One snapshot, so the count and the page agree.
        let mut tx = self.db.reader().begin().await?;
        // ponytail: SQLite sorts every Unmatched Item of the Library to list one page; index
        // `added_at` if Libraries with tens of thousands of them turn up.
        let by_found = sort == UnmatchedItemSort::UNMATCHED_ITEM_SORT_FOUND;
        let mut rows = if by_found {
            unmatched_sql!(by_found).fetch_all(&mut *tx).await?
        } else {
            unmatched_sql!(by_path).fetch_all(&mut *tx).await?
        };
        let mut paging = page.finish(&mut rows, |row| row.place(by_found));
        if page.is_first() {
            let total = unmatched_sql!(count).fetch_one(&mut *tx).await?;
            paging.total_size = Some(i32::try_from(total).unwrap_or(i32::MAX));
        }
        let unmatched = rows
            .into_iter()
            .filter_map(UnmatchedRow::into_unmatched)
            .collect::<Result<_, _>>()?;
        Ok((unmatched, paging))
    }

    /// The first Unmatched Item, by id, of the Library `library`'s Version `version`, or `None` if
    /// the Library has no such Version or it holds no Unmatched Item.
    pub(crate) async fn unmatched_in_version(
        &self,
        library: i64,
        version: i64,
    ) -> Result<Option<Unmatched>, sqlx::Error> {
        macro_rules! in_version {
            ($unmatched:literal) => {
                sqlx::query_as!(
                    UnmatchedRow,
                    $unmatched + " AND library_files.id = ?2 ORDER BY library_items.id LIMIT 1",
                    library,
                    version,
                )
            };
        }
        let row = unmatched_sql!(in_version)
            .fetch_optional(self.db.reader())
            .await?;
        row.and_then(UnmatchedRow::into_unmatched).transpose()
    }

    /// Forgets what Matching left Unmatched in the Library `library`, so it decides those Items
    /// again and asks the Library's Recognisers. An Item whose name an admin pinned keeps its
    /// decision, and a Matched one its Match. Part of the write a scan starts with when the
    /// Library's Recognisers changed (ADR 0050).
    pub(crate) async fn forget_unmatched(
        conn: &mut SqliteConnection,
        library: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "DELETE FROM item_matches
             WHERE state = 'unmatched'
               AND item_id IN (
                   SELECT library_items.id FROM library_items
                   JOIN library_files ON library_files.id = library_items.file_id
                   WHERE library_files.library_id = ?
                     AND NOT EXISTS (SELECT 1 FROM match_pins
                                     WHERE match_pins.library_id = library_files.library_id
                                       AND match_pins.kind = library_items.kind
                                       AND match_pins.title = library_items.title
                                       AND match_pins.year IS library_items.year
                                       AND match_pins.hint IS library_items.hint))",
            library
        )
        .execute(conn)
        .await?;
        Ok(())
    }
}

/// Returns the events for the Requests the Matches make Available or Partially Available, made in
/// the same write so no Request stays open for a Movie or Season a Library has.
pub(super) async fn insert_decisions(
    conn: &mut SqliteConnection,
    decided: Decisions,
    now: i64,
) -> Result<Vec<Event>, sqlx::Error> {
    let mut movies = Vec::new();
    for (item, decision) in decided.items {
        if let Decided::Matched(metadata) = &decision {
            movies.push(metadata.ids.clone());
        }
        let row = MatchRow::from(&decision);
        let written = sqlx::query!(
            "INSERT OR REPLACE INTO item_matches (
                 item_id, state, plugin, title, original_title, show_title,
                 overview, release_date, runtime_minutes, poster, poster_plugin, backdrop,
                 backdrop_plugin, titled_backdrop, titled_backdrop_plugin, logo, logo_plugin,
                 still, genres, content_rating, original_language, show_poster, show_poster_plugin,
                 show_overview, show_first_air_date, unmatched_reason, tied_titles,
                 metadata_version, decided_at, sort_title)
             SELECT ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
                    ?, ?, ?
             WHERE EXISTS (SELECT 1 FROM library_items WHERE id = ?)",
            item,
            row.state,
            row.plugin,
            row.metadata.title,
            row.metadata.original_title,
            row.metadata.show_title,
            row.metadata.overview,
            row.metadata.release_date,
            row.metadata.runtime_minutes,
            row.metadata.poster.map(|image| image.path),
            row.metadata.poster.map(|image| image.plugin),
            row.metadata.backdrop.map(|image| image.path),
            row.metadata.backdrop.map(|image| image.plugin),
            row.metadata.titled_backdrop.map(|image| image.path),
            row.metadata.titled_backdrop.map(|image| image.plugin),
            row.metadata.logo.map(|image| image.path),
            row.metadata.logo.map(|image| image.plugin),
            row.metadata.still,
            row.metadata.genres,
            row.metadata.content_rating,
            row.metadata.original_language,
            row.metadata.show_poster.map(|image| image.path),
            row.metadata.show_poster.map(|image| image.plugin),
            row.metadata.show_overview,
            row.metadata.show_first_air_date,
            row.unmatched_reason,
            row.tied_titles,
            METADATA_VERSION,
            now,
            row.metadata.sort_title,
            item,
        )
        .execute(&mut *conn)
        .await?;
        if written.rows_affected() == 0 {
            continue;
        }
        if let Decided::Matched(metadata) = &decision {
            scores::store(&mut *conn, metadata).await?;
        }
        insert_match_ids(&mut *conn, item, &row).await?;
    }
    let mut seasons = Vec::with_capacity(decided.seasons.len());
    for (plugin, show, season) in decided.seasons {
        air_dates::store(&mut *conn, &plugin, &show, &season).await?;
        seasons.push((show, season));
    }
    let mut happened = make_available(conn, movies).await?;
    happened.extend(make_seasons_available(conn, seasons).await?);
    Ok(happened)
}

/// Stores the ids of `row`'s title and its Show as the Item `item`'s, in place of those it had.
async fn insert_match_ids(
    conn: &mut SqliteConnection,
    item: i64,
    row: &MatchRow<'_>,
) -> Result<(), sqlx::Error> {
    // Deleted explicitly rather than trusting SQLite to cascade the delete REPLACE makes.
    sqlx::query!("DELETE FROM item_match_ids WHERE item_id = ?", item)
        .execute(&mut *conn)
        .await?;
    let ids =
        (row.ids.iter().map(|id| ("title", id))).chain(row.show_ids.iter().map(|id| ("show", id)));
    for (scope, id) in ids {
        let (namespace, value) = (id.namespace(), id.value());
        sqlx::query!(
            "INSERT INTO item_match_ids (item_id, scope, namespace, value) VALUES (?, ?, ?, ?)",
            item,
            scope,
            namespace,
            value
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
pub(super) enum MatchState {
    Matched,
    Unmatched,
}

/// An [`UnmatchedReason`] as `item_matches.unmatched_reason`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "snake_case")]
enum ReasonColumn {
    Ambiguous,
    NoTitleFits,
    UnknownIdHint,
    NotInShow,
}

impl ReasonColumn {
    const fn of(reason: &UnmatchedReason) -> Self {
        match reason {
            UnmatchedReason::Ambiguous(_) => Self::Ambiguous,
            UnmatchedReason::NoTitleFits => Self::NoTitleFits,
            UnmatchedReason::UnknownIdHint => Self::UnknownIdHint,
            UnmatchedReason::NotInShow => Self::NotInShow,
        }
    }

    /// The reason this column and `tied_titles`, the JSON the migration describes, store.
    fn reason(self, tied_titles: Option<&str>) -> Result<UnmatchedReason, sqlx::Error> {
        Ok(match self {
            Self::Ambiguous => UnmatchedReason::Ambiguous(parse_tied(tied_titles.unwrap_or("[]"))?),
            Self::NoTitleFits => UnmatchedReason::NoTitleFits,
            Self::UnknownIdHint => UnmatchedReason::UnknownIdHint,
            Self::NotInShow => UnmatchedReason::NotInShow,
        })
    }
}

fn parse_tied(json: &str) -> Result<Vec<TiedTitle>, sqlx::Error> {
    let decode = |err: Box<dyn std::error::Error + Send + Sync>| sqlx::Error::Decode(err);
    let rows: Vec<(String, String, Option<String>, Option<i32>)> =
        serde_json::from_str(json).map_err(|err| decode(err.into()))?;
    rows.into_iter()
        .map(|(id, title, original_title, year)| {
            let id =
                parse_hint(&id).ok_or_else(|| decode(format!("bad tied title id {id}").into()))?;
            Ok(TiedTitle {
                id,
                title,
                original_title,
                year,
            })
        })
        .collect()
}

fn tied_json(tied: &[TiedTitle]) -> String {
    let rows: Vec<serde_json::Value> = tied
        .iter()
        .map(|t| serde_json::json!([t.id.to_string(), t.title, t.original_title, t.year]))
        .collect();
    serde_json::Value::from(rows).to_string()
}

#[derive(Debug)]
struct MatchRow<'a> {
    state: MatchState,
    plugin: Option<&'a str>,
    ids: ExternalIds,
    show_ids: ExternalIds,
    metadata: MetadataColumns<'a>,
    unmatched_reason: Option<ReasonColumn>,
    tied_titles: Option<String>,
}

/// A Match's metadata as columns of `item_matches`; all empty for an Unmatched Item.
#[derive(Debug, Default)]
struct MetadataColumns<'a> {
    title: Option<&'a str>,
    original_title: Option<&'a str>,
    show_title: Option<&'a str>,
    overview: Option<&'a str>,
    release_date: Option<&'a str>,
    runtime_minutes: Option<u32>,
    poster: Option<ImageColumns<'a>>,
    backdrop: Option<ImageColumns<'a>>,
    titled_backdrop: Option<ImageColumns<'a>>,
    logo: Option<ImageColumns<'a>>,
    still: Option<&'a str>,
    /// A JSON array of their names, which `genres` in the parent module reads.
    genres: Option<String>,
    content_rating: Option<&'a str>,
    original_language: Option<&'a str>,
    show_poster: Option<ImageColumns<'a>>,
    show_overview: Option<&'a str>,
    show_first_air_date: Option<&'a str>,
    /// As [`sort_title`] makes it from the Show's title for an Episode, else from `title`.
    sort_title: Option<String>,
}

/// An [`ImageRef`] as the columns of `item_matches` that store it.
#[derive(Debug, Clone, Copy)]
struct ImageColumns<'a> {
    path: &'a str,
    plugin: &'a str,
}

impl<'a> From<&'a ImageRef> for ImageColumns<'a> {
    fn from(image: &'a ImageRef) -> Self {
        Self {
            path: &image.path,
            plugin: image.plugin.as_str(),
        }
    }
}

impl<'a> From<&'a Decided> for MatchRow<'a> {
    fn from(decided: &'a Decided) -> Self {
        match decided {
            Decided::Unmatched(reason) => Self {
                state: MatchState::Unmatched,
                plugin: None,
                ids: ExternalIds::default(),
                show_ids: ExternalIds::default(),
                metadata: MetadataColumns::default(),
                unmatched_reason: reason.as_ref().map(ReasonColumn::of),
                tied_titles: match reason {
                    Some(UnmatchedReason::Ambiguous(tied)) => Some(tied_json(tied)),
                    Some(
                        UnmatchedReason::NoTitleFits
                        | UnmatchedReason::UnknownIdHint
                        | UnmatchedReason::NotInShow,
                    )
                    | None => None,
                },
            },
            Decided::Matched(metadata) => Self {
                state: MatchState::Matched,
                plugin: Some(metadata.plugin.as_str()),
                ids: metadata.ids.clone(),
                show_ids: metadata.show_ids.clone(),
                metadata: MetadataColumns {
                    title: Some(&metadata.title),
                    original_title: metadata.original_title.as_deref(),
                    show_title: metadata.show_title.as_deref(),
                    overview: metadata.overview.as_deref(),
                    release_date: metadata.release_date.as_deref(),
                    runtime_minutes: metadata.runtime_minutes,
                    poster: metadata.poster.as_ref().map(ImageColumns::from),
                    backdrop: metadata.backdrop.as_ref().map(ImageColumns::from),
                    titled_backdrop: metadata.titled_backdrop.as_ref().map(ImageColumns::from),
                    logo: metadata.logo.as_ref().map(ImageColumns::from),
                    still: metadata.still.as_deref(),
                    genres: Some(serde_json::Value::from(metadata.genres.clone()).to_string()),
                    content_rating: metadata.content_rating.as_deref(),
                    original_language: metadata.original_language.as_deref(),
                    show_poster: metadata.show_poster.as_ref().map(ImageColumns::from),
                    show_overview: metadata.show_overview.as_deref(),
                    show_first_air_date: metadata.show_first_air_date.as_deref(),
                    sort_title: Some(sort_title(
                        metadata.show_title.as_deref().unwrap_or(&metadata.title),
                    )),
                },
                unmatched_reason: None,
                tied_titles: None,
            },
        }
    }
}

#[cfg(test)]
#[path = "decisions_tests.rs"]
mod tests;
