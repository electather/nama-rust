//! Libraries: named collections of one kind of media, each backed by one or more folders.
//!
//! [`Libraries`] is the only way in. It checks every name and folder before anything is
//! written, and it keeps the rules a Library lives by: a folder belongs to at most one Library,
//! and a Library's kind is fixed when it is created.

use std::{collections::HashMap, path::Path, sync::Arc};

use sqlx::SqliteConnection;

use crate::db::Database;

/// The kind of media a Library holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
pub(crate) enum Kind {
    Movies,
    Shows,
}

/// A Library as it is stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Library {
    pub(crate) id: i64,
    pub(crate) name: String,
    pub(crate) kind: Kind,
    /// Canonical absolute paths, in the order the admin gave them.
    pub(crate) folders: Vec<String>,
}

/// Why a change to the Libraries was refused or failed.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    /// The request itself is wrong: a blank name, or a folder that isn't a readable directory.
    #[error("{0}")]
    Invalid(String),
    /// No Library has this id.
    #[error("there is no Library {0}")]
    NotFound(i64),
    /// A folder in the request already belongs to another Library.
    #[error("the folder {folder} already belongs to the Library {library:?}")]
    FolderTaken { folder: String, library: String },
    /// The database failed.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// The Server's Libraries, kept in its database.
#[derive(Debug)]
pub(crate) struct Libraries {
    db: Arc<Database>,
}

impl Libraries {
    pub(crate) const fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Creates a Library holding `kind`, backed by `folders`.
    pub(crate) async fn create(
        &self,
        name: &str,
        kind: Kind,
        folders: Vec<String>,
    ) -> Result<Library, Error> {
        let name = check_name(name)?;
        let folders = check_folders(folders).await?;
        self.db
            .write(move |tx| Box::pin(insert(tx, name, kind, folders)))
            .await
    }

    /// Every Library, oldest first.
    pub(crate) async fn list(&self) -> Result<Vec<Library>, Error> {
        let mut tx = self.db.reader().begin().await?;
        let rows = sqlx::query!(
            r#"SELECT id AS "id!", name, kind AS "kind: Kind" FROM libraries ORDER BY id"#
        )
        .fetch_all(&mut *tx)
        .await?;
        let mut folders: HashMap<i64, Vec<String>> = HashMap::new();
        for folder in sqlx::query!(
            "SELECT library_id, path FROM library_folders ORDER BY library_id, position"
        )
        .fetch_all(&mut *tx)
        .await?
        {
            folders
                .entry(folder.library_id)
                .or_default()
                .push(folder.path);
        }
        tx.commit().await?;
        Ok(rows
            .into_iter()
            .map(|row| Library {
                folders: folders.remove(&row.id).unwrap_or_default(),
                id: row.id,
                name: row.name,
                kind: row.kind,
            })
            .collect())
    }

    /// The Library with this id.
    pub(crate) async fn get(&self, id: i64) -> Result<Library, Error> {
        let mut tx = self.db.reader().begin().await?;
        let row = sqlx::query!(
            r#"SELECT name, kind AS "kind: Kind" FROM libraries WHERE id = ?"#,
            id
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(Error::NotFound(id))?;
        let folders = sqlx::query_scalar!(
            "SELECT path FROM library_folders WHERE library_id = ? ORDER BY position",
            id
        )
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(Library {
            id,
            name: row.name,
            kind: row.kind,
            folders,
        })
    }

    /// Renames the Library with this id and replaces its folders. Its kind stays as it is.
    pub(crate) async fn update(
        &self,
        id: i64,
        name: &str,
        folders: Vec<String>,
    ) -> Result<Library, Error> {
        let name = check_name(name)?;
        let folders = check_folders(folders).await?;
        self.db
            .write(move |tx| Box::pin(replace(tx, id, name, folders)))
            .await
    }

    /// Deletes the Library with this id. The files in its folders are left alone.
    pub(crate) async fn delete(&self, id: i64) -> Result<(), Error> {
        self.db.write(move |tx| Box::pin(delete(tx, id))).await
    }
}

/// Stores a new Library, whose name and folders have been checked, and returns it.
async fn insert(
    conn: &mut SqliteConnection,
    name: String,
    kind: Kind,
    folders: Vec<String>,
) -> Result<Library, Error> {
    ensure_folders_free(conn, &folders, None).await?;
    let id = sqlx::query_scalar!(
        r#"INSERT INTO libraries (name, kind) VALUES (?, ?) RETURNING id AS "id!""#,
        name,
        kind,
    )
    .fetch_one(&mut *conn)
    .await?;
    insert_folders(conn, id, &folders).await?;
    Ok(Library {
        id,
        name,
        kind,
        folders,
    })
}

/// Gives the Library `id` a new name and folders, which have been checked, and returns it.
async fn replace(
    conn: &mut SqliteConnection,
    id: i64,
    name: String,
    folders: Vec<String>,
) -> Result<Library, Error> {
    let kind = sqlx::query_scalar!(
        r#"UPDATE libraries SET name = ? WHERE id = ? RETURNING kind AS "kind: Kind""#,
        name,
        id,
    )
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(Error::NotFound(id))?;
    ensure_folders_free(conn, &folders, Some(id)).await?;
    sqlx::query!("DELETE FROM library_folders WHERE library_id = ?", id)
        .execute(&mut *conn)
        .await?;
    insert_folders(conn, id, &folders).await?;
    Ok(Library {
        id,
        name,
        kind,
        folders,
    })
}

/// Deletes the Library `id`. Its folders go with it (`ON DELETE CASCADE`).
async fn delete(conn: &mut SqliteConnection, id: i64) -> Result<(), Error> {
    let deleted = sqlx::query!("DELETE FROM libraries WHERE id = ?", id)
        .execute(&mut *conn)
        .await?;
    if deleted.rows_affected() == 0 {
        return Err(Error::NotFound(id));
    }
    Ok(())
}

/// The name with surrounding whitespace removed, unless nothing is left.
fn check_name(name: &str) -> Result<String, Error> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::Invalid("a Library's name can't be blank".to_owned()));
    }
    Ok(name.to_owned())
}

/// Checks that there is at least one folder and that each is an absolute path to a directory the
/// Server can read, named once, and returns their canonical paths. Canonical paths make two
/// spellings of one folder (a trailing slash, a symlink) the same folder.
async fn check_folders(folders: Vec<String>) -> Result<Vec<String>, Error> {
    if folders.is_empty() {
        return Err(Error::Invalid(
            "a Library needs at least one folder".to_owned(),
        ));
    }
    let mut canonical = Vec::with_capacity(folders.len());
    for folder in folders {
        let invalid = |problem: String| Error::Invalid(format!("the folder {folder:?} {problem}"));
        if !Path::new(&folder).is_absolute() {
            return Err(invalid("is not an absolute path".to_owned()));
        }
        let path = tokio::fs::canonicalize(&folder)
            .await
            .map_err(|err| invalid(format!("can't be found: {err}")))?;
        // Listing fails for anything that isn't a directory, and for a directory the Server
        // isn't allowed to read.
        let mut entries = tokio::fs::read_dir(&path)
            .await
            .map_err(|err| invalid(format!("is not a directory the Server can read: {err}")))?;
        entries
            .next_entry()
            .await
            .map_err(|err| invalid(format!("is not a directory the Server can read: {err}")))?;
        let path = path.into_os_string().into_string().map_err(|path| {
            invalid(format!(
                "resolves to {}, which is not valid UTF-8",
                Path::new(&path).display()
            ))
        })?;
        if canonical.contains(&path) {
            return Err(invalid("is named more than once".to_owned()));
        }
        canonical.push(path);
    }
    Ok(canonical)
}

/// Fails with [`Error::FolderTaken`] if any of `folders` belongs to a Library other than
/// `except`.
async fn ensure_folders_free(
    conn: &mut SqliteConnection,
    folders: &[String],
    except: Option<i64>,
) -> Result<(), Error> {
    for folder in folders {
        let owner = sqlx::query_scalar!(
            "SELECT libraries.name FROM library_folders
             JOIN libraries ON libraries.id = library_folders.library_id
             WHERE library_folders.path = ? AND library_folders.library_id IS NOT ?",
            folder,
            except,
        )
        .fetch_optional(&mut *conn)
        .await?;
        if let Some(library) = owner {
            return Err(Error::FolderTaken {
                folder: folder.clone(),
                library,
            });
        }
    }
    Ok(())
}

/// Adds `folders` to the Library `id`, in order.
async fn insert_folders(
    conn: &mut SqliteConnection,
    id: i64,
    folders: &[String],
) -> Result<(), sqlx::Error> {
    for (position, folder) in (0_i64..).zip(folders) {
        sqlx::query!(
            "INSERT INTO library_folders (path, library_id, position) VALUES (?, ?, ?)",
            folder,
            id,
            position,
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}
