//! Calls the Server's API over a real socket, the way clients will.

#[path = "support/refused.rs"]
mod refused;
#[path = "support/server.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod server;

use std::net::SocketAddr;

use buffa::EnumValue;
use connectrpc::{ErrorCode, client::HttpClient};
use nama_proto::nama::v1::{
    FoundHardware, FoundProgram, GetPlaybackSettingsRequest, GetPlaybackStatusRequest,
    GetPlaybackStatusResponse, GetServerInfoRequest, GetServerInfoResponse,
    GetServerSettingsRequest, Gpu, HardwareAcceleration, PlaybackLimit, PlaybackSettings,
    RefusalReason, ServerServiceClient, ServerSettings, ToneMapping, UpdatePlaybackSettingsRequest,
    UpdateServerSettingsRequest,
};
#[cfg(unix)]
use nama_server::ffmpeg::{fake, fake::script, find_gpus};
use nama_server::{config::Config, ffmpeg::Ffmpeg};
use refused::assert_refused;
use server::TestServer;

fn config_named(name: &str) -> Config {
    Config {
        name: name.to_owned(),
        ..Config::default()
    }
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn get_server_info(client: &ServerServiceClient<HttpClient>) -> GetServerInfoResponse {
    client
        .get_server_info(GetServerInfoRequest::default())
        .await
        .unwrap()
        .into_owned()
}

/// Every transport the Server promises: HTTP/1.1 and cleartext HTTP/2, each with JSON and binary
/// protobuf bodies.
#[tokio::test]
async fn get_server_info_returns_the_server_name_and_version_over_every_transport() {
    let server = TestServer::builder()
        .config(config_named("Living Room"))
        .start()
        .await;
    let transports = [
        (
            "HTTP/1.1 JSON",
            HttpClient::plaintext(),
            server.client_config().json(),
        ),
        (
            "HTTP/1.1 binary",
            HttpClient::plaintext(),
            server.client_config().proto(),
        ),
        (
            "HTTP/2 JSON",
            HttpClient::plaintext_http2_only(),
            server.client_config().json(),
        ),
        (
            "HTTP/2 binary",
            HttpClient::plaintext_http2_only(),
            server.client_config().proto(),
        ),
    ];

    for (transport, http, config) in transports {
        let info = get_server_info(&ServerServiceClient::new(http, config)).await;

        assert_eq!(info.name, "Living Room", "{transport}");
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"), "{transport}");
    }
}

/// What `curl -H 'Content-Type: application/json' -d '{}' .../GetServerInfo` sends.
#[tokio::test]
async fn a_plain_http1_json_post_gets_a_json_answer() {
    let server = TestServer::builder()
        .config(config_named("Attic"))
        .start()
        .await;

    let (status, content_type, body) =
        http1_post_json(server.addr, "/nama.v1.ServerService/GetServerInfo", "{}").await;

    assert_eq!(status, 200);
    assert_eq!(content_type, "application/json");
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        body,
        serde_json::json!({
            "name": "Attic",
            "version": env!("CARGO_PKG_VERSION"),
        })
    );
}

/// Sends a bare HTTP/1.1 request with no Connect client involved, so only the wire format is
/// shared with the Server. Returns the status, content type and body.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn http1_post_json(addr: SocketAddr, path: &str, body: &str) -> (u16, String, Vec<u8>) {
    use http_body_util::{BodyExt, Full};
    use hyper::{Request, body::Bytes, client::conn::http1};
    use hyper_util::rt::TokioIo;

    let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let (mut sender, connection) = http1::handshake(TokioIo::new(stream)).await.unwrap();
    tokio::spawn(connection);

    let request = Request::post(path)
        .header("host", addr.to_string())
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(body.to_owned())))
        .unwrap();
    let response = sender.send_request(request).await.unwrap();

    let status = response.status().as_u16();
    let content_type = response.headers()["content-type"]
        .to_str()
        .unwrap()
        .to_owned();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, content_type, bytes.to_vec())
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn server_url(client: &ServerServiceClient<HttpClient>) -> String {
    client
        .get_server_settings(GetServerSettingsRequest::default())
        .await
        .unwrap()
        .into_owned()
        .settings
        .url
        .clone()
}

fn set_server_url(url: &str) -> UpdateServerSettingsRequest {
    UpdateServerSettingsRequest {
        settings: ServerSettings {
            url: url.to_owned(),
            ..Default::default()
        }
        .into(),
        ..Default::default()
    }
}

#[tokio::test]
async fn the_server_url_is_empty_until_an_admin_sets_it() {
    let server = TestServer::builder().start().await;
    let client = ServerServiceClient::new(HttpClient::plaintext(), server.client_config());
    let unset = server_url(&client).await;

    client
        .update_server_settings(set_server_url(" https://nama.example.com "))
        .await
        .unwrap();

    assert_eq!(unset, "");
    assert_eq!(server_url(&client).await, "https://nama.example.com");
}

#[tokio::test]
async fn a_server_url_that_isnt_an_http_url_is_refused_naming_it_and_nothing_changes() {
    let server = TestServer::builder().start().await;
    let client = ServerServiceClient::new(HttpClient::plaintext(), server.client_config());
    client
        .update_server_settings(set_server_url("https://nama.example.com"))
        .await
        .unwrap();

    for not_a_url in [
        "nama.example.com",
        "ftp://nama.example.com",
        "https://nama.example.com/?a=b",
    ] {
        let updated = client
            .update_server_settings(set_server_url(not_a_url))
            .await;

        let refusal = assert_refused(
            updated,
            ErrorCode::InvalidArgument,
            RefusalReason::REFUSAL_REASON_NOT_A_URL,
        );
        assert_eq!(refusal.field, "settings.url", "{not_a_url}");
    }
    assert_eq!(server_url(&client).await, "https://nama.example.com");
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn playback_status(ffmpeg: Ffmpeg) -> GetPlaybackStatusResponse {
    let server = TestServer::builder().ffmpeg(ffmpeg).start().await;
    ServerServiceClient::new(HttpClient::plaintext(), server.client_config())
        .get_playback_status(GetPlaybackStatusRequest::default())
        .await
        .unwrap()
        .into_owned()
}

#[tokio::test]
async fn the_playback_status_shows_the_path_and_version_of_the_ffmpeg_and_ffprobe_found() {
    let found = |name: &str| nama_server::ffmpeg::FoundProgram {
        path: format!("/usr/bin/{name}").into(),
        version: "7.1.1".to_owned(),
    };

    let status = playback_status(Ffmpeg {
        ffmpeg: Ok(found("ffmpeg")),
        ffprobe: Ok(found("ffprobe")),
        gpus: vec![],
    })
    .await;

    let shown = |path: &str| FoundProgram {
        path: path.to_owned(),
        version: "7.1.1".to_owned(),
        ..Default::default()
    };
    assert_eq!(status.ffmpeg.as_option(), Some(&shown("/usr/bin/ffmpeg")));
    assert_eq!(status.ffprobe.as_option(), Some(&shown("/usr/bin/ffprobe")));
}

/// The status of a Server whose ffmpeg, a shell script in `bin`, lists `encoders` as its encoders
/// and `filter` as its one filter, and passes every test encode.
#[cfg(unix)]
async fn playback_status_listing(
    bin: &std::path::Path,
    encoders: &[&str],
    filter: &str,
) -> GetPlaybackStatusResponse {
    let encoders = encoders.join(" ");
    let ffmpeg = format!(
        "#!/bin/sh\ncase \"$*\" in *-encoders*) printf ' V....D %s  H.264\\n' {encoders} ;; \
         *-filters*) echo ' ... {filter}  V->V  A filter' ;; esac\n"
    );
    playback_status(Ffmpeg {
        ffmpeg: Ok(script(bin, "ffmpeg", &ffmpeg)),
        ffprobe: Ok(script(bin, "ffprobe", "#!/bin/sh\n")),
        gpus: vec![],
    })
    .await
}

#[cfg(unix)]
#[tokio::test]
async fn the_playback_status_says_an_ffmpeg_like_fedoras_lacks_libx264_and_zscale() {
    let bin = tempfile::tempdir().unwrap();

    let status = playback_status_listing(bin.path(), &["libopenh264"], "scale").await;

    assert!(!status.has_libx264, "{status:?}");
    assert!(!status.has_zscale, "{status:?}");
    assert!(!status.can_transcode, "{status:?}");
}

#[cfg(unix)]
#[tokio::test]
async fn the_playback_status_says_an_ffmpeg_listing_libx264_and_zscale_has_them() {
    let bin = tempfile::tempdir().unwrap();

    let status = playback_status_listing(bin.path(), &["libx264"], "zscale").await;

    assert!(status.has_libx264, "{status:?}");
    assert!(status.has_zscale, "{status:?}");
    assert!(status.can_transcode, "{status:?}");
}

#[cfg(unix)]
#[tokio::test]
async fn the_playback_status_says_what_tone_maps_on_each_hardware_and_which_automatic_picks() {
    let found = |api: HardwareAcceleration, tone_mapping: ToneMapping, automatic| FoundHardware {
        api: api.into(),
        tone_maps: tone_mapping == ToneMapping::TONE_MAPPING_GPU,
        tone_mapping: tone_mapping.into(),
        automatic,
        ..Default::default()
    };
    let cases = [
        (
            ["libx264", "h264_videotoolbox", "h264_qsv"],
            "zscale",
            ToneMapping::TONE_MAPPING_SOFTWARE,
        ),
        (
            ["libopenh264", "h264_videotoolbox", "h264_qsv"],
            "scale",
            ToneMapping::TONE_MAPPING_NONE,
        ),
    ];
    for (encoders, filter, without_gpu) in cases {
        let bin = tempfile::tempdir().unwrap();

        let status = playback_status_listing(bin.path(), &encoders, filter).await;

        assert_eq!(
            status.hardware,
            [
                found(
                    HardwareAcceleration::HARDWARE_ACCELERATION_QSV,
                    ToneMapping::TONE_MAPPING_GPU,
                    true
                ),
                found(
                    HardwareAcceleration::HARDWARE_ACCELERATION_VIDEOTOOLBOX,
                    without_gpu,
                    false
                ),
            ],
            "with {filter}"
        );
    }
}

/// An ffmpeg, a shell script in `root` with a fake `/sys` and `/dev/dri` beside it, that passes
/// VAAPI on an AMD Radeon RX 9070 XT at `0000:03:00.0`, and QSV and VAAPI on an Intel integrated
/// GPU at `0000:00:02.0`.
#[cfg(unix)]
fn ffmpeg_on_two_gpus(root: &std::path::Path) -> Ffmpeg {
    let (sys, dri) = (root.join("sys"), root.join("dri"));
    fake::gpu(&sys, &dri, "renderD128", "0000:00:02.0", "0x8086");
    fake::gpu(&sys, &dri, "renderD129", "0000:03:00.0", "0x1002");
    let ffmpeg = "#!/bin/sh\ncase \"$*\" in \
                  *-v\\ verbose*renderD129*) echo 'VAAPI driver: Mesa Gallium driver 26.2.6 for \
                                               AMD Radeon RX 9070 XT (radeonsi)' >&2 ;; \
                  *-encoders*) printf ' V....D %s  H.264\\n' libx264 h264_qsv h264_vaapi ;; \
                  *-filters*) echo ' ... zscale  V->V  A filter' ;; esac\n";
    Ffmpeg {
        ffmpeg: Ok(script(root, "ffmpeg", ffmpeg)),
        ffprobe: Ok(script(root, "ffprobe", "#!/bin/sh\n")),
        gpus: find_gpus(&dri, &sys.join("class/drm")),
    }
}

#[cfg(unix)]
#[tokio::test]
async fn the_playback_status_lists_each_api_on_each_gpu_with_its_id_and_name() {
    let root = tempfile::tempdir().unwrap();
    let found = |api: HardwareAcceleration, id: &str, name: &str, tone_maps, automatic| {
        let tone_mapping = if tone_maps {
            ToneMapping::TONE_MAPPING_GPU
        } else {
            ToneMapping::TONE_MAPPING_SOFTWARE
        };
        FoundHardware {
            api: api.into(),
            gpu: Gpu {
                id: id.to_owned(),
                name: name.to_owned(),
                ..Default::default()
            }
            .into(),
            tone_maps,
            tone_mapping: tone_mapping.into(),
            automatic,
            ..Default::default()
        }
    };

    let status = playback_status(ffmpeg_on_two_gpus(root.path())).await;

    let amd = "AMD Radeon RX 9070 XT";
    let intel = "Intel integrated GPU";
    assert_eq!(
        status.hardware,
        [
            found(
                HardwareAcceleration::HARDWARE_ACCELERATION_VAAPI,
                "0000:03:00.0",
                amd,
                true,
                true
            ),
            found(
                HardwareAcceleration::HARDWARE_ACCELERATION_QSV,
                "0000:00:02.0",
                intel,
                true,
                false
            ),
            found(
                HardwareAcceleration::HARDWARE_ACCELERATION_VAAPI,
                "0000:00:02.0",
                intel,
                true,
                false
            ),
        ]
    );
}

#[tokio::test]
async fn without_ffmpeg_the_playback_status_says_it_has_neither_libx264_nor_zscale() {
    let status = playback_status(Ffmpeg {
        ffmpeg: Err("ffmpeg wasn't found on PATH".to_owned()),
        ffprobe: Err("ffprobe wasn't found on PATH".to_owned()),
        gpus: vec![],
    })
    .await;

    assert!(!status.has_libx264, "{status:?}");
    assert!(!status.has_zscale, "{status:?}");
    assert!(!status.can_transcode, "{status:?}");
}

#[tokio::test]
async fn the_playback_status_says_why_the_server_cant_use_ffmpeg_or_ffprobe() {
    let status = playback_status(Ffmpeg {
        ffmpeg: Err("ffmpeg wasn't found at the configured path /opt/ffmpeg".to_owned()),
        ffprobe: Err("ffprobe wasn't found on PATH".to_owned()),
        gpus: vec![],
    })
    .await;

    assert!(status.ffmpeg.is_unset(), "{status:?}");
    assert!(status.ffprobe.is_unset(), "{status:?}");
    assert_eq!(
        status.ffmpeg_problem,
        "ffmpeg wasn't found at the configured path /opt/ffmpeg"
    );
    assert_eq!(status.ffprobe_problem, "ffprobe wasn't found on PATH");
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn segment_folder(client: &ServerServiceClient<HttpClient>) -> String {
    client
        .get_playback_settings(GetPlaybackSettingsRequest::default())
        .await
        .unwrap()
        .into_owned()
        .settings
        .segment_folder
        .clone()
}

fn set_segment_folder(folder: &str) -> UpdatePlaybackSettingsRequest {
    UpdatePlaybackSettingsRequest {
        settings: PlaybackSettings {
            segment_folder: folder.to_owned(),
            ..Default::default()
        }
        .into(),
        ..Default::default()
    }
}

#[tokio::test]
async fn the_segment_folder_is_empty_until_an_admin_sets_it() {
    let server = TestServer::builder().start().await;
    let client = ServerServiceClient::new(HttpClient::plaintext(), server.client_config());
    let unset = segment_folder(&client).await;

    client
        .update_playback_settings(set_segment_folder(" /dev/shm/nama "))
        .await
        .unwrap();

    assert_eq!(unset, "");
    assert_eq!(segment_folder(&client).await, "/dev/shm/nama");
}

#[tokio::test]
async fn the_remote_playback_limit_is_original_until_an_admin_sets_it() {
    let server = TestServer::builder().start().await;
    let client = ServerServiceClient::new(HttpClient::plaintext(), server.client_config());
    let settings = || async {
        (client.get_playback_settings(GetPlaybackSettingsRequest::default()))
            .await
            .unwrap()
            .into_owned()
            .settings
            .remote_playback_limit
    };
    let unset = settings().await;

    client
        .update_playback_settings(UpdatePlaybackSettingsRequest {
            settings: PlaybackSettings {
                remote_playback_limit: PlaybackLimit::PLAYBACK_LIMIT_720P.into(),
                max_transcodes: Some(2),
                ..Default::default()
            }
            .into(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(unset, PlaybackLimit::PLAYBACK_LIMIT_ORIGINAL);
    assert_eq!(settings().await, PlaybackLimit::PLAYBACK_LIMIT_720P);
}

#[tokio::test]
async fn the_transcode_cap_and_hardware_acceleration_are_the_defaults_until_an_admin_sets_them() {
    let server = TestServer::builder().start().await;
    let client = ServerServiceClient::new(HttpClient::plaintext(), server.client_config());
    let settings = async || {
        let got = client.get_playback_settings(GetPlaybackSettingsRequest::default());
        let settings = got.await.unwrap().into_owned().settings;
        (settings.max_transcodes, settings.hardware_acceleration)
    };
    let unset = settings().await;

    client
        .update_playback_settings(UpdatePlaybackSettingsRequest {
            settings: PlaybackSettings {
                max_transcodes: Some(1),
                hardware_acceleration: HardwareAcceleration::HARDWARE_ACCELERATION_VAAPI.into(),
                ..Default::default()
            }
            .into(),
            ..Default::default()
        })
        .await
        .unwrap();

    let automatic = HardwareAcceleration::HARDWARE_ACCELERATION_UNSPECIFIED.into();
    assert_eq!(unset, (None, automatic));
    let vaapi = HardwareAcceleration::HARDWARE_ACCELERATION_VAAPI.into();
    assert_eq!(settings().await, (Some(1), vaapi));
}

#[tokio::test]
async fn a_transcode_cap_set_to_0_is_refused_naming_it_and_nothing_changes() {
    let server = TestServer::builder().start().await;
    let client = ServerServiceClient::new(HttpClient::plaintext(), server.client_config());

    let updated = client
        .update_playback_settings(UpdatePlaybackSettingsRequest {
            settings: PlaybackSettings {
                segment_folder: "/dev/shm/nama".to_owned(),
                max_transcodes: Some(0),
                ..Default::default()
            }
            .into(),
            ..Default::default()
        })
        .await;

    let refusal = assert_refused(
        updated,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
    );
    assert_eq!(refusal.field, "settings.max_transcodes");
    assert_eq!(
        refusal.params.get("minTranscodes").map(String::as_str),
        Some("1")
    );
    assert_eq!(segment_folder(&client).await, "");
}

fn keep_behind(minutes: Option<u32>) -> UpdatePlaybackSettingsRequest {
    UpdatePlaybackSettingsRequest {
        settings: PlaybackSettings {
            keep_behind_minutes: minutes,
            ..Default::default()
        }
        .into(),
        ..Default::default()
    }
}

#[tokio::test]
async fn keep_for_skipping_back_is_returned_as_saved_and_unset_again_once_cleared() {
    let server = TestServer::builder().start().await;
    let client = ServerServiceClient::new(HttpClient::plaintext(), server.client_config());
    let saved = async || {
        let got = client.get_playback_settings(GetPlaybackSettingsRequest::default());
        got.await.unwrap().into_owned().settings.keep_behind_minutes
    };
    let unset = saved().await;

    client
        .update_playback_settings(keep_behind(Some(12)))
        .await
        .unwrap();
    let set = saved().await;
    client
        .update_playback_settings(keep_behind(None))
        .await
        .unwrap();

    assert_eq!((unset, set, saved().await), (None, Some(12), None));
}

#[tokio::test]
async fn keep_for_skipping_back_set_to_0_is_refused_naming_it_and_nothing_changes() {
    let server = TestServer::builder().start().await;
    let client = ServerServiceClient::new(HttpClient::plaintext(), server.client_config());
    client
        .update_playback_settings(keep_behind(Some(12)))
        .await
        .unwrap();

    let updated = client.update_playback_settings(keep_behind(Some(0))).await;

    let refusal = assert_refused(
        updated,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
    );
    assert_eq!(refusal.field, "settings.keep_behind_minutes");
    assert_eq!(
        refusal.params.get("minMinutes").map(String::as_str),
        Some("1")
    );
    let got = client
        .get_playback_settings(GetPlaybackSettingsRequest::default())
        .await
        .unwrap()
        .into_owned()
        .settings;
    assert_eq!(got.keep_behind_minutes, Some(12));
}

#[tokio::test]
async fn a_segment_folder_that_isnt_an_absolute_path_is_refused_naming_it_and_nothing_changes() {
    let server = TestServer::builder().start().await;
    let client = ServerServiceClient::new(HttpClient::plaintext(), server.client_config());
    client
        .update_playback_settings(set_segment_folder("/dev/shm/nama"))
        .await
        .unwrap();

    let updated = client
        .update_playback_settings(set_segment_folder("shm/nama"))
        .await;

    let refusal = assert_refused(
        updated,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_FOLDER_NOT_ABSOLUTE,
    );
    assert_eq!(refusal.field, "settings.segment_folder");
    assert_eq!(segment_folder(&client).await, "/dev/shm/nama");
}

/// Saving the Playback settings with `api` on the GPU `id`, named `name` by the client, and the
/// most Transcodes at once `max_transcodes`.
fn pick_gpu(
    api: HardwareAcceleration,
    id: &str,
    name: &str,
    max_transcodes: Option<u32>,
) -> UpdatePlaybackSettingsRequest {
    UpdatePlaybackSettingsRequest {
        settings: PlaybackSettings {
            hardware_acceleration: api.into(),
            gpu: gpu(id, name).into(),
            max_transcodes,
            ..Default::default()
        }
        .into(),
        ..Default::default()
    }
}

fn gpu(id: &str, name: &str) -> Gpu {
    Gpu {
        id: id.to_owned(),
        name: name.to_owned(),
        ..Default::default()
    }
}

#[cfg(unix)]
#[tokio::test]
async fn a_picked_gpu_is_saved_under_its_found_name_and_kept_when_another_field_changes() {
    let root = tempfile::tempdir().unwrap();
    let server = TestServer::builder()
        .ffmpeg(ffmpeg_on_two_gpus(root.path()))
        .start()
        .await;
    let client = ServerServiceClient::new(HttpClient::plaintext(), server.client_config());
    let vaapi = HardwareAcceleration::HARDWARE_ACCELERATION_VAAPI;

    let saved = client
        .update_playback_settings(pick_gpu(vaapi, "0000:00:02.0", "made up", None))
        .await
        .unwrap()
        .into_owned()
        .settings;
    client
        .update_playback_settings(pick_gpu(vaapi, "0000:00:02.0", "", Some(3)))
        .await
        .unwrap();

    let intel = gpu("0000:00:02.0", "Intel integrated GPU");
    assert_eq!(saved.gpu.as_option(), Some(&intel));
    let got = client
        .get_playback_settings(GetPlaybackSettingsRequest::default())
        .await
        .unwrap()
        .into_owned()
        .settings;
    assert_eq!(got.hardware_acceleration, EnumValue::from(vaapi));
    assert_eq!(got.gpu.as_option(), Some(&intel));
    assert_eq!(got.max_transcodes, Some(3));
}

#[cfg(unix)]
#[tokio::test]
async fn a_picked_gpu_gone_after_a_restart_is_still_returned_and_kept_when_another_field_changes() {
    let (before, after) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let server = TestServer::builder()
        .ffmpeg(ffmpeg_on_two_gpus(before.path()))
        .start()
        .await;
    let vaapi = HardwareAcceleration::HARDWARE_ACCELERATION_VAAPI;
    let amd = "0000:03:00.0";
    ServerServiceClient::new(HttpClient::plaintext(), server.client_config())
        .update_playback_settings(pick_gpu(vaapi, amd, "", None))
        .await
        .unwrap();
    let only_intel = ffmpeg_on_two_gpus(after.path());
    let server = TestServer::builder()
        .ffmpeg(Ffmpeg {
            gpus: (only_intel.gpus.iter())
                .filter(|gpu| gpu.id() != amd)
                .cloned()
                .collect(),
            ..only_intel
        })
        .restart(server)
        .await;
    let client = ServerServiceClient::new(HttpClient::plaintext(), server.client_config());

    let got = client
        .get_playback_settings(GetPlaybackSettingsRequest::default())
        .await
        .unwrap()
        .into_owned()
        .settings;
    let saved = client
        .update_playback_settings(pick_gpu(vaapi, amd, "", Some(3)))
        .await
        .unwrap()
        .into_owned()
        .settings;

    let gone = gpu(amd, "AMD Radeon RX 9070 XT");
    assert_eq!(got.hardware_acceleration, EnumValue::from(vaapi));
    assert_eq!(got.gpu.as_option(), Some(&gone));
    assert_eq!(saved.gpu.as_option(), Some(&gone));
    assert_eq!(saved.max_transcodes, Some(3));
}

#[cfg(unix)]
#[tokio::test]
async fn a_gpu_its_api_didnt_pass_on_or_with_an_api_that_takes_none_is_refused_naming_it() {
    let root = tempfile::tempdir().unwrap();
    let server = TestServer::builder()
        .ffmpeg(ffmpeg_on_two_gpus(root.path()))
        .start()
        .await;
    let client = ServerServiceClient::new(HttpClient::plaintext(), server.client_config());
    let never_seen = "0000:0a:00.0";
    let amd = "0000:03:00.0";

    for (api, id) in [
        (
            HardwareAcceleration::HARDWARE_ACCELERATION_VAAPI,
            never_seen,
        ),
        (HardwareAcceleration::HARDWARE_ACCELERATION_QSV, amd),
        (HardwareAcceleration::HARDWARE_ACCELERATION_UNSPECIFIED, amd),
        (HardwareAcceleration::HARDWARE_ACCELERATION_OFF, amd),
        (HardwareAcceleration::HARDWARE_ACCELERATION_NVENC, amd),
        (
            HardwareAcceleration::HARDWARE_ACCELERATION_VIDEOTOOLBOX,
            amd,
        ),
    ] {
        let updated = client
            .update_playback_settings(pick_gpu(api, id, "AMD Radeon RX 9070 XT", None))
            .await;

        let refusal = assert_refused(
            updated,
            ErrorCode::InvalidArgument,
            RefusalReason::REFUSAL_REASON_GPU_NOT_OFFERED,
        );
        assert_eq!(refusal.field, "settings.gpu", "{api:?}");
        assert_eq!(refusal.params.get("gpuId").map(String::as_str), Some(id));
    }
    let got = client
        .get_playback_settings(GetPlaybackSettingsRequest::default())
        .await
        .unwrap()
        .into_owned()
        .settings;
    let automatic = HardwareAcceleration::HARDWARE_ACCELERATION_UNSPECIFIED;
    assert_eq!(got.hardware_acceleration, EnumValue::from(automatic));
    assert_eq!(got.gpu.as_option(), None);
}
