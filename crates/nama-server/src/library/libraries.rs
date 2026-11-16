//! Libraries: named collections of one kind of media, each backed by one or more folders.
//!
//! [`Libraries`] is the only way in. It checks every name and folder before anything is
//! written, and it keeps the rules a Library lives by: a folder belongs to at most one Library,
//! and a Library's kind is fixed when it is created.

use std::{collections::HashMap, path::Path, sync::Arc};

use nama_plugin_api::metadata::{ExternalId, InvalidId};
use sqlx::SqliteConnection;

use crate::db::Database;

/// The kind of media a Library holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
pub(crate) enum Kind {
    /// Movies, one per title.
    Movies,
    /// Shows, made of Seasons and Episodes.
    Shows,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Library {
    pub(crate) id: i64,
    pub(crate) name: String,
    pub(crate) kind: Kind,
    /// Canonical absolute paths, in the order the admin gave them.
    pub(crate) folders: Vec<String>,
    /// Those of `folders` the Server couldn't write to when it last scanned the Library, so media
    /// in them isn't deleted (ADR 0038).
    pub(crate) read_only: Vec<String>,
    /// The namespaces of the title sources its titles are searched and Matched with, first
    /// asked first.
    pub(crate) title_sources: Vec<String>,
    /// The ids of the Plugins whose Recognisers it asks, first asked first; none asks only the
    /// Kernel's own Matching (ADR 0050).
    pub(crate) recognisers: Vec<String>,
}

/// The orders a Library asks its Plugins in, as a create or an update gives them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Order {
    /// Title source namespaces, first asked first; empty keeps the stored order, or TMDB alone
    /// for a new Library.
    pub(crate) title_sources: Vec<String>,
    /// Recogniser Plugin ids, first asked first; `None` keeps the stored order, or none for a
    /// new Library. The caller checks each names a Recogniser.
    pub(crate) recognisers: Option<Vec<String>>,
}

/// Why a change to the Libraries was refused or failed.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("a Library's name can't be blank")]
    NameBlank,
    #[error("a Library needs at least one folder")]
    FoldersEmpty,
    /// The folder at `index` in the request, as given, breaks a rule.
    #[error("the folder {folder:?} {problem}")]
    Folder {
        index: usize,
        folder: String,
        problem: FolderProblem,
    },
    #[error("there is no Library {0}")]
    NotFound(i64),
    /// A folder in the request already belongs to another Library.
    #[error("the folder {folder} already belongs to the Library {library:?}")]
    FolderTaken { folder: String, library: String },
    /// The title sources given aren't one or more well-formed namespaces, each named once;
    /// `namespace` is the first that breaks it, none if none was given.
    #[error(
        "a Library's title sources must be one or more well-formed namespaces, each named once"
    )]
    TitleSources { namespace: Option<String> },
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Why a folder can't be one of a Library's.
#[derive(Debug, thiserror::Error)]
pub(crate) enum FolderProblem {
    #[error("is not an absolute path")]
    NotAbsolute,
    // In `NotFound` and `NotReadable`, the operating system's text stays in the message and never
    // goes in a Refusal's params: it can't be translated.
    #[error("can't be found: {0}")]
    NotFound(std::io::Error),
    #[error("is not a directory the Server can read: {0}")]
    NotReadable(std::io::Error),
    /// The path it resolves to, shown lossily.
    #[error("resolves to {0}, which is not valid UTF-8")]
    NotUtf8(String),
    #[error("is named more than once")]
    Repeated,
}

/// The Server's Libraries, kept in its database.
#[derive(Debug, Clone)]
pub(crate) struct Libraries {
    db: Arc<Database>,
}

impl Libraries {
    pub(crate) const fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Creates a Library searching TMDB alone.
    #[cfg(test)]
    pub(crate) async fn create(
        &self,
        name: &str,
        kind: Kind,
        folders: Vec<String>,
    ) -> Result<Library, Error> {
        (self.create_in_order(name, kind, folders, Order::default())).await
    }

    /// Creates a Library asking its Plugins in `order`, in one write.
    pub(crate) async fn create_in_order(
        &self,
        name: &str,
        kind: Kind,
        folders: Vec<String>,
        order: Order,
    ) -> Result<Library, Error> {
        let name = check_name(name)?;
        let folders = check_folders(folders).await?;
        if !order.title_sources.is_empty() {
            check_title_sources(&order.title_sources)?;
        }
        self.db
            .write(move |tx| Box::pin(create(tx, name, kind, folders, order)))
            .await
    }

    /// Every Library, oldest first.
    pub(crate) async fn list(&self) -> Result<Vec<Library>, Error> {
        let mut tx = self.db.reader().begin().await?;
        let rows = sqlx::query!(
            r#"SELECT id AS "id!", name, kind AS "kind: Kind", title_sources, recognisers
               FROM libraries ORDER BY id"#
        )
        .fetch_all(&mut *tx)
        .await?;
        let mut folders: HashMap<i64, Vec<String>> = HashMap::new();
        let mut read_only: HashMap<i64, Vec<String>> = HashMap::new();
        for folder in sqlx::query!(
            r#"SELECT library_id, path, read_only AS "read_only: bool" FROM library_folders
               ORDER BY library_id, position"#
        )
        .fetch_all(&mut *tx)
        .await?
        {
            if folder.read_only {
                let library = read_only.entry(folder.library_id).or_default();
                library.push(folder.path.clone());
            }
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
                read_only: read_only.remove(&row.id).unwrap_or_default(),
                id: row.id,
                name: row.name,
                kind: row.kind,
                title_sources: split_sources(&row.title_sources),
                recognisers: split_recognisers(&row.recognisers),
            })
            .collect())
    }

    pub(crate) async fn get(&self, id: i64) -> Result<Library, Error> {
        let mut tx = self.db.reader().begin().await?;
        let row = sqlx::query!(
            r#"SELECT name, kind AS "kind: Kind", title_sources, recognisers FROM libraries
               WHERE id = ?"#,
            id
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(Error::NotFound(id))?;
        let rows = sqlx::query!(
            r#"SELECT path, read_only AS "read_only: bool" FROM library_folders
               WHERE library_id = ? ORDER BY position"#,
            id
        )
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        let read_only = (rows.iter().filter(|folder| folder.read_only))
            .map(|folder| folder.path.clone())
            .collect();
        Ok(Library {
            id,
            name: row.name,
            kind: row.kind,
            folders: rows.into_iter().map(|folder| folder.path).collect(),
            read_only,
            title_sources: split_sources(&row.title_sources),
            recognisers: split_recognisers(&row.recognisers),
        })
    }

    /// Records that the Server can write to each of the Library `id`'s folders but `read_only`.
    pub(crate) async fn set_read_only(&self, id: i64, read_only: Vec<String>) -> Result<(), Error> {
        self.db
            .write(move |tx| Box::pin(set_read_only(tx, id, read_only)))
            .await
    }

    /// Sets the order of the Library `id`'s title sources, by namespace. Titles it already
    /// Matched stay as they are: nothing Matches them again.
    #[cfg(test)]
    pub(crate) async fn set_title_sources(&self, id: i64, order: Vec<String>) -> Result<(), Error> {
        check_title_sources(&order)?;
        let order = order.join(",");
        self.db
            .write(move |tx| Box::pin(set_title_sources(tx, id, order)))
            .await
    }

    /// Sets the order of the Library `id`'s Recognisers, by Plugin id.
    #[cfg(test)]
    pub(crate) async fn set_recognisers(&self, id: i64, order: Vec<String>) -> Result<(), Error> {
        let order = order.join(",");
        self.db
            .write(move |tx| Box::pin(set_recognisers(tx, id, order)))
            .await
    }

    /// Renames the Library, replaces its folders and whichever of its orders `order` gives, in
    /// one write; its kind can't change.
    pub(crate) async fn update(
        &self,
        id: i64,
        name: &str,
        folders: Vec<String>,
        order: Order,
    ) -> Result<Library, Error> {
        let name = check_name(name)?;
        let folders = check_folders(folders).await?;
        if !order.title_sources.is_empty() {
            check_title_sources(&order.title_sources)?;
        }
        self.db
            .write(move |tx| Box::pin(update(tx, id, name, folders, order)))
            .await
    }

    /// Deletes the Library. The files in its folders are left alone.
    pub(crate) async fn delete(&self, id: i64) -> Result<(), Error> {
        self.db.write(move |tx| Box::pin(delete(tx, id))).await
    }

    /// Records `read`, the ids of the enabled Recognisers the Library `id` lists, in its order and
    /// joined with commas, as those its starting scan reads with, and returns whether they differ
    /// from its last scan's. Part of the scan's write that reads its files again (ADR 0050).
    pub(crate) async fn record_recognisers_read(
        conn: &mut SqliteConnection,
        id: i64,
        read: &str,
    ) -> Result<bool, sqlx::Error> {
        let changed = sqlx::query!(
            "UPDATE libraries SET recognisers_read = ? WHERE id = ? AND recognisers_read IS NOT ?",
            read,
            id,
            read,
        )
        .execute(conn)
        .await?;
        Ok(changed.rows_affected() > 0)
    }
}

/// `name`, `folders` and `order` must already be checked; what `order` leaves empty keeps the
/// column's default.
async fn create(
    conn: &mut SqliteConnection,
    name: String,
    kind: Kind,
    folders: Vec<String>,
    order: Order,
) -> Result<Library, Error> {
    let Order {
        mut title_sources,
        recognisers,
    } = order;
    ensure_folders_free(conn, &folders, None).await?;
    let id = sqlx::query_scalar!(
        r#"INSERT INTO libraries (name, kind) VALUES (?, ?) RETURNING id AS "id!""#,
        name,
        kind,
    )
    .fetch_one(&mut *conn)
    .await?;
    insert_folders(conn, id, &folders).await?;
    if title_sources.is_empty() {
        title_sources = vec!["tmdb".to_owned()];
    } else {
        set_title_sources(conn, id, title_sources.join(",")).await?;
    }
    let recognisers = recognisers.unwrap_or_default();
    if !recognisers.is_empty() {
        set_recognisers(conn, id, recognisers.join(",")).await?;
    }
    Ok(Library {
        id,
        name,
        kind,
        folders,
        read_only: Vec::new(),
        title_sources,
        recognisers,
    })
}

/// `name`, `folders` and `order` must already be checked; what `order` leaves out keeps the
/// stored order.
async fn update(
    conn: &mut SqliteConnection,
    id: i64,
    name: String,
    folders: Vec<String>,
    order: Order,
) -> Result<Library, Error> {
    let Order {
        title_sources,
        recognisers,
    } = order;
    let row = sqlx::query!(
        r#"UPDATE libraries SET name = ? WHERE id = ?
           RETURNING kind AS "kind: Kind", title_sources, recognisers"#,
        name,
        id,
    )
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(Error::NotFound(id))?;
    ensure_folders_free(conn, &folders, Some(id)).await?;
    // A folder it keeps stays read-only until a scan says otherwise; a new one holds nothing to
    // delete until a scan, which checks it.
    let read_only: Vec<String> = sqlx::query_scalar!(
        "SELECT path FROM library_folders WHERE library_id = ? AND read_only = 1",
        id
    )
    .fetch_all(&mut *conn)
    .await?;
    sqlx::query!("DELETE FROM library_folders WHERE library_id = ?", id)
        .execute(&mut *conn)
        .await?;
    insert_folders(conn, id, &folders).await?;
    let read_only: Vec<String> = (read_only.into_iter())
        .filter(|folder| folders.contains(folder))
        .collect();
    set_read_only(conn, id, read_only.clone()).await?;
    let title_sources = if title_sources.is_empty() {
        split_sources(&row.title_sources)
    } else {
        set_title_sources(conn, id, title_sources.join(",")).await?;
        title_sources
    };
    let recognisers = match recognisers {
        Some(recognisers) => {
            set_recognisers(conn, id, recognisers.join(",")).await?;
            recognisers
        }
        None => split_recognisers(&row.recognisers),
    };
    Ok(Library {
        id,
        name,
        kind: row.kind,
        folders,
        read_only,
        title_sources,
        recognisers,
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

async fn set_read_only(
    conn: &mut SqliteConnection,
    id: i64,
    read_only: Vec<String>,
) -> Result<(), Error> {
    sqlx::query!(
        "UPDATE library_folders SET read_only = 0 WHERE library_id = ?",
        id
    )
    .execute(&mut *conn)
    .await?;
    for folder in read_only {
        sqlx::query!(
            "UPDATE library_folders SET read_only = 1 WHERE library_id = ? AND path = ?",
            id,
            folder,
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// `order` must already be checked and joined with commas.
async fn set_title_sources(
    conn: &mut SqliteConnection,
    id: i64,
    order: String,
) -> Result<(), Error> {
    let updated = sqlx::query!(
        "UPDATE libraries SET title_sources = ? WHERE id = ?",
        order,
        id
    )
    .execute(&mut *conn)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(Error::NotFound(id));
    }
    Ok(())
}

/// `order` must be Plugin ids joined with commas, none if empty.
async fn set_recognisers(conn: &mut SqliteConnection, id: i64, order: String) -> Result<(), Error> {
    let updated = sqlx::query!(
        "UPDATE libraries SET recognisers = ? WHERE id = ?",
        order,
        id
    )
    .execute(&mut *conn)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(Error::NotFound(id));
    }
    Ok(())
}

/// Fails unless `order` is one or more namespaces by `ExternalId`'s rule, each named once, as
/// [`Libraries::set_title_sources`] needs, so a caller can check before it changes anything else.
pub(crate) fn check_title_sources(order: &[String]) -> Result<(), Error> {
    if order.is_empty() {
        return Err(Error::TitleSources { namespace: None });
    }
    for (seen, namespace) in (1..).zip(order) {
        // Any value will do: only the namespace is being checked.
        let malformed = matches!(
            ExternalId::new(namespace, "_"),
            Err(InvalidId::Namespace(_))
        );
        let repeated = order.iter().take(seen).filter(|n| *n == namespace).count() > 1;
        if malformed || repeated {
            return Err(Error::TitleSources {
                namespace: Some(namespace.clone()),
            });
        }
    }
    Ok(())
}

/// The stored, comma-separated `title_sources`, as a list. A well-formed namespace has no comma.
fn split_sources(stored: &str) -> Vec<String> {
    stored.split(',').map(str::to_owned).collect()
}

/// The stored, comma-separated `recognisers`, as a list; empty names none. A Plugin id has no
/// comma.
fn split_recognisers(stored: &str) -> Vec<String> {
    (stored.split(',').filter(|id| !id.is_empty()))
        .map(str::to_owned)
        .collect()
}

fn check_name(name: &str) -> Result<String, Error> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::NameBlank);
    }
    Ok(name.to_owned())
}

// Canonical paths make two spellings of one folder (a trailing slash, a symlink) the same folder.
async fn check_folders(folders: Vec<String>) -> Result<Vec<String>, Error> {
    if folders.is_empty() {
        return Err(Error::FoldersEmpty);
    }
    let mut canonical = Vec::with_capacity(folders.len());
    for (index, folder) in folders.into_iter().enumerate() {
        match check_folder(&folder, &canonical).await {
            Ok(path) => canonical.push(path),
            Err(problem) => {
                return Err(Error::Folder {
                    index,
                    folder,
                    problem,
                });
            }
        }
    }
    Ok(canonical)
}

/// `folder`'s canonical path, if it can be a Library's and isn't in `earlier` yet.
async fn check_folder(folder: &str, earlier: &[String]) -> Result<String, FolderProblem> {
    if !Path::new(folder).is_absolute() {
        return Err(FolderProblem::NotAbsolute);
    }
    let path = tokio::fs::canonicalize(folder)
        .await
        .map_err(FolderProblem::NotFound)?;
    ensure_readable_dir(&path)
        .await
        .map_err(FolderProblem::NotReadable)?;
    let path = path
        .into_os_string()
        .into_string()
        .map_err(|path| FolderProblem::NotUtf8(Path::new(&path).display().to_string()))?;
    if earlier.contains(&path) {
        return Err(FolderProblem::Repeated);
    }
    Ok(path)
}

/// Fails unless `path` is a directory the Server can list and open the files in.
async fn ensure_readable_dir(path: &Path) -> std::io::Result<()> {
    // Listing fails for anything that isn't a directory, and for a directory without read
    // permission.
    tokio::fs::read_dir(path).await?.next_entry().await?;
    // Opening what's inside also needs search (execute) permission, which reaching `.` through
    // the directory checks. Windows has no separate permission for this, and its verbatim
    // `\\?\` paths don't resolve `.`, so the check is Unix-only.
    #[cfg(unix)]
    tokio::fs::metadata(path.join(".")).await?;
    Ok(())
}

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

#[cfg(test)]
#[path = "libraries_tests.rs"]
mod tests;
