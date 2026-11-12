//! The images the Server keeps, as plain files in the data directory (ADR 0037).
//!
//! Only [`ImageStore`] reads or writes them (`AGENTS.md`). A key names an image by a few parts,
//! each escaped to one file name, so no key reaches outside the store. Resized copies are kept
//! apart, in a `resized` folder, and can be deleted at any time (ADR 0039).

mod resized;

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
use tower_http::services::ServeFile;

use resized::ResizeThread;

/// The widths an image is resized to, smallest first: those the web app's Poster grid and
/// Backdrops are shown at, at one to three device pixels per CSS pixel. Any other is refused.
pub const WIDTHS: [u32; 7] = [320, 480, 640, 960, 1280, 1920, 2560];

/// The folder in the data directory resized copies are kept in (`AGENTS.md`).
const RESIZED_DIR: &str = "resized";

/// Stored images, under the data directory. Clones share one thread that resizes images
/// (`AGENTS.md`).
#[derive(Debug, Clone)]
pub struct ImageStore {
    dir: PathBuf,
    resizer: ResizeThread,
}

/// A resized copy asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resize {
    width: u32,
    cache_limit: u64,
}

impl Resize {
    /// A copy `width` pixels wide, keeping the resized copies within `cache_limit` bytes, or
    /// `None` if `width` isn't one of [`WIDTHS`].
    #[must_use]
    pub fn to(width: u32, cache_limit: u64) -> Option<Self> {
        WIDTHS
            .contains(&width)
            .then_some(Self { width, cache_limit })
    }
}

impl ImageStore {
    /// The images stored in `data_dir`.
    #[must_use]
    pub fn new(data_dir: &Path) -> Self {
        Self {
            dir: data_dir.to_owned(),
            resizer: ResizeThread::start(),
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
    /// With a `resize`, it is sent as a JPEG copy that wide, made the first time it is asked for,
    /// or else as it is stored where it can't or needn't be resized (see `make_copy`), varying by
    /// `Accept` either way. The copy is made from the image's key alone, so it is never made again
    /// if the image at `key` changes.
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
        resize: Option<Resize>,
        request: Request,
    ) -> io::Result<Response> {
        let Some(resize) = resize else {
            return serve_file(self.path(key), content_type, request).await;
        };
        let copy = self.copy_path(key, resize.width);
        let mut response = if self.ensure_copy(key, &copy, resize).await {
            // The copy can go between making it and sending it, deleted with the folder or by
            // another request's eviction, so the original stands in for it.
            let (head, _empty) = request.into_parts();
            let for_copy = Request::from_parts(head.clone(), Body::empty());
            let sent = serve_file(copy, "image/jpeg", for_copy).await?;
            if sent.status() == StatusCode::NOT_FOUND {
                let for_original = Request::from_parts(head, Body::empty());
                serve_file(self.path(key), content_type, for_original).await?
            } else {
                sent
            }
        } else {
            serve_file(self.path(key), content_type, request).await?
        };
        // Browsers that accept JPEG XL are to get it at the same URL (#739).
        response
            .headers_mut()
            .insert(header::VARY, HeaderValue::from_static("accept"));
        Ok(response)
    }

    /// Whether the copy at `copy` of the image at `key` is there to send, made now if it wasn't.
    /// An image that can't be resized is logged and sent as stored.
    async fn ensure_copy(&self, key: &[&str], copy: &Path, resize: Resize) -> bool {
        let sent = copy.to_owned();
        let touch = tokio::task::spawn_blocking(move || resized::touch(&sent)).await;
        match touch.map_err(io::Error::from).and_then(|touched| touched) {
            Ok(()) => return true,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            // It's there, and not marking it only lets eviction pick it sooner.
            Err(err) => {
                tracing::warn!("could not mark {} as sent: {err}", copy.display());
                return true;
            }
        }
        let (original, copy) = (self.path(key), copy.to_owned());
        let cache = self.dir.join(RESIZED_DIR);
        let resized = self
            .resizer
            .run(move || {
                let copied = resized::make_copy(&original, &copy, resize.width);
                if matches!(copied, Ok(true))
                    && let Err(err) = resized::evict(&cache, resize.cache_limit, &copy)
                {
                    tracing::error!("could not keep the resized images within their limit: {err}");
                }
                copied.map_err(|err| format!("could not resize {}: {err}", original.display()))
            })
            .await;
        // The thread having stopped, or a resize having panicked, is one more failed resize.
        let copied = resized
            .map_err(|err| err.to_string())
            .and_then(|copied| copied);
        copied.unwrap_or_else(|err| {
            tracing::warn!("{err}, so it is sent as it is");
            false
        })
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

    fn path(&self, key: &[&str]) -> PathBuf {
        under(self.dir.clone(), key)
    }

    /// Where the copy `width` pixels wide of the image at `key` is kept.
    fn copy_path(&self, key: &[&str], width: u32) -> PathBuf {
        under(self.dir.join(RESIZED_DIR), key).join(format!("{width}.jpg"))
    }
}

/// `key` as a path in `folder`, each part one escaped file name.
fn under(folder: PathBuf, key: &[&str]) -> PathBuf {
    key.iter()
        .fold(folder, |path, part| path.join(file_name(part)))
}

/// The file at `path` as the answer to `request`, sent as `content_type`, as [`ImageStore::serve`]
/// says.
async fn serve_file(
    path: PathBuf,
    content_type: &'static str,
    request: Request,
) -> io::Result<Response> {
    let mut response = match ServeFile::new(path).try_call(request).await {
        Ok(response) => response.into_response(),
        // Without a fallback, `try_call` hands a missing file back as an error.
        Err(err) if err.kind() == io::ErrorKind::NotFound => StatusCode::NOT_FOUND.into_response(),
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
    if status.is_success() || status == StatusCode::NOT_MODIFIED {
        headers.insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("private, max-age=31536000, immutable"),
        );
    }
    Ok(response)
}

/// Writes `bytes` to `path` so no reader ever sees a partial file, even if two writes of the
/// same file race or the machine stops midway.
pub(crate) async fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let (path, bytes) = (path.to_owned(), bytes.to_vec());
    tokio::task::spawn_blocking(move || write_now(&path, &bytes)).await?
}

/// [`write_atomically`], blocking the thread it runs on.
fn write_now(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;
    // Its name starts with `.`, which `file_name` never gives, so no key names it.
    let mut partial = tempfile::NamedTempFile::new_in(dir)?;
    partial.write_all(bytes)?;
    // Synced before the rename, or a crash can leave a whole name over an empty file.
    partial.as_file().sync_all()?;
    partial.persist(path)?;
    Ok(())
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

#[cfg(test)]
mod tests {
    use axum::{body::Body, extract::Request};

    use super::ImageStore;

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
            .serve(KEY, "image/jpeg", None, request)
            .await
            .unwrap();

        assert_eq!(response.status(), 404);
    }
}
