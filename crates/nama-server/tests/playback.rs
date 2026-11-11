//! Playing Versions through the generated `PlaybackService` client, and serving them and their
//! subtitles, over a real socket, with the TMDB Plugin enabled against a fake TMDB.

#[path = "support/browsing.rs"]
mod browsing;
#[path = "support/http.rs"]
mod http;
#[path = "support/refused.rs"]
mod refused;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use browsing::{ALIEN_1080P, ALIEN_2160P, Fixture, tmdb_id};
use buffa::EnumValue;
use connectrpc::{
    ConnectError, ErrorCode,
    client::{ClientConfig, HttpClient},
};
use http::{fetch, fetch_range};
use nama_proto::nama::v1::{
    AudioCodec, ClearMatchRequest, Container, DeleteLibraryRequest, DeleteUserRequest,
    DeviceDecoding, EventType, HdrFormat, LibraryKind, LibraryServiceClient, ListEpisodesRequest,
    PickedTrackKind, PlanPlaybackRequest, Playback, PlaybackEnded, PlaybackLimit, PlaybackMethod,
    PlaybackPlan, PlaybackServiceClient, PlaybackSettings, PlaybackStarted, RefusalReason,
    ReportTrackPickRequest, ServerServiceClient, SetMatchRequest, StopPlaybackRequest,
    SubtitleFormat, Track, UpdatePlaybackSettingsRequest, UserServiceClient, Version, VideoCodec,
    VideoTrack,
};
#[cfg(unix)]
use nama_server::ffmpeg::fake::script;
use nama_server::{
    config::Config,
    ffmpeg::{self, Ffmpeg},
};
use refused::assert_refused;
use server::{
    Builder, SignedIn, TestServer, WAIT, subscribe,
    tmdb::{ALIEN, DARK},
};

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    /// Starts a Server with the ffmpeg on `PATH` and a Movies Library holding Alien, with English
    /// SDH and French Forced SRT subtitles, scanned and Matched.
    async fn subtitled_alien() -> Self {
        Self::alien_written_by(ALIEN_1080P, |path| {
            write_english_and_french_srt(path, "Bonjour");
        })
        .await
    }

    /// Starts a Server with the ffmpeg on `PATH` and a Movies Library holding Alien as `file`,
    /// which `write` writes, scanned and Matched.
    async fn alien_written_by(file: &str, write: impl FnOnce(&Path)) -> Self {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let ffmpeg = Ffmpeg::find(&Config::default(), &path);
        assert!(
            ffmpeg.direct_play_only_because().is_none(),
            "this test needs ffmpeg and ffprobe on PATH"
        );
        let fx = Self::start_with_ffmpeg(LibraryKind::LIBRARY_KIND_MOVIES, ffmpeg).await;
        let file = fx.folder.join(file);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        write(&file);
        fx.scan_and_match().await;
        fx
    }

    /// Fetches Alien's subtitles, so the Server caches them.
    async fn cache_subtitles(&self) {
        let version = *self.version_ids(ALIEN).await.first().unwrap();
        let token = Some(self.server.admin_token.as_str());
        for subtitle in self.plan(version).await.unwrap().subtitles {
            assert_eq!(fetch(&self.server, &subtitle.url, token).await.status, 200);
        }
        assert_ne!(self.cached_subtitles(), Vec::<PathBuf>::new());
    }

    /// What the Server's subtitle cache holds a folder for, as the Versions it caches.
    fn cached_subtitles(&self) -> Vec<PathBuf> {
        let versions = self.server.data_dir().join("subtitles/versions");
        fs::read_dir(versions).map_or_else(
            |_| Vec::new(),
            |entries| entries.map(|entry| entry.unwrap().path()).collect(),
        )
    }

    async fn scan_and_probe(&self) {
        self.server.scan_and_probe(&self.media).await;
    }

    /// The Admin's Playback Plan for the Version `version`, on [`chrome`].
    async fn plan(&self, version: i64) -> Result<PlaybackPlan, ConnectError> {
        self.plan_on_device(version, &[], 0).await
    }

    /// As [`Self::plan`], on a Device where the methods `failed` just failed to play it, and which
    /// plays `audio_channels` channels at once.
    async fn plan_on_device(
        &self,
        version: i64,
        failed: &[PlaybackMethod],
        audio_channels: u32,
    ) -> Result<PlaybackPlan, ConnectError> {
        let mut device = chrome();
        device.audio_channels = audio_channels;
        self.ask_for_plan(PlanPlaybackRequest {
            version_id: version,
            device_decoding: device.into(),
            failed_methods: failed.iter().copied().map(EnumValue::from).collect(),
            ..Default::default()
        })
        .await
    }

    /// As [`Self::plan`], from `start_position_ms`, with the audio Track at the place
    /// `audio_track` in the Version's Tracks.
    async fn plan_with_audio(
        &self,
        version: i64,
        audio_track: u32,
        start_position_ms: u64,
    ) -> Result<PlaybackPlan, ConnectError> {
        self.ask_for_plan(PlanPlaybackRequest {
            version_id: version,
            start_position_ms,
            audio_track: Some(audio_track),
            device_decoding: chrome().into(),
            ..Default::default()
        })
        .await
    }

    async fn ask_for_plan(
        &self,
        request: PlanPlaybackRequest,
    ) -> Result<PlaybackPlan, ConnectError> {
        ask_for_plan_as(self.server.client_config(), request).await
    }

    /// As [`Self::ask_for_plan`], through a proxy that says it relays for the address `client`.
    async fn ask_for_plan_relayed_for(
        &self,
        client: &str,
        request: PlanPlaybackRequest,
    ) -> Result<PlaybackPlan, ConnectError> {
        let config = (self.server.client_config()).with_default_header("x-forwarded-for", client);
        ask_for_plan_as(config, request).await
    }

    /// Sets the default Playback Limit for remote playback to `limit`, as the Admin.
    async fn set_remote_limit(&self, limit: PlaybackLimit) {
        ServerServiceClient::new(HttpClient::plaintext(), self.server.client_config())
            .update_playback_settings(UpdatePlaybackSettingsRequest {
                settings: PlaybackSettings {
                    remote_playback_limit: limit.into(),
                    max_transcodes: Some(2),
                    ..Default::default()
                }
                .into(),
                ..Default::default()
            })
            .await
            .unwrap();
    }
}

/// Where a browser's `<video>` plays the Version `version` from.
fn version_url(version: &Version) -> String {
    format!("/media/versions/{}", version.id)
}

#[tokio::test]
async fn a_version_is_served_whole_with_what_a_video_element_needs() {
    let fx = Fixture::alien().await;
    let movie = fx.get_movie(fx.library, ALIEN).await.unwrap();

    let reply = fetch(
        &fx.server,
        &version_url(&movie.versions[0]),
        Some(&fx.server.admin_token),
    )
    .await;

    assert_eq!(reply.status, 200);
    assert_eq!(reply.body, b"video");
    assert_eq!(reply.header("content-type"), "video/x-matroska");
    assert_eq!(reply.header("accept-ranges"), "bytes");
    assert_ne!(reply.header("last-modified"), "");
}

/// A `<video>` seeks by asking for a range of bytes.
#[tokio::test]
async fn a_range_of_a_version_is_served_as_exactly_those_bytes() {
    let fx = Fixture::alien().await;
    let movie = fx.get_movie(fx.library, ALIEN).await.unwrap();

    let reply = fetch_range(
        &fx.server,
        &version_url(&movie.versions[0]),
        Some(&fx.server.admin_token),
        Some("bytes=1-3"),
    )
    .await;

    assert_eq!(reply.status, 206);
    assert_eq!(reply.body, b"ide");
    assert_eq!(reply.header("content-range"), "bytes 1-3/5");
}

/// The session cookie is what lets a browser's `<video src>` play a Version (ADR 0014).
#[tokio::test]
async fn a_version_without_a_session_is_refused() {
    let fx = Fixture::alien().await;
    let movie = fx.get_movie(fx.library, ALIEN).await.unwrap();

    let reply = fetch(&fx.server, &version_url(&movie.versions[0]), None).await;

    assert_eq!(reply.status, 401);
}

#[tokio::test]
async fn an_unknown_version_is_not_found() {
    let fx = Fixture::alien().await;

    let reply = fetch(
        &fx.server,
        "/media/versions/999",
        Some(&fx.server.admin_token),
    )
    .await;

    assert_eq!(reply.status, 404);
}

/// A stored file is a Version only while it is Matched.
#[tokio::test]
async fn a_file_whose_match_was_cleared_is_not_found() {
    let fx = Fixture::alien().await;
    let movie = fx.get_movie(fx.library, ALIEN).await.unwrap();
    let item = fx.item(ALIEN_1080P);
    fx.matches
        .clear_match(ClearMatchRequest {
            item_id: item,
            ..Default::default()
        })
        .await
        .unwrap();

    let reply = fetch(
        &fx.server,
        &version_url(&movie.versions[0]),
        Some(&fx.server.admin_token),
    )
    .await;

    assert_eq!(reply.status, 404);
}

/// Versions are served by id, so a client can't name a file by its path.
#[tokio::test]
async fn a_path_in_place_of_a_version_id_is_a_bad_request() {
    let fx = Fixture::alien().await;

    let reply = fetch(
        &fx.server,
        "/media/versions/..%2F..%2Fnama.db",
        Some(&fx.server.admin_token),
    )
    .await;

    assert_eq!(reply.status, 400);
}

#[tokio::test]
async fn a_remux_writes_its_segments_to_the_segment_folder_an_admin_set() {
    let (fx, version) = Fixture::alien_mkv(system_ffmpeg(), "aac").await;
    let tmpfs = tempfile::tempdir().unwrap();
    ServerServiceClient::new(HttpClient::plaintext(), fx.server.client_config())
        .update_playback_settings(UpdatePlaybackSettingsRequest {
            settings: PlaybackSettings {
                segment_folder: tmpfs.path().to_str().unwrap().to_owned(),
                max_transcodes: Some(2),
                ..Default::default()
            }
            .into(),
            ..Default::default()
        })
        .await
        .unwrap();

    let plan = fx.plan(version).await.unwrap();

    ffmpegs_own_playlist(tmpfs.path(), &plan.url).await;
}

/// A Server whose ffprobe, a shell script in `bin`, reads every file as an H.264 + AAC MP4.
#[cfg(unix)]
fn ffprobe_reading_h264_and_aac(bin: &Path) -> Ffmpeg {
    let ffprobe = r#"#!/bin/sh
cat <<'EOF'
{"streams": [{"codec_name": "h264", "codec_type": "video"},
             {"codec_name": "aac", "codec_type": "audio", "disposition": {"default": 1}}],
 "format": {"format_name": "mov,mp4,m4a,3gp,3g2,mj2", "duration": "1.000000"}}
EOF
"#;
    Ffmpeg {
        ffprobe: Ok(script(bin, "ffprobe", ffprobe)),
        ..Ffmpeg::default()
    }
}

/// The codecs of `plan`'s Tracks.
fn codecs(plan: &PlaybackPlan) -> Vec<&str> {
    plan.tracks
        .iter()
        .map(|track| track.codec.as_str())
        .collect()
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn an_h264_and_aac_mp4_direct_plays_through_its_plans_url() {
    let bin = tempfile::tempdir().unwrap();
    let ffmpeg = ffprobe_reading_h264_and_aac(bin.path());
    let fx = Fixture::start_with_ffmpeg(LibraryKind::LIBRARY_KIND_MOVIES, ffmpeg).await;
    fx.write("Alien (1979)/Alien (1979).mp4", b"video");
    fx.scan_and_match().await;
    let version = fx.version_ids(ALIEN).await[0];

    let plan = fx.plan(version).await.unwrap();

    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY);
    assert_eq!(codecs(&plan), ["h264", "aac"]);
    assert_eq!(
        plan.reasons,
        [
            "Its Tracks need no change",
            "Its length is known",
            "This Device decodes its container and Tracks",
            "This Server lacks ffmpeg or ffprobe, so it can only Direct Play",
            "Nothing has failed to play it on this Device",
            "This Server isn't running as many Transcodes as it may at once",
            "It plays the file's own audio Track",
            "This Device is on the local network",
            "The Playback Limit is Original",
            "No subtitles are burned into its picture",
        ]
    );
    let played = fetch(&fx.server, &plan.url, Some(&fx.server.admin_token)).await;
    assert_eq!(played.body, b"video");
}

/// A Server whose ffmpeg and ffprobe are shell scripts in `bin`: ffprobe reads a file whose name
/// says 2160p as an HEVC and AAC MKV and any other as an H.264 and AAC one, and ffmpeg lists
/// `encoder` as its one encoder and no filters, at once, but writes nothing until it's stopped.
#[cfg(unix)]
fn ffmpeg_writing_nothing(bin: &Path, encoder: &str) -> Ffmpeg {
    let ffprobe = r#"#!/bin/sh
case "$*" in *2160p*) video=hevc ;; *) video=h264 ;; esac
cat <<EOF
{"streams": [{"codec_name": "$video", "codec_type": "video"},
             {"codec_name": "aac", "codec_type": "audio", "disposition": {"default": 1}}],
 "format": {"format_name": "matroska,webm", "duration": "1.000000"}}
EOF
"#;
    let ffmpeg = format!(
        "#!/bin/sh\ncase \"$*\" in *-encoders*) echo ' V....D {encoder}  H.264' ;; \
         *-filters*) ;; *) exec sleep 600 ;; esac\n"
    );
    Ffmpeg {
        ffmpeg: Ok(script(bin, "ffmpeg", &ffmpeg)),
        ffprobe: Ok(script(bin, "ffprobe", ffprobe)),
        gpus: vec![],
    }
}

#[cfg(unix)]
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    /// Starts a Server with [`ffmpeg_writing_nothing`] listing libx264 in `bin` that runs one Transcode at once,
    /// and a Movies Library holding Alien in 1080p and 2160p, and returns it with the ids of those
    /// Versions, which Chrome Remuxes and Transcodes.
    async fn alien_with_room_for_one_transcode(bin: &Path) -> (Self, [i64; 2]) {
        let fx = Self::start_with_ffmpeg(
            LibraryKind::LIBRARY_KIND_MOVIES,
            ffmpeg_writing_nothing(bin, "libx264"),
        )
        .await;
        fx.write(ALIEN_1080P, b"video");
        fx.write(ALIEN_2160P, b"video");
        fx.scan_and_match().await;
        ServerServiceClient::new(HttpClient::plaintext(), fx.server.client_config())
            .update_playback_settings(UpdatePlaybackSettingsRequest {
                settings: PlaybackSettings {
                    max_transcodes: Some(1),
                    ..Default::default()
                }
                .into(),
                ..Default::default()
            })
            .await
            .unwrap();
        let versions = fx.version_ids(ALIEN).await.try_into().unwrap();
        (fx, versions)
    }

    /// The Admin's Device's Transcode of the Version `version`.
    async fn transcode(&self, version: i64) -> PlaybackPlan {
        let plan = self.plan(version).await.unwrap();
        assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_TRANSCODE);
        plan
    }
}

/// `who`'s Playback Plan for the Version `version`, on their own Device, which is [`chrome`].
#[cfg(unix)]
async fn plan_as(who: &SignedIn, version: i64) -> Result<PlaybackPlan, ConnectError> {
    let request = PlanPlaybackRequest {
        version_id: version,
        device_decoding: chrome().into(),
        ..Default::default()
    };
    ask_for_plan_as(who.client_config(), request).await
}

// The fake ffmpeg and ffprobe are shell scripts.
#[cfg(unix)]
#[tokio::test]
async fn with_the_cap_at_1_a_second_device_that_needs_a_transcode_is_told_the_server_is_busy() {
    let bin = tempfile::tempdir().unwrap();
    let (fx, [_, hevc]) = Fixture::alien_with_room_for_one_transcode(bin.path()).await;
    let bishop = fx.server.user_with_default_permissions("Bishop").await;
    fx.transcode(hevc).await;

    let planned = plan_as(&bishop, hevc).await;

    let refusal = assert_refused(
        planned,
        ErrorCode::ResourceExhausted,
        RefusalReason::REFUSAL_REASON_SERVER_BUSY,
    );
    assert_eq!(
        refusal.params.get("maxTranscodes").map(String::as_str),
        Some("1")
    );
}

// The fake ffmpeg and ffprobe are shell scripts.
#[cfg(unix)]
#[tokio::test]
async fn with_the_cap_full_a_device_that_plays_a_remux_gets_one_and_the_plan_says_why() {
    let bin = tempfile::tempdir().unwrap();
    let (fx, [h264, hevc]) = Fixture::alien_with_room_for_one_transcode(bin.path()).await;
    let bishop = fx.server.user_with_default_permissions("Bishop").await;
    fx.transcode(hevc).await;

    let plan = plan_as(&bishop, h264).await.unwrap();

    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_REMUX);
    assert_eq!(
        plan.reasons.get(6).map(String::as_str),
        Some(
            "This Server is busy, running as many Transcodes as it may at once (1), so it can't \
             Transcode it"
        )
    );
}

// The fake ffmpeg and ffprobe are shell scripts.
#[cfg(unix)]
#[tokio::test]
async fn when_a_transcodes_session_ends_its_slot_frees_up_for_the_next() {
    let bin = tempfile::tempdir().unwrap();
    let (fx, [_, hevc]) = Fixture::alien_with_room_for_one_transcode(bin.path()).await;
    let bishop = fx.server.user_with_default_permissions("Bishop").await;
    let admins = fx.transcode(hevc).await;

    PlaybackServiceClient::new(HttpClient::plaintext(), fx.server.client_config())
        .stop_playback(StopPlaybackRequest {
            plan_id: admins.id,
            ..Default::default()
        })
        .await
        .unwrap();

    let plan = plan_as(&bishop, hevc).await.unwrap();
    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_TRANSCODE);
}

// ADR 0026. The fake ffmpeg and ffprobe are shell scripts.
#[cfg(unix)]
#[tokio::test]
async fn with_an_ffmpeg_lacking_libx264_a_version_needing_a_transcode_cant_be_played_and_says_why()
{
    let bin = tempfile::tempdir().unwrap();
    // As Fedora's ffmpeg-free lists it.
    let ffmpeg = ffmpeg_writing_nothing(bin.path(), "libopenh264");
    let fx = Fixture::start_with_ffmpeg(LibraryKind::LIBRARY_KIND_MOVIES, ffmpeg).await;
    fx.write(ALIEN_1080P, b"video");
    fx.write(ALIEN_2160P, b"video");
    fx.scan_and_match().await;
    let [h264, hevc] = fx.version_ids(ALIEN).await.try_into().unwrap();

    let needs_transcode = fx.plan(hevc).await.unwrap();
    let remux = fx.plan(h264).await.unwrap();

    assert_eq!(
        needs_transcode.method,
        PlaybackMethod::PLAYBACK_METHOD_UNSPECIFIED
    );
    assert_eq!(
        needs_transcode.reasons[3],
        "This Server's ffmpeg lacks libx264, so it can't Transcode"
    );
    assert_eq!(remux.method, PlaybackMethod::PLAYBACK_METHOD_REMUX);
}

// The fake ffmpeg and ffprobe are shell scripts.
#[cfg(unix)]
#[tokio::test]
async fn an_unprobed_version_whose_direct_play_failed_cant_be_played_and_starts_nothing() {
    let bin = tempfile::tempdir().unwrap();
    // With ffmpeg, the Server's own step would allow a Remux, so only the Tracks' step keeps it
    // out. Its filters and encoders are listed at once, as empty, so planning needn't wait.
    let ffmpeg_script =
        "#!/bin/sh\ncase \"$*\" in *-filters*|*-encoders*) ;; *) exec sleep 600 ;; esac\n";
    let ffmpeg = Ffmpeg {
        ffmpeg: Ok(script(bin.path(), "ffmpeg", ffmpeg_script)),
        ffprobe: Ok(script(bin.path(), "ffprobe", "#!/bin/sh\nexit 1\n")),
        gpus: vec![],
    };
    let fx = Fixture::start_with_ffmpeg(LibraryKind::LIBRARY_KIND_MOVIES, ffmpeg).await;
    fx.write(ALIEN_1080P, b"video");
    fx.scan_and_match().await;
    let version = fx.version_ids(ALIEN).await[0];
    let started = [EventType::EVENT_TYPE_PLAYBACK_STARTED];
    let mut subscription = subscribe(&fx.server.client_config(), &started).await;
    let direct_play = [PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY];

    let unplayable = fx.plan_on_device(version, &direct_play, 0).await.unwrap();

    assert_eq!(
        unplayable.method,
        PlaybackMethod::PLAYBACK_METHOD_UNSPECIFIED
    );
    assert_eq!((unplayable.id, unplayable.url.as_str()), (0, ""));
    // The next event is the Direct Play's, so the unplayable plan published none.
    let direct = fx.plan(version).await.unwrap();
    let told = PlaybackStarted {
        playback: admins_playback(version, direct).into(),
        ..Default::default()
    };
    assert_eq!(subscription.next().await, told.into());
}

// ADR 0026.
#[tokio::test]
async fn without_ffprobe_a_version_direct_plays_and_its_plan_says_its_tracks_arent_known() {
    let fx = Fixture::alien().await;
    let version = fx.version_ids(ALIEN).await[0];

    let plan = fx.plan(version).await.unwrap();

    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY);
    assert_eq!(plan.url, format!("/media/versions/{version}"));
    assert_eq!(plan.tracks, []);
    assert_eq!(
        plan.reasons,
        [
            "Its Tracks aren't known, so they're taken to need no change, and it can only be \
             Direct Played",
            "Its length isn't known, which a Remux's or a Transcode's playlist needs, so it can \
             only be Direct Played",
            "Its Tracks aren't known, so this Device is taken to decode them",
            "This Server lacks ffmpeg or ffprobe, so it can only Direct Play",
            "Nothing has failed to play it on this Device",
            "This Server isn't running as many Transcodes as it may at once",
            "It plays the file's own audio Track",
            "This Device is on the local network",
            "The Playback Limit is Original",
            "No subtitles are burned into its picture",
        ]
    );
}

#[tokio::test]
async fn each_plan_tells_subscribers_its_playback_started() {
    let fx = Fixture::alien().await;
    let version = fx.version_ids(ALIEN).await[0];
    let started = [EventType::EVENT_TYPE_PLAYBACK_STARTED];
    let mut subscription = subscribe(&fx.server.client_config(), &started).await;

    let first = fx.plan(version).await.unwrap();
    let replacing = fx.plan(version).await.unwrap();

    for plan in [first, replacing] {
        let told = PlaybackStarted {
            playback: admins_playback(version, plan).into(),
            ..Default::default()
        };
        assert_eq!(subscription.next().await, told.into());
    }
}

#[tokio::test]
async fn a_direct_play_its_player_stops_tells_subscribers_its_playback_ended() {
    let fx = Fixture::alien().await;
    let version = fx.version_ids(ALIEN).await[0];
    let config = fx.server.client_config();
    let ended = [EventType::EVENT_TYPE_PLAYBACK_ENDED];
    let mut subscription = subscribe(&config, &ended).await;
    let plan = fx.plan(version).await.unwrap();
    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY);

    PlaybackServiceClient::new(HttpClient::plaintext(), config)
        .stop_playback(StopPlaybackRequest {
            plan_id: plan.id,
            ..Default::default()
        })
        .await
        .unwrap();

    let told = PlaybackEnded {
        playback: admins_playback(version, plan).into(),
        ..Default::default()
    };
    assert_eq!(subscription.next().await, told.into());
}

/// What Observers are told of the Admin playing the Version `version` by `plan`.
fn admins_playback(version: i64, plan: PlaybackPlan) -> Playback {
    Playback {
        // The first User and Device in a fresh database: the Admin's.
        user_id: 1,
        device_id: 1,
        version_id: version,
        method: plan.method,
        tracks: plan.tracks,
        reasons: plan.reasons,
        ..Default::default()
    }
}

/// What Chrome says it decodes, in part: H.264 and AAC in MP4, and HLS through Media Source
/// Extensions.
fn chrome() -> DeviceDecoding {
    DeviceDecoding {
        containers: vec![Container::CONTAINER_MP4.into()],
        video_codecs: vec![VideoCodec::VIDEO_CODEC_H264.into()],
        audio_codecs: vec![AudioCodec::AUDIO_CODEC_AAC.into()],
        mse: true,
        ..Default::default()
    }
}

/// The Playback Plan `request` asks for, asked as `who`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn ask_for_plan_as(
    who: ClientConfig,
    request: PlanPlaybackRequest,
) -> Result<PlaybackPlan, ConnectError> {
    let response = PlaybackServiceClient::new(HttpClient::plaintext(), who)
        .plan_playback(request)
        .await?
        .into_owned();
    Ok(response.plan.into_option().unwrap())
}

/// The ffmpeg and ffprobe on `PATH`, which these tests need.
fn system_ffmpeg() -> Ffmpeg {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let ffmpeg = Ffmpeg::find(&Config::default(), &path);
    assert!(
        ffmpeg.ffmpeg.is_ok() && ffmpeg.ffprobe.is_ok(),
        "this test needs ffmpeg and ffprobe on PATH"
    );
    ffmpeg
}

#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    /// Starts a Server with `ffmpeg` and a Movies Library holding Alien as an MKV of H.264 and 5.1
    /// audio encoded with `audio`, as [`ffmpeg::write_h264_with_5_1`] takes it, and returns it with
    /// that Version's id.
    async fn alien_mkv(ffmpeg: Ffmpeg, audio: &str) -> (Self, i64) {
        Self::alien_written(ffmpeg, |file| ffmpeg::write_h264_with_5_1(file, audio)).await
    }

    /// Starts a Server with `ffmpeg` and a Movies Library holding Alien as an MKV `write` writes,
    /// and returns it with that Version's id.
    async fn alien_written(
        ffmpeg: Ffmpeg,
        write: impl FnOnce(&Path) -> std::io::Result<()>,
    ) -> (Self, i64) {
        Self::alien_written_on(TestServer::builder().ffmpeg(ffmpeg), write).await
    }

    /// [`Self::alien_written`] on a Server `builder` starts.
    async fn alien_written_on(
        builder: Builder,
        write: impl FnOnce(&Path) -> std::io::Result<()>,
    ) -> (Self, i64) {
        let fx = Self::start_on(LibraryKind::LIBRARY_KIND_MOVIES, builder).await;
        let file = fx.folder.join("Alien (1979)/Alien (1979).mkv");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        write(&file).unwrap();
        fx.scan_and_match().await;
        let version = fx.version_ids(ALIEN).await[0];
        (fx, version)
    }

    /// The body of `path` on the Server, fetched as the Admin, which must be served.
    async fn fetch_ok(&self, path: &str) -> Vec<u8> {
        let reply = fetch(&self.server, path, Some(&self.server.admin_token)).await;
        assert_eq!(reply.status, 200, "{path}");
        reply.body
    }
}

/// Each segment's duration in a playlist, as written, and the files it lists.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn segments(playlist: &str) -> (Vec<f64>, Vec<&str>) {
    let durations = (playlist.lines())
        .filter_map(|line| line.strip_prefix("#EXTINF:"))
        .map(|rest| rest.trim_end_matches(',').parse().unwrap())
        .collect();
    let files = (playlist.lines())
        .filter(|line| !line.starts_with('#'))
        .collect();
    (durations, files)
}

/// `url` with its last part replaced by `file`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn beside(url: &str, file: &str) -> String {
    let (folder, _) = url.rsplit_once('/').unwrap();
    format!("{folder}/{file}")
}

/// The HLS playlist ffmpeg wrote to the segment folder `folder` for the plan at `url`, once it's
/// done.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn ffmpegs_own_playlist(folder: &Path, url: &str) -> String {
    let id = url.split('/').nth(3).unwrap();
    let path = folder.join(format!("nama-plan-{id}")).join("ffmpeg.m3u8");
    tokio::time::timeout(WAIT, async {
        loop {
            if let Ok(playlist) = fs::read_to_string(&path) {
                return playlist;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn an_h264_and_aac_mkv_is_remuxed_over_hls_with_its_audio_copied() {
    let (fx, version) = Fixture::alien_mkv(system_ffmpeg(), "aac").await;

    let plan = fx.plan(version).await.unwrap();

    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_REMUX);
    let reply = fetch(&fx.server, &plan.url, Some(&fx.server.admin_token)).await;
    assert_eq!(
        reply.header("content-type"),
        "application/vnd.apple.mpegurl"
    );
    let playlist = String::from_utf8(reply.body).unwrap();
    assert!(
        playlist.contains("#EXT-X-MAP:URI=\"init.mp4\""),
        "{playlist}"
    );
    assert!(playlist.ends_with("#EXT-X-ENDLIST\n"), "{playlist}");
    let (durations, files) = segments(&playlist);
    assert_eq!(files, ["0.m4s", "1.m4s", "2.m4s"]);
    let init = fx.fetch_ok(&beside(&plan.url, "init.mp4")).await;
    assert_eq!(
        init.get(4..8),
        Some(&b"ftyp"[..]),
        "an MP4 starts with its type"
    );
    for file in files {
        let segment = fx.fetch_ok(&beside(&plan.url, file)).await;
        assert_ne!(segment.len(), 0, "{file}");
    }
    assert_eq!(
        played_audio(&fx, &plan.url, "0.m4s").await,
        "aac,6",
        "copied, though this Device doesn't say it plays 5.1"
    );
    // HLS time is the file's time, so Watch State needs no conversion: the playlist cuts where
    // ffmpeg cut a file per keyframe, from the start of the file to its end.
    let default = fx.server.data_dir().join("transcodes");
    let (ffmpegs, _) = segments(&ffmpegs_own_playlist(&default, &plan.url).await);
    let ffmpegs_cuts = cuts(&ffmpegs);
    for cut in cuts(&durations) {
        assert!(
            ffmpegs_cuts.iter().any(|theirs| (cut - theirs).abs() < 0.1),
            "{durations:?} against {ffmpegs:?}"
        );
    }
}

/// Where segments lasting `durations` end.
fn cuts(durations: &[f64]) -> Vec<f64> {
    let ends = durations.iter().scan(0.0, |end, duration| {
        *end += duration;
        Some(*end)
    });
    ends.collect()
}

/// A Server whose ffprobe, a shell script in `bin`, writes the arguments of each run to `log`
/// and then runs the ffprobe on `PATH`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
#[cfg(unix)]
fn ffprobe_logging_its_runs(bin: &Path, log: &Path) -> Ffmpeg {
    let ffmpeg = system_ffmpeg();
    let real = ffmpeg.ffprobe.unwrap().path;
    let ffprobe = format!(
        "#!/bin/sh\necho \"$*\" >> '{}'\nexec '{}' \"$@\"\n",
        log.display(),
        real.display()
    );
    Ffmpeg {
        ffprobe: Ok(script(bin, "ffprobe", &ffprobe)),
        ..ffmpeg
    }
}

// The logging ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn remuxing_a_version_again_reuses_the_keyframes_listed_the_first_time() {
    let bin = tempfile::tempdir().unwrap();
    let log = bin.path().join("runs.log");
    let (fx, version) = Fixture::alien_mkv(ffprobe_logging_its_runs(bin.path(), &log), "aac").await;

    let first = fx.plan(version).await.unwrap();
    let second = fx.plan(version).await.unwrap();

    assert_ne!(first.url, second.url, "each plan runs on its own");
    assert_eq!(
        fx.fetch_ok(&first.url).await,
        fx.fetch_ok(&second.url).await
    );
    let runs = fs::read_to_string(&log).unwrap();
    let listings = runs.lines().filter(|run| run.contains("packet=pts_time"));
    assert_eq!(listings.count(), 1, "{runs}");
}

/// As for Direct Play, the session cookie is what lets a browser's HLS player fetch them.
#[tokio::test]
async fn a_remuxs_playlist_and_segments_without_a_session_are_refused() {
    let (fx, version) = Fixture::alien_mkv(system_ffmpeg(), "aac").await;
    let plan = fx.plan(version).await.unwrap();

    for path in [plan.url.clone(), beside(&plan.url, "0.m4s")] {
        let reply = fetch(&fx.server, &path, None).await;

        assert_eq!(reply.status, 401, "{path}");
    }
}

/// The codec and channels of the audio the plan at `url` plays in its segment `segment`, such as
/// `aac,6`, as ffprobe reads it with its init segment.
async fn played_audio(fx: &Fixture, url: &str, segment: &str) -> String {
    played_streams(fx, url, "a", segment, "codec_name,channels").await
}

/// The `entries` of the streams of type `kind`, as ffprobe's `-select_streams` and
/// `-show_entries stream=` take them, that the plan at `url` plays in its segment `segment`, as
/// ffprobe reads it with its init segment.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn played_streams(
    fx: &Fixture,
    url: &str,
    kind: &str,
    segment: &str,
    entries: &str,
) -> String {
    // The segment first, so ffmpeg starts there and the init segment is that run's.
    let segment = fx.fetch_ok(&beside(url, segment)).await;
    let mut mp4 = fx.fetch_ok(&beside(url, "init.mp4")).await;
    mp4.extend(segment);
    let file = tempfile::NamedTempFile::new().unwrap();
    fs::write(file.path(), mp4).unwrap();
    let output = std::process::Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", kind])
        .args([
            "-show_entries",
            &format!("stream={entries}"),
            "-of",
            "csv=p=0",
        ])
        .arg(file.path())
        .output()
        .unwrap();
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[tokio::test]
async fn dts_on_a_device_that_plays_stereo_is_played_as_stereo_aac() {
    let (fx, version) = Fixture::alien_mkv(system_ffmpeg(), "dca").await;

    let plan = fx.plan_on_device(version, &[], 2).await.unwrap();

    assert_eq!(played_audio(&fx, &plan.url, "0.m4s").await, "aac,2");
}

/// Writes 14 seconds of H.264 with English 5.1 AAC, played by default, and a stereo AAC
/// commentary, as Matroska, to `path`, with the ffmpeg on `PATH`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn write_english_and_a_stereo_commentary(path: &Path) {
    let status = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-y"])
        .args(["-f", "lavfi", "-i", "testsrc=size=320x240:rate=24"])
        .args(["-f", "lavfi", "-i", "sine,aformat=channel_layouts=5.1"])
        .args(["-f", "lavfi", "-i", "sine=frequency=880"])
        .args(["-map", "0", "-map", "1", "-map", "2", "-t", "14"])
        .args(["-c:v", "libx264", "-preset", "ultrafast", "-g", "48"])
        .args(["-c:a", "aac", "-ac:a:1", "2"])
        .args([
            "-metadata:s:a:0",
            "language=eng",
            "-disposition:a:0",
            "default",
        ])
        .args(["-metadata:s:a:1", "language=eng", "-disposition:a:1", "0"])
        .args(["-metadata:s:a:1", "title=Commentary", "-f", "matroska"])
        .arg(path)
        .status()
        .unwrap();
    assert!(status.success(), "ffmpeg failed: {status}");
}

#[tokio::test]
async fn switching_to_the_commentary_mid_play_remuxes_it_from_there_with_its_audio() {
    let fx = Fixture::alien_written_by(ALIEN_1080P, write_english_and_a_stereo_commentary).await;
    let version = fx.version_ids(ALIEN).await[0];

    let commentary = fx.plan_with_audio(version, 2, 7_000).await.unwrap();

    assert_eq!(commentary.method, PlaybackMethod::PLAYBACK_METHOD_REMUX);
    assert_eq!(commentary.audio_track, Some(2));
    // 7 seconds in is in the second segment, as they're cut about 6 seconds apart.
    assert_eq!(played_audio(&fx, &commentary.url, "1.m4s").await, "aac,2");
}

#[tokio::test]
async fn an_hevc_mkv_is_transcoded_to_h264_and_aac_over_hls_in_six_second_segments() {
    let (fx, version) = Fixture::alien_written(system_ffmpeg(), ffmpeg::write_hevc_with_5_1).await;

    let plan = fx.plan(version).await.unwrap();

    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_TRANSCODE);
    let playlist = String::from_utf8(fx.fetch_ok(&plan.url).await).unwrap();
    let (durations, files) = segments(&playlist);
    assert_eq!(files, ["0.m4s", "1.m4s", "2.m4s", "3.m4s"], "20 seconds");
    assert_eq!(durations[..3], [6.0, 6.0, 6.0]);
    for file in files {
        let segment = fx.fetch_ok(&beside(&plan.url, file)).await;
        assert_ne!(segment.len(), 0, "{file}");
    }
    let video = played_streams(&fx, &plan.url, "v", "0.m4s", "codec_name,width,height").await;
    assert_eq!(video, "h264,320,240");
    assert_eq!(played_audio(&fx, &plan.url, "0.m4s").await, "aac,2");
}

/// A Server behind a reverse proxy at the test's own address, which it trusts.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn behind_proxy() -> Builder {
    TestServer::builder().config(Config {
        trusted_proxies: vec!["127.0.0.1".parse().unwrap()],
        ..Config::default()
    })
}

/// A plan for `version` on Chrome, with the Playback Limit `picked`.
fn picking(version: i64, picked: PlaybackLimit) -> PlanPlaybackRequest {
    PlanPlaybackRequest {
        version_id: version,
        playback_limit: picked.into(),
        device_decoding: chrome().into(),
        ..Default::default()
    }
}

const REMOTE_720P: &str = "This Device is outside the local network, so the default Playback \
                           Limit for remote playback, 720p, applies";
const LOCAL: &str = "This Device is on the local network";

#[tokio::test]
async fn a_1080p_h264_mkv_played_from_outside_the_local_network_transcodes_to_the_remote_default() {
    let behind = behind_proxy().ffmpeg(system_ffmpeg());
    let (fx, version) = Fixture::alien_written_on(behind, ffmpeg::write_1080p_h264).await;
    fx.set_remote_limit(PlaybackLimit::PLAYBACK_LIMIT_720P)
        .await;
    let unpicked = picking(version, PlaybackLimit::PLAYBACK_LIMIT_UNSPECIFIED);
    let original = picking(version, PlaybackLimit::PLAYBACK_LIMIT_ORIGINAL);
    let remote = "203.0.113.7";

    let defaulted = fx
        .ask_for_plan_relayed_for(remote, unpicked.clone())
        .await
        .unwrap();
    let picked_original = fx.ask_for_plan_relayed_for(remote, original).await.unwrap();
    let local = fx.ask_for_plan(unpicked).await.unwrap();

    assert_eq!(defaulted.method, PlaybackMethod::PLAYBACK_METHOD_TRANSCODE);
    assert_eq!(defaulted.playback_limit, PlaybackLimit::PLAYBACK_LIMIT_720P);
    assert_eq!(
        played_streams(&fx, &defaulted.url, "v", "0.m4s", "width,height").await,
        "1280,720"
    );
    assert_eq!(
        picked_original.method,
        PlaybackMethod::PLAYBACK_METHOD_REMUX
    );
    assert_eq!(local.method, PlaybackMethod::PLAYBACK_METHOD_REMUX);
    assert_eq!(local.playback_limit, PlaybackLimit::PLAYBACK_LIMIT_ORIGINAL);
    assert_eq!(
        local.playback_limits,
        [
            PlaybackLimit::PLAYBACK_LIMIT_720P,
            PlaybackLimit::PLAYBACK_LIMIT_480P
        ]
    );
}

#[tokio::test]
async fn behind_a_trusted_proxy_the_forwarded_client_address_decides_whether_playback_is_remote() {
    let fx = Fixture::alien_on(behind_proxy()).await;
    fx.set_remote_limit(PlaybackLimit::PLAYBACK_LIMIT_720P)
        .await;
    let version = fx.version_ids(ALIEN).await[0];
    let unpicked = picking(version, PlaybackLimit::PLAYBACK_LIMIT_UNSPECIFIED);

    let remote = fx
        .ask_for_plan_relayed_for("203.0.113.7", unpicked.clone())
        .await
        .unwrap();
    let local = fx
        .ask_for_plan_relayed_for("192.168.1.20", unpicked)
        .await
        .unwrap();

    assert_eq!(remote.reasons[7], REMOTE_720P);
    assert_eq!(remote.playback_limit, PlaybackLimit::PLAYBACK_LIMIT_720P);
    assert_eq!(local.reasons[7], LOCAL);
    assert_eq!(local.playback_limit, PlaybackLimit::PLAYBACK_LIMIT_ORIGINAL);
}

#[tokio::test]
async fn behind_an_untrusted_proxy_the_peers_address_decides_whether_playback_is_remote() {
    let fx = Fixture::alien().await;
    fx.set_remote_limit(PlaybackLimit::PLAYBACK_LIMIT_720P)
        .await;
    let version = fx.version_ids(ALIEN).await[0];

    let plan = fx
        .ask_for_plan_relayed_for(
            "203.0.113.7",
            picking(version, PlaybackLimit::PLAYBACK_LIMIT_UNSPECIFIED),
        )
        .await
        .unwrap();

    assert_eq!(plan.reasons[7], LOCAL);
    assert_eq!(plan.playback_limit, PlaybackLimit::PLAYBACK_LIMIT_ORIGINAL);
}

#[tokio::test]
async fn a_pgs_track_turned_on_mid_play_is_burned_into_a_transcode_from_there() {
    let (fx, version) = Fixture::alien_written(system_ffmpeg(), ffmpeg::write_black_with_pgs).await;

    let plan = fx
        .ask_for_plan(PlanPlaybackRequest {
            version_id: version,
            start_position_ms: 7000,
            subtitle_track: Some(1),
            device_decoding: chrome().into(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_TRANSCODE);
    assert_eq!(
        (plan.subtitle_track, plan.burn_in_tracks),
        (Some(1), vec![1])
    );
    // The player asks first for the segment 7 seconds falls in, where ffmpeg starts.
    let picture = last_frame(&fx, &plan.url, "1.m4s").await;
    assert!(picture[108 * 320 + 108] > 200, "the square is burned in");
    assert!(
        picture[10 * 320 + 10] < 50,
        "the picture around it stays black"
    );
}

/// The last frame of the segment `segment` of the 320×240 plan at `url`, as 8-bit grey pixels row
/// by row.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn last_frame(fx: &Fixture, url: &str, segment: &str) -> Vec<u8> {
    // The segment first, so ffmpeg starts there and the init segment is that run's.
    let segment = fx.fetch_ok(&beside(url, segment)).await;
    let mut mp4 = fx.fetch_ok(&beside(url, "init.mp4")).await;
    mp4.extend(segment);
    let file = tempfile::NamedTempFile::new().unwrap();
    fs::write(file.path(), mp4).unwrap();
    let output = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(file.path())
        .args(["-f", "rawvideo", "-pix_fmt", "gray", "-"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    output
        .stdout
        .rchunks_exact(320 * 240)
        .next()
        .unwrap()
        .to_vec()
}

#[tokio::test]
async fn hdr10_transcoded_for_a_browser_on_an_hdr_screen_stays_hdr10_as_its_tracks_say() {
    let write = |file: &Path| ffmpeg::write_4k_with_5_1(file, "smpte2084");
    let (fx, version) = Fixture::alien_written(system_ffmpeg(), write).await;

    let plan = fx
        .ask_for_plan(PlanPlaybackRequest {
            version_id: version,
            device_decoding: DeviceDecoding {
                hdr: true,
                ..chrome()
            }
            .into(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_TRANSCODE);
    let video = Track {
        codec: "h264".to_owned(),
        kind: VideoTrack {
            width: 3840,
            height: 2160,
            frame_rate: 24.0,
            bit_depth: 8,
            hdr: HdrFormat::HDR_FORMAT_HDR10.into(),
            ..Default::default()
        }
        .into(),
        ..Default::default()
    };
    assert_eq!(plan.tracks.first(), Some(&video));
    let colours = "color_transfer,color_primaries";
    let played = played_streams(&fx, &plan.url, "v", "0.m4s", colours).await;
    assert_eq!(played, "smpte2084,bt2020");
}

#[tokio::test]
async fn a_version_whose_direct_play_just_failed_on_the_device_cant_be_played_and_says_why() {
    let fx = Fixture::alien().await;
    let version = fx.version_ids(ALIEN).await[0];

    let plan = fx
        .plan_on_device(version, &[PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY], 0)
        .await
        .unwrap();

    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_UNSPECIFIED);
    assert_eq!(plan.url, "");
    assert_eq!(plan.reasons[4], "Direct Play failed on this Device");
}

/// The pure tests in `src/playback.rs` cover what these choices do; this checks they reach it.
#[tokio::test]
async fn a_picked_playback_limit_and_a_failed_transcode_reach_the_plan() {
    let fx = Fixture::alien().await;
    let version = fx.version_ids(ALIEN).await[0];

    let plan = fx
        .ask_for_plan(PlanPlaybackRequest {
            failed_methods: vec![PlaybackMethod::PLAYBACK_METHOD_TRANSCODE.into()],
            ..picking(version, PlaybackLimit::PLAYBACK_LIMIT_480P)
        })
        .await
        .unwrap();

    assert_eq!(plan.reasons[4], "Transcode failed on this Device");
    assert_eq!(
        plan.reasons[8],
        "Its picture size isn't known, so it's taken to fit the 480p Playback Limit"
    );
}

#[tokio::test]
async fn planning_an_unknown_version_is_refused() {
    let fx = Fixture::alien().await;

    let planned = fx.plan(999).await;

    assert_refused(
        planned,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_VERSION_NOT_FOUND,
    );
}

/// Writes a second of video with English SDH and French Forced SRT subtitles, the French saying
/// `french`, as Matroska, to `path`, with the ffmpeg on `PATH`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn write_english_and_french_srt(path: &Path, french_says: &str) {
    let srt = tempfile::tempdir().unwrap();
    let (english, french) = (srt.path().join("eng.srt"), srt.path().join("fre.srt"));
    fs::write(&english, "1\n00:00:00,000 --> 00:00:01,000\nHello\n").unwrap();
    fs::write(
        &french,
        format!("1\n00:00:00,000 --> 00:00:01,000\n{french_says}\n"),
    )
    .unwrap();
    let status = std::process::Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=size=64x64:rate=24",
        ])
        .arg("-i")
        .arg(&english)
        .arg("-i")
        .arg(&french)
        .args([
            "-map", "0", "-map", "1", "-map", "2", "-t", "1", "-c:v", "ffv1", "-c:s", "srt",
        ])
        .args([
            "-metadata:s:s:0",
            "language=eng",
            "-metadata:s:s:0",
            "title=English SDH",
        ])
        .args(["-disposition:s:0", "hearing_impaired"])
        .args([
            "-metadata:s:s:1",
            "language=fre",
            "-disposition:s:1",
            "forced",
        ])
        .args(["-f", "matroska"])
        .arg(path)
        .status()
        .unwrap();
    assert!(status.success(), "ffmpeg failed: {status}");
}

#[tokio::test]
async fn an_mkvs_srt_tracks_are_offered_with_their_plan_and_served_as_webvtt() {
    let fx = Fixture::subtitled_alien().await;
    let version = fx.version_ids(ALIEN).await[0];

    let plan = fx.plan(version).await.unwrap();

    let offered: Vec<_> = (plan.subtitles.iter())
        .map(|subtitle| {
            let track = &subtitle.track;
            let (language, title) = (track.language.as_deref(), track.title.as_deref());
            (language, title, track.forced, track.sdh)
        })
        .collect();
    assert_eq!(
        offered,
        [
            (Some("eng"), Some("English SDH"), false, true),
            (Some("fre"), None, true, false),
        ]
    );
    let token = Some(fx.server.admin_token.as_str());
    let french = fetch(&fx.server, &plan.subtitles[1].url, token).await;
    assert_eq!(french.status, 200);
    assert_eq!(french.header("content-type"), "text/vtt; charset=utf-8");
    let french = String::from_utf8(french.body).unwrap();
    assert!(french.starts_with("WEBVTT"), "{french}");
    assert!(
        french.contains("00:00.000 --> 00:01.000\nBonjour"),
        "{french}"
    );
    let again = fetch(&fx.server, &plan.subtitles[1].url, token).await;
    assert_eq!(String::from_utf8(again.body).unwrap(), french);
}

#[tokio::test]
async fn a_replaced_file_probed_again_serves_its_own_subtitles_not_the_cached_ones() {
    let fx = Fixture::subtitled_alien().await;
    let version = fx.version_ids(ALIEN).await[0];
    let token = Some(fx.server.admin_token.as_str());
    let old_french = &fx.plan(version).await.unwrap().subtitles[1].url;
    let cached = fetch(&fx.server, old_french, token).await;
    assert!(String::from_utf8(cached.body).unwrap().contains("Bonjour"));

    write_english_and_french_srt(&fx.folder.join(ALIEN_1080P), "Salut");
    fx.scan_and_match().await;
    let french = &fx.plan(version).await.unwrap().subtitles[1].url;
    let replaced = fetch(&fx.server, french, token).await;

    let replaced = String::from_utf8(replaced.body).unwrap();
    assert!(
        replaced.contains("00:00.000 --> 00:01.000\nSalut"),
        "{replaced}"
    );
}

#[tokio::test]
async fn a_removed_versions_cached_subtitles_are_removed() {
    let fx = Fixture::subtitled_alien().await;
    fx.cache_subtitles().await;

    fs::remove_file(fx.folder.join(ALIEN_1080P)).unwrap();

    fx.scan_and_probe().await;

    assert_eq!(fx.cached_subtitles(), Vec::<PathBuf>::new());
}

#[tokio::test]
async fn a_deleted_librarys_cached_subtitles_are_removed() {
    let fx = Fixture::subtitled_alien().await;
    fx.cache_subtitles().await;

    let libraries = LibraryServiceClient::new(HttpClient::plaintext(), fx.server.client_config());
    libraries
        .delete_library(DeleteLibraryRequest {
            id: fx.library,
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(fx.cached_subtitles(), Vec::<PathBuf>::new());
}

#[tokio::test]
async fn an_mp4s_mov_text_track_is_served_as_webvtt() {
    let webvtt = served_webvtt("Alien (1979)/Alien (1979).mp4", HELLO_SRT, "mov_text").await;

    assert!(webvtt.starts_with("WEBVTT"), "{webvtt}");
    assert!(
        webvtt.contains("00:00.000 --> 00:01.000\nHello"),
        "{webvtt}"
    );
}

#[tokio::test]
async fn an_embedded_webvtt_track_is_served() {
    let webvtt = served_webvtt(ALIEN_1080P, HELLO_SRT, "webvtt").await;

    assert!(webvtt.starts_with("WEBVTT"), "{webvtt}");
    assert!(
        webvtt.contains("00:00.000 --> 00:01.000\nHello"),
        "{webvtt}"
    );
}

#[tokio::test]
async fn an_ass_track_is_served_as_plain_webvtt_cues() {
    let webvtt = served_webvtt(ALIEN_1080P, STYLED_ASS, "ass").await;

    assert_eq!(
        webvtt,
        "WEBVTT\n\n\
         00:00:00.000 --> 00:00:00.500\nHello there\nfriend\n\n\
         00:00:00.500 --> 00:00:01.000\nStyled line\n\n"
    );
}

/// A subtitle script's file name, which tells ffmpeg its format, and what it says.
type Script = (&'static str, &'static str);

const HELLO_SRT: Script = ("hello.srt", "1\n00:00:00,000 --> 00:00:01,000\nHello\n");

/// Two lines, positioned, coloured and styled, and a shape drawn over the whole second.
const STYLED_ASS: Script = (
    "styled.ass",
    r"[Script Info]
ScriptType: v4.00+

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1
Style: Fancy,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,-1,-1,-1,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:00.50,Default,,0,0,0,,{\an8\pos(10,10)\c&H0000FF&\b1}Hello{\b0} {\i1}there{\i0}\Nfriend
Dialogue: 0,0:00:00.50,0:00:01.00,Fancy,,0,0,0,,Styled line
Dialogue: 0,0:00:00.00,0:00:01.00,Default,,0,0,0,,{\p1}m 0 0 l 100 0 100 100 0 100{\p0}
",
);

/// What the Server serves for the one subtitle Track of an Alien at `file`, which is `script`
/// stored as `codec`.
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a test helper fails its test by panicking"
)]
async fn served_webvtt(file: &str, script: Script, codec: &str) -> String {
    let fx = Fixture::alien_written_by(file, |path| write_subtitled(path, script, codec)).await;
    let version = fx.version_ids(ALIEN).await[0];
    let plan = fx.plan(version).await.unwrap();
    let token = Some(fx.server.admin_token.as_str());
    let reply = fetch(&fx.server, &plan.subtitles[0].url, token).await;
    assert_eq!(reply.status, 200);
    String::from_utf8(reply.body).unwrap()
}

/// Writes a second of video with `script` as its one subtitle Track, stored as `codec`, to
/// `path`, in the format its extension names, with the ffmpeg on `PATH`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn write_subtitled(path: &Path, (name, contents): Script, codec: &str) {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join(name);
    fs::write(&script, contents).unwrap();
    let status = std::process::Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=size=64x64:rate=24",
            "-i",
        ])
        .arg(&script)
        .args([
            "-map", "0", "-map", "1", "-t", "1", "-c:v", "mpeg4", "-c:s", codec,
        ])
        .arg(path)
        .status()
        .unwrap();
    assert!(status.success(), "ffmpeg failed: {status}");
}

/// A Server whose ffmpeg and ffprobe are shell scripts in `bin`: ffprobe reads every file as an
/// H.264 and AAC MKV with a PGS and an SRT subtitle, and ffmpeg, which writes the arguments of
/// each run to `log`, reads a subtitle out as `PGS` for a `.sup` and `WEBVTT` for WebVTT.
#[cfg(unix)]
fn ffmpeg_reading_pgs_and_srt(bin: &Path, log: &Path) -> Ffmpeg {
    let ffprobe = r#"#!/bin/sh
cat <<'EOF'
{"streams": [{"codec_name": "h264", "codec_type": "video"},
             {"codec_name": "aac", "codec_type": "audio", "disposition": {"default": 1}},
             {"codec_name": "hdmv_pgs_subtitle", "codec_type": "subtitle"},
             {"codec_name": "subrip", "codec_type": "subtitle"}],
 "format": {"format_name": "matroska,webm", "duration": "1.000000"}}
EOF
"#;
    let ffmpeg = format!(
        "#!/bin/sh\necho \"$*\" >> '{}'\ncase \"$*\" in *'-f sup'*) printf PGS ;; \
         *'-f webvtt'*) printf WEBVTT ;; esac\n",
        log.display()
    );
    Ffmpeg {
        ffmpeg: Ok(script(bin, "ffmpeg", &ffmpeg)),
        ffprobe: Ok(script(bin, "ffprobe", ffprobe)),
        gpus: vec![],
    }
}

// The fake ffmpeg and ffprobe are shell scripts.
#[cfg(unix)]
#[tokio::test]
async fn a_pgs_track_is_read_out_as_sup_once_and_thrown_away_with_its_versions_webvtt() {
    let bin = tempfile::tempdir().unwrap();
    let log = bin.path().join("runs.log");
    let ffmpeg = ffmpeg_reading_pgs_and_srt(bin.path(), &log);
    let fx = Fixture::start_with_ffmpeg(LibraryKind::LIBRARY_KIND_MOVIES, ffmpeg).await;
    fx.write(ALIEN_1080P, b"video");
    fx.scan_and_match().await;
    let version = fx.version_ids(ALIEN).await[0];
    let token = Some(fx.server.admin_token.as_str());
    let plan = fx
        .ask_for_plan(PlanPlaybackRequest {
            version_id: version,
            device_decoding: DeviceDecoding {
                subtitle_formats: vec![SubtitleFormat::SUBTITLE_FORMAT_PGS.into()],
                ..chrome()
            }
            .into(),
            ..Default::default()
        })
        .await
        .unwrap();
    let pgs = &plan.subtitles[0];
    assert_eq!(pgs.format, SubtitleFormat::SUBTITLE_FORMAT_PGS);

    let first = fetch(&fx.server, &pgs.url, token).await;
    let again = fetch(&fx.server, &pgs.url, token).await;
    fetch(&fx.server, &plan.subtitles[1].url, token).await;

    assert_eq!(first.status, 200);
    assert_eq!(first.header("content-type"), "application/octet-stream");
    assert_eq!((first.body, again.body), (b"PGS".to_vec(), b"PGS".to_vec()));
    let runs = fs::read_to_string(&log).unwrap();
    let read_outs: Vec<_> = runs.lines().filter(|run| run.contains("0:s:0")).collect();
    assert_eq!(read_outs.len(), 1, "{runs}");
    assert!(read_outs[0].contains("-c:s copy -f sup"), "{runs}");
    fs::remove_file(fx.folder.join(ALIEN_1080P)).unwrap();
    fx.scan_and_probe().await;
    assert_eq!(fx.cached_subtitles(), Vec::<PathBuf>::new());
}

/// Anyone who may play a Version may fetch its subtitles: anyone with a session.
#[tokio::test]
async fn a_subtitle_without_a_session_is_refused() {
    let fx = Fixture::subtitled_alien().await;
    let version = fx.version_ids(ALIEN).await[0];
    let plan = fx.plan(version).await.unwrap();

    let reply = fetch(&fx.server, &plan.subtitles[0].url, None).await;

    assert_eq!(reply.status, 401);
}

#[tokio::test]
async fn a_track_that_isnt_a_subtitle_is_not_found_as_one() {
    let fx = Fixture::subtitled_alien().await;
    let version = fx.version_ids(ALIEN).await[0];
    let token = Some(fx.server.admin_token.as_str());

    let (video, beyond) = (
        format!("/media/versions/{version}/subtitles/0"),
        format!("/media/versions/{version}/subtitles/9"),
    );

    assert_eq!(fetch(&fx.server, &video, token).await.status, 404);
    assert_eq!(fetch(&fx.server, &beyond, token).await.status, 404);
}

const DARK_S01E01: &str = "Dark {tmdb-70523}/Season 01/Dark - S01E01.mkv";
const DARK_S02E01: &str = "Dark {tmdb-70523}/Season 02/Dark - S02E01.mkv";
/// The place of the German subtitle in each of Dark's Episodes, as
/// [`ffprobe_reading_english_and_german_subtitles`] reads them.
const GERMAN: u32 = 3;

/// A Server whose ffprobe, a shell script in `bin`, reads every file as an H.264 + AAC MP4 with
/// English, then German, SRT subtitles, tagged with ISO 639-2/B codes as files often are.
#[cfg(unix)]
fn ffprobe_reading_english_and_german_subtitles(bin: &Path) -> Ffmpeg {
    let ffprobe = r#"#!/bin/sh
cat <<'EOF'
{"streams": [{"codec_name": "h264", "codec_type": "video"},
             {"codec_name": "aac", "codec_type": "audio", "disposition": {"default": 1}},
             {"codec_name": "subrip", "codec_type": "subtitle", "tags": {"language": "eng"}},
             {"codec_name": "subrip", "codec_type": "subtitle", "tags": {"language": "ger"}}],
 "format": {"format_name": "mov,mp4,m4a,3gp,3g2,mj2", "duration": "1.000000"}}
EOF
"#;
    Ffmpeg {
        ffprobe: Ok(script(bin, "ffprobe", ffprobe)),
        ..Ffmpeg::default()
    }
}

#[cfg(unix)]
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    /// Starts a Server with a Shows Library holding Dark's S01E01 and S02E01, each with English and
    /// German subtitles, scanned and Matched, and gives the Versions of those two Episodes.
    async fn subtitled_dark(bin: &Path) -> (Self, [i64; 2]) {
        let ffmpeg = ffprobe_reading_english_and_german_subtitles(bin);
        let mut fx = Self::start_with_ffmpeg(LibraryKind::LIBRARY_KIND_SHOWS, ffmpeg).await;
        fx.write(DARK_S01E01, b"video");
        fx.write(DARK_S02E01, b"video");
        fx.items = fx.scan_and_match().await;
        let first = fx.dark_version(1).await;
        let second = fx.dark_version(2).await;
        (fx, [first, second])
    }

    /// The Version of the first Episode of Dark's Season `season`.
    async fn dark_version(&self, season: u32) -> i64 {
        let episodes = (self.browse)
            .list_episodes(ListEpisodesRequest {
                library_id: self.library,
                show_id: tmdb_id(DARK).into(),
                season_number: season,
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned()
            .episodes;
        episodes[0].versions[0].id
    }

    /// Reports that the Admin, playing the Version `version`, picked the subtitle at `track`, or
    /// turned subtitles off.
    async fn pick_subtitle(&self, version: i64, track: Option<u32>) -> Result<(), ConnectError> {
        PlaybackServiceClient::new(HttpClient::plaintext(), self.server.client_config())
            .report_track_pick(ReportTrackPickRequest {
                version_id: version,
                kind: PickedTrackKind::PICKED_TRACK_KIND_SUBTITLE.into(),
                track,
                ..Default::default()
            })
            .await?;
        Ok(())
    }
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_subtitle_picked_in_an_episode_starts_the_shows_next_episode_until_turned_off() {
    let bin = tempfile::tempdir().unwrap();
    let (fx, [first, second]) = Fixture::subtitled_dark(bin.path()).await;
    let before = fx.plan(first).await.unwrap();

    fx.pick_subtitle(first, Some(GERMAN)).await.unwrap();
    let next = fx.plan(second).await.unwrap();
    fx.pick_subtitle(second, None).await.unwrap();
    let after_off = fx.plan(first).await.unwrap();

    assert_eq!(before.starting_subtitle, None);
    assert_eq!(next.starting_subtitle, Some(GERMAN));
    assert_eq!(
        next.subtitle_track, None,
        "the Device shows it, so none is burned in"
    );
    let places: Vec<_> = next
        .subtitles
        .iter()
        .map(|subtitle| subtitle.place)
        .collect();
    assert_eq!(places, [2, GERMAN]);
    assert_eq!(after_off.starting_subtitle, None);
}

#[cfg(unix)]
#[tokio::test]
async fn a_shows_subtitle_preference_is_its_users_alone_and_a_request_for_none_gets_none() {
    let bin = tempfile::tempdir().unwrap();
    let (fx, [first, second]) = Fixture::subtitled_dark(bin.path()).await;
    let ripley = fx.server.user("Ripley", &[]).await;
    fx.pick_subtitle(first, Some(GERMAN)).await.unwrap();

    let ripleys = plan_as(&ripley, second).await.unwrap();
    let none = fx
        .ask_for_plan(PlanPlaybackRequest {
            version_id: second,
            no_subtitles: true,
            device_decoding: chrome().into(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(ripleys.starting_subtitle, None);
    assert_eq!(none.starting_subtitle, None);
}

#[cfg(unix)]
#[tokio::test]
async fn picking_a_track_that_isnt_a_subtitle_or_on_no_version_is_refused() {
    let bin = tempfile::tempdir().unwrap();
    let (fx, [first, _]) = Fixture::subtitled_dark(bin.path()).await;

    let audio = fx.pick_subtitle(first, Some(1)).await;
    let past_the_end = fx.pick_subtitle(first, Some(9)).await;
    let no_version = fx.pick_subtitle(first + 100, None).await;

    for refused in [audio, past_the_end] {
        let refusal = assert_refused(
            refused,
            ErrorCode::NotFound,
            RefusalReason::REFUSAL_REASON_TRACK_NOT_FOUND,
        );
        assert_eq!(refusal.field, "track");
    }
    assert_refused(
        no_version,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_VERSION_NOT_FOUND,
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_subtitle_picked_in_one_version_of_a_movie_starts_its_other_versions() {
    let bin = tempfile::tempdir().unwrap();
    let ffmpeg = ffprobe_reading_english_and_german_subtitles(bin.path());
    let fx = Fixture::start_with_ffmpeg(LibraryKind::LIBRARY_KIND_MOVIES, ffmpeg).await;
    fx.write(ALIEN_1080P, b"video");
    fx.write(ALIEN_2160P, b"video");
    fx.scan_and_match().await;
    let [hd, uhd] = fx.version_ids(ALIEN).await[..] else {
        panic!("Alien should have two Versions");
    };

    fx.pick_subtitle(hd, Some(GERMAN)).await.unwrap();

    assert_eq!(fx.plan(uhd).await.unwrap().starting_subtitle, Some(GERMAN));
}

#[cfg(unix)]
#[tokio::test]
async fn a_plan_request_that_names_a_subtitle_is_no_pick() {
    let bin = tempfile::tempdir().unwrap();
    let (fx, [first, second]) = Fixture::subtitled_dark(bin.path()).await;

    fx.ask_for_plan(PlanPlaybackRequest {
        version_id: first,
        subtitle_track: Some(GERMAN),
        device_decoding: chrome().into(),
        ..Default::default()
    })
    .await
    .unwrap();

    assert_eq!(fx.plan(second).await.unwrap().starting_subtitle, None);
}

#[cfg(unix)]
#[tokio::test]
async fn a_pick_in_a_version_of_an_unmatched_item_is_accepted_and_keeps_nothing() {
    let bin = tempfile::tempdir().unwrap();
    let (fx, [first, second]) = Fixture::subtitled_dark(bin.path()).await;
    let item = fx.item(DARK_S01E01);
    fx.matches
        .clear_match(ClearMatchRequest {
            item_id: item,
            ..Default::default()
        })
        .await
        .unwrap();

    let picked = fx.pick_subtitle(first, Some(GERMAN)).await;
    fx.matches
        .set_match(SetMatchRequest {
            item_id: item,
            title_id: tmdb_id(DARK).into(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(picked.is_ok(), "{picked:?}");
    assert_eq!(fx.plan(second).await.unwrap().starting_subtitle, None);
}

#[cfg(unix)]
#[tokio::test]
async fn a_user_with_a_subtitle_preference_can_be_deleted() {
    let bin = tempfile::tempdir().unwrap();
    let (fx, [first, _]) = Fixture::subtitled_dark(bin.path()).await;
    let ripley = fx.server.user("Ripley", &[]).await;
    PlaybackServiceClient::new(HttpClient::plaintext(), ripley.client_config())
        .report_track_pick(ReportTrackPickRequest {
            version_id: first,
            kind: PickedTrackKind::PICKED_TRACK_KIND_SUBTITLE.into(),
            track: Some(GERMAN),
            ..Default::default()
        })
        .await
        .unwrap();

    let deleted = UserServiceClient::new(HttpClient::plaintext(), fx.server.client_config())
        .delete_user(DeleteUserRequest {
            user_id: ripley.id,
            ..Default::default()
        })
        .await;

    assert!(deleted.is_ok(), "{deleted:?}");
}
