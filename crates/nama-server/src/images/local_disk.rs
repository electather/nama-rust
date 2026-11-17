//! The Built-in [`Blobs`] store: files in the data directory, key `a/b/c` being the file
//! `<data_dir>/a/b/c`, the layout the image store has always had (ADR 0037).

use std::{
    ffi::OsStr,
    io,
    path::{Path, PathBuf},
};

use tempfile::NamedTempFile;

use super::{Blobs, RESIZED_DIR, synced_beside, unless_gone};
use crate::db::BoxFuture;

/// Objects as files in the data directory.
#[derive(Debug)]
pub(super) struct LocalDisk {
    dir: PathBuf,
}

impl LocalDisk {
    /// The objects kept in `data_dir`.
    pub(super) fn new(data_dir: &Path) -> Self {
        Self {
            dir: data_dir.to_owned(),
        }
    }

    /// The file `key` names.
    ///
    /// # Errors
    ///
    /// `InvalidInput` for a key with an empty part or one starting with `.`: `file_name` gives
    /// neither, and they could reach outside the store or name a write's temporary file.
    fn path(&self, key: &str) -> io::Result<PathBuf> {
        let mut path = self.dir.clone();
        for part in key.split('/') {
            if part.is_empty() || part.starts_with('.') {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("{key:?} isn't an object's key"),
                ));
            }
            path.push(part);
        }
        Ok(path)
    }
}

impl Blobs for LocalDisk {
    fn put(&self, key: &str, bytes: Vec<u8>) -> BoxFuture<'_, io::Result<()>> {
        let path = self.path(key);
        Box::pin(async move {
            let path = path?;
            tokio::task::spawn_blocking(move || write_into(&path, &bytes)).await?
        })
    }

    fn put_file(&self, key: &str, file: NamedTempFile) -> BoxFuture<'_, io::Result<()>> {
        let path = self.path(key);
        Box::pin(async move {
            let path = path?;
            tokio::task::spawn_blocking(move || rename_into(file, &path)).await?
        })
    }

    fn local_path(&self, key: &str) -> Option<PathBuf> {
        self.path(key).ok()
    }

    fn get(&self, key: &str) -> BoxFuture<'_, io::Result<Option<Vec<u8>>>> {
        let path = self.path(key);
        Box::pin(async move {
            match tokio::fs::read(path?).await {
                Ok(bytes) => Ok(Some(bytes)),
                Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
                Err(err) => Err(err),
            }
        })
    }

    fn delete(&self, key: &str) -> BoxFuture<'_, io::Result<()>> {
        let (root, path) = (self.dir.clone(), self.path(key));
        Box::pin(async move {
            let path = path?;
            unless_gone(tokio::fs::remove_file(&path).await)?;
            tokio::task::spawn_blocking(move || remove_empty_folders(&root, &path)).await?;
            Ok(())
        })
    }

    fn list(&self, prefix: &str) -> BoxFuture<'_, io::Result<Vec<String>>> {
        let (root, prefix) = (self.dir.clone(), prefix.to_owned());
        // Every key starting with `prefix` is under the folder its last `/` ends.
        let top = if prefix.is_empty() {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "listing everything is refused",
            ))
        } else {
            match prefix.rsplit_once('/') {
                None | Some(("", _)) => Ok(root.clone()),
                Some((folder, _)) => self.path(folder),
            }
        };
        Box::pin(async move {
            let top = top?;
            tokio::task::spawn_blocking(move || list_under(&root, &top, &prefix)).await?
        })
    }

    fn exists(&self, key: &str) -> BoxFuture<'_, io::Result<bool>> {
        let path = self.path(key);
        Box::pin(async move {
            match tokio::fs::metadata(path?).await {
                // Something else at `key`, such as a folder, isn't an object.
                Ok(metadata) => Ok(metadata.is_file()),
                Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
                Err(err) => Err(err),
            }
        })
    }
}

/// How many times a put makes its folder again after a delete removed it (`remove_empty_folders`).
const FOLDER_TRIES: usize = 3;

/// Writes `bytes` to `path` through a synced temporary file beside it, making its folder if need
/// be. Blocks, so call it from `spawn_blocking`.
fn write_into(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut tries = 1;
    loop {
        // `synced_beside` makes the folder, which a delete can remove before the temporary file
        // is in it; once it is, the folder isn't empty and stays.
        match synced_beside(path, bytes) {
            Err(err) if err.kind() == io::ErrorKind::NotFound && tries < FOLDER_TRIES => tries += 1,
            partial => {
                partial?.persist(path)?;
                return Ok(());
            }
        }
    }
}

/// Renames `file` to `path`, making its folder if need be. Blocks, so call it from
/// `spawn_blocking`.
fn rename_into(mut file: NamedTempFile, path: &Path) -> io::Result<()> {
    let mut tries = 1;
    loop {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        // A rename, so `file` must be on the data directory's filesystem. A delete can remove the
        // folder just made before it lands (`remove_empty_folders`).
        match file.persist(path) {
            Ok(_stored) => return Ok(()),
            Err(err) if err.error.kind() == io::ErrorKind::NotFound && tries < FOLDER_TRIES => {
                file = err.file;
                tries += 1;
            }
            Err(err) => return Err(err.error),
        }
    }
}

/// Removes the folders `deleted` was in that it left empty, up to but not including `root`, as
/// deleting a folder of images always has. Stops at the first that isn't empty or can't be
/// removed: the object is gone either way. Blocks, so call it from `spawn_blocking`.
fn remove_empty_folders(root: &Path, deleted: &Path) {
    for folder in deleted.ancestors().skip(1) {
        if folder == root || !folder.starts_with(root) {
            return;
        }
        match std::fs::remove_dir(folder) {
            Err(err) if err.kind() != io::ErrorKind::NotFound => return,
            _ => {}
        }
    }
}

/// The key of every file under `top` in the store at `root` that starts with `prefix`. Blocks,
/// so call it from `spawn_blocking`.
fn list_under(root: &Path, top: &Path, prefix: &str) -> io::Result<Vec<String>> {
    let mut keys = Vec::new();
    let walk = walkdir::WalkDir::new(top)
        .into_iter()
        .filter_entry(|entry| {
            // Only folders a key starting with `prefix` can be in are entered.
            entry.depth() == 0
                || key_of(root, entry.path()).is_some_and(|key| {
                    key.starts_with(prefix) || prefix.starts_with(&format!("{key}/"))
                })
        });
    for entry in walk {
        match entry {
            Ok(entry) if entry.file_type().is_file() => {
                keys.extend(key_of(root, entry.path()).filter(|key| key.starts_with(prefix)));
            }
            Ok(_) => {}
            // No such `top` yet, or a file deleted while listing.
            Err(err) if err.io_error().map(io::Error::kind) == Some(io::ErrorKind::NotFound) => {}
            Err(err) => return Err(err.into()),
        }
    }
    Ok(keys)
}

/// The key of the file at `path` in the store at `root`, unless it is in the resized copies'
/// folder or a name on the way starts with `.`, as a write's temporary file does.
fn key_of(root: &Path, path: &Path) -> Option<String> {
    let parts: Vec<&str> = path
        .strip_prefix(root)
        .ok()?
        .iter()
        .map(OsStr::to_str)
        .collect::<Option<_>>()?;
    let kept =
        parts.first() != Some(&RESIZED_DIR) && !parts.iter().any(|part| part.starts_with('.'));
    kept.then(|| parts.join("/"))
}

#[cfg(test)]
#[path = "local_disk_tests.rs"]
mod tests;
