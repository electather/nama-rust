//! The Nama Server as a library, so its pieces can be tested on their own.

mod api;
pub mod config;
pub mod db;
mod libraries;
pub mod plugins;
mod scanner;

use std::{future::Future, io, sync::Arc};

use tokio::net::TcpListener;

use crate::{config::Config, db::Database, scanner::Scanner};

/// Serves the Server's API on `listener`, backed by `db`, until `shutdown` resolves, then lets
/// requests in flight finish. Once the returned future resolves the API holds no handle to `db`.
///
/// Every Library is scanned in the background when serving starts. Scans still running when
/// `shutdown` resolves are cancelled, and the returned future waits for them to stop.
///
/// Clients speak the Connect protocol over HTTP/1.1 or cleartext HTTP/2, with JSON or binary
/// protobuf bodies.
///
/// # Errors
///
/// The returned future fails if accepting connections on `listener` fails.
pub fn serve(
    listener: TcpListener,
    config: &Config,
    db: Arc<Database>,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> impl Future<Output = io::Result<()>> + Send + 'static {
    let scanner = Scanner::new(Arc::clone(&db));
    let app = api::router(config, db, scanner.clone());
    async move {
        scanner.start_all().await;
        let served = axum::serve(listener, app)
            .with_graceful_shutdown(shutdown)
            .await;
        scanner.shutdown().await;
        served
    }
}
