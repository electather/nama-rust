//! The Nama Server as a library, so its pieces can be tested on their own.

mod api;
pub mod artwork;
pub mod config;
pub mod contents;
pub mod db;
mod events;
mod libraries;
mod matching;
pub mod plugins;
mod scanner;

use std::{future::Future, io, sync::Arc};

use tokio::net::TcpListener;

use crate::{config::Config, db::Database, events::EventBus, plugins::Registry, scanner::Scanner};

/// Serves the Server's API on `listener`, backed by `db` and the Plugins enabled in `plugins`,
/// until `shutdown` resolves, then lets requests in flight finish. Once the returned future
/// resolves the API holds no handle to `db`.
///
/// What happens is published to clients subscribed through `EventService` and to Observers at
/// [`nama_plugin_api::points::Events`]. Subscriptions end when `shutdown` resolves.
///
/// Every Library is scanned, and its items Matched, in the background when serving starts. Scans
/// still running when `shutdown` resolves are cancelled, and the returned future waits for them
/// to stop.
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
    plugins: Registry,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> impl Future<Output = io::Result<()>> + Send + 'static {
    let events = EventBus::new(plugins.clone());
    let scanner = Scanner::new(Arc::clone(&db), plugins.clone(), events.clone());
    let app = api::router(config, db, plugins, scanner.clone(), events.clone());
    async move {
        scanner.start_all().await;
        let served = axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                shutdown.await;
                // A subscription never ends by itself, and the shutdown waits for every response.
                events.close();
            })
            .await;
        scanner.shutdown().await;
        served
    }
}
