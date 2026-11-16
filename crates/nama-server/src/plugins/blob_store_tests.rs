use std::{
    collections::BTreeMap,
    io::{self, Write},
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicUsize, Ordering},
    },
};

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    blob_store::BlobStore,
    contract::{
        BlobDeleted, BlobExists, BlobGot, BlobKey, BlobPage, BlobPut, BlobToPut, BlobsAsked,
    },
};

use crate::{
    images::{Blobs, MAX_BYTES},
    plugins::{PluginBlobs, Registry},
};

const ID: PluginId = PluginId::from_static("stored");
/// How many keys the fake lists on one page, small so a test pages.
const PAGE: usize = 2;
/// A key the fake fails to store, with [`BROKEN`] as its error.
const BROKEN_KEY: &str = "broken";
const BROKEN: &str = "the bucket is on fire";

/// A Built-in Plugin whose blob store keeps objects in memory.
struct Stored(Arc<Fake>);

impl Plugin for Stored {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            extension_points: vec![PointKey::provide::<dyn BlobStore>()],
            ..Manifest::new(ID)
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        let store: Arc<dyn BlobStore> = Arc::<Fake>::clone(&self.0);
        registrar.provide::<dyn BlobStore>(Priority::DEFAULT, store);
        Ok(())
    }
}

#[derive(Default)]
struct Fake {
    objects: Mutex<BTreeMap<String, Vec<u8>>>,
    puts: AtomicUsize,
    lists: AtomicUsize,
}

impl Fake {
    fn objects(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, Vec<u8>>> {
        self.objects.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl BlobStore for Fake {
    fn put<'a>(&'a self, blob: &'a BlobToPut) -> BoxFuture<'a, Result<BlobPut, PluginError>> {
        self.puts.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            if blob.key == BROKEN_KEY {
                return Err(BROKEN.into());
            }
            self.objects().insert(blob.key.clone(), blob.bytes.clone());
            Ok(BlobPut::default())
        })
    }

    fn get<'a>(&'a self, key: &'a BlobKey) -> BoxFuture<'a, Result<BlobGot, PluginError>> {
        Box::pin(async move {
            let bytes = self.objects().get(&key.key).cloned();
            Ok(BlobGot {
                found: bytes.is_some(),
                bytes: bytes.unwrap_or_default(),
                ..BlobGot::default()
            })
        })
    }

    fn delete<'a>(&'a self, key: &'a BlobKey) -> BoxFuture<'a, Result<BlobDeleted, PluginError>> {
        Box::pin(async move {
            self.objects().remove(&key.key);
            Ok(BlobDeleted::default())
        })
    }

    /// Pages of [`PAGE`] keys, the token being the last key handed out.
    fn list<'a>(&'a self, asked: &'a BlobsAsked) -> BoxFuture<'a, Result<BlobPage, PluginError>> {
        self.lists.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            let objects = self.objects();
            let mut keys: Vec<String> = objects
                .keys()
                .filter(|key| key.starts_with(&asked.prefix))
                .filter(|key| asked.page_token.is_empty() || **key > asked.page_token)
                .take(PAGE + 1)
                .cloned()
                .collect();
            let more = keys.len() > PAGE;
            keys.truncate(PAGE);
            let next_page_token = if more {
                keys.last().cloned().unwrap_or_default()
            } else {
                String::new()
            };
            Ok(BlobPage {
                keys,
                next_page_token,
                ..BlobPage::default()
            })
        })
    }

    fn exists<'a>(&'a self, key: &'a BlobKey) -> BoxFuture<'a, Result<BlobExists, PluginError>> {
        Box::pin(async move {
            Ok(BlobExists {
                exists: self.objects().contains_key(&key.key),
                ..BlobExists::default()
            })
        })
    }
}

/// A Registry with the fake store enabled, the fake, and the Plugin's objects as [`Blobs`].
fn stored() -> (Registry, Arc<Fake>, PluginBlobs) {
    let registry = Registry::new();
    let fake = Arc::new(Fake::default());
    registry
        .enable(Arc::new(Stored(Arc::clone(&fake))))
        .unwrap();
    let blobs = PluginBlobs::new(registry.extension_points(), ID);
    (registry, fake, blobs)
}

fn temp_file(bytes: &[u8]) -> tempfile::NamedTempFile {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.write_all(bytes).unwrap();
    file
}

#[tokio::test]
async fn an_object_put_is_got_until_deleted() {
    let (_registry, _fake, blobs) = stored();

    blobs.put("posters/a", b"poster".to_vec()).await.unwrap();
    blobs.put_file("logos/b", temp_file(b"logo")).await.unwrap();

    assert_eq!(
        blobs.get("posters/a").await.unwrap(),
        Some(b"poster".to_vec())
    );
    assert_eq!(blobs.get("logos/b").await.unwrap(), Some(b"logo".to_vec()));
    assert!(blobs.exists("posters/a").await.unwrap());

    blobs.delete("posters/a").await.unwrap();
    assert_eq!(blobs.get("posters/a").await.unwrap(), None);
    assert!(!blobs.exists("posters/a").await.unwrap());
}

#[tokio::test]
async fn listing_follows_every_page() {
    let (_registry, fake, blobs) = stored();
    for key in ["a/1", "a/2", "a/3", "a/4", "a/5", "b/1"] {
        blobs.put(key, Vec::new()).await.unwrap();
    }

    let listed = blobs.list("a/").await.unwrap();

    assert_eq!(listed, ["a/1", "a/2", "a/3", "a/4", "a/5"]);
    assert_eq!(fake.lists.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn listing_everything_is_refused_without_asking_the_plugin() {
    let (_registry, fake, blobs) = stored();

    let err = blobs.list("").await.unwrap_err();

    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    assert_eq!(fake.lists.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn an_object_over_the_cap_is_refused_before_the_plugin_is_asked() {
    let (_registry, fake, blobs) = stored();
    let over = vec![0; MAX_BYTES + 1];

    let put = blobs.put("big", over.clone()).await.unwrap_err();
    let put_file = blobs.put_file("big", temp_file(&over)).await.unwrap_err();

    assert_eq!(put.kind(), io::ErrorKind::FileTooLarge);
    assert_eq!(put_file.kind(), io::ErrorKind::FileTooLarge);
    assert_eq!(fake.puts.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn an_object_at_the_cap_is_stored() {
    let (_registry, fake, blobs) = stored();

    blobs
        .put_file("full", temp_file(&vec![7; MAX_BYTES]))
        .await
        .unwrap();

    assert_eq!(fake.objects()["full"].len(), MAX_BYTES);
}

#[tokio::test]
async fn the_plugins_error_is_the_io_errors_message() {
    let (_registry, _fake, blobs) = stored();

    let err = blobs.put(BROKEN_KEY, Vec::new()).await.unwrap_err();

    assert_eq!(err.to_string(), BROKEN);
}

#[tokio::test]
async fn a_disabled_plugin_fails_naming_it() {
    let (registry, _fake, blobs) = stored();
    assert!(registry.disable(&ID).await);

    let err = blobs.get("posters/a").await.unwrap_err();

    assert!(err.to_string().contains(ID.as_str()), "{err}");
}

#[tokio::test]
async fn a_plugin_serving_no_blob_store_fails_naming_it() {
    let (registry, _fake, _blobs) = stored();
    let other = PluginId::from_static("elsewhere");
    let blobs = PluginBlobs::new(registry.extension_points(), other.clone());

    let err = blobs.exists("posters/a").await.unwrap_err();

    assert!(err.to_string().contains(other.as_str()), "{err}");
}

#[tokio::test]
async fn blob_stores_lists_the_enabled_plugins_serving_the_point() {
    let (registry, _fake, _blobs) = stored();
    assert_eq!(registry.blob_stores(), [ID]);

    assert!(registry.disable(&ID).await);
    assert_eq!(registry.blob_stores(), []);
}
