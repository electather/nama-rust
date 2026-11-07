//! Sessions over HTTP (ADR 0014, ADR 0015): one layer in front of every route turns a session
//! cookie or a bearer token into the [`Caller`], and how a cookie session is handed out.
//!
//! The layer only says who is calling. Whether a call needs a session is decided after it, by the
//! API's access layer.

use axum::{
    extract::{Request, State},
    http::{Extensions, HeaderMap, HeaderValue, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::{
    forwarded::RequestSource,
    users::{Caller, DEVICE_LIFETIME, Users},
};

/// The cookie a browser's session travels in.
const COOKIE_NAME: &str = "nama_session";

/// Adds the [`Caller`] to `request`'s extensions if it carries a valid session, as
/// `Authorization: Bearer <token>` or in the session cookie. When that use pushes back the Device's
/// expiry, a session cookie's is pushed back too, unless the response sets the cookie itself.
pub(crate) async fn identify_caller(
    State(users): State<Users>,
    mut request: Request,
    next: Next,
) -> Response {
    let Some(presented) = presented_token(request.headers()) else {
        return next.run(request).await;
    };
    let session = match users.session(presented.token).await {
        Ok(Some(session)) => session,
        Ok(None) => return next.run(request).await,
        Err(err) => {
            tracing::error!("could not look up a session: {err}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let refreshed_cookie = (session.refreshed && presented.in_cookie)
        .then(|| session_cookie(presented.token, request.extensions()));
    request.extensions_mut().insert::<Caller>(session.caller);
    let mut response = next.run(request).await;
    if let Some(cookie) = refreshed_cookie.and_then(|cookie| HeaderValue::try_from(cookie).ok())
        && !response.headers().contains_key(header::SET_COOKIE)
    {
        response.headers_mut().insert(header::SET_COOKIE, cookie);
    }
    response
}

/// The `Set-Cookie` value that hands a browser `token`, to keep for as long as an unused Device
/// lasts. `Secure` only when the request reached a trusted reverse proxy over HTTPS, since a
/// browser drops a `Secure` cookie sent over plain HTTP, which is how a LAN Server is usually
/// reached (ADR 0015, ADR 0018).
pub(crate) fn session_cookie(token: &str, request: &Extensions) -> String {
    cookie(token, DEVICE_LIFETIME, request)
}

/// The `Set-Cookie` value that has a browser forget its session cookie.
pub(crate) fn cleared_session_cookie(request: &Extensions) -> String {
    cookie("", 0, request)
}

/// A session cookie holding `value` for `max_age` seconds, with the attributes every session
/// cookie has. `request` is the extensions of the request it answers, which say whether that
/// arrived over HTTPS.
fn cookie(value: &str, max_age: i64, request: &Extensions) -> String {
    let secure = request
        .get::<RequestSource>()
        .is_some_and(|source| source.https);
    let secure = if secure { "; Secure" } else { "" };
    format!("{COOKIE_NAME}={value}; HttpOnly; SameSite=Strict; Path=/; Max-Age={max_age}{secure}")
}

/// Whether `headers` carry their session in the session cookie, rather than as a bearer token.
pub(crate) fn in_session_cookie(headers: &HeaderMap) -> bool {
    presented_token(headers).is_some_and(|presented| presented.in_cookie)
}

/// A session token a request carried.
struct PresentedToken<'a> {
    token: &'a str,
    /// Whether it came in the session cookie, rather than as a bearer token.
    in_cookie: bool,
}

/// The session token `headers` carry: a bearer token, or else the session cookie.
fn presented_token(headers: &HeaderMap) -> Option<PresentedToken<'_>> {
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split_once(' '))
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
        .map(|(_, token)| PresentedToken {
            token: token.trim(),
            in_cookie: false,
        });
    bearer.or_else(|| {
        headers
            .get_all(header::COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .flat_map(|cookies| cookies.split(';'))
            .find_map(|cookie| cookie.trim().strip_prefix(COOKIE_NAME)?.strip_prefix('='))
            .map(|token| PresentedToken {
                token,
                in_cookie: true,
            })
    })
}
