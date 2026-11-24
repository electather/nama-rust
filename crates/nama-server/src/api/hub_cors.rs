//! Lets the web app at the Hub the Server is Linked with call it from the browser, and no other
//! page (ADR 0105). The grant never covers credentials, and a request carrying a cookie gets none:
//! cookie sessions are safe only while no other origin's calls are approved (ADR 0015).

use axum::{
    extract::{Request, State},
    http::{
        HeaderValue, Method, StatusCode,
        header::{
            ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS,
            ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_REQUEST_METHOD, COOKIE, ORIGIN, VARY,
        },
    },
    middleware::Next,
    response::{IntoResponse as _, Response},
};

use crate::link::Linking;

/// What the web app's Connect calls send besides the bearer token.
const ALLOWED_HEADERS: &str =
    "authorization, content-type, connect-protocol-version, connect-timeout-ms, traceparent";

/// Answers a preflight from the Linked Hub's origin, and lets that origin read the answer to its
/// request; any other request passes through, so its preflight is refused.
pub(super) async fn allow_linked_hub(
    State(linking): State<Linking>,
    request: Request,
    next: Next,
) -> Response {
    let approved = match cookieless_origin(&request) {
        Some(origin)
            if (linking.is_linked_hub_origin(origin.to_str().unwrap_or_default())).await =>
        {
            Some(origin)
        }
        _ => None,
    };
    let mut response = if approved.is_some() && is_preflight(&request) {
        let approval = [
            (ACCESS_CONTROL_ALLOW_METHODS, "GET, POST"),
            (ACCESS_CONTROL_ALLOW_HEADERS, ALLOWED_HEADERS),
        ];
        (StatusCode::NO_CONTENT, approval).into_response()
    } else {
        next.run(request).await
    };
    let headers = response.headers_mut();
    // Approved or not, the answer depends on the origin, so no cache may serve it to another.
    headers.append(VARY, HeaderValue::from_static("origin"));
    if let Some(origin) = approved {
        headers.insert(ACCESS_CONTROL_ALLOW_ORIGIN, origin);
    }
    response
}

fn cookieless_origin(request: &Request) -> Option<HeaderValue> {
    let headers = request.headers();
    if headers.contains_key(COOKIE) {
        return None;
    }
    headers.get(ORIGIN).cloned()
}

fn is_preflight(request: &Request) -> bool {
    request.method() == Method::OPTIONS
        && request
            .headers()
            .contains_key(ACCESS_CONTROL_REQUEST_METHOD)
}
