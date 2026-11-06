//! Sessions over HTTP (ADR 0014, ADR 0015): one layer in front of every route turns a session
//! cookie or a bearer token into the [`Caller`], and how a cookie session is handed out.
//!
//! The layer only says who is calling. Whether a route needs a session is that route's business.

use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::users::{Caller, Users};

/// The cookie a browser's session travels in.
const COOKIE_NAME: &str = "nama_session";

/// Adds the [`Caller`] to `request`'s extensions if it carries a valid session, as
/// `Authorization: Bearer <token>` or in the session cookie.
pub(crate) async fn identify_caller(
    State(users): State<Users>,
    mut request: Request,
    next: Next,
) -> Response {
    if let Some(token) = presented_token(request.headers()) {
        match users.caller(token).await {
            Ok(Some(caller)) => {
                request.extensions_mut().insert::<Caller>(caller);
            }
            Ok(None) => {}
            Err(err) => {
                tracing::error!("could not look up a session: {err}");
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        }
    }
    next.run(request).await
}

/// The `Set-Cookie` value that hands a browser `token`. `Secure` only when the request reached a
/// reverse proxy over HTTPS, since a browser drops a `Secure` cookie sent over plain HTTP, which is
/// how a LAN Server is usually reached (ADR 0015).
pub(crate) fn session_cookie(token: &str, request_headers: &HeaderMap) -> String {
    let secure = request_headers
        .get("x-forwarded-proto")
        .is_some_and(|proto| proto.as_bytes().eq_ignore_ascii_case(b"https"));
    let secure = if secure { "; Secure" } else { "" };
    format!("{COOKIE_NAME}={token}; HttpOnly; SameSite=Strict; Path=/{secure}")
}

/// The session token `headers` carry: a bearer token, or else the session cookie.
fn presented_token(headers: &HeaderMap) -> Option<&str> {
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    bearer.or_else(|| {
        headers
            .get_all(header::COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .flat_map(|cookies| cookies.split(';'))
            .find_map(|cookie| cookie.trim().strip_prefix(COOKIE_NAME)?.strip_prefix('='))
    })
}
