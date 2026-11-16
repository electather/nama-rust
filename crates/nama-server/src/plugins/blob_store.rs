//! The image store's [`Blobs`] kept by a Plugin that serves the blob store point (ADR 0052).

use std::{
    future::Future,
    io::{self, Read},
    sync::Arc,
};

use nama_plugin_api::{
    PluginError, PluginId,
    blob_store::BlobStore,
    contract::{BlobKey, BlobPage, BlobToPut, BlobsAsked},
};
use tempfile::NamedTempFile;

use super::{ExtensionPoints, Failure};
use crate::{
    db::BoxFuture,
    images::{Blobs, MAX_BYTES},
};

/// The objects the Plugin `plugin`'s [`BlobStore`] keeps, asked through `points` at each call, so
/// a call after the Plugin is disabled fails rather than reaching it.
#[derive(Debug, Clone)]
#[cfg_attr(not(test), expect(dead_code, reason = "wired in by bs-switch-api"))]
pub(crate) struct PluginBlobs {
    points: ExtensionPoints,
    plugin: PluginId,
}

#[cfg_attr(not(test), expect(dead_code, reason = "wired in by bs-switch-api"))]
impl PluginBlobs {
    /// The objects `plugin` keeps.
    pub(crate) fn new(points: ExtensionPoints, plugin: PluginId) -> Self {
        Self { points, plugin }
    }

    /// Asks the Plugin's [`BlobStore`] with `call`.
    ///
    /// # Errors
    ///
    /// If the Plugin isn't enabled or serves no blob store (a panic disables it), or with the
    /// Plugin's own error as its message.
    async fn ask<R, F>(&self, call: impl Fn(Arc<dyn BlobStore>) -> F) -> io::Result<R>
    where
        F: Future<Output = Result<R, PluginError>>,
    {
        let answer = self.points.ask(&self.plugin, call).await.ok_or_else(|| {
            io::Error::other(format!(
                "plugin {} isn't an enabled blob store",
                self.plugin
            ))
        })?;
        answer
            .result
            .map_err(|failure: Failure| io::Error::other(failure.message))
    }

    async fn put_blob(&self, blob: BlobToPut) -> io::Result<()> {
        if blob.bytes.len() > MAX_BYTES {
            return Err(too_large());
        }
        let blob = &blob;
        self.ask(|store| async move { store.put(blob).await })
            .await?;
        Ok(())
    }

    async fn page(&self, asked: &BlobsAsked) -> io::Result<BlobPage> {
        self.ask(|store| async move { store.list(asked).await })
            .await
    }

    async fn list_all(&self, prefix: String) -> io::Result<Vec<String>> {
        let mut asked = BlobsAsked {
            prefix,
            ..BlobsAsked::default()
        };
        let mut keys = Vec::new();
        loop {
            let page = self.page(&asked).await?;
            keys.extend(page.keys);
            if page.next_page_token.is_empty() {
                return Ok(keys);
            }
            // A store that hands back the token it was given would be listed forever.
            if page.next_page_token == asked.page_token {
                return Err(io::Error::other(format!(
                    "plugin {} listed the same page twice",
                    self.plugin
                )));
            }
            asked.page_token = page.next_page_token;
        }
    }
}

impl Blobs for PluginBlobs {
    fn put(&self, key: &str, bytes: Vec<u8>) -> BoxFuture<'_, io::Result<()>> {
        let blob = BlobToPut {
            key: key.to_owned(),
            bytes,
            ..BlobToPut::default()
        };
        Box::pin(self.put_blob(blob))
    }

    fn put_file(&self, key: &str, file: NamedTempFile) -> BoxFuture<'_, io::Result<()>> {
        let key = key.to_owned();
        Box::pin(async move {
            let bytes = tokio::task::spawn_blocking(move || read_bounded(&file)).await??;
            self.put_blob(BlobToPut {
                key,
                bytes,
                ..BlobToPut::default()
            })
            .await
        })
    }

    fn get(&self, key: &str) -> BoxFuture<'_, io::Result<Option<Vec<u8>>>> {
        let key = blob_key(key);
        Box::pin(async move {
            let key = &key;
            let got = self
                .ask(|store| async move { store.get(key).await })
                .await?;
            Ok(got.found.then_some(got.bytes))
        })
    }

    fn delete(&self, key: &str) -> BoxFuture<'_, io::Result<()>> {
        let key = blob_key(key);
        Box::pin(async move {
            let key = &key;
            self.ask(|store| async move { store.delete(key).await })
                .await?;
            Ok(())
        })
    }

    fn list(&self, prefix: &str) -> BoxFuture<'_, io::Result<Vec<String>>> {
        if prefix.is_empty() {
            return Box::pin(async {
                Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "listing everything is refused",
                ))
            });
        }
        Box::pin(self.list_all(prefix.to_owned()))
    }

    fn exists(&self, key: &str) -> BoxFuture<'_, io::Result<bool>> {
        let key = blob_key(key);
        Box::pin(async move {
            let key = &key;
            let exists = self
                .ask(|store| async move { store.exists(key).await })
                .await?;
            Ok(exists.exists)
        })
    }
}

#[cfg_attr(not(test), expect(dead_code, reason = "wired in by bs-switch-api"))]
fn blob_key(key: &str) -> BlobKey {
    BlobKey {
        key: key.to_owned(),
        ..BlobKey::default()
    }
}

/// `file`'s bytes, reading no more than one past [`MAX_BYTES`] to refuse a larger one. Blocks, so
/// call it from `spawn_blocking`.
#[cfg_attr(not(test), expect(dead_code, reason = "wired in by bs-switch-api"))]
fn read_bounded(file: &NamedTempFile) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    // `reopen` reads from the start, wherever the writer left the file's offset.
    let limit = u64::try_from(MAX_BYTES).map_or(u64::MAX, |max| max.saturating_add(1));
    file.reopen()?.take(limit).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_BYTES {
        return Err(too_large());
    }
    Ok(bytes)
}

fn too_large() -> io::Error {
    io::Error::new(
        io::ErrorKind::FileTooLarge,
        format!("an object over {MAX_BYTES} bytes is refused"),
    )
}

#[cfg(test)]
#[path = "blob_store_tests.rs"]
mod tests;
