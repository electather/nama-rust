//! Each User's Subtitle Appearance: kept on the Server for themself alone, so it follows them to
//! every Device.

use crate::support::{refused, server};

use buffa::EnumValue;
use connectrpc::{ErrorCode, client::HttpClient};
use nama_proto::nama::v1::{
    DeleteUserRequest, GetSubtitleAppearanceRequest, PlaybackServiceClient, RefusalReason,
    SetSubtitleBackgroundRequest, SetSubtitleSizeRequest, SubtitleBackground as Background,
    SubtitleSize as Size, UserServiceClient,
};
use refused::assert_refused;
use server::{SignedIn, TestServer};

fn playback(user: &SignedIn) -> PlaybackServiceClient<HttpClient> {
    PlaybackServiceClient::new(HttpClient::plaintext(), user.client_config())
}

/// The caller's size and background.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn appearance(client: &PlaybackServiceClient<HttpClient>) -> (Size, Background) {
    let got = client
        .get_subtitle_appearance(GetSubtitleAppearanceRequest::default())
        .await
        .unwrap()
        .into_owned();
    (
        got.size.as_known().unwrap(),
        got.background.as_known().unwrap(),
    )
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn set_size(client: &PlaybackServiceClient<HttpClient>, size: Size) {
    client
        .set_subtitle_size(SetSubtitleSizeRequest {
            size: size.into(),
            ..Default::default()
        })
        .await
        .unwrap();
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn set_background(client: &PlaybackServiceClient<HttpClient>, background: Background) {
    client
        .set_subtitle_background(SetSubtitleBackgroundRequest {
            background: background.into(),
            ..Default::default()
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn a_users_size_and_background_are_theirs_alone_on_every_device_each_set_on_its_own() {
    let server = TestServer::builder().start().await;
    let ada = server.user("Ada", &[]).await;
    let lin = playback(&server.user("Lin", &[]).await);
    let phone = playback(&ada);
    let tv = playback(&server.sign_in(ada.id, "Ada").await);

    let new = appearance(&phone).await;
    set_size(&phone, Size::Largest).await;
    let sized = appearance(&tv).await;
    set_background(&tv, Background::Box).await;
    let boxed = appearance(&phone).await;
    set_size(&tv, Size::Small).await;
    let resized = appearance(&phone).await;
    let others = appearance(&lin).await;

    assert_eq!(new, (Size::Default, Background::Outline));
    assert_eq!(sized, (Size::Largest, Background::Outline));
    assert_eq!(boxed, (Size::Largest, Background::Box));
    assert_eq!(resized, (Size::Small, Background::Box));
    assert_eq!(others, (Size::Default, Background::Outline));
}

#[tokio::test]
async fn an_unspecified_or_unknown_size_or_background_is_refused() {
    let server = TestServer::builder().start().await;
    let ada = playback(&server.user("Ada", &[]).await);

    for value in [0, 99] {
        let size = ada
            .set_subtitle_size(SetSubtitleSizeRequest {
                size: EnumValue::from(value),
                ..Default::default()
            })
            .await;
        let background = ada
            .set_subtitle_background(SetSubtitleBackgroundRequest {
                background: EnumValue::from(value),
                ..Default::default()
            })
            .await;

        for (refused, field) in [
            (size.map(drop), "size"),
            (background.map(drop), "background"),
        ] {
            let refusal = assert_refused(
                refused,
                ErrorCode::InvalidArgument,
                RefusalReason::REFUSAL_REASON_UNKNOWN_ENUM_VALUE,
            );
            assert_eq!(refusal.field, field);
        }
    }
    assert_eq!(appearance(&ada).await, (Size::Default, Background::Outline));
}

#[tokio::test]
async fn a_user_who_picked_an_appearance_can_be_deleted() {
    let server = TestServer::builder().start().await;
    let ada = server.user("Ada", &[]).await;
    set_size(&playback(&ada), Size::Large).await;

    let deleted = UserServiceClient::new(HttpClient::plaintext(), server.client_config())
        .delete_user(DeleteUserRequest {
            user_id: ada.id,
            ..Default::default()
        })
        .await;

    assert!(deleted.is_ok(), "{deleted:?}");
}
