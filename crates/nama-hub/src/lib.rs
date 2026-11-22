//! The Nama Hub as a library, so tests can start it in process.

mod api;
pub mod config;
pub mod db;
pub mod titles;

use std::{future::Future, io};

use sqlx::PgPool;
use tokio::net::TcpListener;

/// Serves the Hub API on `listener`, backed by `db`, which must be migrated already, until
/// `shutdown` resolves, then lets requests in flight finish.
///
/// Clients speak the Connect protocol over HTTP/1.1 or cleartext HTTP/2, with JSON or binary
/// protobuf bodies.
///
/// # Errors
///
/// The returned future fails if accepting connections on `listener` fails.
pub fn serve(
    listener: TcpListener,
    db: PgPool,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> impl Future<Output = io::Result<()>> + Send + 'static {
    let app = api::router(db);
    async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown)
            .await
    }
}
