//! The Nama Server as a library, so its pieces can be tested on their own.

mod api;
mod automation;
pub mod backup;
pub use library::{Artwork, ArtworkError};
pub use link::ShareWithHub;
mod browse;
mod certificate;
pub mod config;
mod connections;
pub mod db;
pub mod diagnostics;
mod events;
pub mod ffmpeg;
mod home_rows;
pub mod images;
mod library;
mod link;
#[cfg(test)]
mod log_capture;
mod matching;
mod page;
mod playback;
pub mod plugin_settings;
pub mod plugins;
mod port_mapping;
pub mod relay;
mod requests;
mod scanner;
mod server_addresses;
mod server_settings;
mod signed_urls;
mod status;
mod sync;
mod tls;
pub mod users;
mod watchlist;
#[cfg(feature = "web")]
mod web;

use std::{future::Future, io, sync::Arc, time::Duration};

use tokio::{net::TcpListener, sync::mpsc};

use crate::{
    automation::Automation,
    backup::Backups,
    config::Config,
    db::Database,
    events::EventBus,
    ffmpeg::{Abilities, Ffmpeg},
    home_rows::HomeRows,
    images::{CUSTOM_IMAGES_DIR, ImageStore, PluginLogos},
    library::{Contents, CustomImages, LocalImages},
    playback::{
        Leftovers, PreviewFrames, ProbePasses, Probing, RunningPlans, Segments, SubtitleCache,
        TextPasses, WholeFileReads,
    },
    plugin_settings::PluginSettings,
    plugins::{PluginBlobs, log_jobs_time_zone},
    requests::Requests,
    scanner::Scanner,
    server_settings::Settings,
    users::{SetupCode, Users},
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
/// The Hub [`Config::hub_url`] names, if any, is told the Server's Server Addresses every minute
/// and as soon as the Server URL is saved, and while the Server is Linked with it, it keeps its
/// port open on the router with UPnP or NAT-PMP, unless [`Config::forwarded_port`] names the one
/// the Admin forwarded. The enabled Plugin that links its Users, if any, is asked for each User's
/// Watch State on their other Servers after each scan.
///
/// While the Server is Linked and the Hub names it, the Server gets a certificate for its names
/// from [`Config::acme_directory`] and keeps it renewed, and its public name is its Server URL
/// while the Admin sets none (ADR 0057). With [`Config::relay`], it keeps connections waiting at
/// that Relay with a token the Hub signs, and serves the clients they carry.
///
/// While the Admin shares Diagnostics, what the Server's exporters post through `sharing`, and the
/// web app's spans, go to that Hub while the Server is Linked with it (ADR 0054).
///
/// `Setup` and `SignIn` are rate limited by the client's IP address. That is the peer's, or through
/// the Relay the one the Relay names, unless the peer is in [`Config::trusted_proxies`], whose
/// `X-Forwarded-For` names the client.
///
/// Clients speak the Connect protocol over HTTP/1.1 or HTTP/2, with JSON or binary protobuf
/// bodies, in cleartext, or over TLS on the same port once the Server has a certificate. With the
/// `web` feature, any other `GET` gets the web app.
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
    sharing: ShareWithHub,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> impl Future<Output = io::Result<()>> + Send + 'static {
    let points = plugins.extension_points();
    let watch_folders = config.watch_folders;
    let settings = Settings::new(Arc::clone(&db), config.data_dir.join("transcodes"));
    let events = EventBus::new(Arc::clone(&db), points.clone(), settings.server_url());
    let ffmpeg = Arc::new(ffmpeg);
    let subtitles = SubtitleCache::new(&config.data_dir);
    let images = ImageStore::new(&config.data_dir, Arc::clone(&db));
    let (passes, reads) = (ProbePasses::default(), WholeFileReads::default());
    let probing = Probing::new(
        Arc::clone(&db),
        Arc::clone(&ffmpeg),
        passes.clone(),
        reads.clone(),
    );
    let leftovers = Leftovers::new(Arc::clone(&db), subtitles.clone(), images.clone());
    let segments = Segments::new(Arc::clone(&db), points.clone());
    // Followed now, so no pass a scan at startup finishes is missed.
    let probed_for_segments = passes.follow();
    let contents = Contents::new(Arc::clone(&db));
    let running = RunningPlans::new(
        settings.playback(),
        Arc::clone(&ffmpeg),
        contents.clone(),
        probing.clone(),
        subtitles.clone(),
        events.clone(),
    );
    let preview_frames = PreviewFrames::new(
        Arc::clone(&db),
        images.clone(),
        Arc::clone(&ffmpeg),
        running.clone(),
        passes.follow(),
    );
    let text_passes = TextPasses::new(
        &db,
        Arc::clone(&ffmpeg),
        running.clone(),
        subtitles.clone(),
        reads.clone(),
        passes.follow(),
    );
    let requests = Requests::new(Arc::clone(&db), events.clone(), points.clone());
    let users =
        Users::new(Arc::clone(&db), points.clone()).linking_through(link::server_link(config, &db));
    let home_rows = HomeRows::new(Arc::clone(&db), points.clone(), requests.clone());
    let browser_spans = api::BrowserSpans::new(
        config.otlp_endpoint.as_deref(),
        settings.clone(),
        users.linking().clone(),
    );
    let custom_images = CustomImages::new(Arc::clone(&db), images.clone());
    let local_images = LocalImages::new(Arc::clone(&db), images.clone());
    let artwork = Artwork::new(images.clone(), points).resizing_ahead(
        settings.images(),
        custom_images.clone(),
        local_images.clone(),
    );
    let plugin_logos = PluginLogos::new(images.clone());
    let scanner = Scanner::new(
        Arc::clone(&db),
        plugins.clone(),
        artwork.clone(),
        custom_images.clone(),
        local_images,
        events.clone(),
        Some(probing.clone()),
        leftovers,
        running.clone(),
        requests.clone(),
    );
    let automation = Automation::serve(
        Arc::clone(&db),
        scanner.clone(),
        plugins.clone(),
        users.clone(),
        Backups::new(&config.data_dir, &config.file, settings.clone()),
    );
    let abilities = Abilities::new(Arc::clone(&ffmpeg));
    let status = status::Sources {
        db: Arc::clone(&db),
        contents: contents.clone(),
        ffmpeg: Arc::clone(&ffmpeg),
        abilities: abilities.clone(),
        running: running.clone(),
        data_dir: config.data_dir.clone(),
    };
    let server_url = settings.server_url();
    let url_saves = settings.url_saves();
    // `listen` differs from the bound address only when it asks for port 0.
    let listening = listener.local_addr().unwrap_or(config.listen);
    let syncing = sync::Syncing::new(Arc::clone(&db), plugins.extension_points(), users.clone());
    let linking = users.linking().clone();
    let (forwarded_port, relay) = (config.forwarded_port, config.relay);
    let certificate = certificate::Certificate::default();
    let (data_dir, acme_directory) = (config.data_dir.clone(), config.acme_directory.clone());
    let load_settings = load_settings(settings.clone(), images.clone(), plugins.clone());
    let app = api::router(
        config,
        listening,
        db,
        plugins.clone(),
        users,
        scanner.clone(),
        custom_images,
        plugin_logos,
        images,
        artwork,
        probing.clone(),
        subtitles,
        reads.clone(),
        events.clone(),
        requests.clone(),
        home_rows.clone(),
        settings,
        running.clone(),
        abilities.clone(),
        preview_frames.clone(),
        browser_spans.clone(),
        syncing.clone(),
        setup_code,
        ffmpeg,
    );
    #[cfg(feature = "web")]
    let app = app.layer(axum::middleware::from_fn(web::serve));
    async move {
        sharing.serve(Some(linking.clone())).await;
        log_jobs_time_zone();
        // Before anything can publish an event, so that no notification misses its link, and
        // before serving, so the sweeps delete no plan's files or upload but an earlier run's.
        load_settings.await;
        fill_sort_titles(&contents).await;
        // Before the scans start, so it sees their Matching finish.
        let scan_follower = syncing.follow_scans(&events);
        // Before serving, so it hears the first look at each Link.
        let link_follower = syncing.follow_links();
        scanner.start_all().await;
        preview_frames.start();
        text_passes.start();
        // Quiet long enough that a file being copied in is scanned once it's whole; every six hours
        // for the folders whose changes the OS doesn't report, such as network shares.
        if watch_folders {
            scanner.watch(Duration::from_secs(30), Duration::from_hours(6));
        }
        // After each probe pass, and hourly for a Version due again (ADR 0075).
        segments.start(probed_for_segments, Duration::from_hours(1));
        // Every minute, as Seerr asks how its Requests are progressing (ADR 0012).
        let came_up = home_rows.refresh_as_plugins_come_up(plugins.take_came_up());
        requests.start(came_up, Duration::from_mins(1));
        // Before serving, so the first client over TLS gets it, whether or not the Hub answers.
        certificate.load(&data_dir).await;
        let renewing = certificate::keep_renewed(
            certificate.clone(),
            linking.clone(),
            server_url.clone(),
            relay,
            data_dir,
            acme_directory,
        );
        let (relaying, relayed) = mpsc::channel(1);
        let dialing = relay.map(|relay| relay::keep_dialing(relay, linking.clone(), relaying));
        let telling_addresses = server_addresses::keep_telling(
            linking,
            server_url,
            url_saves,
            listening,
            forwarded_port,
            relay,
        );
        let recording_status = status::keep_recording(status);
        let closing = events.clone();
        let served = tls::serve(listener, relayed, app, Arc::new(certificate), async move {
            shutdown.await;
            // A subscription never ends by itself, and the shutdown waits for every response.
            closing.close();
        })
        .await;
        // Awaited, so its database handle is gone before the database closes.
        recording_status.abort();
        let _: Result<(), _> = recording_status.await;
        // First, so their ffmpeg doesn't start on a Version as the plans stop.
        preview_frames.shutdown().await;
        text_passes.shutdown().await;
        running.shutdown().await;
        abilities.shutdown().await;
        // The Scanner first, so a scan finishing now can't start a pass once Probing has stopped.
        scanner.shutdown().await;
        probing.shutdown().await;
        // After text passes and Probing, the last to start reads, so none runs on a closed
        // database.
        reads.shutdown().await;
        // Before the Plugin settings, as it asks Segment sources.
        segments.shutdown().await;
        // Before the Plugin settings, which save a Plugin that crashes as it is asked about progress.
        requests.shutdown().await;
        home_rows.shutdown().await;
        // Before the Plugins stop and the database closes, as a share in flight holds the Link.
        browser_spans.shutdown().await;
        // The same for the Server's own shares, and so the caller's handle on the database is the
        // last once this ends: the exporters sharing through `sharing` outlive serving.
        sharing.serve(None).await;
        // Telling the Hub again at the next start loses nothing.
        telling_addresses.abort();
        renewing.abort();
        // The Relay drops the connections waiting there once they close.
        if let Some(dialing) = dialing {
            dialing.abort();
        }
        // The next start's scans pull and sync again, and its first looks push each Link again.
        scan_follower.abort();
        link_follower.abort();
        plugins.shutdown().await;
        // Last, once no Plugin can call the Kernel: the Plugins hold it only weakly.
        drop(automation);
        // Once nothing publishes, so the inboxes miss no notification, and before the database
        // closes.
        events.shutdown().await;
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

/// Reads the Server settings, logging a failure and going on with the defaults, deletes what goes
/// as Plugins are found uninstalled, has `images` keep its originals in the store they name, then
/// sweeps the segment files and uploads an earlier run left, and the Plugins' logos uploaded
/// before #1750.
async fn load_settings(settings: Settings, images: ImageStore, plugins: PluginSettings) {
    if let Err(err) = settings.load().await {
        tracing::error!(
            "reading the Server settings failed, so each is its default and notifications have \
             no links: {err}"
        );
    }
    plugins.forget_uninstalled(&settings).await;
    if let Some(plugin) = settings.now().image_store {
        let blobs = PluginBlobs::new(plugins.extension_points(), plugin.clone());
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
    // A Plugin's logo was a Custom Image before #1750, whose migration dropped the rows but can't
    // reach the store, so this goes after `keep_in`.
    // ponytail: lists the folder on every start; drop it once no Server upgrades from before #1750.
    if let Err(err) = images.delete_dir(&[CUSTOM_IMAGES_DIR, "plugin"]).await {
        tracing::warn!("could not delete the Plugins' former Custom Images: {err}");
    }
}
