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
mod playback;
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
mod watch_states;
#[cfg(feature = "web")]
mod web;

use std::{future::Future, io, net::SocketAddr, sync::Arc};

use tokio::net::TcpListener;

use crate::{
    config::Config,
    contents::Contents,
    db::Database,
    events::EventBus,
    ffmpeg::Ffmpeg,
    matching::Matcher,
    playback::running::RunningPlans,
    playback::subtitles,
    plugin_settings::PluginSettings,
    requests::Requests,
    scanner::Scanner,
    server_settings::{RemotePlaybackLimit, SegmentFolder, ServerUrl},
    tracks::Tracks,
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
    let registry = plugins.registry();
    let server_url = ServerUrl::default();
    let events = EventBus::new(Arc::clone(&db), registry.clone(), server_url.clone());
    let ffmpeg = Arc::new(ffmpeg);
    let subtitles = subtitles::Cache::new(&config.data_dir);
    let scanner = Scanner::new(
        Arc::clone(&db),
        registry.clone(),
        events.clone(),
        Arc::clone(&ffmpeg),
        subtitles.clone(),
    );
    let matcher = Matcher::new(Arc::clone(&db), registry.clone(), events.clone());
    let requests = Requests::new(Arc::clone(&db), matcher, events.clone(), registry.clone());
    let segment_folder = SegmentFolder::new(config.data_dir.join("transcodes"));
    let remote_limit = RemotePlaybackLimit::default();
    let running = RunningPlans::new(
        segment_folder.clone(),
        Arc::clone(&ffmpeg),
        Contents::new(Arc::clone(&db)),
        Tracks::new(Arc::clone(&db)),
        events.clone(),
    );
    let load_settings = load_settings(
        server_url.clone(),
        segment_folder.clone(),
        remote_limit.clone(),
        running.clone(),
        Arc::clone(&db),
    );
    let app = api::router(
        config,
        db,
        plugins.clone(),
        scanner.clone(),
        events.clone(),
        requests.clone(),
        server_url,
        segment_folder,
        remote_limit,
        running.clone(),
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
        requests.start(plugins.take_came_up());
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
        running.stop_all().await;
        scanner.shutdown().await;
        // Before the Plugin settings, which save a Plugin that crashes as it is asked about progress.
        requests.shutdown().await;
        plugins.shutdown().await;
        served
    }
}

/// Reads the Server URL, the segment folder, the remote Playback Limit, and `running`'s Transcode
/// cap and what Transcodes run on from `db`, logging a failure and going on with the default, then sweeps the segment files an earlier
/// run left.
async fn load_settings(
    server_url: ServerUrl,
    segment_folder: SegmentFolder,
    remote_limit: RemotePlaybackLimit,
    running: RunningPlans,
    db: Arc<Database>,
) {
    if let Err(err) = server_url.load(&db).await {
        tracing::error!("reading the Server URL failed, so notifications have no links: {err}");
    }
    if let Err(err) = segment_folder.load(&db).await {
        tracing::error!(
            "reading the segment folder failed, so segment files go to the default: {err}"
        );
    }
    if let Err(err) = running.max_transcodes().load(&db).await {
        tracing::error!("reading the Transcode cap failed, so it's the default: {err}");
    }
    if let Err(err) = remote_limit.load(&db).await {
        tracing::error!(
            "reading the remote Playback Limit failed, so remote playback isn't limited: {err}"
        );
    }
    if let Err(err) = running.hardware_acceleration().load(&db).await {
        tracing::error!("reading what Transcodes run on failed, so it's Automatic: {err}");
    }
    playback::running::sweep_leftovers(&segment_folder).await;
}
