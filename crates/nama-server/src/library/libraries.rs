//! Libraries: named collections of one kind of media, each backed by one or more folders.
//!
//! [`Libraries`] is the only way in. It checks every name and folder before anything is
//! written, and it keeps the rules a Library lives by: a folder belongs to at most one Library,
//! and a Library's kind is fixed when it is created.

use std::{collections::HashMap, sync::Arc};

use nama_plugin_api::metadata::{ExternalId, InvalidId, SourceManifest, TMDB};
use sqlx::SqliteConnection;

use super::folders::{FolderProblem, check_folders, ensure_folders_free, insert_folders};
use crate::{db::Database, users::name_key};

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
    /// Those of `folders` its last scan found no file in where an earlier one found some. That
    /// scan kept what's stored under them, as when a share isn't mounted.
    pub(crate) empty_folders: Vec<String>,
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
    /// Title source namespaces, first asked first; empty keeps the stored order. A new Library's
    /// names at least one: the Scanner gives one naming none the default.
    pub(crate) title_sources: Vec<String>,
    /// Recogniser Plugin ids, first asked first; `None` keeps the stored order, or none for a
    /// new Library.
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
    /// Another Library, named `library`, has the name, ignoring case and how accents were typed.
    #[error("the Library {library:?} already has this name")]
    NameTaken { library: String },
    /// A folder in the request already belongs to another Library.
    #[error("the folder {folder} already belongs to the Library {library:?}")]
    FolderTaken { folder: String, library: String },
    /// A folder in the request is `inside` another Library's folder, or holds it.
    #[error(
        "the folder {folder} {} {library_folder}, a folder of the Library {library:?}",
        if *inside { "is inside" } else { "holds" }
    )]
    FolderOverlaps {
        folder: String,
        library: String,
        library_folder: String,
        inside: bool,
    },
    /// The title sources given aren't one or more well-formed namespaces, each named once;
    /// `namespace` is the first that breaks it, none if none was given.
    #[error(
        "a Library's title sources must be one or more well-formed namespaces, each named once"
    )]
    TitleSources { namespace: Option<String> },
    /// The title sources name `namespace`, which no Plugin the Server has keeps or looks titles
    /// up by.
    #[error("no Plugin keeps or looks titles up by {namespace:?}")]
    TitleSourceUnknown { namespace: String },
    /// No source the title sources name searches, so the Library could never Match (ADR 0045).
    #[error("no source in this order searches for titles; add one that does, such as TMDB")]
    NoTitleSourceSearches,
    /// The Recognisers name `plugin` twice, or it has a comma or is no Plugin whose Manifest names
    /// the Recogniser point (ADR 0050).
    #[error("{plugin:?} is no Recogniser's Plugin, or is named twice")]
    RecognisersInvalid { plugin: String },
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
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
        let order = Order {
            title_sources: default_title_sources(),
            recognisers: None,
        };
        (self.create_in_order(name, kind, folders, order)).await
    }

    /// Creates a Library asking its Plugins in `order`, which must already be checked, in one
    /// write.
    pub(crate) async fn create_in_order(
        &self,
        name: &str,
        kind: Kind,
        folders: Vec<String>,
        order: Order,
    ) -> Result<Library, Error> {
        // Stored joined with commas, so an empty order would read back as one blank namespace.
        debug_assert!(
            !order.title_sources.is_empty(),
            "a new Library's title sources weren't defaulted and checked"
        );
        let name = check_name(name)?;
        let folders = check_folders(folders).await?;
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
        let mut empty: HashMap<i64, Vec<String>> = HashMap::new();
        for folder in sqlx::query!(
            r#"SELECT library_id, path, read_only AS "read_only: bool",
                      looks_empty AS "looks_empty: bool"
               FROM library_folders ORDER BY library_id, position"#
        )
        .fetch_all(&mut *tx)
        .await?
        {
            if folder.read_only {
                let library = read_only.entry(folder.library_id).or_default();
                library.push(folder.path.clone());
            }
            if folder.looks_empty {
                let library = empty.entry(folder.library_id).or_default();
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
                empty_folders: empty.remove(&row.id).unwrap_or_default(),
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
            r#"SELECT path, read_only AS "read_only: bool", looks_empty AS "looks_empty: bool"
               FROM library_folders WHERE library_id = ? ORDER BY position"#,
            id
        )
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        let read_only = (rows.iter().filter(|folder| folder.read_only))
            .map(|folder| folder.path.clone())
            .collect();
        let empty_folders = (rows.iter().filter(|folder| folder.looks_empty))
            .map(|folder| folder.path.clone())
            .collect();
        Ok(Library {
            id,
            name: row.name,
            kind: row.kind,
            folders: rows.into_iter().map(|folder| folder.path).collect(),
            read_only,
            empty_folders,
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

    /// Records that each of the Library `id`'s folders but `empty` holds files, or never held any.
    pub(crate) async fn set_empty_folders(&self, id: i64, empty: Vec<String>) -> Result<(), Error> {
        self.db
            .write(move |tx| Box::pin(set_empty_folders(tx, id, empty)))
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

    /// Renames the Library, replaces its folders and whichever of its orders `order`, which must
    /// already be checked, gives, in one write; its kind can't change.
    pub(crate) async fn update(
        &self,
        id: i64,
        name: &str,
        folders: Vec<String>,
        order: Order,
    ) -> Result<Library, Error> {
        let name = check_name(name)?;
        let folders = check_folders(folders).await?;
        self.db
            .write(move |tx| Box::pin(update(tx, id, name, folders, order)))
            .await
    }

    /// Deletes the Library. The files in its folders are left alone.
    pub(crate) async fn delete(&self, id: i64) -> Result<(), Error> {
        self.db.write(move |tx| Box::pin(delete(tx, id))).await
    }

    /// Whether `read`, as [`Self::record_recognisers_read`] takes it, differs from the Recognisers
    /// the Library `id`'s last scan recorded. False for a Library there isn't.
    pub(crate) async fn recognisers_changed(&self, id: i64, read: &str) -> sqlx::Result<bool> {
        let changed = sqlx::query_scalar!(
            r#"SELECT recognisers_read IS NOT ? AS "changed!: bool" FROM libraries WHERE id = ?"#,
            read,
            id,
        )
        .fetch_optional(self.db.reader())
        .await?;
        Ok(changed.unwrap_or(false))
    }

    /// Records `read`, the Library `id`'s enabled Recognisers in its order, joined with commas; if
    /// they differ from its last scan's, Contents reads its unparsed files again and forgets its
    /// Unmatched, in one write (ADR 0050). Unchanged, it only reads, so waits behind no write.
    pub(super) async fn read_again_if_recognisers_changed(
        &self,
        id: i64,
        read: String,
    ) -> Result<(), sqlx::Error> {
        if !self.recognisers_changed(id, &read).await? {
            return Ok(());
        }
        (self.db)
            .write(move |tx| Box::pin(read_again_if_recognisers_changed(tx, id, read)))
            .await
    }

    /// Records `read`; whether it differs from the last scan's.
    async fn record_recognisers_read(
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

/// The title sources of a Library created naming none.
pub(crate) fn default_title_sources() -> Vec<String> {
    vec![TMDB.to_owned()]
}

/// `name`, `folders` and `order` must already be checked.
async fn create(
    conn: &mut SqliteConnection,
    name: String,
    kind: Kind,
    folders: Vec<String>,
    order: Order,
) -> Result<Library, Error> {
    let Order {
        title_sources,
        recognisers,
    } = order;
    ensure_name_free(conn, &name).await?;
    ensure_folders_free(conn, &folders, None).await?;
    let id = sqlx::query_scalar!(
        r#"INSERT INTO libraries (name, kind) VALUES (?, ?) RETURNING id AS "id!""#,
        name,
        kind,
    )
    .fetch_one(&mut *conn)
    .await?;
    insert_folders(conn, id, &folders).await?;
    set_title_sources(conn, id, title_sources.join(",")).await?;
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
        empty_folders: Vec::new(),
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
    let current = sqlx::query_scalar!("SELECT name FROM libraries WHERE id = ?", id)
        .fetch_optional(&mut *conn)
        .await?
        .ok_or(Error::NotFound(id))?;
    // Keeping its name, even in other letter case, lets a Library that shared it before names were
    // unique keep it.
    if name_key(&name) != name_key(&current) {
        ensure_name_free(conn, &name).await?;
    }
    let row = sqlx::query!(
        r#"UPDATE libraries SET name = ? WHERE id = ?
           RETURNING kind AS "kind: Kind", title_sources, recognisers"#,
        name,
        id,
    )
    .fetch_one(&mut *conn)
    .await?;
    ensure_folders_free(conn, &folders, Some(id)).await?;
    // A folder it keeps stays read-only, or empty, until a scan says otherwise; a new one holds
    // nothing to delete or keep until a scan, which checks it.
    let kept = sqlx::query!(
        r#"SELECT path, read_only AS "read_only: bool", looks_empty AS "looks_empty: bool"
           FROM library_folders WHERE library_id = ? AND (read_only = 1 OR looks_empty = 1)"#,
        id
    )
    .fetch_all(&mut *conn)
    .await?;
    sqlx::query!("DELETE FROM library_folders WHERE library_id = ?", id)
        .execute(&mut *conn)
        .await?;
    insert_folders(conn, id, &folders).await?;
    let kept_where = |flag: fn(&_) -> bool| -> Vec<String> {
        (kept
            .iter()
            .filter(|folder| flag(folder) && folders.contains(&folder.path)))
        .map(|folder| folder.path.clone())
        .collect()
    };
    let (read_only, empty_folders) = (
        kept_where(|folder| folder.read_only),
        kept_where(|folder| folder.looks_empty),
    );
    set_read_only(conn, id, read_only.clone()).await?;
    set_empty_folders(conn, id, empty_folders.clone()).await?;
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
        empty_folders,
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

async fn set_empty_folders(
    conn: &mut SqliteConnection,
    id: i64,
    empty: Vec<String>,
) -> Result<(), Error> {
    sqlx::query!(
        "UPDATE library_folders SET looks_empty = 0 WHERE library_id = ?",
        id
    )
    .execute(&mut *conn)
    .await?;
    for folder in empty {
        sqlx::query!(
            "UPDATE library_folders SET looks_empty = 1 WHERE library_id = ? AND path = ?",
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

async fn read_again_if_recognisers_changed(
    conn: &mut SqliteConnection,
    id: i64,
    read: String,
) -> Result<(), sqlx::Error> {
    if Libraries::record_recognisers_read(conn, id, &read).await? {
        super::Contents::read_unparsed_again(conn, id).await?;
        super::Contents::forget_unmatched(conn, id).await?;
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

/// Fails unless `order` is one or more namespaces by `ExternalId`'s rule, each named once, as a
/// stored order needs.
fn check_title_sources(order: &[String]) -> Result<(), Error> {
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

/// Fails unless `order`, sent in place of a Library's `stored` one, is well formed, names only
/// namespaces some source in `known` (every Plugin's, enabled or not) keeps or looks titles up by,
/// and names one a source that searches keeps. A namespace only looked up by, such as `imdb`, may
/// be named to prefer its ids (ADR 0042). What `stored` already names isn't judged, and a stored
/// namespace no source in `known` keeps may be the one that searches, so a Library whose Plugin
/// went away stays editable.
pub(crate) fn check_known(
    order: &[String],
    stored: &[String],
    known: &[SourceManifest],
) -> Result<(), Error> {
    if order == stored {
        return Ok(());
    }
    // First, so a malformed or repeated namespace is refused as that.
    check_title_sources(order)?;
    let is_known = |namespace: &String| {
        (known.iter())
            .any(|source| source.namespace == *namespace || source.looks_up.contains(namespace))
    };
    let searches = |namespace: &String| {
        let mut keeping = (known.iter()).filter(|source| source.namespace == *namespace);
        match keeping.next() {
            Some(first) => first.searches || keeping.any(|source| source.searches),
            None => stored.contains(namespace),
        }
    };
    let mut added = order.iter().filter(|namespace| !stored.contains(namespace));
    if let Some(namespace) = added.find(|namespace| !is_known(namespace)) {
        return Err(Error::TitleSourceUnknown {
            namespace: namespace.clone(),
        });
    }
    if !order.iter().any(searches) {
        return Err(Error::NoTitleSourceSearches);
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

/// Fails if a Library already has `name`, by the key a User's name is unique under. Folded in
/// Rust, as SQLite's `lower()` and `NOCASE` fold only ASCII.
async fn ensure_name_free(conn: &mut SqliteConnection, name: &str) -> Result<(), Error> {
    let key = name_key(name);
    let names = sqlx::query_scalar!("SELECT name FROM libraries")
        .fetch_all(&mut *conn)
        .await?;
    match names.into_iter().find(|other| name_key(other) == key) {
        Some(library) => Err(Error::NameTaken { library }),
        None => Ok(()),
    }
}

#[cfg(test)]
#[path = "libraries_tests.rs"]
mod tests;
