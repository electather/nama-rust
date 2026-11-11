//! The images the Server keeps, as plain files in the data directory (ADR 0037).
//!
//! Only [`ImageStore`] reads or writes them (`AGENTS.md`). A key names an image by a few parts,
//! each escaped to one file name, so no key reaches outside the store: a downloaded Poster's is
//! `artwork`, its Plugin and its Provider's path, which is where it was kept before the store
//! existed.

use std::{
    io::{self, Write as _},
    path::{Path, PathBuf},
};

use axum::{
    extract::Request,
    response::{IntoResponse, Response},
};
use tower_http::services::ServeFile;

/// Stored images, under the data directory.
#[derive(Debug, Clone)]
pub struct ImageStore {
    dir: PathBuf,
}

impl ImageStore {
    /// The images stored in `data_dir`.
    #[must_use]
    pub fn new(data_dir: &Path) -> Self {
        Self {
            dir: data_dir.to_owned(),
        }
    }

    /// Stores `bytes` as the image at `key`, replacing any there, so no reader ever sees part of
    /// it.
    ///
    /// # Errors
    ///
    /// If the file can't be written.
    pub async fn write(&self, key: &[&str], bytes: &[u8]) -> io::Result<()> {
        write_atomically(&self.path(key), bytes).await
    }

    /// Whether an image is stored at `key`.
    ///
    /// # Errors
    ///
    /// If the store can't be read.
    pub async fn contains(&self, key: &[&str]) -> io::Result<bool> {
        tokio::fs::try_exists(self.path(key)).await
    }

    /// The image at `key` as the answer to `request`: 404 if none is stored, 304 for a matching
    /// `If-None-Match`, else the bytes with an `ETag`. The caller sets the content type.
    ///
    /// # Errors
    ///
    /// If the file can't be read.
    pub async fn serve(&self, key: &[&str], request: Request) -> io::Result<Response> {
        let response = ServeFile::new(self.path(key)).try_call(request).await?;
        Ok(response.into_response())
    }

    /// Deletes the image at `key`, if one is stored.
    ///
    /// # Errors
    ///
    /// If the file is there but can't be deleted.
    pub async fn delete(&self, key: &[&str]) -> io::Result<()> {
        match tokio::fs::remove_file(self.path(key)).await {
            Err(err) if err.kind() != io::ErrorKind::NotFound => Err(err),
            _ => Ok(()),
        }
    }

    fn path(&self, key: &[&str]) -> PathBuf {
        key.iter()
            .fold(self.dir.clone(), |path, part| path.join(file_name(part)))
    }
}

/// Writes `bytes` to `path` so no reader ever sees a partial file, even if two writes of the
/// same file race or the machine stops midway.
pub(crate) async fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let (path, bytes) = (path.to_owned(), bytes.to_vec());
    tokio::task::spawn_blocking(move || {
        let dir = path.parent().unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(dir)?;
        // Its name starts with `.`, which `file_name` never gives, so no key names it.
        let mut partial = tempfile::NamedTempFile::new_in(dir)?;
        partial.write_all(&bytes)?;
        // Synced before the rename, or a crash can leave a whole name over an empty file.
        partial.as_file().sync_all()?;
        partial.persist(&path)?;
        Ok(())
    })
    .await?
}

/// `text` as one safe file name: letters, digits, `-`, `_` and (except first) `.` are kept, and
/// every other byte is written as `%XX`. Distinct texts give distinct names, and no name is `.`
/// or `..` or holds a separator.
pub(crate) fn file_name(text: &str) -> String {
    let mut name = String::with_capacity(text.len());
    for (i, byte) in text.bytes().enumerate() {
        let kept =
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_') || (byte == b'.' && i > 0);
        if kept {
            name.push(char::from(byte));
        } else {
            name.push('%');
            for nibble in [byte >> 4, byte & 0xF] {
                let digit = char::from_digit(u32::from(nibble), 16).unwrap_or('0');
                name.push(digit.to_ascii_uppercase());
            }
        }
    }
    name
}

#[cfg(test)]
mod tests {
    use super::ImageStore;

    const KEY: &[&str] = &["artwork", "tmdb", "/poster.jpg"];

    #[tokio::test]
    async fn a_failed_write_leaves_nothing_behind() {
        let data = tempfile::tempdir().unwrap();
        let store = ImageStore::new(data.path());
        // A folder where the image goes, so the write can't put it there.
        let blocked = data.path().join("artwork/tmdb/%2Fposter.jpg");
        std::fs::create_dir_all(blocked.join("in-the-way")).unwrap();

        let written = store.write(KEY, b"\xff\xd8").await;

        assert!(written.is_err());
        let left: Vec<_> = std::fs::read_dir(blocked.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(left, ["%2Fposter.jpg"]);
    }

    #[tokio::test]
    async fn deleting_an_image_twice_leaves_none_stored() {
        let data = tempfile::tempdir().unwrap();
        let store = ImageStore::new(data.path());
        store.write(KEY, b"\xff\xd8").await.unwrap();

        store.delete(KEY).await.unwrap();

        assert!(!store.contains(KEY).await.unwrap());
        store.delete(KEY).await.unwrap();
    }
}
