//! Each User turning off playing the next Episode by itself, for themself alone.

#[path = "support/server.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod server;

use connectrpc::client::HttpClient;
use nama_proto::nama::v1::{
    DeleteUserRequest, GetAutoplayRequest, PlaybackServiceClient, SetAutoplayRequest,
    UserServiceClient,
};
use server::TestServer;

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn is_on(client: &PlaybackServiceClient<HttpClient>) -> bool {
    client
        .get_autoplay(GetAutoplayRequest::default())
        .await
        .unwrap()
        .into_owned()
        .on
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn set(client: &PlaybackServiceClient<HttpClient>, on: bool) {
    client
        .set_autoplay(SetAutoplayRequest {
            on,
            ..Default::default()
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn autoplay_is_on_for_a_new_user_and_off_for_that_user_alone_until_turned_on() {
    let server = TestServer::builder().start().await;
    let ada = server.user("Ada", &[]).await;
    let lin = server.user("Lin", &[]).await;
    let ada = PlaybackServiceClient::new(HttpClient::plaintext(), ada.client_config());
    let lin = PlaybackServiceClient::new(HttpClient::plaintext(), lin.client_config());

    let new = is_on(&ada).await;
    set(&ada, false).await;
    set(&ada, false).await;
    let off = is_on(&ada).await;
    let others = is_on(&lin).await;
    set(&ada, true).await;
    let back_on = is_on(&ada).await;

    assert!(new);
    assert!(!off);
    assert!(others);
    assert!(back_on);
}

#[tokio::test]
async fn a_user_with_autoplay_off_can_be_deleted() {
    let server = TestServer::builder().start().await;
    let ada = server.user("Ada", &[]).await;
    set(
        &PlaybackServiceClient::new(HttpClient::plaintext(), ada.client_config()),
        false,
    )
    .await;

    let deleted = UserServiceClient::new(HttpClient::plaintext(), server.client_config())
        .delete_user(DeleteUserRequest {
            user_id: ada.id,
            ..Default::default()
        })
        .await;

    assert!(deleted.is_ok(), "{deleted:?}");
}
