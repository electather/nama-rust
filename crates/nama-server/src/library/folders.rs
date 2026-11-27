//! A Library's folders: checking the ones a create or an update names, storing them, and listing
//! the Server's folders to pick one from.

use std::path::Path;

use sqlx::SqliteConnection;

use super::libraries::Error;

/// Why a folder can't be one of a Library's, or can't be listed to pick one from.
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

/// A folder on the Server, as `list_server_folders` lists it.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ServerFolder {
    pub(crate) name: String,
    pub(crate) path: String,
}

/// The folders directly inside `path`, sorted by name ignoring case: `/`'s for an empty `path`,
/// or on Windows the drives that exist. Only `NotAbsolute`, `NotFound` and `NotReadable` come back.
// ponytail: one response per folder, page it if a 5,000-folder movies/ listing lags
pub(crate) async fn list_server_folders(path: &str) -> Result<Vec<ServerFolder>, FolderProblem> {
    #[cfg(windows)]
    if path.is_empty() {
        return Ok(drives().await);
    }
    let path = if path.is_empty() { "/" } else { path };
    if !Path::new(path).is_absolute() {
        return Err(FolderProblem::NotAbsolute);
    }
    let not_a_folder = || std::io::Error::from(std::io::ErrorKind::NotADirectory);
    match tokio::fs::metadata(path).await {
        Ok(found) if found.is_dir() => {}
        Ok(_) => return Err(FolderProblem::NotFound(not_a_folder())),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(FolderProblem::NotFound(err));
        }
        Err(err) => return Err(FolderProblem::NotReadable(err)),
    }
    let mut folders = Vec::new();
    let mut entries = tokio::fs::read_dir(path)
        .await
        .map_err(FolderProblem::NotReadable)?;
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(FolderProblem::NotReadable)?
    {
        if let Some(folder) = listed_folder(&entry).await {
            folders.push(folder);
        }
    }
    folders.sort_by(|a, b| {
        let by_case = a.name.to_lowercase().cmp(&b.name.to_lowercase());
        by_case.then_with(|| a.name.cmp(&b.name))
    });
    Ok(folders)
}

/// `entry` as a folder to list, unless it's left out: a dot-folder, a hidden or system folder on
/// Windows, one that can't be stat'ed or isn't valid UTF-8, or anything that isn't a folder.
async fn listed_folder(entry: &tokio::fs::DirEntry) -> Option<ServerFolder> {
    let name = entry.file_name().into_string().ok()?;
    if name.starts_with('.') {
        return None;
    }
    #[cfg(windows)]
    if hidden_or_system(entry).await {
        return None;
    }
    // `fs::metadata` follows a symlink, so a broken one or one to a file is left out here.
    let path = entry.path();
    if !tokio::fs::metadata(&path).await.ok()?.is_dir() {
        return None;
    }
    Some(ServerFolder {
        name,
        path: path.into_os_string().into_string().ok()?,
    })
}

#[cfg(windows)]
async fn hidden_or_system(entry: &tokio::fs::DirEntry) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    const HIDDEN_OR_SYSTEM: u32 = 0x2 | 0x4;
    // A `DirEntry`'s metadata is the link's own on Windows, so a hidden junction such as
    // `Documents and Settings` stays out though its target isn't hidden.
    entry.metadata().await.map_or(true, |found| {
        found.file_attributes() & HIDDEN_OR_SYSTEM != 0
    })
}

/// The drives that exist, which an empty path lists on Windows.
#[cfg(windows)]
async fn drives() -> Vec<ServerFolder> {
    let mut drives = Vec::new();
    for letter in 'A'..='Z' {
        let drive = format!("{letter}:\\");
        if tokio::fs::metadata(&drive)
            .await
            .is_ok_and(|found| found.is_dir())
        {
            drives.push(ServerFolder {
                name: drive.clone(),
                path: drive,
            });
        }
    }
    drives
}

#[cfg(test)]
#[path = "folders_tests.rs"]
mod tests;
