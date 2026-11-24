//! Serving a Version's file as it is: off the local network, a User capped for remote playback
//! gets it only through a plan that Direct Plays it, which kept to their cap.

use super::*;

const REMOTE: &str = "203.0.113.7";
const LOCAL: &str = "192.168.1.20";

/// Alien's Version on a Server behind a trusted proxy, and Bishop, capped at 720p remotely.
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a test helper fails its test by panicking"
)]
async fn capped_bishop() -> (Fixture, i64, SignedIn) {
    let fx = Fixture::alien_on(behind_proxy()).await;
    let version = fx.version_ids(ALIEN).await[0];
    let bishop = fx.server.user_with_default_permissions("Bishop").await;
    (UserServiceClient::new(HttpClient::plaintext(), fx.server.client_config()))
        .set_permissions(SetPermissionsRequest {
            user_id: bishop.id,
            remote_playback_limit: PlaybackLimit::PLAYBACK_LIMIT_720P.into(),
            ..Default::default()
        })
        .await
        .unwrap();
    (fx, version, bishop)
}

/// The status of `token`'s request for the Version `version`'s file, relayed for `client`.
async fn status_for(fx: &Fixture, version: i64, token: &str, client: &str) -> u16 {
    let path = format!("/media/versions/{version}");
    let headers = [("x-forwarded-for", client)];
    (fetch_with(&fx.server, &path, Some(token), &headers).await).status
}

#[tokio::test]
async fn a_capped_user_off_the_local_network_is_refused_a_version_no_plan_direct_plays() {
    let (fx, version, bishop) = capped_bishop().await;

    assert_eq!(status_for(&fx, version, &bishop.token, REMOTE).await, 403);
}

#[tokio::test]
async fn a_capped_user_off_the_local_network_is_sent_a_version_their_plan_direct_plays() {
    let (fx, version, bishop) = capped_bishop().await;
    let remotely = (bishop.client_config()).with_default_header("x-forwarded-for", REMOTE);
    let original = picking(version, PlaybackLimit::PLAYBACK_LIMIT_ORIGINAL);
    let plan = ask_for_plan_as(remotely, original).await.unwrap();
    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY);

    assert_eq!(status_for(&fx, version, &bishop.token, REMOTE).await, 200);
}

#[tokio::test]
async fn a_capped_user_on_the_local_network_is_sent_a_version_with_no_plan() {
    let (fx, version, bishop) = capped_bishop().await;

    let status = status_for(&fx, version, &bishop.token, LOCAL).await;

    assert_eq!(status, 200);
}

#[tokio::test]
async fn an_uncapped_user_off_the_local_network_is_sent_a_version_with_no_plan() {
    let (fx, version, _) = capped_bishop().await;

    let status = status_for(&fx, version, &fx.server.admin_token, REMOTE).await;

    assert_eq!(status, 200);
}
