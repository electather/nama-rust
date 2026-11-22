//! The Nama Hub as a library, so tests can start it in process.

mod accounts;
mod api;
pub mod config;
pub mod db;
mod links;
pub mod mail;
pub mod signing;
pub mod titles;
#[cfg(feature = "web")]
mod web;

use std::{future::Future, io, net::SocketAddr, sync::Arc};

use nama_forwarded::IpRange;
use signing::Keys;
use sqlx::PgPool;
use tokio::net::TcpListener;

use crate::mail::Mail;

/// Serves the Hub API on `listener`, backed by `db`, which must be migrated already, signing with
/// `keys`, until `shutdown` resolves, then lets requests in flight finish. Forwarded headers are
/// believed only from a peer in `trusted_proxies` (ADR 0018). Sign-in Links are emailed through
/// `mail`, and without it aren't offered. Must be called inside a Tokio runtime, which sends them.
///
/// Clients speak the Connect protocol over HTTP/1.1 or cleartext HTTP/2, with JSON or binary
/// protobuf bodies. With the `web` feature, any other `GET` gets the Hub's web pages.
///
/// # Errors
///
/// The returned future fails if accepting connections on `listener` fails.
#[expect(
    clippy::too_many_arguments,
    reason = "each is a separate handle the Hub is built from; a struct bundling them would exist \
              only for this call"
)]
pub fn serve(
    listener: TcpListener,
    db: PgPool,
    keys: Keys,
    trusted_proxies: &[IpRange],
    mail: Option<Mail>,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> impl Future<Output = io::Result<()>> + Send + 'static {
    let app = api::router(db, Arc::new(keys), trusted_proxies, mail);
    #[cfg(feature = "web")]
    let app = app.layer(axum::middleware::from_fn(web::serve));
    let app = app.into_make_service_with_connect_info::<SocketAddr>();
    async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown)
            .await
    }
}
