//! Signed playback URLs (ADR 0105): a caller with a bearer token gets URLs a `<video>`, an HLS
//! player and a subtitle `<track>` load with no cookie or header, each for its one Version or plan
//! and for a few minutes.

use super::*;

impl Fixture {
    /// The Admin's Playback Plan for the Version `version` on [`chrome`], asked with a bearer
    /// token.
    async fn plan_as_bearer(&self, version: i64) -> PlaybackPlan {
        self.ask_as_bearer(PlanPlaybackRequest {
            version_id: version,
            device_decoding: chrome().into(),
            ..Default::default()
        })
        .await
    }

    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    async fn ask_as_bearer(&self, request: PlanPlaybackRequest) -> PlaybackPlan {
        ask_for_plan_as(self.server.bearer_client_config(), request)
            .await
            .unwrap()
    }

    /// The status of a `GET` for `url` with no cookie or header.
    async fn status(&self, url: &str) -> u16 {
        fetch(&self.server, url, None).await.status
    }
}

/// `url`, signed, with the media path it was signed for replaced by `path`, such as
/// `versions/7`, so it keeps the signature but asks for another path.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn signature_moved_to(url: &str, path: &str) -> String {
    let (signature, _) = url.rsplit_once("/versions/").unwrap();
    format!("{signature}/{path}")
}

#[tokio::test]
async fn a_bearer_callers_direct_play_loads_with_no_session() {
    let fx = Fixture::alien().await;
    let version = fx.version_ids(ALIEN).await[0];

    let plan = fx.plan_as_bearer(version).await;

    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY);
    assert_ne!(plan.url, format!("/media/versions/{version}"));
    let played = fetch(&fx.server, &plan.url, None).await;
    assert_eq!((played.status, played.body), (200, b"video".to_vec()));
}

#[tokio::test]
async fn a_bearer_callers_remux_playlist_and_segments_load_with_no_session() {
    let (fx, version) = Fixture::alien_mkv(system_ffmpeg(), "aac").await;

    let plan = fx.plan_as_bearer(version).await;

    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_REMUX);
    for url in [
        &plan.url,
        &beside(&plan.url, "init.mp4"),
        &beside(&plan.url, "0.m4s"),
    ] {
        assert_eq!(fx.status(url).await, 200, "{url}");
    }
}

#[tokio::test]
async fn a_bearer_callers_subtitles_load_with_no_session() {
    let fx = Fixture::subtitled_alien().await;
    let version = fx.version_ids(ALIEN).await[0];

    let plan = fx.plan_as_bearer(version).await;

    assert_ne!(plan.subtitles, []);
    for subtitle in &plan.subtitles {
        assert_eq!(fx.status(&subtitle.url).await, 200, "{}", subtitle.url);
    }
}

// The fake ffmpeg and ffprobe are shell scripts.
#[cfg(unix)]
#[tokio::test]
async fn a_bearer_callers_ass_subtitle_and_its_fonts_load_with_no_session() {
    let bin = tempfile::tempdir().unwrap();
    let ffmpeg = ffmpeg_reading_ass_and_a_font(bin.path(), &bin.path().join("runs.log"));
    let fx = Fixture::start_with_ffmpeg(LibraryKind::LIBRARY_KIND_MOVIES, ffmpeg).await;
    fx.write(ALIEN_1080P, b"video");
    fx.scan_and_match().await;
    let version = fx.version_ids(ALIEN).await[0];

    let plan = (fx.ask_as_bearer(PlanPlaybackRequest {
        version_id: version,
        device_decoding: DeviceDecoding {
            subtitle_formats: vec![SubtitleFormat::SUBTITLE_FORMAT_ASS.into()],
            ..chrome()
        }
        .into(),
        ..Default::default()
    }))
    .await;

    let [ass] = plan.subtitles.as_slice() else {
        panic!("not one subtitle: {:?}", plan.subtitles);
    };
    assert_eq!(ass.format, SubtitleFormat::SUBTITLE_FORMAT_ASS);
    assert_eq!(ass.font_urls.len(), 1);
    for url in ass.font_urls.iter().chain([&ass.url]) {
        assert_eq!(fx.status(url).await, 200, "{url}");
    }
}

#[tokio::test]
async fn a_signed_url_loads_neither_another_version_or_plan_nor_its_versions_download() {
    let fx = Fixture::alien().await;
    let version = fx.version_ids(ALIEN).await[0];
    let plan = fx.plan_as_bearer(version).await;

    let other_version = signature_moved_to(&plan.url, &format!("versions/{}", version + 1));
    let its_download = signature_moved_to(&plan.url, &format!("downloads/{version}"));
    let other_plan = signature_moved_to(&plan.url, &format!("plans/{}/index.m3u8", plan.id + 1));

    // Signed, the unknown Version and plan would be 404, and the download 409.
    for url in [other_version, its_download, other_plan] {
        assert_eq!(fx.status(&url).await, 401, "{url}");
    }
}

#[tokio::test]
async fn a_signed_playback_url_loads_for_a_few_minutes_and_not_once_it_expires() {
    let fx = Fixture::alien_on(TestServer::builder().moves_clock()).await;
    let version = fx.version_ids(ALIEN).await[0];
    let plan = fx.plan_as_bearer(version).await;

    fx.server.advance_clock(Duration::from_mins(4));
    let within = fx.status(&plan.url).await;
    fx.server.advance_clock(Duration::from_mins(7));
    let after = fx.status(&plan.url).await;

    assert_eq!((within, after), (200, 401));
}

#[tokio::test]
async fn a_signed_playback_url_outlasts_its_minutes_while_its_plan_plays_and_not_once_it_stops() {
    let fx = Fixture::alien_on(TestServer::builder().moves_clock()).await;
    let version = fx.version_ids(ALIEN).await[0];
    let plan = fx.plan_as_bearer(version).await;

    // Each fetch keeps the plan, which stops a minute after the last.
    let mut playing = vec![];
    for _ in 0..24 {
        fx.server.advance_clock(Duration::from_secs(30));
        playing.push(fx.status(&plan.url).await);
    }
    fx.server.advance_clock(Duration::from_mins(2));
    let stopped = fx.status(&plan.url).await;

    assert_eq!((playing, stopped), (vec![200; 24], 401));
}
