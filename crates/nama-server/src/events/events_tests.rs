use nama_proto::nama::v1::{
    Event, FollowedShow, FollowedShowCreated, LibraryItemAdded, NotificationKind, PageRequest,
    Permission, Playback, PlaybackStarted, Request, RequestCancelled, RequestCreated, RequestState,
    RequestStateChanged, Requester, RequesterAdded,
};

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use nama_plugin_api::{
    BoxFuture, PluginError, PluginId,
    notifier::{Notification, Notifier, Recipient, RecipientGone},
};

use axum::http::StatusCode;
use url::Url;

use super::{Audience, EventBus, Filter, Viewer, deliver, event, users_to_tell};
use crate::{
    content_ratings::MaximumAge,
    db::Database,
    page::Page,
    plugins::{ExtensionPoints, ProviderError, without_secrets},
    server_settings::{ServerUrl, web_push_key},
    users::{
        LibraryAccess, Permissions, User, list_inbox, personal_settings_of,
        replace_personal_settings, set_notification_preference,
    },
    web_push::{assert_valid_vapid, browser, push_service, subscribe as subscribe_web_push},
};

fn user(id: i64, permissions: Permissions) -> User {
    User {
        id,
        name: format!("User {id}"),
        permissions,
        created_at: 0,
    }
}

/// Users 1 and 2 have a new User's Permissions, 3 has `MANAGE_REQUESTS` and 4 has `ADMIN`.
fn users() -> Vec<User> {
    vec![
        user(1, Permissions::NEW_USER),
        user(2, Permissions::NEW_USER),
        user(3, std::iter::once(Permission::ManageRequests).collect()),
        user(4, Permissions::ADMIN),
    ]
}

fn request(state: RequestState, requesters: &[i64]) -> Request {
    Request {
        state: state.into(),
        requesters: requesters
            .iter()
            .map(|&user_id| Requester {
                user_id,
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
}

fn created(state: RequestState, requesters: &[i64]) -> Event {
    event(RequestCreated {
        request: request(state, requesters).into(),
        ..Default::default()
    })
}

fn moved_to(state: RequestState, requesters: &[i64]) -> Event {
    event(RequestStateChanged {
        request: request(state, requesters).into(),
        ..Default::default()
    })
}

fn playback_of(user_id: i64) -> Event {
    event(PlaybackStarted {
        playback: Playback {
            user_id,
            ..Default::default()
        }
        .into(),
        ..Default::default()
    })
}

#[test]
fn each_viewer_gets_the_events_and_paths_their_permissions_let_them_see() {
    const REQUESTER: i64 = 3;
    const PLAYER: i64 = 4;
    let added = |path: &str| {
        event(LibraryItemAdded {
            library_id: 1,
            item_id: 1,
            path: path.to_owned(),
            ..Default::default()
        })
    };
    let (with_path, without_path) = (added("/media/Alien.mkv"), added(""));
    let requested = created(RequestState::Pending, &[REQUESTER]);
    let followed = event(FollowedShowCreated {
        followed_show: FollowedShow {
            requesters: request(RequestState::Pending, &[REQUESTER]).requesters,
            ..Default::default()
        }
        .into(),
        ..Default::default()
    });
    let played = playback_of(PLAYER);
    let requesting = || std::iter::once(Permission::Request).collect();
    // Each viewer, and what they get of `with_path`, `requested`, `followed` and `played`.
    let table = [
        (
            user(1, Permissions::ADMIN),
            [
                Some(&with_path),
                Some(&requested),
                Some(&followed),
                Some(&played),
            ],
        ),
        (
            user(2, std::iter::once(Permission::ManageLibraries).collect()),
            [Some(&with_path), None, None, None],
        ),
        (
            user(REQUESTER, requesting()),
            [Some(&without_path), Some(&requested), Some(&followed), None],
        ),
        (
            user(PLAYER, requesting()),
            [Some(&without_path), None, None, Some(&played)],
        ),
        (
            user(5, requesting()),
            [Some(&without_path), None, None, None],
        ),
    ];

    for (subscriber, expected) in table {
        let viewer = Viewer::of(&subscriber, LibraryAccess::All, MaximumAge::default());
        let got = [&with_path, &requested, &followed, &played]
            .map(|published| viewer.sees(published.clone()));
        assert_eq!(
            got,
            expected.map(Option::<&Event>::cloned),
            "User {}",
            subscriber.id
        );
    }
}

#[test]
fn a_viewer_gets_library_events_only_about_the_libraries_they_see() {
    let added = |library_id: i64| {
        event(LibraryItemAdded {
            library_id,
            item_id: 1,
            ..Default::default()
        })
    };
    let viewer = Viewer::of(
        &user(1, Permissions::ADMIN),
        LibraryAccess::Only(std::iter::once(1).collect()),
        MaximumAge::default(),
    );

    assert_eq!(viewer.sees(added(1)), Some(added(1)));
    assert_eq!(viewer.sees(added(2)), None);
    assert_eq!(viewer.sees(playback_of(1)), Some(playback_of(1)));
}

#[test]
fn naming_libraries_leaves_out_events_about_none() {
    let about_a_library = Filter {
        libraries: vec![5],
        ..Default::default()
    };

    assert!(!about_a_library.lets_through(&playback_of(1)));
}

#[test]
fn approving_or_declining_a_request_picks_its_requesters_and_nobody_else() {
    for state in [RequestState::Approved, RequestState::Declined] {
        assert_eq!(
            users_to_tell(&moved_to(state, &[2, 1]), &users(), &[]),
            [2, 1],
            "{state:?}"
        );
    }
}

#[test]
fn a_request_failing_or_becoming_available_picks_its_requesters() {
    for state in [
        RequestState::Failed,
        RequestState::PartiallyAvailable,
        RequestState::Available,
    ] {
        assert_eq!(
            users_to_tell(&moved_to(state, &[1]), &users(), &[]),
            [1],
            "{state:?}"
        );
    }
}

#[test]
fn a_followed_shows_season_opening_available_picks_its_requesters() {
    for state in [RequestState::PartiallyAvailable, RequestState::Available] {
        assert_eq!(
            users_to_tell(&created(state, &[1, 2]), &users(), &[]),
            [1, 2],
            "{state:?}"
        );
    }
}

#[test]
fn a_new_pending_request_picks_every_user_with_manage_requests() {
    let users = users();
    assert_eq!(
        users_to_tell(&created(RequestState::Pending, &[1]), &users, &[]),
        [3, 4]
    );
    assert_eq!(
        users_to_tell(&created(RequestState::Pending, &[3]), &users, &[]),
        [3, 4],
        "an asker with MANAGE_REQUESTS is told as one of them"
    );
}

#[test]
fn a_request_deleting_media_cancelled_picks_its_requesters() {
    let cancelled = event(RequestCancelled {
        request: request(RequestState::Processing, &[2, 1]).into(),
        deleted_by: Some("Grace".to_owned()),
        ..Default::default()
    });

    assert_eq!(users_to_tell(&cancelled, &users(), &[]), [2, 1]);
}

#[test]
fn a_user_who_turned_a_kind_off_is_not_picked_for_it_but_still_for_others() {
    let turned_off = [
        (1, NotificationKind::Approved),
        (3, NotificationKind::NewPending),
    ];
    let users = users();

    assert_eq!(
        users_to_tell(
            &moved_to(RequestState::Approved, &[1, 2]),
            &users,
            &turned_off
        ),
        [2]
    );
    assert_eq!(
        users_to_tell(
            &moved_to(RequestState::Available, &[1, 2]),
            &users,
            &turned_off
        ),
        [1, 2]
    );
    assert_eq!(
        users_to_tell(&created(RequestState::Pending, &[1]), &users, &turned_off),
        [4]
    );
}

#[test]
fn other_request_events_pick_nobody() {
    let pending = request(RequestState::Pending, &[1, 2]);
    let others = [
        event(RequesterAdded {
            request: pending.clone().into(),
            user_id: 2,
            ..Default::default()
        }),
        event(RequestCancelled {
            request: pending.into(),
            ..Default::default()
        }),
        // Opened Approved: its one Requester just asked for it.
        created(RequestState::Approved, &[1]),
        moved_to(RequestState::Processing, &[1]),
    ];
    for other in others {
        assert!(users_to_tell(&other, &users(), &[]).is_empty(), "{other:?}");
    }
}

#[test]
fn a_link_changing_picks_only_the_admins_whatever_they_turned_off() {
    let turned_off = [(4, NotificationKind::NewPending)];

    assert_eq!(Audience::Admins.pick(&users(), &turned_off), [4]);
}

/// A Notifier that keeps the Personal settings of each User it is handed, fails for those whose
/// `webhook` is [`FAILING_WEBHOOK`], naming it, and finds those whose `webhook` is
/// [`GONE_WEBHOOK`] gone.
#[derive(Default)]
struct KeepsUsers(Mutex<Vec<BTreeMap<String, String>>>);

const FAILING_WEBHOOK: &str = "https://chat.example.com/hooks/secret-42";
const GONE_WEBHOOK: &str = "https://chat.example.com/hooks/deleted";

impl Notifier for KeepsUsers {
    fn deliver<'a>(
        &'a self,
        _notification: &'a Notification,
        recipient: &'a Recipient,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        Box::pin(async move {
            let Recipient::User(values) = recipient else {
                return Ok(());
            };
            self.0.lock().unwrap().push(values.clone());
            match values.get("webhook") {
                Some(webhook) if webhook == FAILING_WEBHOOK => {
                    Err(format!("{webhook} answered 500").into())
                }
                Some(webhook) if webhook == GONE_WEBHOOK => Err(RecipientGone.into()),
                _ => Ok(()),
            }
        })
    }
}

async fn insert_user(db: &Database, name: &str) -> i64 {
    let name = name.to_owned();
    db.write(move |tx| {
        Box::pin(async move {
            sqlx::query_scalar(
                "INSERT INTO users (username, display_username, permissions, created_at,
                                    updated_at)
                 VALUES (?, ?, 0, 0, 0) RETURNING id",
            )
            .bind(&name)
            .bind(&name)
            .fetch_one(&mut **tx)
            .await
        })
    })
    .await
    .unwrap()
}

fn webhook(url: &str) -> BTreeMap<String, String> {
    BTreeMap::from([("webhook".to_owned(), url.to_owned())])
}

#[tokio::test]
async fn a_request_event_reaches_each_told_user_with_their_own_personal_settings_and_no_secret_in_errors()
 {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
    let plugin = PluginId::from_static("chat");
    let mut ids = Vec::new();
    for (name, url) in [
        ("ada", "https://chat.example.com/hooks/ada"),
        ("grace", FAILING_WEBHOOK),
        ("lin", "https://chat.example.com/hooks/lin"),
    ] {
        let id = insert_user(&db, name).await;
        let values = vec![("webhook".to_owned(), url.to_owned(), true)];
        replace_personal_settings(&db, id, &plugin, values)
            .await
            .unwrap();
        ids.push(id);
    }
    let (ada, grace) = (ids[0], ids[1]);
    let notifier = Arc::new(KeepsUsers::default());
    // Approved tells the Requesters, Ada and Grace, and not Lin.
    let told = Arc::new((
        Audience::Request(moved_to(RequestState::Approved, &[ada, grace])),
        Notification::default(),
    ));

    let failed = deliver(
        Arc::clone(&db),
        plugin,
        Arc::<KeepsUsers>::clone(&notifier),
        told,
    )
    .await
    .expect_err("Grace\'s delivery fails")
    .failure()
    .message;

    let mut reached = notifier.0.lock().unwrap().clone();
    reached.sort();
    assert_eq!(
        reached,
        [
            webhook("https://chat.example.com/hooks/ada"),
            webhook(FAILING_WEBHOOK)
        ]
    );
    assert!(failed.contains(&format!("User {grace}")), "{failed}");
    assert!(!failed.contains(FAILING_WEBHOOK), "{failed}");
}

#[derive(Default)]
struct KeepsRecipients(Mutex<Vec<Recipient>>);

impl Notifier for KeepsRecipients {
    fn deliver<'a>(
        &'a self,
        _notification: &'a Notification,
        recipient: &'a Recipient,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        self.0.lock().unwrap().push(recipient.clone());
        Box::pin(async { Ok(()) })
    }
}

#[tokio::test]
async fn shutting_down_stores_what_was_published_before_and_lets_go_of_the_database() {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
    let ada = insert_user(&db, "ada").await;
    let bus = EventBus::new(
        Arc::clone(&db),
        ExtensionPoints::default(),
        ServerUrl::default(),
    );

    bus.publish(vec![moved_to(RequestState::Approved, &[ada])]);
    bus.shutdown().await;
    bus.publish(vec![moved_to(RequestState::Declined, &[ada])]);
    drop(bus);

    assert_eq!(Arc::strong_count(&db), 1);
    let page = Page::read(&PageRequest::default(), b"").unwrap();
    let (stored, _) = list_inbox(&db, ada, &page).await.unwrap();
    let kinds: Vec<_> = (stored.iter())
        .map(|notification| notification.kind.as_known())
        .collect();
    assert_eq!(kinds, [Some(NotificationKind::Approved)]);
}

#[tokio::test]
async fn a_user_who_turned_a_kind_off_is_not_told_it_and_the_channel_still_is() {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
    let plugin = PluginId::from_static("chat");
    let mut ids = Vec::new();
    for name in ["ada", "lin"] {
        let id = insert_user(&db, name).await;
        let values = vec![("webhook".to_owned(), name.to_owned(), false)];
        replace_personal_settings(&db, id, &plugin, values)
            .await
            .unwrap();
        ids.push(id);
    }
    let (ada, lin) = (ids[0], ids[1]);
    set_notification_preference(&db, ada, NotificationKind::Approved, false)
        .await
        .unwrap();
    let notifier = Arc::new(KeepsRecipients::default());
    let told = Arc::new((
        Audience::Request(moved_to(RequestState::Approved, &[ada, lin])),
        Notification::default(),
    ));

    deliver(db, plugin, Arc::<KeepsRecipients>::clone(&notifier), told)
        .await
        .unwrap_or_else(|_| panic!("every delivery succeeds"));

    let mut reached = notifier.0.lock().unwrap().clone();
    reached.sort_by_key(|recipient| matches!(recipient, Recipient::User(_)));
    assert!(
        reached == [Recipient::Channel, Recipient::User(webhook("lin"))],
        "Ada turned Approved off, so only the channel and Lin are told"
    );
}

#[tokio::test]
async fn a_user_recipient_the_notifier_finds_gone_loses_their_personal_settings() {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
    let plugin = PluginId::from_static("chat");
    let mut ids = Vec::new();
    for (name, url) in [
        ("ada", GONE_WEBHOOK),
        ("lin", "https://chat.example.com/hooks/lin"),
    ] {
        let id = insert_user(&db, name).await;
        let values = vec![("webhook".to_owned(), url.to_owned(), true)];
        replace_personal_settings(&db, id, &plugin, values)
            .await
            .unwrap();
        ids.push(id);
    }
    let (ada, lin) = (ids[0], ids[1]);
    let told = Arc::new((
        Audience::Request(moved_to(RequestState::Approved, &[ada, lin])),
        Notification::default(),
    ));

    deliver(
        Arc::clone(&db),
        plugin.clone(),
        Arc::new(KeepsUsers::default()),
        told,
    )
    .await
    .unwrap_or_else(|_| panic!("a gone recipient is no failure"));

    assert!(
        personal_settings_of(&db, ada, &plugin)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        !personal_settings_of(&db, lin, &plugin)
            .await
            .unwrap()
            .is_empty()
    );
}

#[test]
fn an_empty_secret_leaves_a_message_alone() {
    assert_eq!(
        without_secrets("it failed".to_owned(), &[String::new()]),
        "it failed"
    );
}

#[tokio::test]
async fn a_published_notification_is_pushed_to_its_users_browsers_but_not_a_kind_turned_off() {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
    let (ada, lin) = (insert_user(&db, "ada").await, insert_user(&db, "lin").await);
    set_notification_preference(&db, ada, NotificationKind::Approved, false)
        .await
        .unwrap();
    let (ada_endpoint, ada_got) = push_service(StatusCode::CREATED).await;
    let (lin_endpoint, lin_got) = push_service(StatusCode::CREATED).await;
    subscribe_web_push(&db, ada, browser(&ada_endpoint))
        .await
        .unwrap();
    subscribe_web_push(&db, lin, browser(&lin_endpoint))
        .await
        .unwrap();
    let bus = EventBus::new(
        Arc::clone(&db),
        ExtensionPoints::default(),
        ServerUrl::default(),
    );

    bus.publish(vec![moved_to(RequestState::Approved, &[ada, lin])]);
    bus.shutdown().await;

    assert!(ada_got.lock().unwrap().is_empty());
    let (headers, _) = lin_got.lock().unwrap().pop().unwrap();
    let key = web_push_key(&db).await.unwrap();
    let origin = Url::parse(&lin_endpoint)
        .unwrap()
        .origin()
        .ascii_serialization();
    let authorization = headers["authorization"].to_str().unwrap();
    assert_valid_vapid(authorization, &origin, key.public_key());
}
