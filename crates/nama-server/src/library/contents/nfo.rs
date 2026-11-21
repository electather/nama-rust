//! The `.nfo` files beside a Library's videos, and the ID Hints each Item takes from them, which
//! replace its name's (ADR 0087).

use std::{borrow::Cow, collections::HashMap};

use nama_plugin_api::metadata::ExternalId;
use sqlx::SqliteConnection;

use super::{Contents, Item, ItemKind, ItemRow, parse_hint};

/// An `.nfo` file as a scan found it, with the ID Hints it names, strongest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NfoFile {
    pub(crate) path: String,
    pub(crate) size: i64,
    pub(crate) modified_ns: i64,
    pub(crate) ids: Vec<ExternalId>,
}

/// The ID Hints an Item takes from `.nfo` files, each empty without one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct NfoHints {
    /// Its title's: a Movie's own, or an Episode's Show's.
    pub(crate) title: Vec<ExternalId>,
    /// An Episode's own.
    pub(crate) episode: Vec<ExternalId>,
}

/// An Item, the path of its file, and the ID Hints it took from `.nfo` files.
#[derive(Debug)]
pub(crate) struct NfoItem {
    pub(crate) id: i64,
    pub(crate) path: String,
    pub(crate) item: Item,
    pub(crate) hints: NfoHints,
}

impl Contents {
    /// The `.nfo` files stored for the Library `library`.
    pub(crate) async fn nfo_files(&self, library: i64) -> sqlx::Result<Vec<NfoFile>> {
        let rows = sqlx::query!(
            "SELECT path, size, modified_ns, ids FROM nfo_files WHERE library_id = ?",
            library
        )
        .fetch_all(self.db.reader())
        .await?;
        Ok((rows.into_iter())
            .map(|row| NfoFile {
                path: row.path,
                size: row.size,
                modified_ns: row.modified_ns,
                ids: ids_of(Some(&row.ids)),
            })
            .collect())
    }

    /// Every Item of the Library `library`, with the ID Hints it took from `.nfo` files.
    pub(crate) async fn nfo_items(&self, library: i64) -> sqlx::Result<Vec<NfoItem>> {
        let rows = sqlx::query!(
            r#"SELECT library_items.id AS "id!", library_files.path,
                      library_items.kind AS "kind: ItemKind", library_items.title,
                      library_items.year, library_items.hint, library_items.season,
                      library_items.episode, library_items.nfo_hints,
                      library_items.episode_nfo_hints
               FROM library_items
               JOIN library_files ON library_files.id = library_items.file_id
               WHERE library_files.library_id = ?"#,
            library
        )
        .fetch_all(self.db.reader())
        .await?;
        Ok((rows.into_iter())
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
                Some(NfoItem {
                    id: row.id,
                    path: row.path,
                    item,
                    hints: NfoHints {
                        title: ids_of(row.nfo_hints.as_deref()),
                        episode: ids_of(row.episode_nfo_hints.as_deref()),
                    },
                })
            })
            .collect())
    }

    /// Stores `found`, the `.nfo` files of the Library `library` that are new or changed.
    pub(crate) async fn keep_nfo_files(
        &self,
        library: i64,
        found: Vec<NfoFile>,
    ) -> sqlx::Result<()> {
        self.db
            .write(move |tx| Box::pin(keep(tx, library, found)))
            .await
    }

    /// Forgets the `.nfo` files of the Library `library` at the paths `gone`.
    pub(crate) async fn forget_nfo_files(
        &self,
        library: i64,
        gone: Vec<String>,
    ) -> sqlx::Result<()> {
        self.db
            .write(move |tx| Box::pin(forget(tx, library, gone)))
            .await
    }

    /// Gives each Item its new ID Hints from `.nfo` files, and forgets what Matching decided
    /// about it, so the scan Matches it again, unless its Match agrees with them (ADR 0087). An
    /// Item whose name an admin pinned keeps its decision.
    pub(crate) async fn set_nfo_hints(&self, changed: Vec<(i64, NfoHints)>) -> sqlx::Result<()> {
        self.db
            .write(move |tx| Box::pin(set_hints(tx, changed)))
            .await
    }

    /// The ID Hints each Item of the Library `library` took from `.nfo` files, for those that
    /// took any.
    pub(crate) async fn nfo_hints(&self, library: i64) -> sqlx::Result<HashMap<i64, NfoHints>> {
        let rows = sqlx::query!(
            r#"SELECT library_items.id AS "id!", library_items.nfo_hints,
                      library_items.episode_nfo_hints
               FROM library_items
               JOIN library_files ON library_files.id = library_items.file_id
               WHERE library_files.library_id = ?
                 AND (library_items.nfo_hints IS NOT NULL
                      OR library_items.episode_nfo_hints IS NOT NULL)"#,
            library
        )
        .fetch_all(self.db.reader())
        .await?;
        Ok((rows.into_iter())
            .map(|row| {
                let hints = NfoHints {
                    title: ids_of(row.nfo_hints.as_deref()),
                    episode: ids_of(row.episode_nfo_hints.as_deref()),
                };
                (row.id, hints)
            })
            .collect())
    }
}

async fn keep(conn: &mut SqliteConnection, library: i64, found: Vec<NfoFile>) -> sqlx::Result<()> {
    for file in found {
        let ids = ids_json(&file.ids);
        sqlx::query!(
            "INSERT INTO nfo_files (library_id, path, size, modified_ns, ids)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT (library_id, path)
             DO UPDATE SET size = excluded.size, modified_ns = excluded.modified_ns,
                           ids = excluded.ids",
            library,
            file.path,
            file.size,
            file.modified_ns,
            ids,
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

async fn forget(conn: &mut SqliteConnection, library: i64, gone: Vec<String>) -> sqlx::Result<()> {
    for path in gone {
        sqlx::query!(
            "DELETE FROM nfo_files WHERE library_id = ? AND path = ?",
            library,
            path
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

async fn set_hints(conn: &mut SqliteConnection, changed: Vec<(i64, NfoHints)>) -> sqlx::Result<()> {
    for (item, hints) in changed {
        let title = (!hints.title.is_empty()).then(|| ids_json(&hints.title));
        let episode = (!hints.episode.is_empty()).then(|| ids_json(&hints.episode));
        sqlx::query!(
            "UPDATE library_items SET nfo_hints = ?, episode_nfo_hints = ? WHERE id = ?",
            title,
            episode,
            item
        )
        .execute(&mut *conn)
        .await?;
        // Kept when Matched to a title, and for an Episode an Episode, its new hints name, as on
        // the first scan to read an `.nfo` the name already Matched by.
        sqlx::query!(
            "DELETE FROM item_matches
             WHERE item_id = ?1
               AND NOT EXISTS (
                   SELECT 1 FROM library_items
                   JOIN library_files ON library_files.id = library_items.file_id
                   JOIN match_pins ON match_pins.library_id = library_files.library_id
                                  AND match_pins.kind = library_items.kind
                                  AND match_pins.title = library_items.title
                                  AND match_pins.year IS library_items.year
                                  AND match_pins.hint IS library_items.hint
                   WHERE library_items.id = ?1)
               AND NOT (
                   state = 'matched'
                   AND EXISTS (
                       SELECT 1 FROM item_match_ids AS ids
                       JOIN library_items ON library_items.id = ids.item_id
                       JOIN json_each(?2) AS hint
                         ON hint.value = ids.namespace || '-' || ids.value
                       WHERE ids.item_id = ?1
                         AND ids.scope = CASE library_items.kind
                                             WHEN 'episode' THEN 'show' ELSE 'title' END)
                   AND (?3 IS NULL OR EXISTS (
                       SELECT 1 FROM item_match_ids AS ids
                       JOIN json_each(?3) AS hint
                         ON hint.value = ids.namespace || '-' || ids.value
                       WHERE ids.item_id = ?1 AND ids.scope = 'title')))",
            item,
            title,
            episode
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// `ids` as a JSON array of how each displays, in order.
fn ids_json(ids: &[ExternalId]) -> String {
    serde_json::Value::from_iter(ids.iter().map(ToString::to_string)).to_string()
}

/// The ids in [`ids_json`]'s `json`, skipping any that doesn't parse; none for NULL.
fn ids_of(json: Option<&str>) -> Vec<ExternalId> {
    let shown: Vec<String> = json
        .and_then(|json| serde_json::from_str(json).ok())
        .unwrap_or_default();
    shown.iter().filter_map(|id| parse_hint(id)).collect()
}
