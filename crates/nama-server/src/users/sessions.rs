//! Sessions over HTTP (ADR 0014, ADR 0015): one layer in front of every route turns a session
//! cookie or a bearer token into its [`Device`] and the [`Caller`], and how a cookie session is
//! handed out.
//!
//! The layer only says who is calling. Whether a call needs a session is decided after it, by the
//! API's access layer.

use axum::{
    extract::{Request, State},
    http::{Extensions, HeaderMap, HeaderValue, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};

use super::{Caller, DEVICE_LIFETIME, Device, Users};
use nama_forwarded::RequestSource;

const COOKIE_NAME: &str = "nama_session";

/// The cookie's name over HTTPS. Browsers only take a `__Host-` cookie that is `Secure` and has no
/// `Domain`, so another Server under the same Server-names domain can't set one this Server reads
/// (cookie tossing). That domain isn't on the Public Suffix List yet (ADR 0057).
const HTTPS_COOKIE_NAME: &str = "__Host-nama_session";

/// Adds the [`Device`] to `request`'s extensions if it carries a valid session, and the
/// [`Caller`] too while that Device acts as one of its Users (ADR 0106), as
/// `Authorization: Bearer <token>` or in the session cookie. When that use pushes back the Device's
/// expiry, a session cookie's is pushed back too, unless the response sets the cookie itself.
pub(crate) async fn identify_caller(
    State(users): State<Users>,
    mut request: Request,
    next: Next,
) -> Response {
    let Some(presented) = presented_token(request.headers(), request.extensions()) else {
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
    if let Some(user) = session.user {
        request.extensions_mut().insert(Caller {
            user,
            device: session.device.clone(),
        });
    }
    request.extensions_mut().insert::<Device>(session.device);
    let mut response = next.run(request).await;
    if let Some(cookie) = refreshed_cookie.and_then(|cookie| HeaderValue::try_from(cookie).ok())
        && !response.headers().contains_key(header::SET_COOKIE)
    {
        response.headers_mut().insert(header::SET_COOKIE, cookie);
    }
    response
}

/// The `Set-Cookie` value that hands a browser `token`, to keep for as long as an unused Device
/// lasts. `Secure`, and named `__Host-`, only when the request reached the Server's own TLS or a
/// trusted reverse proxy over HTTPS, since a browser drops a `Secure` cookie sent over plain HTTP,
/// which is how a LAN Server is usually reached (ADR 0015, ADR 0018).
pub(crate) fn session_cookie(token: &str, request: &Extensions) -> String {
    cookie(token, DEVICE_LIFETIME, request)
}

pub(crate) fn cleared_session_cookie(request: &Extensions) -> String {
    cookie("", 0, request)
}

/// A session cookie holding `value` for `max_age` seconds, with the attributes every session
/// cookie has. `request` is the extensions of the request it answers, which say whether that
/// arrived over HTTPS.
fn cookie(value: &str, max_age: i64, request: &Extensions) -> String {
    let name = cookie_name(request);
    let secure = if is_https(request) { "; Secure" } else { "" };
    format!("{name}={value}; HttpOnly; SameSite=Strict; Path=/; Max-Age={max_age}{secure}")
}

fn is_https(request: &Extensions) -> bool {
    request
        .get::<RequestSource>()
        .is_some_and(|source| source.https)
}

/// The session cookie's name for a request with these extensions. Over HTTPS only the `__Host-`
/// name counts, so a plain-named cookie another host tossed in signs no one in.
fn cookie_name(request: &Extensions) -> &'static str {
    if is_https(request) {
        HTTPS_COOKIE_NAME
    } else {
        COOKIE_NAME
    }
}

pub(crate) fn in_session_cookie(headers: &HeaderMap, request: &Extensions) -> bool {
    presented_token(headers, request).is_some_and(|presented| presented.in_cookie)
}

/// The session token a request with `headers` and `request`'s extensions carries, valid or not,
/// so a sign-in adds its User to the Device holding it (ADR 0106).
pub(crate) fn session_token<'a>(headers: &'a HeaderMap, request: &Extensions) -> Option<&'a str> {
    presented_token(headers, request).map(|presented| presented.token)
}

struct PresentedToken<'a> {
    token: &'a str,
    in_cookie: bool,
}

/// The session token `headers` carry: a bearer token, or else the session cookie under the name
/// [`cookie_name`] gives for `request`.
fn presented_token<'a>(headers: &'a HeaderMap, request: &Extensions) -> Option<PresentedToken<'a>> {
    let name = cookie_name(request);
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
            .find_map(|cookie| cookie.trim().strip_prefix(name)?.strip_prefix('='))
            .map(|token| PresentedToken {
                token,
                in_cookie: true,
            })
    })
}

#[cfg(test)]
#[path = "sessions_tests.rs"]
mod tests;
