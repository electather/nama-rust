//! The Nama Server as a library, so its pieces can be tested on their own.

mod api;
mod automation;
pub mod backup;
pub use library::{Artwork, ArtworkError};
mod browse;
pub mod config;
mod connections;
pub mod db;
pub mod diagnostics;
mod events;
pub mod ffmpeg;
mod forwarded;
mod home_rows;
pub mod images;
mod library;
#[cfg(test)]
mod log_capture;
mod matching;
mod page;
mod playback;
pub mod plugin_settings;
pub mod plugins;
mod requests;
mod scanner;
mod server_settings;
pub mod users;
#[cfg(feature = "web")]
mod web;

use std::{future::Future, io, net::SocketAddr, sync::Arc, time::Duration};

use tokio::net::TcpListener;

use crate::{
    automation::Automation,
    config::Config,
    db::Database,
    events::EventBus,
    ffmpeg::{Abilities, Ffmpeg},
    home_rows::HomeRows,
    images::{ImageStore, PluginLogos},
    library::{Contents, CustomImages, LocalImages},
    playback::Probing,
    playback::{RunningPlans, SubtitleCache},
    plugin_settings::PluginSettings,
    plugins::{ExtensionPoints, PluginBlobs},
    requests::Requests,
    scanner::Scanner,
    server_settings::Settings,
    users::SetupCode,
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
#[expect(
    clippy::too_many_lines,
    reason = "it builds the Server's parts and starts them in order, which reads best in one place"
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
    let watch_folders = config.watch_folders;
    let settings = Settings::new(Arc::clone(&db), config.data_dir.join("transcodes"));
    let events = EventBus::new(Arc::clone(&db), points.clone(), settings.server_url());
    let ffmpeg = Arc::new(ffmpeg);
    let subtitles = SubtitleCache::new(&config.data_dir);
    let probing = Probing::new(Arc::clone(&db), Arc::clone(&ffmpeg), subtitles);
    let contents = Contents::new(Arc::clone(&db));
    let running = RunningPlans::new(
        settings.playback(),
        Arc::clone(&ffmpeg),
        contents.clone(),
        probing.clone(),
        events.clone(),
    );
    let requests = Requests::new(Arc::clone(&db), events.clone(), points.clone());
    let home_rows = HomeRows::new(Arc::clone(&db), points.clone(), requests.clone());
    let images = ImageStore::new(&config.data_dir, Arc::clone(&db));
    let artwork = Artwork::new(images.clone(), points.clone());
    let plugin_logos = PluginLogos::new(images.clone());
    let custom_images = CustomImages::new(Arc::clone(&db), images.clone());
    let automation_points = points.clone();
    let scanner = Scanner::new(
        Arc::clone(&db),
        points,
        artwork.clone(),
        custom_images.clone(),
        LocalImages::new(Arc::clone(&db), images.clone()),
        events.clone(),
        Some(probing.clone()),
        running.clone(),
        requests.clone(),
    );
    let automation = Automation::serve(Arc::clone(&db), scanner.clone(), automation_points);
    let abilities = Abilities::new(Arc::clone(&ffmpeg));
    let load_settings = load_settings(settings.clone(), images.clone(), plugins.extension_points());
    let app = api::router(
        config,
        db,
        plugins.clone(),
        scanner.clone(),
        custom_images,
        plugin_logos,
        images,
        artwork,
        probing.clone(),
        events.clone(),
        requests.clone(),
        home_rows.clone(),
        settings,
        running.clone(),
        abilities.clone(),
        setup_code,
        ffmpeg,
    );
    #[cfg(feature = "web")]
    let app = app.layer(axum::middleware::from_fn(web::serve));
    async move {
        // Before anything can publish an event, so that no notification misses its link, and
        // before serving, so the sweeps delete no plan's files or upload but an earlier run's.
        load_settings.await;
        fill_sort_titles(&contents).await;
        scanner.start_all().await;
        // Quiet long enough that a file being copied in is scanned once it's whole; every six hours
        // for the folders whose changes the OS doesn't report, such as network shares.
        if watch_folders {
            scanner.watch(Duration::from_secs(30), Duration::from_hours(6));
        }
        // Every minute, as Seerr asks how its Requests are progressing (ADR 0012).
        let came_up = home_rows.refresh_as_plugins_come_up(plugins.take_came_up());
        requests.start(came_up, Duration::from_mins(1));
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
        home_rows.shutdown().await;
        plugins.shutdown().await;
        // Last, once no Plugin can call the Kernel: the Plugins hold it only weakly.
        drop(automation);
        served
    }
}

/// Stores the sort titles Matches from before them lack, logging a failure: before serving, so no
/// list sorts those first.
async fn fill_sort_titles(contents: &Contents) {
    if let Err(err) = contents.fill_sort_titles().await {
        tracing::error!("storing sort titles failed, so older titles sort first: {err}");
    }
}

/// Reads the Server settings, logging a failure and going on with the defaults, has `images`
/// keep its originals in the store they name, then sweeps the segment files and uploads an earlier
/// run left.
async fn load_settings(settings: Settings, images: ImageStore, points: ExtensionPoints) {
    if let Err(err) = settings.load().await {
        tracing::error!(
            "reading the Server settings failed, so each is its default and notifications have \
             no links: {err}"
        );
    }
    if let Some(plugin) = settings.now().image_store {
        let blobs = PluginBlobs::new(points, plugin.clone());
        // Kept anyway (ADR 0052): the images come back once the Plugin is enabled again.
        if !blobs.is_served() {
            tracing::warn!(
                "the image store is the Plugin {plugin}, which isn't an enabled blob store, so \
                 stored images can't be read until it is enabled again or another store is \
                 picked under Settings"
            );
        }
        images.keep_in(Arc::new(blobs));
    }
    playback::sweep_leftovers(&settings.playback()).await;
    if let Err(err) = images.sweep_uploads().await {
        tracing::warn!("could not delete the uploads an earlier run left: {err}");
    }
}
