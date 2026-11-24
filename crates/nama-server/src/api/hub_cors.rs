//! Lets the web app at the Hub the Server is Linked with call it from the browser, and no other
//! page (ADR 0105). The grant never covers credentials, and a request carrying a cookie gets none:
//! cookie sessions are safe only while no other origin's calls are approved (ADR 0015). It asks
//! for no bearer token, since the page trades its Hub-signed token for one here first. The rule is
//! in `crates/nama-server/AGENTS.md`, "CORS for the Hub only".

use axum::{
    extract::{Request, State},
    http::{
        HeaderMap, HeaderValue, Method, StatusCode,
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

/// Answers a preflight from the origin of the Hub the Server is Linked with, and lets that origin
/// read the answer to its request; any other request passes through, so its preflight is refused.
pub(super) async fn allow_linked_hub(
    State(linking): State<Linking>,
    request: Request,
    next: Next,
) -> Response {
    let approved = approved_origin(&linking, request.headers()).await;
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

/// The request's origin, if it carries no cookie and is the Hub's while the Server is Linked.
async fn approved_origin(linking: &Linking, headers: &HeaderMap) -> Option<HeaderValue> {
    if headers.contains_key(COOKIE) {
        return None;
    }
    let origin = headers.get(ORIGIN)?;
    (linking.is_linked_hub_origin(origin.to_str().ok()?).await).then(|| origin.clone())
}

fn is_preflight(request: &Request) -> bool {
    request.method() == Method::OPTIONS
        && request
            .headers()
            .contains_key(ACCESS_CONTROL_REQUEST_METHOD)
}
