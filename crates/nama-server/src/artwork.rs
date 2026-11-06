//! Posters and backdrops, cached on disk.
//!
//! A Match stores its title's images as paths the metadata Provider handed out, along with the
//! Plugin whose Provider that was. An image is downloaded through that Provider only when
//! something first asks for it, then kept under `artwork/` in the data directory and served from
//! there, across restarts too. Callers should only ask for images some Match stores, so clients
//! can't use the cache to fetch arbitrary images.

use std::{
    io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use nama_plugin_api::{
    PluginId,
    metadata::{MetadataError, MetadataProvider},
};

use crate::plugins::Registry;

/// The directory under the data directory that holds the cache.
const DIR: &str = "artwork";

/// Images on disk, fetched from metadata Providers on first use.
#[derive(Debug, Clone)]
pub struct Artwork {
    dir: PathBuf,
    plugins: Registry,
}

/// An image in the cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedImage {
    /// The file holding it.
    pub path: PathBuf,
    /// Its media type, such as `image/jpeg`, by its extension.
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
            plugins,
        }
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
        let fetched = self
            .plugins
            .call_provider_of::<dyn MetadataProvider, _, _>(plugin, |metadata| {
                let path = path.to_owned();
                async move { metadata.image(&path).await }
            })
            .await;
        let image = match fetched {
            Some(Ok(image)) => image,
            Some(Err(MetadataError::NotFound | MetadataError::UnsupportedId(_))) => {
                return Err(Error::NotFound);
            }
            Some(Err(MetadataError::Failed(error))) => {
                return Err(Error::Unavailable(error.to_string()));
            }
            None => {
                return Err(Error::Unavailable(format!(
                    "{plugin} isn't enabled or doesn't provide metadata"
                )));
            }
        };
        write_atomically(&cached.path, &image.bytes).await?;
        Ok(cached)
    }
}

/// Writes `bytes` to `path` so no reader ever sees a partial file, even if two downloads of the
/// same image race.
async fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
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

/// The media type of an image, by the extension of its path.
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
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    }
}
