//! Each User turning kinds of Request notification on and off for themself: every kind on for a
//! new User, and new Pending offered only to one with `PERMISSION_MANAGE_REQUESTS`.

use crate::support::{refused, server};

use buffa::EnumValue;
use connectrpc::{ErrorCode, client::HttpClient};
use nama_proto::nama::v1::{
    ListNotificationPreferencesRequest, NotificationKind as Kind, Permission, RefusalReason,
    SetNotificationPreferenceRequest, UserServiceClient,
};
use refused::assert_refused;
use server::TestServer;

/// Each kind the caller is offered, in order, with whether it is on.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn listed(client: &UserServiceClient<HttpClient>) -> Vec<(Kind, bool)> {
    let listed = client
        .list_notification_preferences(ListNotificationPreferencesRequest::default())
        .await
        .unwrap()
        .into_owned();
    listed
        .preferences
        .into_iter()
        .map(|preference| (preference.kind.as_known().unwrap(), preference.on))
        .collect()
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn set(client: &UserServiceClient<HttpClient>, kind: Kind, on: bool) {
    client
        .set_notification_preference(SetNotificationPreferenceRequest {
            kind: kind.into(),
            on,
            ..Default::default()
        })
        .await
        .unwrap();
}

const EVERY_REQUESTER_KIND: [Kind; 6] = [
    Kind::Approved,
    Kind::Declined,
    Kind::Failed,
    Kind::PartiallyAvailable,
    Kind::Available,
    Kind::CancelledByDeletion,
];

#[tokio::test]
async fn a_new_user_gets_every_kind_and_only_one_who_manages_requests_is_offered_new_pending() {
    let server = TestServer::builder().start().await;
    let ada = server.user("Ada", &[Permission::Request]).await;
    let grace = server.user("Grace", &[Permission::ManageRequests]).await;
    let ada = UserServiceClient::new(HttpClient::plaintext(), ada.client_config());
    let grace = UserServiceClient::new(HttpClient::plaintext(), grace.client_config());

    assert_eq!(
        listed(&ada).await,
        EVERY_REQUESTER_KIND.map(|kind| (kind, true))
    );
    let mut managing = EVERY_REQUESTER_KIND.map(|kind| (kind, true)).to_vec();
    managing.push((Kind::NewPending, true));
    assert_eq!(listed(&grace).await, managing);
}

#[tokio::test]
async fn a_kind_turned_off_stays_off_for_that_user_alone_until_turned_on() {
    let server = TestServer::builder().start().await;
    let ada = server.user("Ada", &[]).await;
    let lin = server.user("Lin", &[]).await;
    let ada = UserServiceClient::new(HttpClient::plaintext(), ada.client_config());
    let lin = UserServiceClient::new(HttpClient::plaintext(), lin.client_config());

    set(&ada, Kind::Approved, false).await;
    set(&ada, Kind::Approved, false).await;
    let off = listed(&ada).await;
    let others = listed(&lin).await;
    set(&ada, Kind::Approved, true).await;
    let back_on = listed(&ada).await;

    assert_eq!(off[0], (Kind::Approved, false));
    assert!(off[1..].iter().all(|&(_, on)| on), "{off:?}");
    assert!(others.iter().all(|&(_, on)| on), "{others:?}");
    assert!(back_on.iter().all(|&(_, on)| on), "{back_on:?}");
}

#[tokio::test]
async fn an_unspecified_or_unknown_kind_is_refused() {
    let server = TestServer::builder().start().await;
    let ada = server.user("Ada", &[]).await;
    let ada = UserServiceClient::new(HttpClient::plaintext(), ada.client_config());

    for kind in [EnumValue::from(Kind::Unspecified), EnumValue::Unknown(99)] {
        let refused = ada
            .set_notification_preference(SetNotificationPreferenceRequest {
                kind,
                ..Default::default()
            })
            .await;

        let refusal = assert_refused(
            refused,
            ErrorCode::InvalidArgument,
            RefusalReason::REFUSAL_REASON_UNKNOWN_ENUM_VALUE,
        );
        assert_eq!(refusal.field, "kind");
    }
}
