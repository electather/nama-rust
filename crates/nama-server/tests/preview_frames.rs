//! Preview frames (ADR 0076): made in the background after probing, sent on the Playback Plan,
//! served as sheets to a signed-in Device, and deleted with their Version.

#[path = "support/browsing.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod browsing;
#[path = "support/http.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod http;
#[path = "support/server.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod server;

use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

use browsing::{ALIEN_1080P, Fixture};
use connectrpc::client::{ClientConfig, HttpClient};
use http::{Reply, fetch};
use nama_proto::nama::v1::{
    Container, DeleteMediaRequest, DeviceDecoding, GetLibraryRequest, LibraryKind,
    LibraryServiceClient, MediaToDelete, PlanPlaybackRequest, PlaybackPlan, PlaybackServiceClient,
    PreviewFrames, StopPlaybackRequest, UpdateLibraryRequest, VideoCodec, media_to_delete,
};
use nama_server::{
    config::Config,
    ffmpeg::{Ffmpeg, write_17_minutes_h264},
};
use server::tmdb::ALIEN;

/// How long making a short fixture's Preview frames may take.
const MAKING: Duration = Duration::from_secs(60);

/// How long a test waits to see that none are made.
const NONE_MADE: Duration = Duration::from_secs(3);

/// A Server with ffmpeg, its Movies Library holding 17 minutes of Alien, and that Version's id.
struct Frames {
    fx: Fixture,
    version: i64,
}

#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a test helper fails its test by panicking"
)]
impl Frames {
    /// Writes Alien into a new Library with Preview frames `on` or off, then scans and probes it.
    async fn start(on: bool) -> Self {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let ffmpeg = Ffmpeg::find(&Config::default(), &path);
        assert!(
            ffmpeg.ffmpeg.is_ok() && ffmpeg.ffprobe.is_ok(),
            "this test needs ffmpeg and ffprobe on PATH"
        );
        let fx = Fixture::start_with_ffmpeg(LibraryKind::LIBRARY_KIND_MOVIES, ffmpeg).await;
        let frames = Self { fx, version: 0 };
        frames.switch(on).await;
        let file = frames.fx.folder.join(ALIEN_1080P);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        write_17_minutes_h264(&file).unwrap();
        frames.fx.server.scan_and_probe(&frames.fx.media).await;
        let version = frames.fx.version_ids(ALIEN).await[0];
        Self { version, ..frames }
    }

    fn libraries(&self) -> LibraryServiceClient<HttpClient> {
        LibraryServiceClient::new(HttpClient::plaintext(), self.fx.server.client_config())
    }

    fn playback(&self) -> PlaybackServiceClient<HttpClient> {
        PlaybackServiceClient::new(HttpClient::plaintext(), self.fx.server.client_config())
    }

    /// Turns the Library's Preview frames on or off.
    async fn switch(&self, on: bool) {
        let library = (self.libraries())
            .get_library(GetLibraryRequest {
                id: self.fx.library,
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned()
            .library
            .into_option()
            .unwrap();
        (self.libraries())
            .update_library(UpdateLibraryRequest {
                id: library.id,
                name: library.name,
                folders: library.folders,
                preview_frames: Some(on),
                ..Default::default()
            })
            .await
            .unwrap();
    }

    /// The Admin's Direct Play of the Version, on a browser that plays H.264 in Matroska.
    async fn plan(&self) -> PlaybackPlan {
        self.plan_as(self.fx.server.client_config()).await
    }

    /// [`Self::plan`], asked with the client settings `who`.
    async fn plan_as(&self, who: ClientConfig) -> PlaybackPlan {
        let device = DeviceDecoding {
            containers: vec![Container::CONTAINER_MATROSKA.into()],
            video_codecs: vec![VideoCodec::VIDEO_CODEC_H264.into()],
            ..Default::default()
        };
        let planned = PlaybackServiceClient::new(HttpClient::plaintext(), who)
            .plan_playback(PlanPlaybackRequest {
                version_id: self.version,
                device_decoding: device.into(),
                ..Default::default()
            })
            .await
            .unwrap();
        planned.into_owned().plan.into_option().unwrap()
    }

    async fn stop(&self, plan: &PlaybackPlan) {
        (self.playback())
            .stop_playback(StopPlaybackRequest {
                plan_id: plan.id,
                ..Default::default()
            })
            .await
            .unwrap();
    }

    /// The manifest a plan sends, stopping the plan so it doesn't hold up making them.
    async fn manifest(&self) -> Option<PreviewFrames> {
        let plan = self.plan().await;
        self.stop(&plan).await;
        plan.preview_frames.into_option()
    }

    /// Sheet `n`, fetched with `token`.
    async fn sheet(&self, n: usize, token: Option<&str>) -> Reply {
        let url = format!("/media/versions/{}/preview-frames/{n}.jpg", self.version);
        fetch(&self.fx.server, &url, token).await
    }

    /// Sheet `n`, fetched as the Admin.
    async fn admins_sheet(&self, n: usize) -> Reply {
        self.sheet(n, Some(&self.fx.server.admin_token)).await
    }

    /// Waits until the first sheet is served, failing after [`MAKING`].
    async fn wait_until_made(&self) {
        let started = Instant::now();
        while self.admins_sheet(0).await.status != 200 {
            assert!(started.elapsed() < MAKING, "no Preview frames were made");
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// The Version's folder of sheets in the default blob store, the data directory.
    fn stored(&self) -> PathBuf {
        let folder = self.version.to_string();
        (self.fx.server.data_dir())
            .join("preview-frames")
            .join(folder)
    }
}

#[tokio::test]
async fn preview_frames_are_made_after_probing_and_sent_on_the_plan() {
    let frames = Frames::start(true).await;

    frames.wait_until_made().await;

    // 17 minutes at one every 10 s, from 0 s to 1010 s: 102, on two sheets.
    let expected = PreviewFrames {
        interval_ms: 10_000,
        width: 320,
        height: 240,
        columns: 10,
        rows: 10,
        count: 102,
        ..Default::default()
    };
    assert_eq!(frames.manifest().await, Some(expected));
    let first = frames.admins_sheet(0).await;
    assert_eq!(first.headers["content-type"], "image/jpeg");
    assert_eq!(first.body.get(..2), Some(&[0xFF, 0xD8][..]), "not a JPEG");
    assert_eq!(frames.admins_sheet(1).await.status, 200);
    assert_eq!(frames.admins_sheet(2).await.status, 404);
}

#[tokio::test]
async fn a_sheet_is_served_to_a_signed_in_device_and_refused_without_a_session() {
    let frames = Frames::start(true).await;
    frames.wait_until_made().await;
    let ada = frames.fx.server.user("ada", &[]).await;

    assert_eq!(frames.sheet(0, Some(&ada.token)).await.status, 200);
    assert_eq!(frames.sheet(0, None).await.status, 401);
}

/// An `<img>` can't send a bearer token, so a bearer caller fetches a sheet after the signed
/// prefix of its plan's URL, which covers the Version's media (ADR 0105).
#[tokio::test]
async fn a_bearer_callers_sheet_loads_under_its_plans_signed_prefix() {
    let frames = Frames::start(true).await;
    frames.wait_until_made().await;

    let plan = frames
        .plan_as(frames.fx.server.bearer_client_config())
        .await;

    let (prefix, _) = plan.url.split_once("versions/").unwrap();
    let sheet = format!("{prefix}versions/{}/preview-frames/0.jpg", frames.version);
    assert_ne!(prefix, "/media/");
    assert_eq!(fetch(&frames.fx.server, &sheet, None).await.status, 200);
}

#[tokio::test]
async fn none_are_made_while_a_plan_plays() {
    let frames = Frames::start(false).await;
    let playing = frames.plan().await;
    assert_ne!(playing.id, 0, "the plan doesn't play: {playing:?}");
    frames.switch(true).await;
    frames.fx.server.scan_and_probe(&frames.fx.media).await;

    tokio::time::sleep(NONE_MADE).await;
    assert_eq!(frames.admins_sheet(0).await.status, 404);
    assert_eq!(frames.manifest().await, None);

    frames.stop(&playing).await;
    frames.wait_until_made().await;
}

#[tokio::test]
async fn none_are_made_for_a_library_with_them_off() {
    let frames = Frames::start(false).await;

    tokio::time::sleep(NONE_MADE).await;

    assert_eq!(frames.admins_sheet(0).await.status, 404);
    assert_eq!(frames.manifest().await, None);
}

#[tokio::test]
async fn deleting_a_version_deletes_its_preview_frames() {
    let frames = Frames::start(true).await;
    frames.wait_until_made().await;
    assert!(frames.stored().is_dir());

    let deleted = (frames.libraries())
        .delete_media(DeleteMediaRequest {
            target: MediaToDelete {
                target: Some(media_to_delete::Target::VersionId(frames.version)),
                ..Default::default()
            }
            .into(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned();

    assert!(deleted.not_deleted.is_empty(), "{deleted:?}");
    assert!(!frames.stored().exists());
    assert_eq!(frames.admins_sheet(0).await.status, 404);
}
