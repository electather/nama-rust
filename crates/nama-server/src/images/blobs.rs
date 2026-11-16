//! Where the image store keeps its originals: one interface, so a store other than the data
//! directory can hold them.

use std::{fmt, io};

use tempfile::NamedTempFile;

use crate::db::BoxFuture;

/// The most bytes one stored object may have: 20 MiB, as the contract's `BLOB_MAX_BYTES` says.
pub(crate) const MAX_BYTES: usize = 20 * 1024 * 1024;

const _: () = assert!(
    MAX_BYTES as u64 == super::MAX_UPLOAD_BYTES,
    "an upload the store accepts must fit one object"
);

/// A store of objects by key, which the image store keeps its originals in.
///
/// A key is the image store's row form of an image's key: each part escaped by `file_name`,
/// joined by `/`.
pub(crate) trait Blobs: Send + Sync + fmt::Debug {
    /// Stores `bytes` at `key`, replacing any object there, so no reader sees part of it.
    fn put(&self, key: &str, bytes: Vec<u8>) -> BoxFuture<'_, io::Result<()>>;

    /// Stores `file`, already written and synced, at `key`, replacing any object there. A store
    /// that can't rename it into place reads it, refusing one over [`MAX_BYTES`].
    fn put_file(&self, key: &str, file: NamedTempFile) -> BoxFuture<'_, io::Result<()>>;

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
