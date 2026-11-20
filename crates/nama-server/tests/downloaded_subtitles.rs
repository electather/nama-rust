//! Subtitles a User downloads for a Version from a subtitle source (ADR 0067), through the
//! generated `PlaybackService` client: who may, and how a download plays and goes with its
//! Version.

#[path = "support/browsing.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod browsing;
#[path = "support/http.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod http;
#[path = "support/refused.rs"]
mod refused;
#[path = "support/server.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod server;

use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex, PoisonError},
};

use browsing::{ALIEN_1080P, Fixture};
use connectrpc::{ConnectError, ErrorCode, client::HttpClient};
use http::fetch;
use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    contract::{SubtitleDownload, SubtitleFile, SubtitleResult, SubtitleResults, SubtitleSearch},
    subtitle_source::SubtitleSource,
};
use nama_proto::nama::v1::{
    AudioCodec, Container, DeleteMediaRequest, DeviceDecoding, DownloadSubtitleRequest,
    FoundSubtitle, GetServerInfoRequest, LibraryKind, LibraryServiceClient, MediaToDelete,
    Permission, PlanPlaybackRequest, PlaybackPlan, PlaybackServiceClient, RefusalReason,
    SearchSubtitlesRequest, ServerServiceClient, VideoCodec, media_to_delete,
};
use nama_server::{
    config::Config,
    ffmpeg::{Ffmpeg, write_1080p_h264},
    plugins::Registry,
};
use refused::assert_refused;
use server::{TestServer, tmdb::ALIEN};

/// The Plugin the fake subtitle source is.
const SOURCE: &str = "fake-subtitles";

/// What the fake source's one subtitle says, as SRT.
const SAYS: &str = "1\n00:00:00,000 --> 00:00:01,000\nDownloaded hello\n";

/// A subtitle source that finds one English SDH SRT for anything, downloads it, and keeps what it
/// was asked.
#[derive(Clone, Default)]
struct FakeSource {
    asked: Arc<Mutex<Vec<SubtitleSearch>>>,
}

impl Plugin for FakeSource {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            extension_points: vec![PointKey::provide::<dyn SubtitleSource>()],
            ..Manifest::new(PluginId::from_static(SOURCE))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.provide::<dyn SubtitleSource>(Priority::DEFAULT, Arc::new(self.clone()));
        Ok(())
    }
}

impl SubtitleSource for FakeSource {
    fn search<'a>(
        &'a self,
        search: &'a SubtitleSearch,
    ) -> BoxFuture<'a, Result<SubtitleResults, PluginError>> {
        let mut asked = self.asked.lock().unwrap_or_else(PoisonError::into_inner);
        asked.push(search.clone());
        Box::pin(async {
            Ok(SubtitleResults {
                results: vec![SubtitleResult {
                    id: "hello-en".to_owned(),
                    language: "en".to_owned(),
                    name: "Alien.1979.1080p.BluRay".to_owned(),
                    sdh: true,
                    ..Default::default()
                }],
                ..Default::default()
            })
        })
    }

    fn download<'a>(
        &'a self,
        _download: &'a SubtitleDownload,
    ) -> BoxFuture<'a, Result<SubtitleFile, PluginError>> {
        Box::pin(async {
            Ok(SubtitleFile {
                data: SAYS.as_bytes().to_vec(),
                format: "srt".to_owned(),
                ..Default::default()
            })
        })
    }
}

/// A subtitle source whose search panics, as a buggy Plugin's might.
struct PanickingSource;

impl Plugin for PanickingSource {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            extension_points: vec![PointKey::provide::<dyn SubtitleSource>()],
            ..Manifest::new(PluginId::from_static("panicking-subtitles"))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.provide::<dyn SubtitleSource>(Priority::DEFAULT, Arc::new(Self));
        Ok(())
    }
}

impl SubtitleSource for PanickingSource {
    #[expect(clippy::panic, reason = "it tests a source that panics")]
    fn search<'a>(
        &'a self,
        _search: &'a SubtitleSearch,
    ) -> BoxFuture<'a, Result<SubtitleResults, PluginError>> {
        Box::pin(async { panic!("the source's search is broken") })
    }

    fn download<'a>(
        &'a self,
        _download: &'a SubtitleDownload,
    ) -> BoxFuture<'a, Result<SubtitleFile, PluginError>> {
        Box::pin(async { Err("it finds nothing to download".into()) })
    }
}

/// A Server with the fake subtitle source enabled, holding Alien in 1080p, scanned and Matched,
/// and that Version's id.
struct Watching {
    fx: Fixture,
    source: FakeSource,
    version: i64,
}

#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a test helper fails its test by panicking"
)]
impl Watching {
    async fn start() -> Self {
        let source = FakeSource::default();
        Self::start_with(source.clone(), Arc::new(source)).await
    }

    /// [`Self::start`] with `enabled` as the one subtitle source, keeping `source` to look at.
    async fn start_with(source: FakeSource, enabled: Arc<dyn Plugin>) -> Self {
        let plugins = Registry::new();
        plugins.enable(enabled).unwrap();
        let path = std::env::var_os("PATH").unwrap_or_default();
        let ffmpeg = Ffmpeg::find(&Config::default(), &path);
        let builder = TestServer::builder().ffmpeg(ffmpeg).plugins(plugins);
        let fx = Fixture::start_on(LibraryKind::LIBRARY_KIND_MOVIES, builder).await;
        let file = fx.folder.join(ALIEN_1080P);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        write_1080p_h264(&file).unwrap();
        // Not just Matching: Windows refuses to delete Alien while ffprobe still reads it.
        fx.server.scan_and_probe(&fx.media).await;
        let version = fx.version_ids(ALIEN).await[0];
        Self {
            fx,
            source,
            version,
        }
    }

    fn playback(&self) -> PlaybackServiceClient<HttpClient> {
        PlaybackServiceClient::new(HttpClient::plaintext(), self.fx.server.client_config())
    }

    /// What the sources find for the Version in English, searched as `playback`'s caller.
    async fn search_as(
        &self,
        playback: &PlaybackServiceClient<HttpClient>,
    ) -> Result<Vec<FoundSubtitle>, ConnectError> {
        let found = playback
            .search_subtitles(SearchSubtitlesRequest {
                version_id: self.version,
                languages: vec!["en".to_owned()],
                ..Default::default()
            })
            .await?;
        Ok(found.into_owned().subtitles)
    }

    /// Downloads `subtitle` for the Version as `playback`'s caller.
    async fn download_as(
        &self,
        playback: &PlaybackServiceClient<HttpClient>,
        subtitle: FoundSubtitle,
    ) -> Result<(), ConnectError> {
        playback
            .download_subtitle(DownloadSubtitleRequest {
                version_id: self.version,
                subtitle: subtitle.into(),
                ..Default::default()
            })
            .await?;
        Ok(())
    }

    /// Searches as the Admin and downloads the first subtitle found.
    async fn download_first(&self) {
        let found = self.search_as(&self.playback()).await.unwrap();
        self.download_as(&self.playback(), found[0].clone())
            .await
            .unwrap();
    }

    /// The Admin's Playback Plan for the Version, on a browser that plays H.264 in MP4.
    async fn plan(&self) -> PlaybackPlan {
        let device = DeviceDecoding {
            containers: vec![Container::CONTAINER_MP4.into()],
            video_codecs: vec![VideoCodec::VIDEO_CODEC_H264.into()],
            audio_codecs: vec![AudioCodec::AUDIO_CODEC_AAC.into()],
            mse: true,
            ..Default::default()
        };
        let planned = self
            .playback()
            .plan_playback(PlanPlaybackRequest {
                version_id: self.version,
                device_decoding: device.into(),
                ..Default::default()
            })
            .await
            .unwrap();
        planned.into_owned().plan.into_option().unwrap()
    }

    /// The text the Server serves at `url`, fetched as the Admin.
    async fn served_text(&self, url: &str) -> String {
        let server = &self.fx.server;
        let reply = fetch(server, url, Some(&server.admin_token)).await;
        assert_eq!(reply.status, 200, "{url}");
        String::from_utf8(reply.body).unwrap()
    }

    /// The Version's folder of downloads in the default blob store, the data directory.
    fn stored(&self) -> PathBuf {
        let folder = self.version.to_string();
        self.fx
            .server
            .data_dir()
            .join("downloaded-subtitles")
            .join(folder)
    }

    /// Sets the mode of the Library's folder, and of the Movie's folder in it, to `mode`.
    #[cfg(unix)]
    fn set_folder_mode(&self, mode: u32) {
        use std::os::unix::fs::PermissionsExt;
        let movie = self.fx.folder.join(ALIEN_1080P);
        for folder in [movie.parent().unwrap(), &self.fx.folder] {
            fs::set_permissions(folder, fs::Permissions::from_mode(mode)).unwrap();
        }
    }
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn searches_subtitles(server: &TestServer) -> bool {
    ServerServiceClient::new(HttpClient::plaintext(), server.anonymous_client_config())
        .get_server_info(GetServerInfoRequest::default())
        .await
        .unwrap()
        .into_owned()
        .searches_subtitles
}

#[tokio::test]
async fn the_server_says_whether_a_subtitle_source_is_enabled() {
    let plugins = Registry::new();
    plugins.enable(Arc::new(FakeSource::default())).unwrap();
    let with_source = TestServer::builder().plugins(plugins).start().await;
    let without = TestServer::builder().start().await;

    assert!(searches_subtitles(&with_source).await);
    assert!(!searches_subtitles(&without).await);
}

#[tokio::test]
async fn searching_and_downloading_subtitles_are_refused_without_download_subtitles() {
    let watching = Watching::start().await;
    let found = watching.search_as(&watching.playback()).await.unwrap();
    let grace = (watching.fx.server)
        .user("grace", &[Permission::PERMISSION_REQUEST])
        .await;
    let as_grace = PlaybackServiceClient::new(HttpClient::plaintext(), grace.client_config());

    assert_refused(
        watching.search_as(&as_grace).await,
        ErrorCode::PermissionDenied,
        RefusalReason::REFUSAL_REASON_PERMISSION_REQUIRED,
    );
    assert_refused(
        watching.download_as(&as_grace, found[0].clone()).await,
        ErrorCode::PermissionDenied,
        RefusalReason::REFUSAL_REASON_PERMISSION_REQUIRED,
    );
}

#[tokio::test]
async fn a_user_with_download_subtitles_finds_the_sources_subtitles_for_the_version() {
    let watching = Watching::start().await;
    let ada = (watching.fx.server)
        .user("ada", &[Permission::PERMISSION_DOWNLOAD_SUBTITLES])
        .await;
    let as_ada = PlaybackServiceClient::new(HttpClient::plaintext(), ada.client_config());

    let found = watching.search_as(&as_ada).await.unwrap();

    let found: Vec<_> = (found.iter())
        .map(|subtitle| {
            let (plugin, id) = (subtitle.plugin_id.as_str(), subtitle.id.as_str());
            (plugin, id, subtitle.language.as_str(), subtitle.sdh)
        })
        .collect();
    assert_eq!(found, [(SOURCE, "hello-en", "en", true)]);
    let asked = watching.source.asked.lock().unwrap().clone();
    let asked: Vec<_> = (asked.iter())
        .map(|search| {
            let ids: Vec<_> = (search.ids.iter())
                .map(|id| format!("{}-{}", id.namespace, id.value))
                .collect();
            (ids, search.file_name.as_str(), search.languages.clone())
        })
        .collect();
    assert_eq!(
        asked,
        [(
            vec!["imdb-tt0078748".to_owned(), format!("tmdb-{ALIEN}")],
            "Alien (1979) - 1080p.mkv",
            vec!["en".to_owned()]
        )]
    );
}

#[tokio::test]
async fn a_search_whose_every_source_panics_is_refused() {
    let watching = Watching::start_with(FakeSource::default(), Arc::new(PanickingSource)).await;

    assert_refused(
        watching.search_as(&watching.playback()).await,
        ErrorCode::Unavailable,
        RefusalReason::REFUSAL_REASON_NO_SUBTITLE_SOURCE,
    );
}

#[tokio::test]
async fn a_downloaded_subtitle_is_the_versions_last_track_and_plays() {
    let watching = Watching::start().await;

    watching.download_first().await;

    let plan = watching.plan().await;
    let downloaded = plan.subtitles.last().unwrap();
    let track = downloaded.track.as_option().unwrap();
    assert_eq!(
        (track.codec.as_str(), track.language.as_deref(), track.sdh),
        ("subrip", Some("en"), true)
    );
    assert_eq!(track.title.as_deref(), Some("Alien.1979.1080p.BluRay"));
    let served = watching.served_text(&downloaded.url).await;
    assert!(served.contains("Downloaded hello"), "{served}");
}

#[cfg(unix)]
#[tokio::test]
async fn a_subtitle_downloads_for_a_version_in_a_read_only_library() {
    let watching = Watching::start().await;
    watching.set_folder_mode(0o555);

    watching.download_first().await;

    let plan = watching.plan().await;
    watching.set_folder_mode(0o755);
    let url = &plan.subtitles.last().unwrap().url;
    assert!(watching.served_text(url).await.contains("Downloaded hello"));
}

#[tokio::test]
async fn deleting_a_version_deletes_its_downloaded_subtitles() {
    let watching = Watching::start().await;
    watching.download_first().await;
    assert!(watching.stored().is_dir());

    let deleted =
        LibraryServiceClient::new(HttpClient::plaintext(), watching.fx.server.client_config())
            .delete_media(DeleteMediaRequest {
                target: MediaToDelete {
                    target: Some(media_to_delete::Target::VersionId(watching.version)),
                    ..Default::default()
                }
                .into(),
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned();

    assert!(deleted.not_deleted.is_empty(), "{deleted:?}");
    assert!(!watching.stored().exists());
}
