//! Where the image store keeps its originals: one interface, so a store other than the data
//! directory can hold them.

use std::{fmt, io, path::PathBuf};

use tempfile::NamedTempFile;

use crate::db::BoxFuture;

/// The most bytes one stored object may have: 20 MiB, as the contract's `BLOB_MAX_BYTES` says.
pub(crate) const MAX_BYTES: usize = 20 * 1024 * 1024;

const _: () = assert!(
    MAX_BYTES as u64 == super::MAX_UPLOAD_BYTES,
    "an upload the store accepts must fit one object"
);

/// The error refusing an object at `key` over [`MAX_BYTES`].
pub(super) fn too_large(key: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::FileTooLarge,
        format!("{key} is over the store's 20 MiB object size limit"),
    )
}

/// A store of objects by key, which the image store keeps its originals in.
///
/// A key is the image store's row form of an image's key: each part escaped by `file_name`,
/// joined by `/`.
pub(crate) trait Blobs: Send + Sync + fmt::Debug {
    /// Stores `bytes` at `key`, replacing any object there, so no reader sees part of it.
    fn put(&self, key: &str, bytes: Vec<u8>) -> BoxFuture<'_, io::Result<()>>;

    /// Stores `file`, already written and synced, at `key`, replacing any object there. A store
    /// that can't rename it into place reads it, refusing one over [`MAX_BYTES`] (`FileTooLarge`).
    fn put_file(&self, key: &str, file: NamedTempFile) -> BoxFuture<'_, io::Result<()>>;

    /// The file the object at `key` would be in, for a store that keeps its objects as local
    /// files, so it can be streamed rather than read whole by [`Blobs::get`], whether or not one is
    /// stored there; `None` for any other store, or for a key no object could have.
    fn local_path(&self, _key: &str) -> Option<PathBuf> {
        None
    }

    /// The object at `key`, or `None` if none is stored there.
    fn get(&self, key: &str) -> BoxFuture<'_, io::Result<Option<Vec<u8>>>>;

    /// Deletes the object at `key`; one already gone counts as deleted.
    fn delete(&self, key: &str) -> BoxFuture<'_, io::Result<()>>;

    /// The key of every object whose key starts with `prefix`.
    ///
    /// # Errors
    ///
    /// `InvalidInput` for an empty `prefix`, so no store ever lists everything it holds.
    fn list(&self, prefix: &str) -> BoxFuture<'_, io::Result<Vec<String>>>;

    /// Whether an object is stored at `key`.
    fn exists(&self, key: &str) -> BoxFuture<'_, io::Result<bool>>;
}
