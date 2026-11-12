//! Posters and backdrops, downloaded when a Match is made and kept in the image store.
//!
//! A Match stores its title's images as paths the metadata Provider handed out, along with the
//! Plugin whose Provider that was. Once a Match is stored, its images are downloaded through that
//! Provider in the background, and kept in the [`ImageStore`] while some Match stores them. One
//! asked for before it arrived is downloaded on the spot. Images a search or lookup offered and no
//! Match stores go to a cache of their own, which can be deleted at any time (ADR 0037). Callers
//! should only ask for images some Match stores, or that a search or lookup offered to clients,
//! so clients can't use the store to fetch arbitrary images.
//!
//! Clients fetch them over plain HTTP at `/media/artwork/<plugin>/<path>` (ADR 0007: media bytes
//! are not RPC), which is served to anyone with a session. A Provider's path names bytes that
//! never change, so a browser caches what it fetched there for good.

use std::{
    collections::HashSet,
    io,
    path::Path,
    sync::{Arc, Mutex, PoisonError},
};

use axum::{
    Router,
    extract::{Path as UrlPath, Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use nama_plugin_api::PluginId;
use tokio::sync::{OwnedSemaphorePermit, RwLock, Semaphore};

use crate::{
    contents::Contents,
    images::{ImageStore, file_name},
    metadata::{self, MetadataSource},
    plugins::ExtensionPoints,
};

/// Where the images Matches store are kept.
const KEPT_DIR: &str = "artwork";

/// Where the images only a search or lookup offered are kept, until someone deletes them.
// ponytail: nothing empties it; bound it like the resized copies' cache if it grows too big.
const OFFERED_DIR: &str = "artwork-cache";

/// How many images are downloaded in the background at once. Each crosses from the Provider as
/// one whole value, so this also bounds the memory downloads take on a 1 GiB machine.
const DOWNLOADS: usize = 2;

/// How many offered images are remembered before [`Artwork::offer`] forgets them all.
const MAX_OFFERED: usize = 10_000;

/// Stored images, fetched from metadata Providers on first use.
#[derive(Debug, Clone)]
pub struct Artwork {
    store: ImageStore,
    metadata: MetadataSource,
    /// The `(plugin, path)` of each image a search or lookup handed out since the Server started.
    offered: Arc<Mutex<HashSet<(String, String)>>>,
    /// Taken by each background download, across every clone.
    downloads: Arc<Semaphore>,
    /// Written while unused images are deleted, and read while one is stored or served, so a
    /// Match stored meanwhile never finds its image there and then has it deleted.
    sweeping: Arc<RwLock<()>>,
}

/// Why an image couldn't be had.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The Provider has no such image.
    #[error("no such image")]
    NotFound,
    /// The Plugin isn't enabled or doesn't provide metadata, or its Provider failed. Worth
    /// trying again later.
    #[error("the image is unavailable: {0}")]
    Unavailable(String),
    /// The store couldn't be read or written.
    #[error("artwork store error: {0}")]
    Io(#[from] io::Error),
}

impl Artwork {
    /// The images stored in `data_dir`, fetching through the metadata Providers enabled in
    /// `plugins`.
    #[must_use]
    pub fn new(data_dir: &Path, plugins: ExtensionPoints) -> Self {
        Self {
            store: ImageStore::new(data_dir),
            metadata: MetadataSource::new(plugins),
            offered: Arc::default(),
            downloads: Arc::new(Semaphore::new(DOWNLOADS)),
            sweeping: Arc::default(),
        }
    }

    /// Where clients fetch the image at `path` from the Plugin `plugin`'s metadata Provider, which
    /// [`router`] serves from now on even though no Match stores it.
    pub(crate) fn offer(&self, plugin: &PluginId, path: &str) -> String {
        let plugin = plugin.as_str();
        let mut offered = self.offered.lock().unwrap_or_else(PoisonError::into_inner);
        // ponytail: forgets every offer at the cap, so a poster shown long ago may 404 until it is
        // searched for again. Evict the oldest instead if that bites.
        if offered.len() >= MAX_OFFERED {
            offered.clear();
        }
        offered.insert((plugin.to_owned(), path.to_owned()));
        url(plugin, path)
    }

    fn was_offered(&self, plugin: &str, path: &str) -> bool {
        let offered = self.offered.lock().unwrap_or_else(PoisonError::into_inner);
        offered.contains(&(plugin.to_owned(), path.to_owned()))
    }

    /// Stores the image at `path` from the Plugin `plugin`'s metadata Provider, downloading it
    /// unless it is stored already.
    ///
    /// # Errors
    ///
    /// If the Provider has no such image, can't be reached, or the store can't be written.
    pub async fn keep(&self, plugin: &PluginId, path: &str) -> Result<(), Error> {
        self.store_in(KEPT_DIR, plugin, path).await
    }

    /// Downloads, in the background, each image a Match in the Library `library` stores that
    /// isn't kept yet, logging those that fail, which the Library's next scan tries again.
    pub(crate) fn download_missing(&self, contents: Contents, library: i64) {
        let artwork = self.clone();
        tokio::spawn(async move {
            match contents.images(Some(library)).await {
                Ok(images) => artwork.download(images).await,
                Err(err) => {
                    tracing::error!(library, "could not list the images to download: {err}");
                }
            }
        });
    }

    /// Downloads each of `images`, a Plugin and a path, unless it is kept already, at most
    /// [`DOWNLOADS`] at a time across the Server. Returns once the last has started.
    async fn download(&self, images: Vec<(String, String)>) {
        for (plugin, path) in images {
            // The semaphore is never closed.
            let Ok(permit) = Arc::clone(&self.downloads).acquire_owned().await else {
                return;
            };
            tokio::spawn(self.clone().download_one(plugin, path, permit));
        }
    }

    /// Keeps one image for [`Self::download`], logging a failure, then lets the next start.
    async fn download_one(self, plugin: String, path: String, _permit: OwnedSemaphorePermit) {
        let _sweeping = self.sweeping.read().await;
        if let Err(err) = self.keep(&PluginId::from(plugin), &path).await {
            tracing::warn!(path, "could not download an image: {err}");
        }
    }

    /// Deletes each kept image no Match in `contents` stores any more, logging a failure.
    pub(crate) async fn drop_unused(&self, contents: &Contents) {
        if let Err(err) = self.try_drop_unused(contents).await {
            tracing::error!("could not delete the images no Match stores: {err}");
        }
    }

    async fn try_drop_unused(
        &self,
        contents: &Contents,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let _sweeping = self.sweeping.write().await;
        let kept = self.store.list(KEPT_DIR).await?;
        let used: HashSet<(String, String)> = contents.images(None).await?.into_iter().collect();
        for key in kept {
            let [_, plugin, path] = key.as_slice() else {
                continue;
            };
            if !used.contains(&(plugin.clone(), path.clone())) {
                self.store.delete(&[KEPT_DIR, plugin, path]).await?;
            }
        }
        Ok(())
    }

    /// Stores the image at `path` from `plugin`'s Provider in `dir`, downloading it unless it is
    /// stored there already.
    async fn store_in(&self, dir: &str, plugin: &PluginId, path: &str) -> Result<(), Error> {
        let key = key(dir, plugin.as_str(), path);
        if self.store.contains(&key).await? {
            return Ok(());
        }
        let image = self
            .metadata
            .image(plugin, path)
            .await
            .map_err(|err| match err {
                metadata::Error::NotFound => Error::NotFound,
                metadata::Error::Unavailable => Error::Unavailable(format!(
                    "{plugin} isn't enabled, doesn't provide metadata, or failed"
                )),
            })?;
        Ok(self.store.write(&key, &image.bytes).await?)
    }

    /// The image at `path` from `plugin`'s Provider as the answer to `request`, stored first if it
    /// isn't yet.
    async fn serve_image(
        &self,
        dir: &str,
        plugin: &str,
        path: &str,
        request: Request,
    ) -> Result<Response, Error> {
        let _sweeping = self.sweeping.read().await;
        self.store_in(dir, &PluginId::from(plugin.to_owned()), path)
            .await?;
        Ok(self
            .store
            .serve(&key(dir, plugin, path), content_type(path), request)
            .await?)
    }
}

/// Where the store keeps `plugin`'s image at `path` in `dir`. Under [`KEPT_DIR`] it is where images
/// were kept before the store existed, so changing it downloads every image again.
fn key<'a>(dir: &'a str, plugin: &'a str, path: &'a str) -> [&'a str; 3] {
    [dir, plugin, path]
}

/// Where clients fetch the image: `/media/artwork/<plugin>/<path>`, each part one URL path segment
/// (escaped as [`file_name`] does, which decodes back to what it was).
pub(crate) fn url(plugin: &str, path: &str) -> String {
    format!("/media/artwork/{}/{}", file_name(plugin), file_name(path))
}

/// Serves `GET` [`url`]s, 404 unless `artwork` offered the image or `contents` has a Match that
/// stores it. Whether the caller has a session is up to the layers the router is put behind.
pub(crate) fn router(artwork: Artwork, contents: Contents) -> Router {
    Router::new()
        .route("/media/artwork/{plugin}/{path}", get(serve))
        .with_state((artwork, contents))
}

async fn serve(
    State((artwork, contents)): State<(Artwork, Contents)>,
    UrlPath((plugin, path)): UrlPath<(String, String)>,
    request: Request,
) -> Response {
    let dir = match contents.stores_image(&plugin, &path).await {
        Ok(true) => KEPT_DIR,
        Ok(false) if artwork.was_offered(&plugin, &path) => OFFERED_DIR,
        Ok(false) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            tracing::error!("could not look up a stored image: {err}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    match artwork.serve_image(dir, &plugin, &path, request).await {
        Ok(response) => response,
        Err(Error::NotFound) => StatusCode::NOT_FOUND.into_response(),
        Err(err @ Error::Unavailable(_)) => {
            tracing::warn!("could not serve {path}: {err}");
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
        Err(err @ Error::Io(_)) => {
            tracing::error!("could not serve {path}: {err}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

fn content_type(path: &str) -> &'static str {
    let extension = Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        // Anything else, SVG included, isn't served as an image: an SVG can carry script.
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use axum::{Router, body::Body, extract::Request, http::header};
    use nama_plugin_api::settings::{SettingValue, Settings};
    use nama_plugin_tmdb::{Endpoints, Tmdb, testing::API_KEY};

    use super::Artwork;
    use crate::{images::ImageStore, plugins::Registry};

    /// How many image requests are running, and the most that ever were.
    #[derive(Default)]
    struct Running {
        now: AtomicUsize,
        most: AtomicUsize,
    }

    /// Answers an image request after a while, counted in `running` meanwhile.
    async fn slow_image(running: Arc<Running>) -> &'static [u8] {
        let now = running.now.fetch_add(1, Ordering::SeqCst) + 1;
        running.most.fetch_max(now, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(50)).await;
        running.now.fetch_sub(1, Ordering::SeqCst);
        b"\xff\xd8"
    }

    #[tokio::test]
    async fn no_more_than_two_images_download_at_once() {
        let running = Arc::new(Running::default());
        let counted = Arc::clone(&running);
        let slow_images = Router::new().fallback(move || slow_image(Arc::clone(&counted)));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(axum::serve(listener, slow_images).into_future());
        let plugins = Registry::new();
        let tmdb = Tmdb::with_endpoints(Endpoints {
            api: url.clone(),
            images: url,
        });
        let key = BTreeMap::from([("api_key".to_owned(), SettingValue::Text(API_KEY.to_owned()))]);
        (plugins.enable_with_settings(Arc::new(tmdb), Settings::new(key))).unwrap();
        let data = tempfile::tempdir().unwrap();
        let artwork = Artwork::new(data.path(), plugins.extension_points());
        let images = (0..5).map(|n| ("tmdb".to_owned(), format!("/{n}.jpg")));

        artwork.download(images.collect()).await;
        let _all_finished = artwork.downloads.acquire_many(2).await.unwrap();

        assert_eq!(running.most.load(Ordering::SeqCst), 2);
        let kept = ImageStore::new(data.path()).list("artwork").await.unwrap();
        assert_eq!(kept.len(), 5);
    }

    #[tokio::test]
    async fn an_svg_is_never_served_as_an_image() {
        let data = tempfile::tempdir().unwrap();
        let key = ["artwork", "tmdb", "/logo.svg"];
        ImageStore::new(data.path())
            .write(&key, b"<svg><script/></svg>")
            .await
            .unwrap();
        let artwork = Artwork::new(data.path(), Registry::new().extension_points());

        let request = Request::get("/").body(Body::empty()).unwrap();
        let response = artwork
            .serve_image("artwork", "tmdb", "/logo.svg", request)
            .await
            .unwrap();

        let headers = response.headers();
        assert_eq!(headers[header::CONTENT_TYPE], "application/octet-stream");
        assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    }
}
