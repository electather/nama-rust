//! Signed URLs (ADR 0105): an `<img>`, a `<video>` or a subtitle `<track>` can't send a bearer
//! token, so a bearer caller gets media URLs that carry a short-lived signature in place of a
//! session: an image's for its one path, a playback URL's for every path under one Version and the
//! plan playing it, so an HLS playlist's relative segment URLs and a Version's Preview frames load
//! under the same `/media/signed/<grant>/<signature>/` prefix as the URL handed out. A player
//! fetches under that prefix for as long as it plays, so a playback URL past its few minutes
//! still loads while its plan runs for its Device, and stops once the plan does.

use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::Uri,
    middleware::Next,
    response::Response,
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::{hmac, rand::SystemRandom};

use crate::db::now;

/// Where a signed URL starts: `/media/signed/<grant>/<signature>/<the rest of the path>`, its grant
/// being an image's `<expires>` or a [`PlaybackGrant`]'s token.
const SIGNED: &str = "/media/signed/";

/// The media paths a signed URL may name, which [`SIGNED`] replaces.
const MEDIA: &str = "/media/";

/// How long a signed URL lasts, at least; it lasts at most twice this, or a playback URL longer
/// while its plan runs.
const LIFETIME_S: i64 = 5 * 60;

/// Signs media URLs for bearer callers, and is what [`admit`] checks them with.
///
/// The key lives only in memory, so a restart ends every URL signed before it, and a client gets
/// new ones by asking again.
#[derive(Debug, Clone)]
pub(crate) struct UrlSigner {
    key: hmac::Key,
}

/// Whether the plan `id` runs for the Device `device`, which keeps a playback URL for it loading
/// past its expiry.
pub(crate) type PlanRuns = Arc<dyn Fn(u64, i64) -> bool + Send + Sync>;

/// Marks a request [`admit`] let through by its URL's signature, which the access layer takes in
/// place of a session.
#[derive(Clone, Copy)]
pub(crate) struct SignedUrl;

/// The User and Device a request for a signed playback URL acts as, which [`admit`] adds to it,
/// since a plan belongs to its User and Device and a User's Library access and cap apply to what
/// it plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Viewer {
    /// The User, by id.
    pub(crate) user: i64,
    /// The Device, by id.
    pub(crate) device: i64,
}

/// What a signed playback URL reaches: the Version `version`'s media and, unless it is 0, the
/// plan `plan`'s, as `viewer`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PlaybackGrant {
    /// Who the plan was made for.
    pub(crate) viewer: Viewer,
    /// The Version, by id.
    pub(crate) version: i64,
    /// The plan, by the id the API gives it, 0 being none.
    pub(crate) plan: u64,
}

impl PlaybackGrant {
    /// The grant and its expiry `token` holds, if it is one [`Self::token`] wrote.
    fn parse(token: &str) -> Option<(i64, Self)> {
        let fields: Vec<&str> = token.split('.').collect();
        let [expires, user, device, version, plan] = fields.as_slice() else {
            return None;
        };
        let viewer = Viewer {
            user: user.parse().ok()?,
            device: device.parse().ok()?,
        };
        let grant = Self {
            viewer,
            version: version.parse().ok()?,
            plan: plan.parse().ok()?,
        };
        Some((expires.parse().ok()?, grant))
    }

    /// This grant until `expires`, as it goes in the URL: dotted, so it is one path segment.
    fn token(&self, expires: i64) -> String {
        let Self {
            viewer,
            version,
            plan,
        } = self;
        format!(
            "{expires}.{}.{}.{version}.{plan}",
            viewer.user, viewer.device
        )
    }

    /// Whether `rest`, a path after `/media/`, is under its Version or its plan.
    fn covers(&self, rest: &str) -> bool {
        // The router matches `..` as it is, but a path that climbs out of its scope is no one's.
        if rest.split('/').any(|part| part == "..") {
            return false;
        }
        let mut parts = rest.split('/');
        match (parts.next(), parts.next()) {
            (Some("versions"), Some(id)) => id == self.version.to_string(),
            (Some("plans"), Some(id)) => self.plan != 0 && id == self.plan.to_string(),
            _ => false,
        }
    }
}

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
        let expires = expiry();
        let signature = self.signature(&image_message(&expires.to_string(), url));
        format!("{SIGNED}{expires}/{signature}/{rest}")
    }

    /// `url` signed, for a few minutes and then while its plan runs, as `grant` says, if it is
    /// under `/media/`; any other URL, such as the empty one of a plan that can't play, as it is.
    /// The signature covers every path `grant` does, so a client may put another such path after
    /// its `/media/signed/.../` prefix.
    pub(crate) fn sign_playback(&self, url: &str, grant: PlaybackGrant) -> String {
        let Some(rest) = url.strip_prefix(MEDIA) else {
            return url.to_owned();
        };
        let token = grant.token(expiry());
        let signature = self.signature(&playback_message(&token));
        format!("{SIGNED}{token}/{signature}/{rest}")
    }

    /// The path `signed` was signed for, and the Viewer it acts as if it is a playback URL, if this
    /// signer signed it for that path and it hasn't expired, or, for a playback URL, its plan still
    /// runs for its Device as `plan_runs` says.
    fn verify(&self, signed: &str, plan_runs: &PlanRuns) -> Option<(String, Option<Viewer>)> {
        let (token, rest) = signed.strip_prefix(SIGNED)?.split_once('/')?;
        let (signature, rest) = rest.split_once('/')?;
        let path = format!("{MEDIA}{rest}");
        let signature = URL_SAFE_NO_PAD.decode(signature).ok()?;
        if let Some((expires, grant)) = PlaybackGrant::parse(token) {
            hmac::verify(&self.key, &playback_message(token), &signature).ok()?;
            let plays = grant.plan != 0 && plan_runs(grant.plan, grant.viewer.device);
            (grant.covers(rest) && (now() < expires || plays)).then_some((path, Some(grant.viewer)))
        } else {
            hmac::verify(&self.key, &image_message(token, &path), &signature).ok()?;
            (now() < token.parse::<i64>().ok()?).then_some((path, None))
        }
    }

    fn signature(&self, message: &[u8]) -> String {
        URL_SAFE_NO_PAD.encode(hmac::sign(&self.key, message))
    }
}

/// When a URL signed now expires: at the end of the next window, so an image's URL stays the same
/// within a window and a browser loads it from its cache.
fn expiry() -> i64 {
    (now() / LIFETIME_S + 2) * LIFETIME_S
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

/// What an image's signature signs. `path` starts with `/`, which no `expires` holds, so no other
/// pair makes the same message.
fn image_message(expires: &str, path: &str) -> Vec<u8> {
    format!("{expires}{path}").into_bytes()
}

/// What a playback URL's signature signs: its grant's `token`, after a word, so no image's message,
/// which starts with a digit, is the same.
fn playback_message(token: &str) -> Vec<u8> {
    format!("playback {token}").into_bytes()
}

/// Lets a request for a URL `signer` signed, before it expires or, for a playback URL, while its
/// plan runs as `plan_runs` says, through as a request for the path it was signed for, marked
/// [`SignedUrl`], and with its [`Viewer`] if it is a playback URL. Any other request goes on as it
/// came, so a bad or expired signature reaches the access layer with no session. Must run before
/// routing, which reads the rewritten path.
pub(crate) async fn admit(
    State((signer, plan_runs)): State<(UrlSigner, PlanRuns)>,
    mut request: Request,
    next: Next,
) -> Response {
    let unsigned = signer.verify(request.uri().path(), &plan_runs);
    if let Some((path, viewer)) = unsigned
        && let Some(uri) = with_path(request.uri(), &path)
    {
        *request.uri_mut() = uri;
        request.extensions_mut().insert(SignedUrl);
        if let Some(viewer) = viewer {
            request.extensions_mut().insert(viewer);
        }
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
