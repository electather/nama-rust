//! The Nama Server as a library, so its pieces can be tested on their own.

mod api;
pub mod config;

use std::{future::Future, io};

use tokio::net::TcpListener;

use crate::config::Config;

/// Serves the Server's API on `listener` until `shutdown` resolves, then lets requests in flight
/// finish.
///
/// Clients speak the Connect protocol over HTTP/1.1 or cleartext HTTP/2, with JSON or binary
/// protobuf bodies.
pub fn serve(
    listener: TcpListener,
    config: &Config,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> impl Future<Output = io::Result<()>> + Send + 'static {
    let app = api::router(config);
    async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown)
            .await
    }
}
