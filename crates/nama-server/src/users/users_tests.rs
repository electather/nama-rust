use buffa::Enumeration;
use nama_proto::nama::v1::Permission;

use super::{rate_limits::Attempt, *};
use crate::plugins::Registry;

async fn create_user(db: &Database, name: &str) -> Result<User, Error> {
    let name = name.to_owned();
    db.write(move |tx| Box::pin(async move { insert_user(tx, &name, Permissions::NEW_USER).await }))
        .await
}

/// An Admin who isn't stored, to make changes as.
fn admin() -> User {
    User {
        id: 0,
        name: "Root".to_owned(),
        permissions: Permissions::ADMIN,
        created_at: 0,
    }
}

const TOKEN: &str = "nama_test-token";

/// 90 days, in seconds, counted independently of the code under test.
const NINETY_DAYS: i64 = 90 * 24 * 60 * 60;

/// [`Users`] on a fresh database in `data_dir` with one User, Ada, who has
/// [`Permissions::NEW_USER`].
async fn ada(data_dir: &std::path::Path) -> (Users, User) {
    let db = Arc::new(Database::open(data_dir).await.unwrap());
    let ada = create_user(&db, "Ada").await.unwrap();
    (Users::new(db, ExtensionPoints::default()), ada)
}

/// [`Users`] on a fresh database in `data_dir` with one Device, whose session is [`TOKEN`],
/// and when it was signed in. It was last seen then, or a second after.
async fn signed_in_device(data_dir: &std::path::Path) -> (Users, i64) {
    let (users, ada) = ada(data_dir).await;
    let signed_in_at = now();
    users
        .db
        .write(move |tx| {
            Box::pin(
                async move { insert_device(tx, ada.id, None, laptop(), &hash_token(TOKEN)).await },
            )
        })
        .await
        .unwrap();
    (users, signed_in_at)
}

#[tokio::test]
async fn a_device_unused_for_90_days_no_longer_authenticates() {
    let data_dir = tempfile::tempdir().unwrap();
    let (users, signed_in_at) = signed_in_device(data_dir.path()).await;

    let session = users
        .session_at(TOKEN, signed_in_at + 1 + NINETY_DAYS)
        .await;

    assert!(session.unwrap().is_none());
}

#[tokio::test]
async fn a_device_used_within_90_days_still_authenticates() {
    let data_dir = tempfile::tempdir().unwrap();
    let (users, signed_in_at) = signed_in_device(data_dir.path()).await;

    let session = users
        .session_at(TOKEN, signed_in_at + NINETY_DAYS - 1)
        .await;

    assert!(session.unwrap().is_some());
}

#[tokio::test]
async fn using_a_device_within_the_hour_it_was_last_seen_leaves_last_seen_alone() {
    let data_dir = tempfile::tempdir().unwrap();
    let (users, signed_in_at) = signed_in_device(data_dir.path()).await;

    let used = users.session_at(TOKEN, signed_in_at + 1800).await.unwrap();
    // Still counted from the sign-in, so not written by the use above.
    let later = users
        .session_at(TOKEN, signed_in_at + 1 + NINETY_DAYS)
        .await;

    assert!(!used.unwrap().refreshed);
    assert!(later.unwrap().is_none(), "last seen was written");
}

#[tokio::test]
async fn using_a_device_an_hour_after_it_was_last_seen_writes_last_seen() {
    let data_dir = tempfile::tempdir().unwrap();
    let (users, signed_in_at) = signed_in_device(data_dir.path()).await;
    let used_at = signed_in_at + 1 + 3600;

    let used = users.session_at(TOKEN, used_at).await.unwrap();
    // Counted from the use above, so it was written.
    let later = users.session_at(TOKEN, used_at + NINETY_DAYS - 1).await;

    assert!(used.unwrap().refreshed);
    assert!(later.unwrap().is_some(), "last seen wasn't written");
}

/// How long a test waits for a sign-out to be noticed before failing.
const WAIT: std::time::Duration = std::time::Duration::from_secs(10);

/// The id of the Device [`signed_in_device`] made, signed in at `signed_in_at`.
async fn device_id(users: &Users, signed_in_at: i64) -> i64 {
    let session = users.session_at(TOKEN, signed_in_at).await.unwrap();
    session.unwrap().caller.device.id
}

#[tokio::test]
async fn waiting_for_a_device_already_signed_out_ends_at_once() {
    let data_dir = tempfile::tempdir().unwrap();
    let (users, signed_in_at) = signed_in_device(data_dir.path()).await;
    let device = device_id(&users, signed_in_at).await;

    // As when the sign-out lands after the session was checked but before the wait began.
    users.sign_out(device).await.unwrap();
    let waited = tokio::time::timeout(WAIT, users.signed_out(device)).await;

    assert!(waited.is_ok(), "the wait didn't end");
}

#[tokio::test]
async fn a_burst_of_other_sign_outs_doesnt_end_the_wait_for_a_device() {
    let data_dir = tempfile::tempdir().unwrap();
    let (users, signed_in_at) = signed_in_device(data_dir.path()).await;
    let device = device_id(&users, signed_in_at).await;
    let mut waiting = std::pin::pin!(users.signed_out(device));

    // More than the waiter can buffer, so it falls behind.
    for other in (device + 1..).take(SIGN_OUTS_BUFFERED * 2) {
        users.sign_out(other).await.unwrap();
    }
    let during_burst =
        tokio::time::timeout(std::time::Duration::from_millis(200), &mut waiting).await;
    // Checked before waiting again, since a wait that ended can't be waited on.
    assert!(during_burst.is_err(), "other sign-outs ended the wait");
    users.sign_out(device).await.unwrap();
    let after_sign_out = tokio::time::timeout(WAIT, waiting).await;

    assert!(
        after_sign_out.is_ok(),
        "signing the Device out didn't end it"
    );
}

#[tokio::test]
async fn a_sign_out_whose_caller_stops_waiting_still_ends_the_wait() {
    let data_dir = tempfile::tempdir().unwrap();
    let (users, signed_in_at) = signed_in_device(data_dir.path()).await;
    let device = device_id(&users, signed_in_at).await;
    let mut waiting = std::pin::pin!(users.signed_out(device));
    let before = tokio::time::timeout(std::time::Duration::from_millis(200), &mut waiting).await;
    assert!(before.is_err(), "the wait ended before the sign-out");

    // Polled once, which queues the write, then dropped, as when a client disconnects.
    let mut abandoned = Box::pin(users.sign_out(device));
    let polled = abandoned
        .as_mut()
        .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()));
    assert!(polled.is_pending(), "the sign-out finished at once");
    drop(abandoned);
    let after = tokio::time::timeout(WAIT, waiting).await;

    assert!(after.is_ok(), "the wait didn't end");
}

#[tokio::test]
async fn waiting_for_a_change_to_permissions_that_already_differ_ends_at_once() {
    let data_dir = tempfile::tempdir().unwrap();
    let (users, ada) = ada(data_dir.path()).await;

    // As when the change lands after the stream's request read them but before the wait began.
    let waiting = users.permissions_changed(ada.id, Permissions::ADMIN);
    let waited = tokio::time::timeout(WAIT, waiting).await;

    assert!(waited.is_ok(), "the wait didn't end");
}

#[tokio::test]
async fn waiting_for_a_change_to_a_user_who_doesnt_exist_ends_at_once() {
    let data_dir = tempfile::tempdir().unwrap();
    let (users, ada) = ada(data_dir.path()).await;

    let waiting = users.permissions_changed(ada.id + 1, ada.permissions);
    let waited = tokio::time::timeout(WAIT, waiting).await;

    assert!(waited.is_ok(), "the wait didn't end");
}

#[tokio::test]
async fn a_name_another_user_has_in_a_different_case_is_taken() {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Database::open(data_dir.path()).await.unwrap();
    create_user(&db, "Émile").await.unwrap();

    let duplicate = create_user(&db, "éMILE").await;

    assert!(matches!(duplicate, Err(Error::NameTaken)), "{duplicate:?}");
}

#[tokio::test]
async fn a_name_another_user_has_composed_differently_or_in_a_different_case_is_taken() {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Database::open(data_dir.path()).await.unwrap();
    create_user(&db, "R\u{E9}my").await.unwrap();

    for duplicate in ["Re\u{301}my", "RE\u{301}MY"] {
        let duplicate = create_user(&db, duplicate).await;

        assert!(matches!(duplicate, Err(Error::NameTaken)), "{duplicate:?}");
    }
}

#[tokio::test]
async fn demoting_two_admins_at_once_leaves_exactly_one() {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
    let users = Users::new(Arc::clone(&db), ExtensionPoints::default());
    let ada = create_user(&db, "Ada").await.unwrap().id;
    let grace = create_user(&db, "Grace").await.unwrap().id;
    users
        .set_permissions(&admin(), ada, Permissions::ADMIN)
        .await
        .unwrap();
    users
        .set_permissions(&admin(), grace, Permissions::ADMIN)
        .await
        .unwrap();

    // Unlike over the API, no access check stands between the two and the database.
    let root = admin();
    let (ada_demoted, grace_demoted) = tokio::join!(
        users.set_permissions(&root, ada, Permissions::NEW_USER),
        users.set_permissions(&root, grace, Permissions::NEW_USER),
    );

    let refused = [&ada_demoted, &grace_demoted]
        .into_iter()
        .filter(|demoted| matches!(demoted, Err(Error::LastAdmin)))
        .count();
    assert_eq!(refused, 1, "{ada_demoted:?} / {grace_demoted:?}");
    let admins = users.list().await.unwrap();
    let admins = admins
        .iter()
        .filter(|user| user.permissions == Permissions::ADMIN);
    assert_eq!(admins.count(), 1);
}

#[test]
fn request_covers_requesting_movies_and_shows_but_not_auto_approval() {
    let request = Permissions::from_iter([Permission::Request]);

    assert!(request.allow(Permission::RequestMovie));
    assert!(request.allow(Permission::RequestTv));
    assert!(!request.allow(Permission::AutoApproveMovie));
    assert!(!request.allow(Permission::ViewRequests));
}

#[test]
fn auto_approve_covers_its_movie_and_show_variants() {
    let auto_approve = Permissions::from_iter([Permission::AutoApprove]);

    assert!(auto_approve.allow(Permission::AutoApproveMovie));
    assert!(auto_approve.allow(Permission::AutoApproveTv));
    assert!(!auto_approve.allow(Permission::RequestMovie));
}

#[test]
fn a_variant_does_not_cover_its_parent_or_its_sibling() {
    let movies = Permissions::from_iter([Permission::RequestMovie]);

    assert!(movies.allow(Permission::RequestMovie));
    assert!(!movies.allow(Permission::Request));
    assert!(!movies.allow(Permission::RequestTv));
}

#[test]
fn admin_passes_every_check() {
    for &needed in Permission::values() {
        assert!(Permissions::ADMIN.allow(needed), "{needed:?}");
    }
}

#[test]
fn manage_libraries_does_not_cover_managing_users() {
    let libraries = Permissions::from_iter([Permission::ManageLibraries]);

    assert!(libraries.allow(Permission::ManageLibraries));
    assert!(!libraries.allow(Permission::ManageUsers));
}

#[test]
fn permissions_list_each_held_one_in_value_order() {
    let held = Permissions::from_iter([
        Permission::ViewRequests,
        Permission::Request,
        Permission::ViewRequests,
    ]);

    assert_eq!(
        held.iter().collect::<Vec<_>>(),
        [Permission::Request, Permission::ViewRequests]
    );
    assert_eq!(held, Permissions::NEW_USER);
}

/// [`Users`] on a fresh database in `data_dir`, whose Authenticator is `local-auth`.
async fn users_with_local_auth(data_dir: &std::path::Path) -> Users {
    let db = Arc::new(Database::open(data_dir).await.unwrap());
    let plugins = Registry::new();
    plugins
        .enable(Arc::new(nama_plugin_local_auth::LocalAuth))
        .unwrap();
    Users::new(db, plugins.extension_points())
}

const PASSWORD: &str = "correct horse battery staple";

fn laptop() -> NewDevice {
    NewDevice {
        name: "Laptop".to_owned(),
        ip_address: "192.0.2.7".parse().unwrap(),
        user_agent: Some("TestAgent/1.0".to_owned()),
    }
}

/// A guess let through by a throwaway rate limiter, which `verify` asks for.
fn admitted() -> Attempt {
    rate_limits::SignInLimits::default()
        .admit(laptop().ip_address, None, std::time::Instant::now())
        .unwrap()
}

/// Signs [`laptop`] in as the User named `name`, whose password is [`PASSWORD`].
async fn sign_in_with_password(users: &Users, name: &str) -> SignedIn {
    let (user, method) = users.verify(&admitted(), name, PASSWORD).await.unwrap();
    users
        .sign_in_as(Proved::Method { user, method }, laptop())
        .await
        .unwrap()
}

/// How the Device `device` signed in: the name of its Sign-in Method, its IP address and user
/// agent, and when it last proved who it is.
async fn sign_in_details(
    users: &Users,
    device: i64,
) -> (Option<String>, String, Option<String>, i64) {
    let row = sqlx::query!(
        r#"SELECT m.name AS "method?", d.ip_address, d.user_agent, d.authenticated_at
               FROM devices d LEFT JOIN sign_in_methods m ON m.id = d.sign_in_method_id
               WHERE d.id = ?"#,
        device
    )
    .fetch_one(users.db.reader())
    .await
    .unwrap();
    (
        row.method,
        row.ip_address,
        row.user_agent,
        row.authenticated_at,
    )
}

/// Gives the User `user` a Sign-in Method that isn't a password, and returns its id.
async fn add_other_sign_in_method(users: &Users, user: i64) -> i64 {
    users
        .db
        .write(move |tx| {
            Box::pin(async move {
                sqlx::query_scalar!(
                    r#"INSERT INTO sign_in_methods
                               (user_id, provider_id, subject, credential, name, created_at,
                                updated_at)
                           VALUES (?, 'test', 'their-id', '', 'Test', 0, 0)
                           RETURNING id AS "id!""#,
                    user
                )
                .fetch_one(&mut **tx)
                .await
            })
        })
        .await
        .unwrap()
}

async fn delete_sign_in_method(users: &Users, method: i64) -> Result<(), sqlx::Error> {
    users
        .db
        .write(move |tx| {
            Box::pin(async move {
                sqlx::query!("DELETE FROM sign_in_methods WHERE id = ?", method)
                    .execute(&mut **tx)
                    .await
            })
        })
        .await?;
    Ok(())
}

async fn expire_devices(users: &Users, at: i64) {
    users
        .db
        .write(move |tx| {
            Box::pin(async move {
                sqlx::query!("UPDATE devices SET expires_at = ?", at)
                    .execute(&mut **tx)
                    .await
            })
        })
        .await
        .unwrap();
}

async fn password_method(users: &Users, user: i64) -> i64 {
    sqlx::query_scalar!(
        r#"SELECT id AS "id!" FROM sign_in_methods WHERE user_id = ? AND subject IS NULL"#,
        user
    )
    .fetch_one(users.db.reader())
    .await
    .unwrap()
}

#[tokio::test]
async fn a_device_signed_in_with_a_password_records_its_method_ip_user_agent_and_when() {
    let data_dir = tempfile::tempdir().unwrap();
    let users = users_with_local_auth(data_dir.path()).await;
    users.create(&admin(), "Ada", PASSWORD).await.unwrap();
    let before = now();

    let signed_in = sign_in_with_password(&users, "ada").await;

    let (method, ip, user_agent, authenticated_at) =
        sign_in_details(&users, signed_in.caller.device.id).await;
    assert_eq!(method.as_deref(), Some("Password"));
    assert_eq!(ip, "192.0.2.7");
    assert_eq!(user_agent.as_deref(), Some("TestAgent/1.0"));
    assert!(
        (before..=now()).contains(&authenticated_at),
        "{authenticated_at}"
    );
}

#[tokio::test]
async fn a_name_signs_in_however_its_accents_are_composed() {
    let data_dir = tempfile::tempdir().unwrap();
    let users = users_with_local_auth(data_dir.path()).await;
    users.create(&admin(), "R\u{E9}my", PASSWORD).await.unwrap();
    users
        .create(&admin(), "Zoe\u{308}", PASSWORD)
        .await
        .unwrap();

    for name in ["Re\u{301}my", "RE\u{301}MY", "Zo\u{EB}"] {
        let signed_in = users.verify(&admitted(), name, PASSWORD).await;

        assert!(signed_in.is_ok(), "{name}: {:?}", signed_in.err());
    }
}

#[tokio::test]
async fn a_user_deleted_between_verify_and_sign_in_is_refused_as_a_wrong_name_or_password() {
    let data_dir = tempfile::tempdir().unwrap();
    let users = users_with_local_auth(data_dir.path()).await;
    let ada = users.create_first_admin("Ada", PASSWORD, laptop()).await;
    let ada = ada.unwrap().caller.user;
    let grace = users.create(&ada, "Grace", PASSWORD).await.unwrap();
    let (user, method) = users.verify(&admitted(), "Grace", PASSWORD).await.unwrap();
    users.delete(&ada, grace).await.unwrap();

    let refused = users
        .sign_in_as(Proved::Method { user, method }, laptop())
        .await;

    assert!(
        matches!(refused, Err(Error::WrongNameOrPassword)),
        "{:?}",
        refused.err()
    );
}

#[tokio::test]
async fn a_device_signed_in_by_a_sign_in_code_records_no_method_and_never_authenticated() {
    let data_dir = tempfile::tempdir().unwrap();
    let users = users_with_local_auth(data_dir.path()).await;
    users.create(&admin(), "Ada", PASSWORD).await.unwrap();
    let approving = sign_in_with_password(&users, "Ada").await;

    let signed_in = users
        .sign_in_as(
            Proved::Code(ApprovingDevice::from(&approving.caller)),
            laptop(),
        )
        .await
        .unwrap();

    let (method, ip, _, authenticated_at) =
        sign_in_details(&users, signed_in.caller.device.id).await;
    assert_eq!(
        (method, ip.as_str(), authenticated_at),
        (None, "192.0.2.7", 0)
    );
}

#[tokio::test]
async fn a_sign_in_code_approved_from_a_device_that_expired_since_is_refused() {
    let data_dir = tempfile::tempdir().unwrap();
    let users = users_with_local_auth(data_dir.path()).await;
    users.create(&admin(), "Ada", PASSWORD).await.unwrap();
    let approving = sign_in_with_password(&users, "Ada").await;
    expire_devices(&users, now() - 1).await;

    let refused = users
        .sign_in_as(
            Proved::Code(ApprovingDevice::from(&approving.caller)),
            laptop(),
        )
        .await;

    assert!(
        matches!(refused, Err(Error::ApprovingDeviceSignedOut)),
        "{:?}",
        refused.err()
    );
}

#[tokio::test]
async fn a_device_past_its_expiry_is_no_session_however_recently_it_was_seen() {
    let data_dir = tempfile::tempdir().unwrap();
    let (users, _) = signed_in_device(data_dir.path()).await;
    expire_devices(&users, now() - 1).await;

    let session = users.session(TOKEN).await;

    assert!(session.unwrap().is_none());
}

#[tokio::test]
async fn deleting_a_sign_in_method_signs_out_only_the_devices_that_used_it() {
    let data_dir = tempfile::tempdir().unwrap();
    let users = users_with_local_auth(data_dir.path()).await;
    let ada = users.create(&admin(), "Ada", PASSWORD).await.unwrap();
    add_other_sign_in_method(&users, ada).await;
    let by_password = sign_in_with_password(&users, "Ada").await;
    let by_code = users
        .sign_in_as(
            Proved::Code(ApprovingDevice::from(&by_password.caller)),
            laptop(),
        )
        .await
        .unwrap();

    let password = password_method(&users, ada).await;
    delete_sign_in_method(&users, password).await.unwrap();

    assert!(!users.still_signed_in(by_password.caller.device.id).await);
    assert!(users.still_signed_in(by_code.caller.device.id).await);
}

#[tokio::test]
async fn a_users_last_sign_in_method_cant_be_deleted() {
    let data_dir = tempfile::tempdir().unwrap();
    let users = users_with_local_auth(data_dir.path()).await;
    let ada = users.create(&admin(), "Ada", PASSWORD).await.unwrap();

    let password = password_method(&users, ada).await;
    let deleted = delete_sign_in_method(&users, password).await;

    assert!(deleted.is_err(), "the last Sign-in Method was deleted");
}

#[tokio::test]
async fn resetting_the_password_of_a_user_without_one_adds_it_and_keeps_their_other_method() {
    let data_dir = tempfile::tempdir().unwrap();
    let users = users_with_local_auth(data_dir.path()).await;
    let ada = create_user(&users.db, "Ada").await.unwrap().id;
    add_other_sign_in_method(&users, ada).await;
    assert!(
        users.user_named("Ada").await.unwrap().is_some(),
        "not found by name"
    );

    users.reset_password(&admin(), ada, PASSWORD).await.unwrap();

    users.verify(&admitted(), "Ada", PASSWORD).await.unwrap();
    let password = password_method(&users, ada).await;
    // Refused if it were Ada's last Sign-in Method.
    delete_sign_in_method(&users, password).await.unwrap();
}
