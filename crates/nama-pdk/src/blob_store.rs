//! A blob store: keeps the Kernel's own stored files, such as the image store's originals.

use extism_pdk::Error;
pub use nama_plugin_proto::{BLOB_MAX_BYTES, nama::plugin::v1::BlobPage};

/// A blob store: keeps the Kernel's own stored files, such as the image store's originals, custom
/// images and Plugin logos, and never media files (ADR 0052). Its Manifest's `points` name
/// [`services::BLOB_STORE`](crate::services::BLOB_STORE), and [`blob_store!`](crate::blob_store!)
/// exports every method, all of which are required.
///
/// A key is `/`-separated, at most 1024 bytes, of ASCII letters, digits and `-_.%/`. An object is
/// at most [`BLOB_MAX_BYTES`]: the Server never puts a larger one, and a store may refuse one. One
/// call carries one object, so keep no more than it in memory:
///
/// ```
/// use nama_pdk::{BlobPage, BlobStore, extism_pdk::Error};
///
/// struct Bucket;
///
/// impl BlobStore for Bucket {
///     fn put(key: String, _bytes: Vec<u8>) -> Result<(), Error> {
///         Err(Error::msg(format!("the bucket is read-only, so {key} wasn't stored")))
///     }
///
///     fn get(_key: String) -> Result<Option<Vec<u8>>, Error> {
///         Ok(None)
///     }
///
///     fn delete(_key: String) -> Result<(), Error> {
///         Ok(())
///     }
///
///     fn list(_prefix: String, _page_token: String) -> Result<BlobPage, Error> {
///         Ok(BlobPage::default())
///     }
///
///     fn exists(_key: String) -> Result<bool, Error> {
///         Ok(false)
///     }
/// }
///
/// nama_pdk::blob_store!(Bucket);
/// ```
pub trait BlobStore {
    /// Stores `bytes` at `key`, replacing any object there.
    ///
    /// # Errors
    ///
    /// When the object wasn't stored, such as when the store is unreachable or the object is over
    /// [`BLOB_MAX_BYTES`]; the Server's write fails with the error's message.
    fn put(key: String, bytes: Vec<u8>) -> Result<(), Error>;

    /// The object at `key`, or `None` if there is none.
    ///
    /// # Errors
    ///
    /// When the store couldn't be read; a missing key is `None`, not an error.
    fn get(key: String) -> Result<Option<Vec<u8>>, Error>;

    /// Deletes the object at `key`; deleting a missing key succeeds.
    ///
    /// # Errors
    ///
    /// When the object couldn't be deleted.
    fn delete(key: String) -> Result<(), Error>;

    /// A page of at most 1000 keys starting with `prefix`, going on from `page_token`, which is
    /// empty for the first page and otherwise the `next_page_token` of the page before. The last
    /// page's `next_page_token` is empty.
    ///
    /// # Errors
    ///
    /// When the store couldn't be listed.
    fn list(prefix: String, page_token: String) -> Result<BlobPage, Error>;

    /// Whether an object is at `key`.
    ///
    /// # Errors
    ///
    /// When the store couldn't be asked.
    fn exists(key: String) -> Result<bool, Error>;
}

/// Exports a [`BlobStore`] as the Plugin's `blob_put`, `blob_get`, `blob_delete`, `blob_list` and
/// `blob_exists`, taking and returning the `nama.plugin.v1` messages the Server exchanges with
/// them.
#[macro_export]
macro_rules! blob_store {
    ($store:ty) => {
        const _: () = {
            // `plugin_fn` expands to paths starting with `extism_pdk`.
            use $crate::{
                BlobPage, BlobStore, Buffa, FnResult, extism_pdk,
                nama_plugin_proto::nama::plugin::v1::{
                    BlobDeleted, BlobExists, BlobGot, BlobKey, BlobPut, BlobToPut, BlobsAsked,
                },
                plugin_fn,
            };

            #[plugin_fn]
            pub fn blob_put(Buffa(blob): Buffa<BlobToPut>) -> FnResult<Buffa<BlobPut>> {
                <$store as BlobStore>::put(blob.key, blob.bytes)?;
                Ok(Buffa(BlobPut::default()))
            }

            #[plugin_fn]
            pub fn blob_get(Buffa(key): Buffa<BlobKey>) -> FnResult<Buffa<BlobGot>> {
                let got = <$store as BlobStore>::get(key.key)?;
                Ok(Buffa(BlobGot {
                    found: got.is_some(),
                    bytes: got.unwrap_or_default(),
                    ..BlobGot::default()
                }))
            }

            #[plugin_fn]
            pub fn blob_delete(Buffa(key): Buffa<BlobKey>) -> FnResult<Buffa<BlobDeleted>> {
                <$store as BlobStore>::delete(key.key)?;
                Ok(Buffa(BlobDeleted::default()))
            }

            #[plugin_fn]
            pub fn blob_list(Buffa(asked): Buffa<BlobsAsked>) -> FnResult<Buffa<BlobPage>> {
                Ok(Buffa(<$store as BlobStore>::list(
                    asked.prefix,
                    asked.page_token,
                )?))
            }

            #[plugin_fn]
            pub fn blob_exists(Buffa(key): Buffa<BlobKey>) -> FnResult<Buffa<BlobExists>> {
                Ok(Buffa(BlobExists {
                    exists: <$store as BlobStore>::exists(key.key)?,
                    ..BlobExists::default()
                }))
            }
        };
    };
}
