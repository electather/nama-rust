//! The Nama Hub as a library, so tests can start it in process.

mod accounts;
mod api;
pub mod config;
pub mod db;
pub mod diagnostics;
mod links;
pub mod mail;
pub mod signing;
mod sync;
pub mod titles;
mod viewing;
#[cfg(feature = "web")]
mod web;

use std::{future::Future, io, net::SocketAddr, sync::Arc};

use nama_forwarded::IpRange;
use signing::Keys;
use sqlx::PgPool;
use tokio::net::TcpListener;

use crate::{diagnostics::Backend, mail::Mail};

/// Serves the Hub API on `listener`, backed by `db`, which must be migrated already, signing with
/// `keys`, until `shutdown` resolves, then lets requests in flight finish. Forwarded headers are
/// believed only from a peer in `trusted_proxies` (ADR 0018). Sign-in Links are emailed through
/// `mail`, and without it aren't offered. What Linked Servers share goes to `diagnostics`, and
/// without it is dropped. Must be called inside a Tokio runtime, which sends them.
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
    diagnostics: Option<Arc<dyn Backend>>,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> impl Future<Output = io::Result<()>> + Send + 'static {
    let app = api::router(db, Arc::new(keys), trusted_proxies, mail, diagnostics);
    #[cfg(feature = "web")]
    let app = app.layer(axum::middleware::from_fn(web::serve));
    let app = app.into_make_service_with_connect_info::<SocketAddr>();
    async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown)
            .await
    }
}

/// Builds every title's vector from Wikidata's facts and the pooled counts, replacing the previous
/// training run's at once, and returns how many titles have one (ADR 0095). Only `train-vectors`
/// and the offline evaluation call it, never the serving process (ADR 0096).
///
/// # Errors
///
/// If a query fails. The previous run's vectors stay.
pub async fn train_vectors(db: &PgPool) -> Result<usize, sqlx::Error> {
    // Here, not in `titles`, which sits below `viewing` and can't read its counts (ADR 0099).
    titles::train(db, &viewing::pooled_counts(db).await?).await
}
