//! The web app (ADR 0004), served by `nama-web`.

use axum::{extract::Request, middleware::Next, response::Response};

/// Serves the web app for every `GET` but an RPC's or media's, which go on to `next`, so an
/// unknown one still gets a 404. `vp dev` proxies the same paths to a Server
/// (`web/vite.config.ts`).
pub(crate) async fn serve(request: Request, next: Next) -> Response {
    nama_web::serve(request, next, &["/media/"]).await
}
