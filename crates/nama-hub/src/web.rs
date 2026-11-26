//! The web app at the Hub (ADR 0105), the same one a Server serves, from `nama-web`.

use axum::{extract::Request, middleware::Next, response::Response};

/// Serves the web app for every `GET` but an RPC's or the key set's, which go on to `next`, so an
/// unknown one still gets a 404. With `NAMA_HUB_URL` set, `vp dev` proxies the Hub's RPCs to it
/// (`web/vite.config.ts`).
pub(crate) async fn serve(request: Request, next: Next) -> Response {
    nama_web::serve(request, next, &["/.well-known/"]).await
}

#[cfg(test)]
#[path = "web_tests.rs"]
mod tests;
