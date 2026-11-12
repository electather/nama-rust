//! The Nama Server as a library, so its pieces can be tested on their own.

mod api;
pub mod artwork;
mod browse;
pub mod codes;
pub mod config;
mod connections;
mod contents;
mod custom_images;
pub mod db;
mod device_names;
mod events;
pub mod ffmpeg;
mod forwarded;
pub mod images;
mod libraries;
mod matching;
mod metadata;
mod playback;
pub mod plugin_settings;
pub mod plugins;
mod probing;
mod qualities;
mod rate_limits;
mod requests;
mod scanner;
mod server_settings;
mod sessions;
mod sign_in;
mod sign_in_codes;
mod track_preferences;
mod tracks;
pub mod users;
mod watch_states;
#[cfg(feature = "web")]
mod web;

use std::{future::Future, io, net::SocketAddr, sync::Arc, time::Duration};

use tokio::net::TcpListener;

use crate::{
    artwork::Artwork,
    codes::SetupCode,
    config::Config,
    contents::Contents,
    custom_images::CustomImages,
    db::Database,
    events::EventBus,
    ffmpeg::{Abilities, Ffmpeg},
    images::ImageStore,
    playback::running::RunningPlans,
    playback::subtitles,
    plugin_settings::PluginSettings,
    probing::Probing,
    requests::Requests,
    scanner::Scanner,
    server_settings::Settings,
};

/// Serves the Server's API on `listener`, backed by `db` and the Plugins `plugins` started,
/// until `shutdown` resolves, then lets requests in flight finish. Once the returned future
/// resolves the API holds no handle to `db`.
///
/// What happens is published to clients subscribed through `EventService` and to Observers at
/// [`nama_plugin_api::points::Events`]. Subscriptions end when `shutdown` resolves.
///
/// Every Library is scanned, and its items Matched, in the background when serving starts, then
/// again 30 s after its folders last changed, and every six hours for changes the OS doesn't
/// report. After each scan, `ffmpeg`'s ffprobe, if it found one, probes the files not probed yet.
/// Scans and probes still running when `shutdown` resolves are cancelled, and the returned future
/// waits for them to stop. The Approved Requests waiting for a Fulfiller are handed to it in the
/// background whenever `plugins` enables one, startup's included, and the Fulfillers are asked how
/// their Processing Requests are progressing, until `shutdown` resolves.
///
/// While the Server has no Users, `Setup` creates the first Admin for whoever gives `setup_code`.
/// Admins see the `ffmpeg` found at startup with `GetPlaybackStatus`. Plans running when
/// `shutdown` resolves are stopped and their segment files deleted.
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
    let points = plugins.extension_points();
    let settings = Settings::new(Arc::clone(&db), config.data_dir.join("transcodes"));
    let events = EventBus::new(Arc::clone(&db), points.clone(), settings.server_url());
    let ffmpeg = Arc::new(ffmpeg);
    let subtitles = subtitles::Cache::new(&config.data_dir);
    let probing = Probing::new(Arc::clone(&db), Arc::clone(&ffmpeg), subtitles.clone());
    let running = RunningPlans::new(
        settings.playback(),
        Arc::clone(&ffmpeg),
        Contents::new(Arc::clone(&db)),
        probing.clone(),
        events.clone(),
    );
    let requests = Requests::new(Arc::clone(&db), events.clone(), points.clone());
    let images = ImageStore::new(&config.data_dir, Arc::clone(&db));
    let artwork = Artwork::new(images.clone(), points.clone());
    let custom_images = CustomImages::new(Arc::clone(&db), images);
    let scanner = Scanner::new(
        Arc::clone(&db),
        points,
        artwork.clone(),
        custom_images.clone(),
        events.clone(),
        Some(probing.clone()),
        Some(running.clone()),
        Some(requests.clone()),
    );
    let abilities = Abilities::new(Arc::clone(&ffmpeg));
    let load_settings = load_settings(settings.clone());
    let app = api::router(
        config,
        db,
        plugins.clone(),
        scanner.clone(),
        custom_images,
        artwork,
        probing.clone(),
        events.clone(),
        requests.clone(),
        settings,
        running.clone(),
        abilities.clone(),
        setup_code,
        ffmpeg,
        subtitles,
    );
    #[cfg(feature = "web")]
    let app = app.layer(axum::middleware::from_fn(web::serve));
    async move {
        // Before anything can publish an event, so that no notification misses its link, and
        // before serving, so the sweep deletes no plan's files but an earlier run's.
        load_settings.await;
        scanner.start_all().await;
        // Quiet long enough that a file being copied in is scanned once it's whole; every six hours
        // for the folders whose changes the OS doesn't report, such as network shares.
        scanner.watch(Duration::from_secs(30), Duration::from_hours(6));
        // Every minute, as Seerr asks how its Requests are progressing (ADR 0012).
        requests.start(plugins.take_came_up(), Duration::from_mins(1));
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
        running.shutdown().await;
        abilities.shutdown().await;
        // The Scanner first, so a scan finishing now can't start a pass once Probing has stopped.
        scanner.shutdown().await;
        probing.shutdown().await;
        // Before the Plugin settings, which save a Plugin that crashes as it is asked about progress.
        requests.shutdown().await;
        plugins.shutdown().await;
        served
    }
}

/// Reads the Server settings, logging a failure and going on with the defaults, then sweeps the
/// segment files an earlier run left.
async fn load_settings(settings: Settings) {
    if let Err(err) = settings.load().await {
        tracing::error!(
            "reading the Server settings failed, so each is its default and notifications have \
             no links: {err}"
        );
    }
    playback::running::sweep_leftovers(&settings.playback()).await;
}
