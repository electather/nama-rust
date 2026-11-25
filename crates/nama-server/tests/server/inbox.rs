//! Each User's inbox: every Request notification the Server tells them is kept there, whether or
//! not a Notifier reaches them, and each User reads and marks only their own.

#[path = "support/refused.rs"]
mod refused;
#[path = "support/server.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod server;

use std::time::{Duration, Instant};

use connectrpc::{ErrorCode, client::HttpClient};
use nama_proto::nama::v1::{
    CountUnreadInboxNotificationsRequest, InboxNotification, InboxServiceClient,
    ListInboxNotificationsRequest, MarkAllInboxNotificationsReadRequest,
    MarkInboxNotificationReadRequest, NotificationKind as Kind, RefusalReason,
    SetNotificationPreferenceRequest, UserServiceClient,
};
use refused::assert_refused;
use server::{
    SignedIn, TestServer, WAIT,
    requests::Requester,
    tmdb::{ALIEN, DUNE},
};

fn inbox(who: &SignedIn) -> InboxServiceClient<HttpClient> {
    InboxServiceClient::new(HttpClient::plaintext(), who.client_config())
}

/// The caller's notifications, newest first.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn listed(inbox: &InboxServiceClient<HttpClient>) -> Vec<InboxNotification> {
    let listed = inbox
        .list_inbox_notifications(ListInboxNotificationsRequest::default())
        .await
        .unwrap();
    listed.into_owned().notifications
}

/// The caller's notifications once they hold `count`, since they are stored in the background.
async fn wait_for(inbox: &InboxServiceClient<HttpClient>, count: usize) -> Vec<InboxNotification> {
    let started = Instant::now();
    loop {
        let listed = listed(inbox).await;
        if listed.len() >= count {
            return listed;
        }
        assert!(started.elapsed() < WAIT, "the inbox holds {listed:?}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn kinds(listed: &[InboxNotification]) -> Vec<(Kind, &str)> {
    (listed.iter())
        .map(|notification| {
            (
                notification.kind.as_known().unwrap_or_default(),
                notification.title.as_str(),
            )
        })
        .collect()
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn unread(inbox: &InboxServiceClient<HttpClient>) -> u32 {
    let counted = inbox
        .count_unread_inbox_notifications(CountUnreadInboxNotificationsRequest::default())
        .await
        .unwrap();
    counted.into_owned().unread
}

#[tokio::test]
async fn each_requester_without_a_notifier_gets_a_notification_but_one_who_turned_its_kind_off() {
    let server = TestServer::builder().tmdb().start().await;
    let ada = server.user_with_default_permissions("Ada").await;
    let ben = server.user_with_default_permissions("Ben").await;
    let lin = server.user_with_default_permissions("Lin").await;
    UserServiceClient::new(HttpClient::plaintext(), lin.client_config())
        .set_notification_preference(SetNotificationPreferenceRequest {
            kind: Kind::Approved.into(),
            on: false,
            ..Default::default()
        })
        .await
        .unwrap();
    let dune = server.requests(&ada).ask(DUNE).await;
    server.requests(&ben).ask(DUNE).await;
    server.requests(&lin).ask(DUNE).await;

    server.approve(dune.id).await;

    let adas = wait_for(&inbox(&ada), 1).await;
    assert_eq!(kinds(&adas), [(Kind::Approved, "Dune")]);
    assert_eq!(adas[0].request_id, dune.id);
    assert_eq!(adas[0].season_number, None);
    assert_eq!(
        kinds(&listed(&inbox(&ben)).await),
        [(Kind::Approved, "Dune")]
    );
    assert_eq!(kinds(&listed(&inbox(&lin)).await), Vec::new());
    let admin = InboxServiceClient::new(HttpClient::plaintext(), server.client_config());
    assert_eq!(
        kinds(&wait_for(&admin, 1).await),
        [(Kind::NewPending, "Dune")]
    );
}

#[tokio::test]
async fn a_user_lists_counts_and_marks_read_only_their_own_notifications_newest_first() {
    let server = TestServer::builder().tmdb().start().await;
    let ada = server.user_with_default_permissions("Ada").await;
    let ben = server.user_with_default_permissions("Ben").await;
    let dune = server.requests(&ada).ask(DUNE).await;
    let alien = server.requests(&ada).ask(ALIEN).await;
    server.requests(&ben).ask(ALIEN).await;
    server.approve(dune.id).await;
    let (adas, bens) = (inbox(&ada), inbox(&ben));
    wait_for(&adas, 1).await;
    server.decline(alien.id, "").await;
    let notifications = wait_for(&adas, 2).await;
    wait_for(&bens, 1).await;

    let theirs = bens
        .mark_inbox_notification_read(MarkInboxNotificationReadRequest {
            id: notifications[0].id,
            ..Default::default()
        })
        .await;
    let before = unread(&adas).await;
    adas.mark_inbox_notification_read(MarkInboxNotificationReadRequest {
        id: notifications[0].id,
        ..Default::default()
    })
    .await
    .unwrap();
    let after_one = unread(&adas).await;
    adas.mark_all_inbox_notifications_read(MarkAllInboxNotificationsReadRequest::default())
        .await
        .unwrap();

    assert_eq!(
        kinds(&notifications),
        [(Kind::Declined, "Alien"), (Kind::Approved, "Dune")]
    );
    let refusal = assert_refused(
        theirs,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_INBOX_NOTIFICATION_NOT_FOUND,
    );
    assert_eq!(refusal.field, "id");
    assert_eq!(
        refusal.params.get("notificationId"),
        Some(&notifications[0].id.to_string())
    );
    assert_eq!((before, after_one, unread(&adas).await), (2, 1, 0));
    assert!(
        listed(&adas)
            .await
            .iter()
            .all(|read| read.read_at.is_some())
    );
    assert_eq!(unread(&bens).await, 1);
    assert_eq!(kinds(&listed(&bens).await), [(Kind::Declined, "Alien")]);
}
