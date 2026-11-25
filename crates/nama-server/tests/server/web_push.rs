//! Web Push: the Server's VAPID public key, which never changes, and each signed-in User's own
//! push subscriptions, which must be a browser's.

#[path = "support/refused.rs"]
mod refused;
#[path = "support/server.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod server;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use connectrpc::{ErrorCode, client::HttpClient};
use nama_proto::nama::v1::{
    GetWebPushKeyRequest, RefusalReason, SubscribeWebPushRequest, UnsubscribeWebPushRequest,
    WebPushServiceClient,
};
use refused::assert_refused;
use server::{SignedIn, TestServer};

fn web_push(who: &SignedIn) -> WebPushServiceClient<HttpClient> {
    WebPushServiceClient::new(HttpClient::plaintext(), who.client_config())
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn public_key(client: &WebPushServiceClient<HttpClient>) -> Vec<u8> {
    let got = client
        .get_web_push_key(GetWebPushKeyRequest::default())
        .await;
    got.unwrap().into_owned().public_key
}

/// A subscription as a browser makes one, with RFC 8291's example browser key.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn browsers(endpoint: &str) -> SubscribeWebPushRequest {
    let p256dh = URL_SAFE_NO_PAD
        .decode(
            "BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcxaOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4",
        )
        .unwrap();
    SubscribeWebPushRequest {
        endpoint: endpoint.to_owned(),
        p256dh,
        auth: vec![1; 16],
        ..Default::default()
    }
}

#[tokio::test]
async fn every_user_gets_the_same_public_key_and_a_restart_keeps_it() {
    let server = TestServer::builder().start().await;
    let ada = server.user_with_default_permissions("Ada").await;
    let admin = WebPushServiceClient::new(HttpClient::plaintext(), server.client_config());

    let key = public_key(&admin).await;

    assert_eq!(key.len(), 65);
    assert_eq!(key.first(), Some(&4));
    assert_eq!(public_key(&web_push(&ada)).await, key);
    let server = TestServer::builder().restart(server).await;
    let restarted = WebPushServiceClient::new(HttpClient::plaintext(), server.client_config());
    assert_eq!(public_key(&restarted).await, key);
}

#[tokio::test]
async fn a_browsers_subscription_is_taken_and_anything_else_refused_naming_its_field() {
    let server = TestServer::builder().start().await;
    let ada = server.user_with_default_permissions("Ada").await;
    let client = web_push(&ada);
    let endpoint = "https://push.example.net/send/ada";
    let mut short_key = browsers(endpoint);
    short_key.p256dh.pop();
    let mut long_auth = browsers(endpoint);
    long_auth.auth.push(1);

    client.subscribe_web_push(browsers(endpoint)).await.unwrap();
    let refused = [
        ("endpoint", browsers("http://push.example.net/send/ada")),
        ("p256dh", short_key),
        ("auth", long_auth),
    ];

    for (field, subscription) in refused {
        let refusal = assert_refused(
            client.subscribe_web_push(subscription).await,
            ErrorCode::InvalidArgument,
            RefusalReason::REFUSAL_REASON_WEB_PUSH_SUBSCRIPTION_INVALID,
        );
        assert_eq!(refusal.field, field);
    }
    let unsubscribe = UnsubscribeWebPushRequest {
        endpoint: endpoint.to_owned(),
        ..Default::default()
    };
    client.unsubscribe_web_push(unsubscribe).await.unwrap();
}
