//! Provide: a blob store, which keeps the Kernel's own stored files, such as the image store's
//! originals, custom images and Plugin logos; never media files (ADR 0052).
//!
//! A Plugin serves it by registering an `Arc<dyn BlobStore>`:
//!
//! ```ignore
//! registrar.provide::<dyn BlobStore>(Priority::DEFAULT, Arc::new(my_store));
//! ```

pub use nama_plugin_proto::BLOB_MAX_BYTES;

use crate::{
    BoxFuture, PluginError,
    contract::{
        BlobDeleted, BlobExists, BlobGot, BlobKey, BlobPage, BlobPut, BlobToPut, BlobsAsked,
    },
};

/// A blob store. It speaks the contract's `BlobStoreService` messages, as an Installed Plugin's
/// exports do, and every method is required.
///
/// A key is `/`-separated, at most 1024 bytes, of ASCII letters, digits and `-_.%/`. An object is
/// at most [`BLOB_MAX_BYTES`]: the Kernel never puts a larger one, and a store may refuse one.
pub trait BlobStore: Send + Sync + 'static {
    /// Stores `blob.bytes` at `blob.key`, replacing any object there.
    ///
    /// # Errors
    ///
    /// When the object couldn't be stored, which fails the Kernel's write.
    fn put<'a>(&'a self, blob: &'a BlobToPut) -> BoxFuture<'a, Result<BlobPut, PluginError>>;

    /// The object at `key`, with `found` false if there is none.
    ///
    /// # Errors
    ///
    /// When the store couldn't be read; a missing key is not an error.
    fn get<'a>(&'a self, key: &'a BlobKey) -> BoxFuture<'a, Result<BlobGot, PluginError>>;

    /// Deletes the object at `key`; deleting a missing key succeeds.
    ///
    /// # Errors
    ///
    /// When the object couldn't be deleted.
    fn delete<'a>(&'a self, key: &'a BlobKey) -> BoxFuture<'a, Result<BlobDeleted, PluginError>>;

    /// A page of at most 1000 keys starting with `asked.prefix`, from `asked.page_token`, with an
    /// empty `next_page_token` on the last page.
    ///
    /// # Errors
    ///
    /// When the store couldn't be listed.
    fn list<'a>(&'a self, asked: &'a BlobsAsked) -> BoxFuture<'a, Result<BlobPage, PluginError>>;

    /// Whether an object is at `key`.
    ///
    /// # Errors
    ///
    /// When the store couldn't be asked.
    fn exists<'a>(&'a self, key: &'a BlobKey) -> BoxFuture<'a, Result<BlobExists, PluginError>>;
}
