//! A Library's folders: checking the ones a create or an update names, and storing them.

use std::path::Path;

use sqlx::SqliteConnection;

use super::libraries::Error;

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

// Canonical paths make two spellings of one folder (a trailing slash, a symlink) the same folder.
pub(super) async fn check_folders(folders: Vec<String>) -> Result<Vec<String>, Error> {
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

/// Fails if one of `folders` is, is inside, or holds a folder of a Library other than `except`.
pub(super) async fn ensure_folders_free(
    conn: &mut SqliteConnection,
    folders: &[String],
    except: Option<i64>,
) -> Result<(), Error> {
    let others = sqlx::query!(
        "SELECT library_folders.path, libraries.name FROM library_folders
         JOIN libraries ON libraries.id = library_folders.library_id
         WHERE library_folders.library_id IS NOT ?",
        except,
    )
    .fetch_all(&mut *conn)
    .await?;
    for folder in folders {
        // First, as a path `starts_with` itself and the overlap check would take this for one.
        if let Some(other) = others.iter().find(|other| other.path == *folder) {
            return Err(Error::FolderTaken {
                folder: folder.clone(),
                library: other.name.clone(),
            });
        }
        // By components, not as strings, so `/media/movies2` isn't inside `/media/movies`.
        let path = Path::new(folder);
        for other in &others {
            let inside = path.starts_with(&other.path);
            if inside || Path::new(&other.path).starts_with(path) {
                return Err(Error::FolderOverlaps {
                    folder: folder.clone(),
                    library: other.name.clone(),
                    library_folder: other.path.clone(),
                    inside,
                });
            }
        }
    }
    Ok(())
}

pub(super) async fn insert_folders(
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
