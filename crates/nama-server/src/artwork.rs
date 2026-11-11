//! Posters and backdrops, cached on disk.
//!
//! A Match stores its title's images as paths the metadata Provider handed out, along with the
//! Plugin whose Provider that was. An image is downloaded through that Provider only when
//! something first asks for it, then kept under `artwork/` in the data directory and served from
//! there, across restarts too. Callers should only ask for images some Match stores, or that a
//! search or lookup offered to clients, so clients can't use the cache to fetch arbitrary images.
//!
//! Clients fetch them over plain HTTP at `/media/artwork/<plugin>/<path>` (ADR 0007: media bytes
//! are not RPC), which is served to anyone with a session.

use std::{
    collections::HashSet,
    io,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicU64, Ordering},
    },
};

use axum::{
    Router,
    extract::{Path as UrlPath, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use nama_plugin_api::PluginId;

use crate::{
    contents::Contents,
    metadata::{self, MetadataSource},
    plugins::Registry,
};

const DIR: &str = "artwork";

/// How many offered images are remembered before [`Artwork::offer`] forgets them all.
const MAX_OFFERED: usize = 10_000;

/// Images on disk, fetched from metadata Providers on first use.
#[derive(Debug, Clone)]
pub struct Artwork {
    dir: PathBuf,
    metadata: MetadataSource,
    /// The `(plugin, path)` of each image a search or lookup handed out since the Server started.
    offered: Arc<Mutex<HashSet<(String, String)>>>,
}

/// An image in the cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedImage {
    /// The file holding it.
    pub path: PathBuf,
    /// Its media type, such as `image/jpeg`, by its extension. Only raster image types are
    /// named; anything else is `application/octet-stream`.
    pub content_type: &'static str,
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
    /// The cache couldn't be read or written.
    #[error("artwork cache error: {0}")]
    Io(#[from] io::Error),
}

impl Artwork {
    /// The cache in `data_dir`, fetching through the metadata Providers enabled in `plugins`.
    #[must_use]
    pub fn new(data_dir: &Path, plugins: Registry) -> Self {
        Self {
            dir: data_dir.join(DIR),
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

    /// The image at `path` from the Plugin `plugin`'s metadata Provider, downloading it first if
    /// it isn't cached yet.
    ///
    /// # Errors
    ///
    /// If the Provider has no such image, can't be reached, or the cache can't be written.
    pub async fn get(&self, plugin: &PluginId, path: &str) -> Result<CachedImage, Error> {
        let cached = CachedImage {
            path: self
                .dir
                .join(file_name(plugin.as_str()))
                .join(file_name(path)),
            content_type: content_type(path),
        };
        if tokio::fs::try_exists(&cached.path).await? {
            return Ok(cached);
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
        write_atomically(&cached.path, &image.bytes).await?;
        Ok(cached)
    }

    async fn read(&self, plugin: &PluginId, path: &str) -> Result<(&'static str, Vec<u8>), Error> {
        let image = self.get(plugin, path).await?;
        let bytes = tokio::fs::read(&image.path).await?;
        Ok((image.content_type, bytes))
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
    match artwork.read(&PluginId::from(plugin), &path).await {
        Ok((content_type, bytes)) => (
            [
                (header::CONTENT_TYPE, content_type),
                // An unnamed type stays a download: no browser guesses it is HTML.
                (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            ],
            bytes,
        )
            .into_response(),
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

/// Writes `bytes` to `path` so no reader ever sees a partial file, even if two downloads of the
/// same image race.
pub(crate) async fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir).await?;
    }
    let mut partial = path.as_os_str().to_owned();
    partial.push(format!(
        ".{}-{}.part",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let partial = PathBuf::from(partial);
    tokio::fs::write(&partial, bytes).await?;
    if let Err(err) = tokio::fs::rename(&partial, path).await {
        let _: io::Result<()> = tokio::fs::remove_file(&partial).await;
        return Err(err);
    }
    Ok(())
}

/// `text` as one safe file name: letters, digits, `-`, `_` and (except first) `.` are kept, and
/// every other byte is written as `%XX`. Distinct texts give distinct names, and no name is `.`
/// or `..` or holds a separator.
fn file_name(text: &str) -> String {
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
