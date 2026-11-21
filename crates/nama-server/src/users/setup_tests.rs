use std::sync::Arc;

use super::tests::{
    PASSWORD, attempt, fail, fresh_sign_in, local_auth_users, retry_after_secs, set_up,
    sign_in_with_ada,
};
use super::*;
use crate::users::Permissions;

/// Sets Ada up on a fresh database in `data_dir`, as [`sign_in_with_ada`] does, then closes it.
///
/// Dropping the database instead leaves its connections closing in the background, racing a
/// reopen of `data_dir` (#1564).
async fn ada_set_up_then_closed(data_dir: &std::path::Path) {
    let (sign_in, _) = sign_in_with_ada(data_dir).await;
    let db = Arc::clone(&sign_in.users.db);
    drop(sign_in);
    Arc::into_inner(db)
        .expect("only this test holds the database")
        .close()
        .await;
}

/// Fails to set up from `LAPTOP_IP` with a wrong Setup Code `times` times at `now`.
async fn fail_setup(sign_in: &SignIn, times: usize, now: Instant) {
    for _ in 0..times {
        let failed = set_up(sign_in, "AAAA-AAAA-AAAA", "Ada", now).await;
        assert!(
            matches!(failed, Err(Error::WrongSetupCode)),
            "{:?}",
            failed.map(|_| ())
        );
    }
}

#[tokio::test]
async fn setup_with_the_right_code_creates_the_first_admin_signed_in_on_the_device() {
    let data_dir = tempfile::tempdir().unwrap();
    let (sign_in, setup_code) = fresh_sign_in(data_dir.path()).await;

    let signed_in = set_up(&sign_in, &setup_code, "Ada", Instant::now())
        .await
        .unwrap();

    assert_eq!(signed_in.caller.user.name, "Ada");
    assert_eq!(signed_in.caller.user.permissions, Permissions::ADMIN);
    assert_eq!(signed_in.caller.device.name, "Laptop");
}

#[tokio::test]
async fn the_51st_setup_from_an_address_after_wrong_codes_is_blocked_even_with_the_right_code() {
    let data_dir = tempfile::tempdir().unwrap();
    let (sign_in, setup_code) = fresh_sign_in(data_dir.path()).await;
    let now = Instant::now();
    fail_setup(&sign_in, 50, now).await;

    let blocked = set_up(&sign_in, &setup_code, "Ada", now).await;

    assert_eq!(retry_after_secs(blocked), Some(15 * 60));
}

#[tokio::test]
async fn the_51st_failure_from_an_address_mixing_sign_in_and_setup_is_blocked() {
    let data_dir = tempfile::tempdir().unwrap();
    let (sign_in, setup_code) = fresh_sign_in(data_dir.path()).await;
    let now = Instant::now();
    fail_setup(&sign_in, 40, now).await;
    fail(&sign_in, "Grace", 10, now).await;

    let blocked = set_up(&sign_in, &setup_code, "Ada", now).await;

    assert_eq!(retry_after_secs(blocked), Some(15 * 60));
}

#[tokio::test]
async fn once_the_server_has_users_setup_is_refused_as_already_set_up_whatever_the_code() {
    let data_dir = tempfile::tempdir().unwrap();
    let (sign_in, setup_code) = sign_in_with_ada(data_dir.path()).await;
    let now = Instant::now();

    let right_code = set_up(&sign_in, &setup_code, "Grace", now).await;
    let wrong_code = set_up(&sign_in, "AAAA-AAAA-AAAA", "Grace", now).await;

    for refused in [right_code, wrong_code] {
        assert!(
            matches!(refused, Err(Error::Users(users::Error::AlreadySetUp))),
            "{:?}",
            refused.map(|_| ())
        );
    }
}

#[tokio::test]
async fn setup_on_a_server_that_started_with_users_is_refused_as_already_set_up() {
    let data_dir = tempfile::tempdir().unwrap();
    ada_set_up_then_closed(data_dir.path()).await;
    let restarted = SignIn::new(local_auth_users(data_dir.path()).await, None);

    let refused = set_up(&restarted, "AAAA-AAAA-AAAA", "Grace", Instant::now()).await;

    assert!(
        matches!(refused, Err(Error::Users(users::Error::AlreadySetUp))),
        "{:?}",
        refused.map(|_| ())
    );
}

/// Calls Setup from `LAPTOP_IP` with a wrong Setup Code 50 times, each refused as already set
/// up, then signs Ada in from there with [`PASSWORD`].
async fn sign_in_ada_after_setups_refused_as_already_set_up(sign_in: &SignIn) {
    let now = Instant::now();
    for _ in 0..50 {
        let refused = set_up(sign_in, "AAAA-AAAA-AAAA", "Grace", now).await;
        assert!(
            matches!(refused, Err(Error::Users(users::Error::AlreadySetUp))),
            "{:?}",
            refused.map(|_| ())
        );
    }
    attempt(sign_in, "Ada", PASSWORD, now).await.unwrap();
}

#[tokio::test]
async fn once_the_server_has_users_setup_refusals_do_not_count_toward_the_address_limit() {
    let data_dir = tempfile::tempdir().unwrap();
    let (sign_in, _) = sign_in_with_ada(data_dir.path()).await;
    sign_in_ada_after_setups_refused_as_already_set_up(&sign_in).await;
}

#[tokio::test]
async fn setup_on_a_server_that_started_with_users_does_not_count_toward_the_address_limit() {
    let data_dir = tempfile::tempdir().unwrap();
    ada_set_up_then_closed(data_dir.path()).await;
    let restarted = SignIn::new(local_auth_users(data_dir.path()).await, None);
    sign_in_ada_after_setups_refused_as_already_set_up(&restarted).await;
}

#[tokio::test]
async fn a_bad_admin_name_with_the_right_setup_code_is_refused_without_counting_as_a_guess() {
    let data_dir = tempfile::tempdir().unwrap();
    let (sign_in, setup_code) = fresh_sign_in(data_dir.path()).await;
    let now = Instant::now();
    for _ in 0..50 {
        let refused = set_up(&sign_in, &setup_code, "   ", now).await;
        assert!(
            matches!(
                refused,
                Err(Error::Users(users::Error::NameLength { field: "name", .. }))
            ),
            "{:?}",
            refused.map(|_| ())
        );
    }

    set_up(&sign_in, &setup_code, "Ada", now).await.unwrap();
}
