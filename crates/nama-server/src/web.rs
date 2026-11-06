//! The web app (ADR 0004): the SPA that `vp build` writes to `web/dist/client`, embedded in the
//! binary by `build.rs`.

use axum::{
    extract::Request,
    http::{Method, header},
    middleware::Next,
    response::{IntoResponse, Response},
};

include!(concat!(env!("OUT_DIR"), "/web_assets.rs"));

/// Answers a `GET` for any path but an RPC's or `/media/…` with the web app's file at that path,
/// or else with `_shell.html`, so a reloaded deep link still loads the app, which then routes to
/// the page. Every other request goes on to `next`, so an unknown RPC still gets a 404. `vp dev`
/// proxies the same paths to a Server (`web/vite.config.ts`).
pub(crate) async fn serve(request: Request, next: Next) -> Response {
    let path = request.uri().path();
    if request.method() != Method::GET || path.starts_with("/nama.") || path.starts_with("/media/")
    {
        return next.run(request).await;
    }
    match asset(path) {
        Some(body) => ([(header::CONTENT_TYPE, content_type(path))], body).into_response(),
        None => ([(header::CONTENT_TYPE, HTML_CONTENT_TYPE)], SHELL).into_response(),
    }
}

const HTML_CONTENT_TYPE: &str = "text/html; charset=utf-8";

/// The media type of a file `vp build` writes, by its extension.
fn content_type(path: &str) -> &'static str {
    match path.rsplit_once('.').map(|(_, extension)| extension) {
        Some("html") => HTML_CONTENT_TYPE,
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("woff2") => "font/woff2",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        _ => "application/octet-stream",
    }
}
