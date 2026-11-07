//! The Nama Server as a library, so its pieces can be tested on their own.

mod api;
pub mod artwork;
pub mod config;
pub mod contents;
pub mod db;
mod device_names;
mod events;
mod forwarded;
mod libraries;
mod matching;
pub mod plugins;
mod rate_limits;
mod scanner;
mod sessions;
mod sign_in_codes;
pub mod users;
mod versions;
mod watch_states;
#[cfg(feature = "web")]
mod web;

use std::{future::Future, io, net::SocketAddr, sync::Arc};

use tokio::net::TcpListener;

use crate::{
    config::Config, db::Database, events::EventBus, plugins::Registry, scanner::Scanner,
    users::SetupCode,
};

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
/// While the Server has no Users, `Setup` creates the first Admin for whoever gives `setup_code`.
/// `Setup` and `SignIn` are rate limited by the client's IP address. That is the peer's, unless the
/// peer is in [`Config::trusted_proxies`], whose `X-Forwarded-For` names the client.
///
/// Clients speak the Connect protocol over HTTP/1.1 or cleartext HTTP/2, with JSON or binary
/// protobuf bodies. With the `web` feature, any other `GET` gets the web app.
///
/// # Errors
///
/// The returned future fails if accepting connections on `listener` fails.
#[expect(
    clippy::too_many_arguments,
    reason = "each is a separate handle the Server is built from; a struct bundling them would \
              exist only for this call"
)]
pub fn serve(
    listener: TcpListener,
    config: &Config,
    db: Arc<Database>,
    plugins: Registry,
    setup_code: Option<SetupCode>,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> impl Future<Output = io::Result<()>> + Send + 'static {
    let events = EventBus::new(plugins.clone());
    let scanner = Scanner::new(Arc::clone(&db), plugins.clone(), events.clone());
    let app = api::router(
        config,
        db,
        plugins,
        scanner.clone(),
        events.clone(),
        setup_code,
    );
    #[cfg(feature = "web")]
    let app = app.layer(axum::middleware::from_fn(web::serve));
    async move {
        scanner.start_all().await;
        let served = axum::serve(
            listener,
            // The peer's address decides whose rate limit counts and which proxies are trusted.
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
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
