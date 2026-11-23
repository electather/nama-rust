//! The Hub's web pages (ADR 0010): the app `vp -C hub build` writes to `web/hub/dist`, embedded in
//! the binary by `build.rs`.

use std::convert::Infallible;

use axum::{
    extract::Request,
    http::{Method, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use tower::{ServiceExt as _, service_fn};
use tower_http::compression::{
    Compression, Predicate as _,
    predicate::{DefaultPredicate, NotForContentType},
};

include!(concat!(env!("OUT_DIR"), "/web_assets.rs"));

/// Answers a `GET` for any path but an RPC's or the key set's with the app's file at that path, or
/// else with `index.html`, so a reloaded deep link still loads the app, which then routes to the
/// page. A file under `/assets/` is marked to be kept for a year, and `index.html` isn't, so a new
/// release still loads. It compresses the file with brotli or gzip when the request accepts one.
/// Every other request goes on to `next`, so an unknown RPC still gets a 404. `vp dev` proxies the
/// same paths to a Hub (`web/hub/vite.config.ts`).
pub(crate) async fn serve(request: Request, next: Next) -> Response {
    let path = request.uri().path();
    let passed_on = path.starts_with("/nama.") || path.starts_with("/.well-known/");
    if request.method() != Method::GET || passed_on {
        return next.run(request).await;
    }
    // `DefaultPredicate` leaves images but SVG alone; fonts are woff2, compressed already.
    // ponytail: compresses the same embedded bytes afresh on every request (brotli 4, gzip 6);
    // precompress them in `build.rs` once that CPU shows.
    let not_compressed_already = DefaultPredicate::new().and(NotForContentType::const_new("font/"));
    let Ok(response) = Compression::new(service_fn(asset_or_shell))
        .compress_when(not_compressed_already)
        .oneshot(request)
        .await;
    response.into_response()
}

async fn asset_or_shell(request: Request) -> Result<Response, Infallible> {
    let path = request.uri().path();
    Ok(match asset(path) {
        // `vp build` names every file under `assets/` by its content's hash, so one never changes.
        Some(body) if path.starts_with("/assets/") => (
            [
                (header::CONTENT_TYPE, content_type(path)),
                (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
            ],
            body,
        )
            .into_response(),
        Some(body) => ([(header::CONTENT_TYPE, content_type(path))], body).into_response(),
        None => ([(header::CONTENT_TYPE, HTML_CONTENT_TYPE)], SHELL).into_response(),
    })
}

const HTML_CONTENT_TYPE: &str = "text/html; charset=utf-8";

fn content_type(path: &str) -> &'static str {
    match path.rsplit_once('.').map(|(_, extension)| extension) {
        Some("html") => HTML_CONTENT_TYPE,
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("woff2") => "font/woff2",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
#[path = "web_tests.rs"]
mod tests;
