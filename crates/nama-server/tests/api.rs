//! Calls the Server's API over a real socket, the way clients will.

#[path = "support/refused.rs"]
mod refused;
#[path = "support/server.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod server;

use std::net::SocketAddr;

use connectrpc::{ErrorCode, client::HttpClient};
use nama_proto::nama::v1::{
    FoundProgram, GetPlaybackSettingsRequest, GetPlaybackStatusRequest, GetPlaybackStatusResponse,
    GetServerInfoRequest, GetServerInfoResponse, GetServerSettingsRequest, HardwareAcceleration,
    PlaybackLimit, PlaybackSettings, RefusalReason, ServerServiceClient, ServerSettings,
    UpdatePlaybackSettingsRequest, UpdateServerSettingsRequest,
};
#[cfg(unix)]
use nama_server::ffmpeg::fake::script;
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

/// The status of a Server whose ffmpeg, a shell script in `bin`, lists `encoder` as its one
/// encoder and `filter` as its one filter.
#[cfg(unix)]
async fn playback_status_listing(
    bin: &std::path::Path,
    encoder: &str,
    filter: &str,
) -> GetPlaybackStatusResponse {
    let ffmpeg = format!(
        "#!/bin/sh\ncase \"$*\" in *-encoders*) echo ' V....D {encoder}  H.264' ;; \
         *-filters*) echo ' ... {filter}  V->V  A filter' ;; esac\n"
    );
    playback_status(Ffmpeg {
        ffmpeg: Ok(script(bin, "ffmpeg", &ffmpeg)),
        ffprobe: Ok(script(bin, "ffprobe", "#!/bin/sh\n")),
    })
    .await
}

#[cfg(unix)]
#[tokio::test]
async fn the_playback_status_says_an_ffmpeg_like_fedoras_lacks_libx264_and_zscale() {
    let bin = tempfile::tempdir().unwrap();

    let status = playback_status_listing(bin.path(), "libopenh264", "scale").await;

    assert!(!status.has_libx264, "{status:?}");
    assert!(!status.has_zscale, "{status:?}");
}

#[cfg(unix)]
#[tokio::test]
async fn the_playback_status_says_an_ffmpeg_listing_libx264_and_zscale_has_them() {
    let bin = tempfile::tempdir().unwrap();

    let status = playback_status_listing(bin.path(), "libx264", "zscale").await;

    assert!(status.has_libx264, "{status:?}");
    assert!(status.has_zscale, "{status:?}");
}

#[tokio::test]
async fn without_ffmpeg_the_playback_status_says_it_has_neither_libx264_nor_zscale() {
    let status = playback_status(Ffmpeg {
        ffmpeg: Err("ffmpeg wasn't found on PATH".to_owned()),
        ffprobe: Err("ffprobe wasn't found on PATH".to_owned()),
    })
    .await;

    assert!(!status.has_libx264, "{status:?}");
    assert!(!status.has_zscale, "{status:?}");
}

#[tokio::test]
async fn the_playback_status_says_why_the_server_cant_use_ffmpeg_or_ffprobe() {
    let status = playback_status(Ffmpeg {
        ffmpeg: Err("ffmpeg wasn't found at the configured path /opt/ffmpeg".to_owned()),
        ffprobe: Err("ffprobe wasn't found on PATH".to_owned()),
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
    assert_eq!(segment_folder(&client).await, "");
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
