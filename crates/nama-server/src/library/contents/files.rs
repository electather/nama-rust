//! The files a scan stored and the changes a scan applies to them.

use std::{borrow::Cow, collections::HashMap};

use nama_proto::nama::v1::{Event, LibraryItemAdded, LibraryItemRemoved};
use sqlx::SqliteConnection;

use super::{Contents, Item, ItemKind, ItemRow};
use crate::{events::event, scanner::PARSER_VERSION};

#[derive(Debug, Clone, Copy)]
pub(crate) struct Stored {
    pub(crate) id: i64,
    pub(crate) size: i64,
    pub(crate) modified_ns: i64,
    /// The version of the name parser that read it.
    pub(crate) parser_version: i64,
}

/// A change to a Library's stored files.
#[derive(Debug)]
pub(crate) enum Change {
    /// Store a file that's new or changed, with what it holds, replacing what was stored.
    Store {
        path: String,
        size: i64,
        modified_ns: i64,
        items: Vec<Item>,
    },
    /// Store what an unchanged file holds as the current name parser reads it. Its Items keep
    /// their ids and Matches, but not their automatic Unmatched decisions, so Matching tries them
    /// again (ADR 0023). A pin on an Item's old name also applies to its new one.
    Reparse {
        file: i64,
        path: String,
        items: Vec<Item>,
    },
    Remove(i64),
}

impl Contents {
    pub(crate) async fn stored(
        &self,
        library: i64,
    ) -> Result<HashMap<String, Stored>, sqlx::Error> {
        Ok(sqlx::query!(
            r#"SELECT id AS "id!", path, size, modified_ns, parser_version FROM library_files
               WHERE library_id = ?"#,
            library
        )
        .fetch_all(self.db.reader())
        .await?
        .into_iter()
        .map(|row| {
            let stored = Stored {
                id: row.id,
                size: row.size,
                modified_ns: row.modified_ns,
                parser_version: row.parser_version,
            };
            (row.path, stored)
        })
        .collect())
    }

    /// Every file stored, in every Library.
    pub(crate) async fn every_stored(&self) -> Result<Vec<Stored>, sqlx::Error> {
        sqlx::query_as!(
            Stored,
            r#"SELECT id AS "id!", size, modified_ns, parser_version FROM library_files"#
        )
        .fetch_all(self.db.reader())
        .await
    }

    /// Returns the events for the Items `changes` removes and adds. Replacing an Item removes what
    /// Matching decided about it.
    pub(crate) async fn apply(
        &self,
        library: i64,
        changes: Vec<Change>,
    ) -> Result<Vec<Event>, sqlx::Error> {
        self.db
            .write(move |tx| Box::pin(apply(tx, library, changes)))
            .await
    }

    pub(crate) async fn library_of(&self, item: i64) -> Result<Option<i64>, sqlx::Error> {
        sqlx::query_scalar!(
            "SELECT library_files.library_id
             FROM library_items
             JOIN library_files ON library_files.id = library_items.file_id
             WHERE library_items.id = ?",
            item
        )
        .fetch_optional(self.db.reader())
        .await
    }

    /// The Library whose file the Version `version` is, if there is such a Version.
    pub(crate) async fn library_of_version(
        &self,
        version: i64,
    ) -> Result<Option<i64>, sqlx::Error> {
        sqlx::query_scalar!("SELECT library_id FROM library_files WHERE id = ?", version)
            .fetch_optional(self.db.reader())
            .await
    }

    /// Files whose names don't follow the naming conventions, so they hold no Items, by path.
    pub(crate) async fn unparsed(&self, library: i64) -> Result<Vec<String>, sqlx::Error> {
        sqlx::query_scalar!(
            "SELECT path FROM library_files
             WHERE library_id = ?
               AND NOT EXISTS (SELECT 1 FROM library_items
                               WHERE library_items.file_id = library_files.id)
             ORDER BY path",
            library
        )
        .fetch_all(self.db.reader())
        .await
    }
}

async fn apply(
    conn: &mut SqliteConnection,
    library: i64,
    changes: Vec<Change>,
) -> Result<Vec<Event>, sqlx::Error> {
    let mut events = Vec::new();
    for change in changes {
        match change {
            Change::Store {
                path,
                size,
                modified_ns,
                items,
            } => {
                let file = sqlx::query_scalar!(
                    r#"INSERT INTO library_files (library_id, path, size, modified_ns, parser_version)
                       VALUES (?, ?, ?, ?, ?)
                       ON CONFLICT (library_id, path)
                       DO UPDATE SET size = excluded.size, modified_ns = excluded.modified_ns,
                                     parser_version = excluded.parser_version
                       RETURNING id AS "id!""#,
                    library,
                    path,
                    size,
                    modified_ns,
                    PARSER_VERSION,
                )
                .fetch_one(&mut *conn)
                .await?;
                let removed = remove_items(conn, file).await?;
                let added = insert_items(conn, file, &items).await?;
                events.extend(removed.into_iter().map(|item| item_removed(library, item)));
                events.extend(
                    added
                        .into_iter()
                        .map(|item| item_added(library, item, path.clone())),
                );
            }
            Change::Reparse { file, path, items } => {
                events.extend(reparse(conn, library, file, &path, &items).await?);
            }
            Change::Remove(file) => {
                let removed = remove_items(conn, file).await?;
                sqlx::query!("DELETE FROM library_files WHERE id = ?", file)
                    .execute(&mut *conn)
                    .await?;
                events.extend(removed.into_iter().map(|item| item_removed(library, item)));
            }
        }
    }
    Ok(events)
}

/// Pairs each of `items` with the stored Item of the file it was parsed as before, by Season and
/// Episode number, so it keeps its id and Match; the unpaired are removed or added.
async fn reparse(
    conn: &mut SqliteConnection,
    library: i64,
    file: i64,
    path: &str,
    items: &[Item],
) -> Result<Vec<Event>, sqlx::Error> {
    sqlx::query!(
        "UPDATE library_files SET parser_version = ? WHERE id = ?",
        PARSER_VERSION,
        file
    )
    .execute(&mut *conn)
    .await?;
    let mut stored = sqlx::query!(
        r#"SELECT id AS "id!", kind AS "kind: ItemKind", title, year, hint, season, episode
           FROM library_items WHERE file_id = ? ORDER BY id"#,
        file
    )
    .fetch_all(&mut *conn)
    .await?;
    let mut unpaired = Vec::new();
    for item in items {
        let new = ItemRow::from(item);
        let paired = stored
            .iter()
            .position(|old| (old.season, old.episode) == (new.season, new.episode));
        let Some(old) = paired.map(|at| stored.swap_remove(at)) else {
            unpaired.push(item.clone());
            continue;
        };
        let old_row = ItemRow {
            kind: old.kind,
            title: Cow::Owned(old.title),
            year: old.year,
            hint: old.hint,
            season: old.season,
            episode: old.episode,
        };
        rename_item(conn, library, old.id, &old_row, &new).await?;
    }
    let mut events = Vec::new();
    for gone in stored {
        sqlx::query!("DELETE FROM library_items WHERE id = ?", gone.id)
            .execute(&mut *conn)
            .await?;
        events.push(item_removed(library, gone.id));
    }
    let added = insert_items(conn, file, &unpaired).await?;
    events.extend(
        added
            .into_iter()
            .map(|item| item_added(library, item, path.to_owned())),
    );
    sqlx::query!(
        "DELETE FROM item_matches
         WHERE state = 'unmatched'
           AND item_id IN (SELECT id FROM library_items WHERE file_id = ?)",
        file
    )
    .execute(&mut *conn)
    .await?;
    Ok(events)
}

/// Stores `new` as what the Item `id` holds, and gives its name any pin on `old`.
async fn rename_item(
    conn: &mut SqliteConnection,
    library: i64,
    id: i64,
    old: &ItemRow<'_>,
    new: &ItemRow<'_>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE library_items SET kind = ?, title = ?, year = ?, hint = ?, season = ?,
                                  episode = ?
         WHERE id = ?",
        new.kind,
        new.title,
        new.year,
        new.hint,
        new.season,
        new.episode,
        id,
    )
    .execute(&mut *conn)
    .await?;
    // A pin is keyed by name, so a new name would lose it. The old pin stays for any Item
    // still named the old way; a name that is already pinned keeps its own pin.
    sqlx::query!(
        "INSERT OR IGNORE INTO match_pins (
             library_id, kind, title, year, hint,
             pinned_id, pinned_at)
         SELECT library_id, kind, ?, ?, ?, pinned_id, pinned_at
         FROM match_pins
         WHERE library_id = ? AND kind = ? AND title = ? AND year IS ? AND hint IS ?",
        new.title,
        new.year,
        new.hint,
        library,
        old.kind,
        old.title,
        old.year,
        old.hint,
    )
    .execute(conn)
    .await?;
    Ok(())
}

fn item_added(library: i64, item: i64, path: String) -> Event {
    event(LibraryItemAdded {
        library_id: library,
        item_id: item,
        path,
        ..Default::default()
    })
}

fn item_removed(library: i64, item: i64) -> Event {
    event(LibraryItemRemoved {
        library_id: library,
        item_id: item,
        ..Default::default()
    })
}

async fn remove_items(conn: &mut SqliteConnection, file: i64) -> Result<Vec<i64>, sqlx::Error> {
    sqlx::query_scalar!(
        r#"DELETE FROM library_items WHERE file_id = ? RETURNING id AS "id!""#,
        file
    )
    .fetch_all(conn)
    .await
}

async fn insert_items(
    conn: &mut SqliteConnection,
    file: i64,
    items: &[Item],
) -> Result<Vec<i64>, sqlx::Error> {
    let mut ids = Vec::with_capacity(items.len());
    for item in items {
        let row = ItemRow::from(item);
        let id = sqlx::query_scalar!(
            r#"INSERT INTO library_items (file_id, kind, title, year, hint, season, episode)
               VALUES (?, ?, ?, ?, ?, ?, ?)
               RETURNING id AS "id!""#,
            file,
            row.kind,
            row.title,
            row.year,
            row.hint,
            row.season,
            row.episode,
        )
        .fetch_one(&mut *conn)
        .await?;
        ids.push(id);
    }
    Ok(ids)
}

#[cfg(test)]
#[path = "files_tests.rs"]
mod tests;
