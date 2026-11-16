//! Settings updates the Server refuses whole: one without its `settings` message, and one naming
//! an enum value the Server doesn't know. Nothing is stored either way.

#[path = "support/refused.rs"]
mod refused;
#[path = "support/server.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod server;

use buffa::EnumValue;
use connectrpc::{ErrorCode, client::HttpClient};
use nama_proto::nama::v1::{
    GetImageSettingsRequest, GetPlaybackSettingsRequest, GetServerSettingsRequest, Gpu,
    HardwareAcceleration, PlaybackLimit, PlaybackSettings, RefusalReason, ServerServiceClient,
    ServerSettings, UpdateImageSettingsRequest, UpdatePlaybackSettingsRequest,
    UpdateServerSettingsRequest,
};
use refused::assert_refused;
use server::TestServer;

/// A segment folder that's absolute where the tests run: on Windows one needs a drive.
#[cfg(windows)]
const FOLDER: &str = r"C:\nama";
#[cfg(not(windows))]
const FOLDER: &str = "/dev/shm/nama";

/// Playback settings with every field but `gpu` away from its default, so a reset would show:
/// a GPU is refused with Off.
fn saved_playback() -> PlaybackSettings {
    PlaybackSettings {
        segment_folder: FOLDER.to_owned(),
        max_transcodes: Some(2),
        keep_behind_minutes: Some(30),
        hardware_acceleration: HardwareAcceleration::HARDWARE_ACCELERATION_OFF.into(),
        remote_playback_limit: PlaybackLimit::PLAYBACK_LIMIT_720P.into(),
        ..Default::default()
    }
}

fn update_playback(settings: PlaybackSettings) -> UpdatePlaybackSettingsRequest {
    UpdatePlaybackSettingsRequest {
        settings: settings.into(),
        ..Default::default()
    }
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn read_playback(client: &ServerServiceClient<HttpClient>) -> PlaybackSettings {
    (client.get_playback_settings(GetPlaybackSettingsRequest::default()))
        .await
        .unwrap()
        .into_owned()
        .settings
        .into_option()
        .unwrap()
}

/// Starts a Server with [`saved_playback`] saved, and a client for it.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn with_playback_saved() -> (TestServer, ServerServiceClient<HttpClient>) {
    let server = TestServer::builder().start().await;
    let client = ServerServiceClient::new(HttpClient::plaintext(), server.client_config());
    (client.update_playback_settings(update_playback(saved_playback())))
        .await
        .unwrap();
    (server, client)
}

#[tokio::test]
async fn playback_settings_without_settings_are_refused_naming_it_and_nothing_changes() {
    let (_server, client) = with_playback_saved().await;
    let before = read_playback(&client).await;

    let updated = client
        .update_playback_settings(UpdatePlaybackSettingsRequest::default())
        .await;

    let refusal = assert_refused(
        updated,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
    );
    assert_eq!(refusal.field, "settings");
    assert_eq!(read_playback(&client).await, before);
}

#[tokio::test]
async fn server_settings_without_settings_are_refused_naming_it_and_the_url_stays() {
    let server = TestServer::builder().start().await;
    let client = ServerServiceClient::new(HttpClient::plaintext(), server.client_config());
    let url = "https://nama.example.com";
    client
        .update_server_settings(UpdateServerSettingsRequest {
            settings: ServerSettings {
                url: url.to_owned(),
                ..Default::default()
            }
            .into(),
            ..Default::default()
        })
        .await
        .unwrap();

    let updated = client
        .update_server_settings(UpdateServerSettingsRequest::default())
        .await;

    let refusal = assert_refused(
        updated,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
    );
    assert_eq!(refusal.field, "settings");
    let got = client.get_server_settings(GetServerSettingsRequest::default());
    assert_eq!(got.await.unwrap().into_owned().settings.url, url);
}

#[tokio::test]
async fn image_settings_without_settings_are_refused_naming_it_and_nothing_changes() {
    let server = TestServer::builder().start().await;
    let client = ServerServiceClient::new(HttpClient::plaintext(), server.client_config());
    server.set_image_settings(true, Some(64)).await;

    let updated = client
        .update_image_settings(UpdateImageSettingsRequest::default())
        .await;

    let refusal = assert_refused(
        updated,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
    );
    assert_eq!(refusal.field, "settings");
    let got = client.get_image_settings(GetImageSettingsRequest::default());
    let kept = got.await.unwrap().into_owned().settings;
    assert_eq!(
        (kept.resize_images, kept.resized_cache_mib),
        (true, Some(64))
    );
}

#[tokio::test]
async fn an_unknown_hardware_acceleration_is_refused_naming_it_and_nothing_changes() {
    let (_server, client) = with_playback_saved().await;
    let before = read_playback(&client).await;
    let on_a_gpu = Gpu {
        id: "0000:00:02.0".to_owned(),
        ..Default::default()
    };

    for gpu in [None, Some(on_a_gpu)] {
        let updated = client
            .update_playback_settings(update_playback(PlaybackSettings {
                hardware_acceleration: EnumValue::Unknown(99),
                gpu: gpu.into(),
                ..saved_playback()
            }))
            .await;

        let refusal = assert_refused(
            updated,
            ErrorCode::InvalidArgument,
            RefusalReason::REFUSAL_REASON_UNKNOWN_ENUM_VALUE,
        );
        assert_eq!(refusal.field, "settings.hardware_acceleration");
    }
    assert_eq!(read_playback(&client).await, before);
}

#[tokio::test]
async fn an_unknown_remote_playback_limit_is_refused_naming_it_and_nothing_changes() {
    let (_server, client) = with_playback_saved().await;
    let before = read_playback(&client).await;

    let updated = client
        .update_playback_settings(update_playback(PlaybackSettings {
            remote_playback_limit: EnumValue::Unknown(99),
            ..saved_playback()
        }))
        .await;

    let refusal = assert_refused(
        updated,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_UNKNOWN_ENUM_VALUE,
    );
    assert_eq!(refusal.field, "settings.remote_playback_limit");
    assert_eq!(read_playback(&client).await, before);
}

#[tokio::test]
async fn unspecified_hardware_acceleration_and_limit_are_saved_as_automatic_and_original() {
    let (_server, client) = with_playback_saved().await;

    client
        .update_playback_settings(update_playback(PlaybackSettings {
            hardware_acceleration: HardwareAcceleration::HARDWARE_ACCELERATION_UNSPECIFIED.into(),
            remote_playback_limit: PlaybackLimit::PLAYBACK_LIMIT_UNSPECIFIED.into(),
            ..saved_playback()
        }))
        .await
        .unwrap();

    let saved = read_playback(&client).await;
    assert_eq!(
        (saved.hardware_acceleration, saved.remote_playback_limit),
        (
            HardwareAcceleration::HARDWARE_ACCELERATION_UNSPECIFIED.into(),
            PlaybackLimit::PLAYBACK_LIMIT_ORIGINAL.into()
        )
    );
}
