//! An Installed Plugin as a blob store: the Kernel's `BlobStore` calls passed through as the
//! contract's `BlobStoreService` rpcs.

use nama_plugin_api::{
    BoxFuture, PluginError,
    blob_store::BlobStore,
    contract::{
        BlobDeleted, BlobExists, BlobGot, BlobKey, BlobPage, BlobPut, BlobToPut, BlobsAsked,
    },
};

use super::{
    Running,
    services::{BLOB_DELETE, BLOB_EXISTS, BLOB_GET, BLOB_LIST, BLOB_PUT},
};

impl BlobStore for Running {
    fn put<'a>(&'a self, blob: &'a BlobToPut) -> BoxFuture<'a, Result<BlobPut, PluginError>> {
        Box::pin(self.call_method(&BLOB_PUT, blob))
    }

    fn get<'a>(&'a self, key: &'a BlobKey) -> BoxFuture<'a, Result<BlobGot, PluginError>> {
        Box::pin(self.call_method(&BLOB_GET, key))
    }

    fn delete<'a>(&'a self, key: &'a BlobKey) -> BoxFuture<'a, Result<BlobDeleted, PluginError>> {
        Box::pin(self.call_method(&BLOB_DELETE, key))
    }

    fn list<'a>(&'a self, asked: &'a BlobsAsked) -> BoxFuture<'a, Result<BlobPage, PluginError>> {
        Box::pin(self.call_method(&BLOB_LIST, asked))
    }

    fn exists<'a>(&'a self, key: &'a BlobKey) -> BoxFuture<'a, Result<BlobExists, PluginError>> {
        Box::pin(self.call_method(&BLOB_EXISTS, key))
    }
}

#[cfg(test)]
#[path = "blob_store_tests.rs"]
mod tests;
