//! The images the Server keeps, as plain files in the data directory (ADR 0037).
//!
//! Only [`ImageStore`] reads or writes them (`AGENTS.md`). A key names an image by a few parts,
//! each escaped to one file name, so no key reaches outside the store.

use std::{
    io::{self, Write as _},
    path::{Path, PathBuf},
};

use axum::{
    body::Body,
    extract::Request,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use futures_util::StreamExt as _;
use image::{ImageFormat, ImageReader};
use sha2::{Digest as _, Sha256};
use tempfile::NamedTempFile;
use tokio::io::AsyncWriteExt as _;
use tower_http::services::ServeFile;

/// The most bytes an uploaded image may have: 20 MiB.
pub const MAX_UPLOAD_BYTES: u64 = 20 * 1024 * 1024;

/// The most pixels an uploaded image may have: 12 megapixels (ADR 0039).
pub const MAX_PIXELS: u64 = 12_000_000;

/// The most bytes an SVG may have, a Plugin's logo being the one image that may be one: 64 KiB.
pub const MAX_SVG_BYTES: usize = 64 * 1024;

/// The content type of an SVG, which [`ImageStore::serve`] sends with headers that keep its
/// scripts from running.
pub const SVG: &str = "image/svg+xml";

/// An uploaded image the store kept: see [`ImageStore::receive`].
#[derive(Debug)]
pub struct Received {
    /// Taken from its bytes, and the last part of its key.
    pub version: String,
    /// What it is, told from its bytes: `image/jpeg`, `image/png`, `image/webp`, or [`SVG`] where
    /// one is accepted.
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
    /// Not a JPEG, PNG or WebP, where an SVG is accepted, and over [`MAX_SVG_BYTES`] for one.
    #[error("the image isn't a JPEG, PNG or WebP, and is over 64 KiB for an SVG")]
    SvgTooBig,
    /// The body stopped arriving, such as when the client went away.
    #[error("the upload broke off: {0}")]
    BrokeOff(axum::Error),
    /// The store couldn't be written.
    #[error("image store error: {0}")]
    Io(#[from] io::Error),
}

/// Stored images, under the data directory.
#[derive(Debug, Clone)]
pub struct ImageStore {
    dir: PathBuf,
}

impl ImageStore {
    /// The images stored in `data_dir`.
    #[must_use]
    pub fn new(data_dir: &Path) -> Self {
        Self {
            dir: data_dir.to_owned(),
        }
    }

    /// Stores `bytes` as the image at `key`, replacing any there, so no reader ever sees part of
    /// it.
    ///
    /// # Errors
    ///
    /// If the file can't be written.
    pub async fn write(&self, key: &[&str], bytes: &[u8]) -> io::Result<()> {
        write_atomically(&self.path(key), bytes).await
    }

    /// Keeps `body`, a JPEG, PNG or WebP of at most [`MAX_UPLOAD_BYTES`] and [`MAX_PIXELS`], or
    /// if `svg_accepted` an SVG of at most [`MAX_SVG_BYTES`], kept as [`clean_svg`] leaves it, in
    /// `dir` under a version taken from its bytes, writing it as it arrives rather than holding it
    /// whole. A refused or broken-off upload leaves nothing behind.
    ///
    /// # Errors
    ///
    /// If the image is refused, the body breaks off, or the store can't be written.
    pub async fn receive(
        &self,
        dir: &[&str],
        body: Body,
        svg_accepted: bool,
    ) -> Result<Received, UploadError> {
        let dir = self.path(dir);
        let partial = {
            let dir = dir.clone();
            tokio::task::spawn_blocking(move || {
                std::fs::create_dir_all(&dir)?;
                // Its name starts with `.`, which `file_name` never gives, so no key names it.
                NamedTempFile::new_in(dir)
            })
            .await
            .map_err(io::Error::from)??
        };
        let version = write_hashed(&partial, body).await?;
        tokio::task::spawn_blocking(move || {
            let content_type = match content_type_of(partial.path()) {
                Err(UploadError::NotAnImage) if svg_accepted => clean_svg_in(&partial)?,
                other => other?,
            };
            // Synced before the rename, or a crash can leave a whole name over an empty file.
            partial.as_file().sync_all()?;
            partial
                .persist(dir.join(&version))
                .map_err(io::Error::from)?;
            Ok(Received {
                version,
                content_type,
            })
        })
        .await
        .map_err(io::Error::from)?
    }

    /// Whether an image is stored at `key`.
    ///
    /// # Errors
    ///
    /// If the store can't be read.
    pub async fn contains(&self, key: &[&str]) -> io::Result<bool> {
        tokio::fs::try_exists(self.path(key)).await
    }

    /// The image at `key` as the answer to `request`, sent as `content_type`: 404 if none is
    /// stored, 304 for a matching `If-None-Match`, else the bytes with an `ETag`.
    ///
    /// Browsers keep the response for good, so serve it only at a URL that names these bytes
    /// alone: a Provider's path, or a version taken from the bytes.
    ///
    /// # Errors
    ///
    /// If the file is there but can't be read.
    pub async fn serve(
        &self,
        key: &[&str],
        content_type: &'static str,
        request: Request,
    ) -> io::Result<Response> {
        let mut response = match ServeFile::new(self.path(key)).try_call(request).await {
            Ok(response) => response.into_response(),
            // Without a fallback, `try_call` hands a missing file back as an error.
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                StatusCode::NOT_FOUND.into_response()
            }
            Err(err) => return Err(err),
        };
        let status = response.status();
        let headers = response.headers_mut();
        // An unnamed type stays a download: no browser guesses it is HTML.
        headers.insert(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        );
        if status.is_success() {
            // `ServeFile` guesses by extension, and would send an SVG as an image.
            headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
        }
        if content_type == SVG {
            // `sandbox` keeps a script from running when someone opens the SVG in its own tab, and
            // `attachment` from it showing there at all; cleaning it is the second layer.
            headers.insert(
                header::CONTENT_SECURITY_POLICY,
                HeaderValue::from_static("default-src 'none'; style-src 'unsafe-inline'; sandbox"),
            );
            headers.insert(
                header::CONTENT_DISPOSITION,
                HeaderValue::from_static("attachment"),
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

    /// Deletes the image at `key`, if one is stored.
    ///
    /// # Errors
    ///
    /// If the file is there but can't be deleted.
    pub async fn delete(&self, key: &[&str]) -> io::Result<()> {
        match tokio::fs::remove_file(self.path(key)).await {
            Err(err) if err.kind() != io::ErrorKind::NotFound => Err(err),
            _ => Ok(()),
        }
    }

    /// The key of every image stored under `dir`, each starting with `dir`.
    ///
    /// # Errors
    ///
    /// If the store can't be read.
    pub async fn list(&self, dir: &str) -> io::Result<Vec<Vec<String>>> {
        let (root, top) = (self.dir.clone(), self.path(&[dir]));
        tokio::task::spawn_blocking(move || list_under(&root, &top)).await?
    }

    fn path(&self, key: &[&str]) -> PathBuf {
        key.iter()
            .fold(self.dir.clone(), |path, part| path.join(file_name(part)))
    }
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

/// Replaces the upload in `partial` with itself as [`clean_svg`] leaves it, and returns [`SVG`].
fn clean_svg_in(partial: &NamedTempFile) -> Result<&'static str, UploadError> {
    if partial.as_file().metadata()?.len() > MAX_SVG_BYTES as u64 {
        return Err(UploadError::SvgTooBig);
    }
    let svg = std::fs::read(partial.path())?;
    let cleaned = clean_svg(&svg).map_err(|_not_svg| UploadError::NotAnImage)?;
    std::fs::write(partial.path(), cleaned)?;
    Ok(SVG)
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
    tokio::task::spawn_blocking(move || {
        let dir = path.parent().unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(dir)?;
        // Its name starts with `.`, which `file_name` never gives, so no key names it.
        let mut partial = NamedTempFile::new_in(dir)?;
        partial.write_all(&bytes)?;
        // Synced before the rename, or a crash can leave a whole name over an empty file.
        partial.as_file().sync_all()?;
        partial.persist(&path)?;
        Ok(())
    })
    .await?
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

/// [`ImageStore::list`] of the folder `top` in the store at `root`. Blocks, so call it from
/// `spawn_blocking`.
fn list_under(root: &Path, top: &Path) -> io::Result<Vec<Vec<String>>> {
    let mut keys = Vec::new();
    for entry in walkdir::WalkDir::new(top) {
        match entry {
            Ok(entry) if entry.file_type().is_file() => keys.extend(key_at(root, entry.path())),
            Ok(_) => {}
            // No such `top` yet, or a file deleted while listing.
            Err(err) if err.io_error().map(io::Error::kind) == Some(io::ErrorKind::NotFound) => {}
            Err(err) => return Err(err.into()),
        }
    }
    Ok(keys)
}

/// The key of the image at `path` in the store at `root`, unless a name on the way isn't one
/// [`file_name`] gives, such as a write's temporary file.
fn key_at(root: &Path, path: &Path) -> Option<Vec<String>> {
    let relative = path.strip_prefix(root).ok()?;
    relative
        .iter()
        .map(|part| part.to_str().and_then(unescape))
        .collect()
}

/// The text `name` is the [`file_name`] of, if it is one.
fn unescape(name: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(name.len());
    let mut rest = name.as_bytes();
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
    let text = String::from_utf8(bytes).ok()?;
    (file_name(&text) == name).then_some(text)
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, extract::Request};

    use super::{ImageStore, file_name, unescape};

    const KEY: &[&str] = &["artwork", "tmdb", "/poster.jpg"];

    #[tokio::test]
    async fn a_failed_write_leaves_nothing_behind() {
        let data = tempfile::tempdir().unwrap();
        let store = ImageStore::new(data.path());
        // A folder where the image goes, so the write can't put it there.
        let blocked = data.path().join("artwork/tmdb/%2Fposter.jpg");
        std::fs::create_dir_all(blocked.join("in-the-way")).unwrap();

        let written = store.write(KEY, b"\xff\xd8").await;

        assert!(written.is_err());
        let left: Vec<_> = std::fs::read_dir(blocked.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(left, ["%2Fposter.jpg"]);
    }

    #[tokio::test]
    async fn deleting_an_image_twice_leaves_none_stored() {
        let data = tempfile::tempdir().unwrap();
        let store = ImageStore::new(data.path());
        store.write(KEY, b"\xff\xd8").await.unwrap();

        store.delete(KEY).await.unwrap();

        assert!(!store.contains(KEY).await.unwrap());
        store.delete(KEY).await.unwrap();
    }

    #[tokio::test]
    async fn an_image_not_stored_is_not_found() {
        let data = tempfile::tempdir().unwrap();
        let request = Request::get("/").body(Body::empty()).unwrap();

        let response = ImageStore::new(data.path())
            .serve(KEY, "image/jpeg", request)
            .await
            .unwrap();

        assert_eq!(response.status(), 404);
    }

    #[tokio::test]
    async fn listing_gives_the_key_of_each_image_stored_under_a_folder() {
        let data = tempfile::tempdir().unwrap();
        let store = ImageStore::new(data.path());
        store.write(KEY, b"\xff\xd8").await.unwrap();
        store.write(&["elsewhere", "x"], b"\xff\xd8").await.unwrap();
        // As a write leaves it while it is under way.
        std::fs::write(data.path().join("artwork/tmdb/.tmpAbC123"), b"").unwrap();

        let listed = store.list("artwork").await.unwrap();

        assert_eq!(listed, [KEY]);
        assert_eq!(
            store.list("nothing-yet").await.unwrap(),
            Vec::<Vec<String>>::new()
        );
    }

    #[test]
    fn a_file_name_unescapes_to_its_text_and_no_other_name_does() {
        for text in ["/poster.jpg", "a b/%.ünï", "tmdb"] {
            assert_eq!(unescape(&file_name(text)).as_deref(), Some(text));
        }
        for name in [".tmpAbC123", "%2f", "%2", "%ZZ", "a%2Fb%"] {
            assert_eq!(unescape(name), None, "{name}");
        }
    }
}
