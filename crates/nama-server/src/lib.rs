//! The Nama Server as a library, so its pieces can be tested on their own.

mod api;
pub mod artwork;
mod browse;
pub mod config;
mod connections;
pub mod contents;
pub mod db;
mod device_names;
mod events;
pub mod ffmpeg;
mod forwarded;
mod libraries;
mod matching;
pub mod plugin_settings;
pub mod plugins;
mod qualities;
mod rate_limits;
mod requests;
mod scanner;
mod server_settings;
mod sessions;
mod sign_in_codes;
mod tracks;
pub mod users;
mod versions;
mod watch_states;
#[cfg(feature = "web")]
mod web;

use std::{future::Future, io, net::SocketAddr, sync::Arc};

use tokio::net::TcpListener;

use crate::{
    config::Config,
    db::Database,
    events::EventBus,
    ffmpeg::Ffmpeg,
    matching::Matcher,
    plugin_settings::PluginSettings,
    requests::{PROGRESS_EVERY, Requests},
    scanner::Scanner,
    server_settings::ServerUrl,
    users::SetupCode,
};

/// Serves the Server's API on `listener`, backed by `db` and the Plugins `plugins` started,
/// until `shutdown` resolves, then lets requests in flight finish. Once the returned future
/// resolves the API holds no handle to `db`.
///
/// What happens is published to clients subscribed through `EventService` and to Observers at
/// [`nama_plugin_api::points::Events`]. Subscriptions end when `shutdown` resolves.
///
/// Every Library is scanned, and its items Matched, in the background when serving starts. After
/// each scan, `ffmpeg`'s ffprobe, if it found one, probes the files not probed yet. Scans and
/// probes still running when `shutdown` resolves are cancelled, and the returned future waits for
/// them to stop. The Approved Requests waiting for a Fulfiller are handed to it in the background
/// whenever `plugins` enables one, startup's included, and the Fulfillers are asked how their
/// Processing Requests are progressing, until `shutdown` resolves.
///
/// While the Server has no Users, `Setup` creates the first Admin for whoever gives `setup_code`.
/// Admins see the `ffmpeg` found at startup with `GetPlaybackStatus`.
///
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
    plugins: PluginSettings,
    setup_code: Option<SetupCode>,
    ffmpeg: Ffmpeg,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> impl Future<Output = io::Result<()>> + Send + 'static {
    let registry = plugins.registry();
    let server_url = ServerUrl::default();
    let events = EventBus::new(Arc::clone(&db), registry.clone(), server_url.clone());
    let ffmpeg = Arc::new(ffmpeg);
    let scanner = Scanner::new(
        Arc::clone(&db),
        registry.clone(),
        events.clone(),
        Arc::clone(&ffmpeg),
    );
    let matcher = Matcher::new(Arc::clone(&db), registry.clone(), events.clone());
    let requests = Requests::new(Arc::clone(&db), matcher, events.clone(), registry.clone());
    let load_server_url = {
        let (server_url, db) = (server_url.clone(), Arc::clone(&db));
        async move {
            if let Err(err) = server_url.load(&db).await {
                tracing::error!(
                    "reading the Server URL failed, so notifications have no links: {err}"
                );
            }
        }
    };
    let app = api::router(
        config,
        db,
        plugins.clone(),
        scanner.clone(),
        events.clone(),
        requests.clone(),
        server_url,
        setup_code,
        ffmpeg,
    );
    #[cfg(feature = "web")]
    let app = app.layer(axum::middleware::from_fn(web::serve));
    async move {
        // Before anything can publish an event, so that no notification misses its link.
        load_server_url.await;
        scanner.start_all().await;
        // In the background, so a slow Fulfiller doesn't hold up serving.
        let progress = requests.clone();
        let following = tokio::spawn(async move { progress.follow_progress(PROGRESS_EVERY).await });
        let came_up = plugins.take_came_up();
        let handing_off =
            tokio::spawn(async move { requests.hand_off_as_fulfillers_come_up(came_up).await });
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
        // The Requests not handed off yet stay Approved, so the next start hands them off. The one
        // being handed off now runs on in a task of its own (`HandOff::request`), which nothing
        // waits for: if the runtime stops first, a Request the Fulfiller took stays Approved and
        // the next start hands it over again, as for an approval in flight.
        handing_off.abort();
        if let Err(err) = handing_off.await
            && err.is_panic()
        {
            tracing::error!("handing the waiting Requests off panicked: {err}");
        }
        plugins.shutdown().await;
        // Progress is kept in memory only, so stopping mid-poll loses nothing.
        following.abort();
        if let Err(err) = following.await
            && err.is_panic()
        {
            tracing::error!("asking the Fulfillers about progress panicked: {err}");
        }
        served
    }
}
