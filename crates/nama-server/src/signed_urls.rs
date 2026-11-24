//! Signed URLs (ADR 0105): an `<img>` can't send a bearer token, so a bearer caller gets media
//! URLs that carry a short-lived signature for their one path in place of a session.

use axum::{
    extract::{Request, State},
    http::Uri,
    middleware::Next,
    response::Response,
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::{hmac, rand::SystemRandom};

use crate::db::now;

/// Where a signed URL starts: `/media/signed/<expires>/<signature>/<the rest of the path>`.
const SIGNED: &str = "/media/signed/";

/// The media paths a signed URL may name, which [`SIGNED`] replaces.
const MEDIA: &str = "/media/";

/// How long a signed URL lasts, at least; it lasts at most twice this.
const LIFETIME_S: i64 = 5 * 60;

/// Signs media URLs for bearer callers, and is what [`admit`] checks them with.
///
/// The key lives only in memory, so a restart ends every URL signed before it, and a client gets
/// new ones by asking again.
#[derive(Debug, Clone)]
pub(crate) struct UrlSigner {
    key: hmac::Key,
}

/// Marks a request [`admit`] let through by its URL's signature, which the access layer takes in
/// place of a session.
#[derive(Clone, Copy)]
pub(crate) struct SignedUrl;

impl UrlSigner {
    /// A signer with a fresh random key.
    #[expect(
        clippy::expect_used,
        reason = "it is made once as the Server starts, which can't serve media safely without it"
    )]
    pub(crate) fn new() -> Self {
        let key = hmac::Key::generate(hmac::HMAC_SHA256, &SystemRandom::new())
            .expect("the OS has randomness for the key");
        Self { key }
    }

    /// `url` signed, for a few minutes, if it is under `/media/`; any other URL, such as an empty
    /// one, as it is, since only media is fetched without a session.
    ///
    /// The signature covers the path alone: a client may change the query, which image routes read
    /// only for a width. Sign no route whose query picks what it serves.
    pub(crate) fn sign(&self, url: &str) -> String {
        let Some(rest) = url.strip_prefix(MEDIA) else {
            return url.to_owned();
        };
        // Up to the end of the next window, so an image's URL stays the same within a window and a
        // browser loads it from its cache.
        let expires = (now() / LIFETIME_S + 2) * LIFETIME_S;
        let signature = self.signature(&expires.to_string(), url);
        format!("{SIGNED}{expires}/{signature}/{rest}")
    }

    /// The path `signed` was signed for, if this signer signed it and it hasn't expired.
    fn verify(&self, signed: &str) -> Option<String> {
        let (expires, rest) = signed.strip_prefix(SIGNED)?.split_once('/')?;
        let (signature, rest) = rest.split_once('/')?;
        let path = format!("{MEDIA}{rest}");
        let signature = URL_SAFE_NO_PAD.decode(signature).ok()?;
        hmac::verify(&self.key, &message(expires, &path), &signature).ok()?;
        (now() < expires.parse::<i64>().ok()?).then_some(path)
    }

    fn signature(&self, expires: &str, path: &str) -> String {
        URL_SAFE_NO_PAD.encode(hmac::sign(&self.key, &message(expires, path)))
    }
}

/// The URL `url` was signed from, if it is a signed one, without checking its signature: for a URL
/// a client hands back, whose use is checked on its own, such as picking an offered image.
pub(crate) fn strip_signature(url: &str) -> String {
    let rest = url
        .strip_prefix(SIGNED)
        .and_then(|rest| rest.splitn(3, '/').nth(2));
    rest.map_or_else(|| url.to_owned(), |rest| format!("{MEDIA}{rest}"))
}

/// `url` as `signer`, if there is one, signs it.
pub(crate) fn signed_by(signer: Option<&UrlSigner>, url: String) -> String {
    match signer {
        Some(signer) => signer.sign(&url),
        None => url,
    }
}

/// What a signature signs. `path` starts with `/`, which no `expires` holds, so no other pair
/// makes the same message.
fn message(expires: &str, path: &str) -> Vec<u8> {
    format!("{expires}{path}").into_bytes()
}

/// Lets a request for a URL `signer` signed, before it expires, through as a request for the path
/// it was signed for, marked [`SignedUrl`]. Any other request goes on as it came, so a bad or
/// expired signature reaches the access layer with no session. Must run before routing, which
/// reads the rewritten path.
pub(crate) async fn admit(
    State(signer): State<UrlSigner>,
    mut request: Request,
    next: Next,
) -> Response {
    let unsigned = signer.verify(request.uri().path());
    if let Some(uri) = unsigned.and_then(|path| with_path(request.uri(), &path)) {
        *request.uri_mut() = uri;
        request.extensions_mut().insert(SignedUrl);
    }
    next.run(request).await
}

/// `uri` with its path replaced by `path`, keeping its query, such as an image's `width`.
fn with_path(uri: &Uri, path: &str) -> Option<Uri> {
    let query = uri
        .query()
        .map_or_else(String::new, |query| format!("?{query}"));
    let mut parts = uri.clone().into_parts();
    parts.path_and_query = Some(format!("{path}{query}").parse().ok()?);
    Uri::from_parts(parts).ok()
}
