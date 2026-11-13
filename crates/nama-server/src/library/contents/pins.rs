//! The Matches an admin pinned by hand, which Matching follows instead of searching.

use std::borrow::Cow;

use nama_plugin_api::metadata::ExternalId;
use nama_proto::nama::v1::Event;
use sqlx::SqliteConnection;

use super::{Contents, Decisions, Item, ItemKind, ItemRow, Name, insert_decisions};
use crate::db::now;

/// A Match an admin set or cleared by hand, which Matching follows instead of searching.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Pin {
    /// Match the Movie, or for Episodes the Show, with this id.
    Title(ExternalId),
    /// Leave the Items Unmatched.
    Unmatched,
}

/// The Items of a Library that share one Item's name.
#[derive(Debug)]
pub(crate) struct Named {
    pub(crate) library: i64,
    pub(crate) name: Name,
    pub(crate) items: Vec<(i64, Item)>,
}

impl Contents {
    /// The Items of the Item `item`'s Library named like it, itself among them, or `None` if
    /// there is no such Item.
    pub(crate) async fn named_like(&self, item: i64) -> Result<Option<Named>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT files.library_id, named.id AS "id!", named.kind AS "kind: ItemKind",
                      named.title, named.year, named.hint, named.season, named.episode
               FROM library_items AS this
               JOIN library_files AS this_file ON this_file.id = this.file_id
               JOIN library_files AS files ON files.library_id = this_file.library_id
               JOIN library_items AS named ON named.file_id = files.id
               WHERE this.id = ?
                 AND named.kind = this.kind
                 AND named.title = this.title
                 AND named.year IS this.year
                 AND named.hint IS this.hint
               ORDER BY named.id"#,
            item
        )
        .fetch_all(self.db.reader())
        .await?;
        let Some(library) = rows.first().map(|row| row.library_id) else {
            return Ok(None);
        };
        let items: Vec<(i64, Item)> = rows
            .into_iter()
            .filter_map(|row| {
                let alike = ItemRow {
                    kind: row.kind,
                    title: Cow::Owned(row.title),
                    year: row.year,
                    hint: row.hint,
                    season: row.season,
                    episode: row.episode,
                }
                .into_item()?;
                Some((row.id, alike))
            })
            .collect();
        let Some(name) = items.first().map(|(_, first)| first.name()) else {
            return Ok(None);
        };
        Ok(Some(Named {
            library,
            name,
            items,
        }))
    }

    /// Replaces any pin the Items called `name` had, in one write with `decided`. Returns the
    /// events for the Requests the Matches make Available or Partially Available, or `None`,
    /// having stored nothing, if the Library is gone.
    pub(crate) async fn pin(
        &self,
        library: i64,
        name: Name,
        pin: Pin,
        decided: impl Into<Decisions>,
    ) -> Result<Option<Vec<Event>>, sqlx::Error> {
        let decided = decided.into();
        self.db
            .write(move |tx| Box::pin(store_pin(tx, library, name, pin, decided)))
            .await
    }
}

async fn store_pin(
    conn: &mut SqliteConnection,
    library: i64,
    name: Name,
    pin: Pin,
    decided: Decisions,
) -> Result<Option<Vec<Event>>, sqlx::Error> {
    let now = now();
    if !insert_pin(conn, library, &name, &pin, now).await? {
        return Ok(None);
    }
    insert_decisions(conn, decided, now).await.map(Some)
}

/// Returns whether it was stored: it isn't if the Library is gone.
async fn insert_pin(
    conn: &mut SqliteConnection,
    library: i64,
    name: &Name,
    pin: &Pin,
    now: i64,
) -> Result<bool, sqlx::Error> {
    let pinned = pin.columns();
    let hint = name.hint.as_ref().map(ExternalId::to_string);
    let inserted = sqlx::query!(
        "INSERT OR REPLACE INTO match_pins (
             library_id, kind, title, year, hint,
             pinned_tmdb_id, pinned_imdb_id, pinned_tvdb_id, pinned_at)
         SELECT ?, ?, ?, ?, ?, ?, ?, ?, ?
         WHERE EXISTS (SELECT 1 FROM libraries WHERE id = ?)",
        library,
        name.kind,
        name.title,
        name.year,
        hint,
        pinned.tmdb,
        pinned.imdb,
        pinned.tvdb,
        now,
        library,
    )
    .execute(conn)
    .await?;
    Ok(inserted.rows_affected() > 0)
}

#[cfg(test)]
#[path = "pins_tests.rs"]
mod tests;
