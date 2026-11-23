//! The images the Server keeps, in a `Blobs` store: by default `LocalDisk`, plain files in the
//! data directory (ADR 0037).
//!
//! Only [`ImageStore`] reads or writes them (`AGENTS.md`). A key names an image by a few parts,
//! each escaped to one file name, so no key reaches outside the store. Resized copies are kept
//! apart, in a `resized` folder, and can be deleted at any time (ADR 0039). An image's thumbhash is
//! kept in the database once it has been resized.

mod blobs;
mod local_disk;
mod plugin_logos;
mod resized;
mod serving;
mod switch;

use std::{
    io::{self, Write as _},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError, RwLock},
};

use axum::{body::Body, http::StatusCode};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use futures_util::StreamExt as _;
use image::{ImageFormat, ImageReader};
use serde::Deserialize;
use sha2::{Digest as _, Sha256};
use tempfile::NamedTempFile;
use tokio::io::AsyncWriteExt as _;

pub(crate) use blobs::{Blobs, MAX_BYTES};
use local_disk::LocalDisk;
pub(crate) use plugin_logos::{PluginLogos, router as plugin_logos_router};
use resized::{Copied, Format, ResizeThread};
pub(crate) use switch::{Persist, SwitchRefused, Switching};

use crate::{db::Database, server_settings::ImageSettings};

/// The widths an image is resized to, smallest first: those the web app's Poster grid and
/// Backdrops are shown at, at one to three device pixels per CSS pixel. Any other is refused.
pub const WIDTHS: [u32; 7] = [320, 480, 640, 960, 1280, 1920, 2560];

/// The Built-in store: files in `data_dir`, as [`ImageStore::new`] keeps them.
pub(crate) fn local_disk(data_dir: &Path) -> Arc<dyn Blobs> {
    Arc::new(LocalDisk::new(data_dir))
}

/// How many originals may be read whole at once (`ImageStore::reading`): a grid of Posters with
/// resizing off asks for many over HTTP/2 at once, which would otherwise hold that many times
/// [`MAX_BYTES`].
const WHOLE_READS: usize = 4;

/// Where the images Matches store are kept.
pub(crate) const KEPT_ARTWORK_DIR: &str = "artwork";

/// Where the images only a Movie or Show search or lookup offered are kept.
pub(crate) const OFFERED_ARTWORK_DIR: &str = "artwork-cache";

/// Where Custom Images are kept.
pub(crate) const CUSTOM_IMAGES_DIR: &str = "custom-images";

/// Where the copies of images beside the media are kept, one folder per Library.
pub(crate) const LOCAL_IMAGES_DIR: &str = "local-images";

/// Where the Plugins' Manifest logos are kept, one folder per Plugin.
pub(crate) const PLUGIN_LOGOS_DIR: &str = "plugin-logos";

/// Where the subtitles Users download are kept, one folder per Version (ADR 0067). Not
/// `subtitles`, where the local-disk store would find the subtitle cache's files.
pub(crate) const DOWNLOADED_SUBTITLES_DIR: &str = "downloaded-subtitles";

/// Where Preview frames are kept, one folder per Version (ADR 0076).
pub(crate) const PREVIEW_FRAMES_DIR: &str = "preview-frames";

/// The first part of every original's key: what a switch to another store copies. Offered
/// artwork is among them, since it is served from the store too, and so are downloaded subtitles
/// and Preview frames, which aren't images but are the Kernel's stored files all the same
/// (ADR 0067).
pub(crate) const ORIGINAL_DIRS: &[&str] = &[
    KEPT_ARTWORK_DIR,
    OFFERED_ARTWORK_DIR,
    CUSTOM_IMAGES_DIR,
    LOCAL_IMAGES_DIR,
    PLUGIN_LOGOS_DIR,
    DOWNLOADED_SUBTITLES_DIR,
    PREVIEW_FRAMES_DIR,
];

/// The folder in the data directory resized copies are kept in (`AGENTS.md`).
const RESIZED_DIR: &str = "resized";

/// The folder in the data directory uploads are written to as they arrive. No image key starts
/// with it, and it is on the data directory's filesystem, so `LocalDisk` can rename one into place.
const UPLOADS_DIR: &str = "uploads";

/// How many locks [`ImageStore::write`] spreads its keys over.
const WRITE_LOCKS: usize = 16;

/// The most bytes an uploaded image may have: 20 MiB.
pub const MAX_UPLOAD_BYTES: u64 = 20 * 1024 * 1024;

/// The most pixels an uploaded image may have: 12 megapixels (ADR 0039).
pub const MAX_PIXELS: u64 = 12_000_000;

/// The most bytes a Plugin's logo, an SVG, may have: 64 KiB.
pub const MAX_SVG_BYTES: usize = 64 * 1024;

/// The content type of an SVG, which [`ImageStore::serve`] sends with headers that keep its
/// scripts from running.
pub const SVG: &str = "image/svg+xml";

/// An uploaded image the store kept: see [`ImageStore::receive`].
#[derive(Debug)]
pub struct Received {
    /// Taken from its bytes, and the last part of its key.
    pub version: String,
    /// What it is, told from its bytes: `image/jpeg`, `image/png` or `image/webp`.
    pub content_type: &'static str,
}

/// Why an upload wasn't kept.
#[derive(Debug, thiserror::Error)]
pub enum UploadError {
    /// Over [`MAX_UPLOAD_BYTES`].
    #[error("the image is over 20 MiB")]
    TooBig,
    /// Its bytes aren't a JPEG, PNG or WebP, whatever its name or `Content-Type` says.
    #[error("the file isn't a JPEG, PNG or WebP image")]
    NotAnImage,
    /// Over [`MAX_PIXELS`].
    #[error("the image is over 12 megapixels")]
    TooManyPixels,
    /// The body stopped arriving, such as when the client went away.
    #[error("the upload broke off: {0}")]
    BrokeOff(axum::Error),
    /// The store couldn't be written.
    #[error("image store error: {0}")]
    Io(#[from] io::Error),
}

/// Stored images, under the data directory, and their thumbhashes in `db`. Clones share one
/// thread that resizes images (`AGENTS.md`).
#[derive(Debug, Clone)]
pub struct ImageStore {
    dir: PathBuf,
    db: Arc<Database>,
    resizer: ResizeThread,
    /// The store the originals are kept in, or while a switch runs one that mirrors writes into
    /// the next (`switch.rs`).
    store: Arc<RwLock<Arc<dyn Blobs>>>,
    /// The running or last failed switch to another store.
    switching: Arc<Mutex<Option<Switching>>>,
    /// Held from fetching an original to resize until its copy is made, so one original at a time
    /// is held in memory for it, and a request that waited behind the one making its copy finds
    /// it rather than fetching the original again.
    resizing: Arc<tokio::sync::Mutex<()>>,
    /// A permit is held by each original read whole, until it is sent or resized, so at most
    /// [`WHOLE_READS`] originals are in memory at once on a small device (ADR 0039, ADR 0052).
    // ponytail: a client that stops reading holds its permit with no timeout, so a few stalled
    // ones stall every whole read; time a send out if that bites.
    reading: Arc<tokio::sync::Semaphore>,
    /// Held from finding a key missing until it is stored: two writes of one key would otherwise
    /// both store it, and the second, replacing the first, changes its `ETag` (#793). Every write
    /// and delete takes the store under it, so a switch holding them all knows none is left
    /// writing only to the old store.
    writing: Arc<[tokio::sync::Mutex<()>; WRITE_LOCKS]>,
}

/// A resized copy asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resize {
    width: u32,
    cache_limit: u64,
}

/// What an image's URL asks for after the `?`.
#[derive(Debug, Deserialize)]
pub(crate) struct Asked {
    width: Option<u32>,
}

impl Asked {
    /// The copy asked for, or `None` for the stored image, which is all there is while `images`
    /// has resizing off (ADR 0039).
    ///
    /// # Errors
    ///
    /// 400, with a text body naming [`WIDTHS`], for a width not on them while resizing is on.
    pub(crate) fn resize(
        &self,
        images: &ImageSettings,
    ) -> Result<Option<Resize>, (StatusCode, String)> {
        let (Some(width), Some(cache_limit)) = (self.width, images.resizing()) else {
            return Ok(None);
        };
        if !WIDTHS.contains(&width) {
            let widths = WIDTHS.map(|allowed| allowed.to_string()).join(", ");
            return Err((
                StatusCode::BAD_REQUEST,
                format!("width must be one of {widths}"),
            ));
        }
        Ok(Some(Resize { width, cache_limit }))
    }
}

impl ImageStore {
    /// The images stored in `data_dir`, their thumbhashes kept in `db`.
    #[must_use]
    pub fn new(data_dir: &Path, db: Arc<Database>) -> Self {
        Self::over(data_dir, db, Arc::new(LocalDisk::new(data_dir)))
    }

    /// The images kept in `store`, their resized copies and uploads in `data_dir`.
    fn over(data_dir: &Path, db: Arc<Database>, store: Arc<dyn Blobs>) -> Self {
        Self {
            dir: data_dir.to_owned(),
            db,
            resizer: ResizeThread::start(),
            store: Arc::new(RwLock::new(store)),
            switching: Arc::default(),
            resizing: Arc::default(),
            reading: Arc::new(tokio::sync::Semaphore::new(WHOLE_READS)),
            writing: Arc::new(std::array::from_fn(|_| tokio::sync::Mutex::default())),
        }
    }

    /// The store the originals are kept in. Every call to it goes through here.
    fn blobs(&self) -> Arc<dyn Blobs> {
        Arc::clone(&self.store.read().unwrap_or_else(PoisonError::into_inner))
    }

    /// The lock a write of `key` holds (see `writing`).
    #[expect(
        clippy::indexing_slicing,
        reason = "a remainder of `WRITE_LOCKS` is below the array's length"
    )]
    fn write_lock(&self, key: &str) -> &tokio::sync::Mutex<()> {
        let hash = key.bytes().fold(0_usize, |hash, byte| {
            hash.wrapping_mul(31).wrapping_add(usize::from(byte))
        });
        &self.writing[hash % WRITE_LOCKS]
    }

    /// Stores `bytes` as the image at `key`, so no reader ever sees part of it, keeping an image
    /// already stored there (a key names its bytes: see the crate's `AGENTS.md`).
    ///
    /// # Errors
    ///
    /// If `bytes` is over 20 MiB, the most one stored object may have (`FileTooLarge`), or the
    /// file can't be written.
    pub async fn write(&self, key: &[&str], bytes: &[u8]) -> io::Result<()> {
        let key = row_key(key);
        if bytes.len() > MAX_BYTES {
            return Err(blobs::too_large(&key));
        }
        let _writing = self.write_lock(&key).lock().await;
        let blobs = self.blobs();
        if blobs.exists(&key).await? {
            return Ok(());
        }
        blobs.put(&key, bytes.to_vec()).await
    }

    /// Keeps `body`, a JPEG, PNG or WebP of at most [`MAX_UPLOAD_BYTES`] and [`MAX_PIXELS`], in
    /// `dir` under a version taken from its bytes, writing it as it arrives rather than holding it
    /// whole. A refused or broken-off upload leaves nothing behind.
    ///
    /// # Errors
    ///
    /// If the image is refused, the body breaks off, or the store can't be written.
    pub async fn receive(&self, dir: &[&str], body: Body) -> Result<Received, UploadError> {
        let uploads = self.dir.join(UPLOADS_DIR);
        let partial = tokio::task::spawn_blocking(move || {
            std::fs::create_dir_all(&uploads)?;
            // Its name starts with `.`, which `file_name` never gives, so no key names it.
            NamedTempFile::new_in(uploads)
        })
        .await
        .map_err(io::Error::from)??;
        let version = write_hashed(&partial, body).await?;
        let (partial, content_type) = tokio::task::spawn_blocking(move || {
            let content_type = content_type_of(partial.path())?;
            // Synced before the rename, or a crash can leave a whole name over an empty file.
            partial.as_file().sync_all()?;
            Ok::<_, UploadError>((partial, content_type))
        })
        .await
        .map_err(io::Error::from)??;
        let key = row_key(&[dir, &[version.as_str()]].concat());
        let _writing = self.write_lock(&key).lock().await;
        let blobs = self.blobs();
        if !blobs.exists(&key).await? {
            blobs.put_file(&key, partial).await?;
        }
        Ok(Received {
            version,
            content_type,
        })
    }

    /// Deletes the uploads a crashed run left, going on past one it can't delete. Only the caller
    /// knows no upload is in flight: startup, before serving.
    ///
    /// # Errors
    ///
    /// If the uploads folder can't be read.
    pub(crate) async fn sweep_uploads(&self) -> io::Result<()> {
        let mut entries = match tokio::fs::read_dir(self.dir.join(UPLOADS_DIR)).await {
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
            entries => entries?,
        };
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let deleted = match entry.file_type().await {
                Ok(kind) if kind.is_dir() => tokio::fs::remove_dir_all(&path).await,
                _ => tokio::fs::remove_file(&path).await,
            };
            if let Err(err) = deleted {
                tracing::warn!("could not delete {}: {err}", path.display());
            }
        }
        Ok(())
    }

    /// Whether an image is stored at `key`.
    ///
    /// # Errors
    ///
    /// If the store can't be read.
    pub async fn contains(&self, key: &[&str]) -> io::Result<bool> {
        self.blobs().exists(&row_key(key)).await
    }

    async fn keep_thumbhash(&self, key: &[&str], thumbhash: Vec<u8>) -> sqlx::Result<()> {
        let key = row_key(key);
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!(
                        "INSERT INTO image_thumbhashes (key, thumbhash) VALUES (?, ?)
                         ON CONFLICT (key) DO UPDATE SET thumbhash = excluded.thumbhash",
                        key,
                        thumbhash,
                    )
                    .execute(&mut **tx)
                    .await
                    .map(drop)
                })
            })
            .await
    }

    /// The key and thumbhash of every image stored under `dir` that has been resized, each key
    /// starting with `dir`.
    ///
    /// # Errors
    ///
    /// If the database can't be read.
    // ponytail: reads every thumbhash under `dir` for each response that shows images; query by
    // key if a store of tens of thousands of images makes that slow.
    pub(crate) async fn thumbhashes(&self, dir: &str) -> sqlx::Result<Vec<(Vec<String>, Vec<u8>)>> {
        let under = format!("{}/", file_name(dir));
        let rows = sqlx::query!(
            "SELECT key, thumbhash FROM image_thumbhashes WHERE instr(key, ?) = 1",
            under
        )
        .fetch_all(self.db.reader())
        .await?;
        Ok(rows
            .into_iter()
            .filter_map(|row| {
                let key = row.key.split('/').map(unescape).collect::<Option<_>>()?;
                Some((key, row.thumbhash))
            })
            .collect())
    }

    /// Deletes the image at `key`, if one is stored, with its resized copies and thumbhash.
    ///
    /// # Errors
    ///
    /// If the file or its copies are there but can't be deleted, or the thumbhash can't be.
    pub async fn delete(&self, key: &[&str]) -> io::Result<()> {
        self.forget_thumbhashes(key).await?;
        let row = row_key(key);
        {
            let _writing = self.write_lock(&row).lock().await;
            self.blobs().delete(&row).await?;
        }
        self.delete_copies(key).await
    }

    /// Deletes every image under `key`, with their resized copies and thumbhashes.
    ///
    /// # Errors
    ///
    /// If an image or the copies are there but can't be deleted, or the thumbhashes can't be.
    pub async fn delete_dir(&self, key: &[&str]) -> io::Result<()> {
        self.forget_thumbhashes(key).await?;
        for stored in self.blobs().list(&format!("{}/", row_key(key))).await? {
            let _writing = self.write_lock(&stored).lock().await;
            self.blobs().delete(&stored).await?;
        }
        self.delete_copies(key).await
    }

    /// Stores `bytes` at `key` as [`Self::write`] does, and returns the local file holding them,
    /// for a reader that needs a path, such as ffmpeg: the store's own file, or a copy where the
    /// local-disk store would keep it.
    ///
    /// # Errors
    ///
    /// As [`Self::write`], or if the copy can't be written.
    // ponytail: a Plugin store's object is kept on local disk too, and isn't made again if that
    // copy is lost; fetch it from the store when it's read if either matters.
    pub(crate) async fn write_on_disk(&self, key: &[&str], bytes: &[u8]) -> io::Result<PathBuf> {
        self.write(key, bytes).await?;
        let path = under(self.dir.clone(), key);
        if self.blobs().local_path(&row_key(key)).as_ref() != Some(&path) {
            write_atomically(&path, bytes).await?;
        }
        Ok(path)
    }

    /// Deletes every object under `key` as [`Self::delete_dir`] does, and the local copies
    /// [`Self::write_on_disk`] kept of them.
    ///
    /// # Errors
    ///
    /// As [`Self::delete_dir`], or if the copies are there but can't be deleted.
    pub(crate) async fn delete_dir_on_disk(&self, key: &[&str]) -> io::Result<()> {
        self.delete_dir(key).await?;
        unless_gone(tokio::fs::remove_dir_all(under(self.dir.clone(), key)).await)
    }

    /// Deletes the thumbhash of the image at `key`, or of every image under it. First, before the
    /// image: one left behind would outlive its image for good.
    async fn forget_thumbhashes(&self, key: &[&str]) -> io::Result<()> {
        let (row, below) = (row_key(key), format!("{}/", row_key(key)));
        let forgotten = self
            .db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!(
                        "DELETE FROM image_thumbhashes WHERE key = ? OR instr(key, ?) = 1",
                        row,
                        below,
                    )
                    .execute(&mut **tx)
                    .await
                    .map(drop)
                })
            })
            .await;
        forgotten.map_err(io::Error::other)
    }

    /// Deletes the resized copies of the image at `key`, or of every image under it. Left, they
    /// would be found by an image stored at `key` again, which then never gets a thumbhash.
    async fn delete_copies(&self, key: &[&str]) -> io::Result<()> {
        unless_gone(tokio::fs::remove_dir_all(self.copies_path(key)).await)
    }

    /// The key of every image stored under `dir`, each starting with `dir`.
    ///
    /// # Errors
    ///
    /// If the store can't be read.
    pub async fn list(&self, dir: &str) -> io::Result<Vec<Vec<String>>> {
        let stored = self.blobs().list(&format!("{}/", file_name(dir))).await?;
        Ok(stored
            .iter()
            // Skips a name `file_name` doesn't give, such as one put there by hand.
            .filter_map(|key| key.split('/').map(unescape).collect())
            .collect())
    }

    /// Where the copy `width` pixels wide in `format` of the image at `key` is kept.
    fn copy_path(&self, key: &[&str], width: u32, format: Format) -> PathBuf {
        self.copies_path(key).join(format.file_name(width))
    }

    /// The folder the resized copies of the image at `key`, or of every image under it, are kept
    /// in.
    fn copies_path(&self, key: &[&str]) -> PathBuf {
        under(self.dir.join(RESIZED_DIR), key)
    }
}

/// `deleted`, with what was already gone counted as deleted.
fn unless_gone(deleted: io::Result<()>) -> io::Result<()> {
    match deleted {
        Err(err) if err.kind() != io::ErrorKind::NotFound => Err(err),
        _ => Ok(()),
    }
}

/// `key` as the database names it: each part escaped as [`file_name`] does, joined by `/`.
fn row_key(key: &[&str]) -> String {
    let parts: Vec<String> = key.iter().map(|part| file_name(part)).collect();
    parts.join("/")
}

/// `key` as a path in `folder`, each part one escaped file name.
fn under(folder: PathBuf, key: &[&str]) -> PathBuf {
    key.iter()
        .fold(folder, |path, part| path.join(file_name(part)))
}

/// Writes `body` to `partial` as it arrives, refusing it past [`MAX_UPLOAD_BYTES`], and returns a
/// version taken from its bytes.
async fn write_hashed(partial: &NamedTempFile, body: Body) -> Result<String, UploadError> {
    let mut file = tokio::fs::File::from_std(partial.as_file().try_clone()?);
    let mut hash = Sha256::new();
    let mut size = 0;
    let mut chunks = body.into_data_stream();
    while let Some(chunk) = chunks.next().await {
        let chunk = chunk.map_err(UploadError::BrokeOff)?;
        size += chunk.len() as u64;
        if size > MAX_UPLOAD_BYTES {
            return Err(UploadError::TooBig);
        }
        hash.update(&chunk);
        file.write_all(&chunk).await?;
    }
    file.flush().await?;
    Ok(version_from(&hash.finalize()))
}

/// A version taken from `bytes`, as [`ImageStore::receive`] takes an upload's.
pub(crate) fn version_of(bytes: &[u8]) -> String {
    version_from(&Sha256::digest(bytes))
}

/// The version of the bytes whose SHA-256 is `hash`: its start, in URL-safe base64.
fn version_from(hash: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(hash.get(..12).unwrap_or_default())
}

/// The content type of the image at `path`, a JPEG, PNG or WebP of at most [`MAX_PIXELS`], told
/// from its bytes. Only its header is read, never its pixels.
fn content_type_of(path: &Path) -> Result<&'static str, UploadError> {
    let reader = ImageReader::open(path)?.with_guessed_format()?;
    let content_type = match reader.format() {
        Some(ImageFormat::Jpeg) => "image/jpeg",
        Some(ImageFormat::Png) => "image/png",
        Some(ImageFormat::WebP) => "image/webp",
        _ => return Err(UploadError::NotAnImage),
    };
    let (width, height) = reader
        .into_dimensions()
        .map_err(|_unreadable| UploadError::NotAnImage)?;
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(UploadError::TooManyPixels);
    }
    Ok(content_type)
}

/// `svg` as `svg-hush` leaves it: only the elements and attributes on its allowlist, which run
/// no script and load nothing from elsewhere.
///
/// # Errors
///
/// If `svg` isn't well-formed XML, or holds no element in the SVG namespace that is kept.
pub fn clean_svg(svg: &[u8]) -> Result<Vec<u8>, NotSvg> {
    let mut cleaned = Vec::new();
    svg_hush::Filter::new()
        .filter(svg, &mut cleaned)
        .map_err(|err| {
            // `FError` says only which side failed; its source says what.
            let why = std::error::Error::source(&err)
                .map_or_else(|| err.to_string(), ToString::to_string);
            NotSvg(why)
        })?;
    Ok(cleaned)
}

/// Why [`clean_svg`] refused a document, in the XML parser's words.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct NotSvg(String);

/// Writes `bytes` to `path` so no reader ever sees a partial file, even if two writes of the
/// same file race or the machine stops midway.
pub(crate) async fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let (path, bytes) = (path.to_owned(), bytes.to_vec());
    tokio::task::spawn_blocking(move || write_now(&path, &bytes)).await?
}

/// [`write_atomically`], blocking the thread it runs on.
fn write_now(path: &Path, bytes: &[u8]) -> io::Result<()> {
    synced_beside(path, bytes)?.persist(path)?;
    Ok(())
}

/// A temporary file of `bytes` in `path`'s folder, synced to disk, to rename to `path`.
fn synced_beside(path: &Path, bytes: &[u8]) -> io::Result<NamedTempFile> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;
    // Its name starts with `.`, which `file_name` never gives, so no key names it.
    let mut partial = NamedTempFile::new_in(dir)?;
    partial.write_all(bytes)?;
    // Synced before the rename, or a crash can leave a whole name over an empty file.
    partial.as_file().sync_all()?;
    Ok(partial)
}

/// `text` as one safe file name: letters, digits, `-`, `_` and (except first) `.` are kept, and
/// every other byte is written as `%XX`. Distinct texts give distinct names, and no name is `.`
/// or `..` or holds a separator.
pub(crate) fn file_name(text: &str) -> String {
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

/// The text `name` is the [`file_name`] of, if it is one.
fn unescape(name: &str) -> Option<String> {
    let text = percent_decode(name)?;
    (file_name(&text) == name).then_some(text)
}

/// `text` with each `%XX` made the byte it stands for, `None` if one is cut short, isn't hex or
/// the result isn't UTF-8. It accepts any encoding of a text; a caller wanting one checks it.
pub(crate) fn percent_decode(text: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(text.len());
    let mut rest = text.as_bytes();
    while let Some((&byte, tail)) = rest.split_first() {
        rest = tail;
        if byte == b'%' {
            let hex = std::str::from_utf8(rest.get(..2)?).ok()?;
            bytes.push(u8::from_str_radix(hex, 16).ok()?);
            rest = rest.get(2..)?;
        } else {
            bytes.push(byte);
        }
    }
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
#[path = "images/images_tests.rs"]
mod tests;
