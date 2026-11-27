//! How the image store answers an HTTP request for an image: the original from its store, or a
//! copy the resized cache gives (ADR 0039, ADR 0052).

use std::{io, path::PathBuf};

use axum::{
    body::{Body, Bytes},
    extract::Request,
    http::{HeaderMap, HeaderValue, StatusCode, header, request::Parts},
    response::{IntoResponse, Response},
};
use tokio::sync::OwnedSemaphorePermit;
use tower_http::services::ServeFile;

use super::{ImageStore, Resize, SVG, resized::Format, row_key, version_of};

impl ImageStore {
    /// The image at `key` as the answer to `request`, sent as `content_type`: 404 if none is
    /// stored, 304 for a matching `If-None-Match`, else the bytes with an `ETag` taken from `key`,
    /// which is the same in every store since a key names its bytes. Where it is sent from a
    /// local file, a resized copy or an original in the local-disk store, it answers `Range`.
    ///
    /// With a `resize`, it is sent as a copy that wide, made the first time it is asked for, or
    /// else as it is stored where it can't or needn't be resized (see `ensure_copy`), varying by
    /// `Accept` either way. The copy is a JPEG XL where `Accept` lists `image/jxl`, else a JPEG, as
    /// it is too where a JPEG XL would be over 7 megapixels.
    /// Making a copy keeps the image's thumbhash, for `thumbhashes`.
    /// The copy is made from the image's key alone, so it is never made again if the image at
    /// `key` changes.
    ///
    /// Browsers keep the response for good, so serve it only at a URL that names these bytes
    /// alone: a Provider's path, or a version taken from the bytes.
    ///
    /// # Errors
    ///
    /// If the store can't be read.
    pub async fn serve(
        &self,
        key: &[&str],
        content_type: &'static str,
        resize: Option<Resize>,
        request: Request,
    ) -> io::Result<Response> {
        let (head, _empty) = request.into_parts();
        let Some(resize) = resize else {
            return self.serve_original(key, content_type, &head).await;
        };
        let asked = format_asked(&head.headers);
        let made = self.copy_to_send(key, resize, asked).await;
        let mut response = if let Some((copy, format)) = made {
            // The copy can go between making it and sending it, deleted with the folder or by
            // another request's eviction, so the original stands in for it.
            let sent = serve_file(copy, format.content_type(), &head).await?;
            if sent.status() == StatusCode::NOT_FOUND {
                self.serve_original(key, content_type, &head).await?
            } else {
                sent
            }
        } else {
            self.serve_original(key, content_type, &head).await?
        };
        response
            .headers_mut()
            .insert(header::VARY, HeaderValue::from_static("accept"));
        Ok(response)
    }

    /// The original at `key` as the answer to a request with `head`, sent as `content_type`, as
    /// [`ImageStore::serve`] says: streamed, answering Range, from a store that keeps it as a
    /// local file, else read whole under one of `reading`'s permits.
    // ponytail: a Plugin store's original is held whole, at most `MAX_UPLOAD_BYTES`, while it is
    // sent, and answers no Range request (ADR 0052); a streaming get in the contract is the fix,
    // once someone running one reports memory pressure.
    async fn serve_original(
        &self,
        key: &[&str],
        content_type: &'static str,
        head: &Parts,
    ) -> io::Result<Response> {
        let row = row_key(key);
        let etag = format!("\"{}\"", version_of(row.as_bytes()));
        let blobs = self.blobs();
        let mut response = if is_none_match(&head.headers, &etag) {
            // The ETag comes from the key, so whether it is stored is all a 304 needs to know.
            if blobs.exists(&row).await? {
                StatusCode::NOT_MODIFIED.into_response()
            } else {
                StatusCode::NOT_FOUND.into_response()
            }
        } else if let Some(path) = blobs.local_path(&row) {
            // `ServeFile` checks preconditions against its own validators, from the file's
            // modified time and size, which a switch of stores changes (#793); the key's ETag,
            // checked above and set below, is the only one, as in any other store.
            let mut head = head.clone();
            for precondition in [
                header::IF_MATCH,
                header::IF_UNMODIFIED_SINCE,
                header::IF_NONE_MATCH,
                header::IF_MODIFIED_SINCE,
            ] {
                head.headers.remove(precondition);
            }
            let mut sent = serve_file(path, content_type, &head).await?;
            sent.headers_mut().remove(header::LAST_MODIFIED);
            sent
        } else {
            let reading = self.whole_read().await?;
            match blobs.get(&row).await? {
                Some(bytes) => (
                    [(header::CONTENT_TYPE, HeaderValue::from_static(content_type))],
                    // The permit goes once the body's last reference does, when it is sent.
                    Bytes::from_owner(Held {
                        bytes,
                        _reading: reading,
                    }),
                )
                    .into_response(),
                None => StatusCode::NOT_FOUND.into_response(),
            }
        };
        if response.status() != StatusCode::NOT_FOUND {
            let etag = HeaderValue::from_str(&etag).map_err(io::Error::other)?;
            response.headers_mut().insert(header::ETAG, etag);
        }
        guard(&mut response, content_type);
        Ok(response)
    }
}

/// Whether `headers` hold an `If-None-Match` that `etag` matches, compared weakly as RFC 9110
/// says.
fn is_none_match(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get_all(header::IF_NONE_MATCH)
        .iter()
        .filter_map(|value| value.to_str().ok())
        // Splitting at every `,` is safe for ours: a version never holds one.
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .any(|tag| tag == "*" || tag.strip_prefix("W/").unwrap_or(tag) == etag)
}

/// JPEG XL where `headers` hold an `Accept` that lists `image/jxl` at a quality above 0, else
/// JPEG.
pub(super) fn format_asked(headers: &HeaderMap) -> Format {
    let is_quality_zero = |param: &str| {
        param.split_once('=').is_some_and(|(name, value)| {
            name.trim().eq_ignore_ascii_case("q")
                && value
                    .trim()
                    .parse::<f32>()
                    .is_ok_and(|quality| quality <= 0.0)
        })
    };
    let accepts_jxl = headers
        .get_all(header::ACCEPT)
        .iter()
        .filter_map(|accept| accept.to_str().ok())
        .flat_map(|accept| accept.split(','))
        .any(|range| {
            let mut parts = range.split(';');
            parts
                .next()
                .is_some_and(|media| media.trim().eq_ignore_ascii_case("image/jxl"))
                && !parts.any(is_quality_zero)
        });
    if accepts_jxl {
        Format::Jxl
    } else {
        Format::Jpeg
    }
}

/// An original read whole, holding its permit of `ImageStore::reading` until it is dropped.
struct Held {
    bytes: Vec<u8>,
    _reading: OwnedSemaphorePermit,
}

impl AsRef<[u8]> for Held {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

/// The file at `path` as the answer to a request with `head`, sent as `content_type`, as
/// [`ImageStore::serve`] says.
async fn serve_file(
    path: PathBuf,
    content_type: &'static str,
    head: &Parts,
) -> io::Result<Response> {
    let request = Request::from_parts(head.clone(), Body::empty());
    let mut response = match ServeFile::new(path).try_call(request).await {
        Ok(response) => response.into_response(),
        // Without a fallback, `try_call` hands a missing file back as an error.
        Err(err) if err.kind() == io::ErrorKind::NotFound => StatusCode::NOT_FOUND.into_response(),
        Err(err) => return Err(err),
    };
    if response.status().is_success() {
        // `ServeFile` guesses by extension.
        response
            .headers_mut()
            .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    }
    guard(&mut response, content_type);
    Ok(response)
}

/// Adds the headers every image is sent with to `response`, an image sent as `content_type`:
/// those that keep a browser from running it, and on a 200 or 304 those that keep it for good.
fn guard(response: &mut Response, content_type: &'static str) {
    let status = response.status();
    let headers = response.headers_mut();
    // An unnamed type stays a download: no browser guesses it is HTML.
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    if content_type == SVG {
        // `sandbox` keeps a script from running when someone opens the SVG in its own tab, and
        // `attachment` from it showing there at all, should `clean_svg` ever miss one
        // (`AGENTS.md`).
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
}

#[cfg(test)]
#[path = "serving_tests.rs"]
mod tests;
