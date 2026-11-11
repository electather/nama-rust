//! Posters and backdrops, cached on disk.
//!
//! A Match stores its title's images as paths the metadata Provider handed out, along with the
//! Plugin whose Provider that was. An image is downloaded through that Provider only when
//! something first asks for it, then kept in the [`ImageStore`] and served from there, across
//! restarts too. Callers should only ask for images some Match stores, or that a
//! search or lookup offered to clients, so clients can't use the cache to fetch arbitrary images.
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
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use nama_plugin_api::PluginId;

use crate::{
    contents::Contents,
    images::{ImageStore, file_name},
    metadata::{self, MetadataSource},
    plugins::ExtensionPoints,
};

const DIR: &str = "artwork";

/// How many offered images are remembered before [`Artwork::offer`] forgets them all.
const MAX_OFFERED: usize = 10_000;

/// Stored images, fetched from metadata Providers on first use.
#[derive(Debug, Clone)]
pub struct Artwork {
    store: ImageStore,
    metadata: MetadataSource,
    /// The `(plugin, path)` of each image a search or lookup handed out since the Server started.
    offered: Arc<Mutex<HashSet<(String, String)>>>,
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
        let key = [DIR, plugin.as_str(), path];
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
        plugin: &str,
        path: &str,
        request: Request,
    ) -> Result<Response, Error> {
        self.keep(&PluginId::from(plugin.to_owned()), path).await?;
        let mut response = self.store.serve(&[DIR, plugin, path], request).await?;
        let status = response.status();
        let headers = response.headers_mut();
        // An unnamed type stays a download: no browser guesses it is HTML.
        headers.insert(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        );
        if status.is_success() {
            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static(content_type(path)),
            );
        }
        if status.is_success() || status == StatusCode::NOT_MODIFIED {
            headers.insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("private, max-age=31536000, immutable"),
            );
        }
        Ok(response)
    }
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
    let allowed = if artwork.was_offered(&plugin, &path) {
        Ok(true)
    } else {
        contents.stores_image(&plugin, &path).await
    };
    match allowed {
        Ok(true) => {}
        Ok(false) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            tracing::error!("could not look up a stored image: {err}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    }
    match artwork.serve_image(&plugin, &path, request).await {
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
    use axum::{body::Body, extract::Request, http::header};

    use super::Artwork;
    use crate::{images::ImageStore, plugins::Registry};

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
            .serve_image("tmdb", "/logo.svg", request)
            .await
            .unwrap();

        let headers = response.headers();
        assert_eq!(headers[header::CONTENT_TYPE], "application/octet-stream");
        assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    }
}
