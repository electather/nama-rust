//! `local-auth` as the Kernel sees it: the Authenticator it registers.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use nama_plugin_api::{
    Plugin, Registrar, Registration,
    auth::{Authenticator, PasswordError},
};
use nama_plugin_local_auth::LocalAuth;

/// The Authenticator `local-auth` registers when it is enabled.
#[expect(
    clippy::expect_used,
    reason = "registering no Authenticator fails the test that asked for it"
)]
fn authenticator() -> Arc<dyn Authenticator> {
    let mut registrar = Registrar::new();
    LocalAuth.register(&mut registrar);
    registrar
        .into_registrations()
        .iter()
        .find_map(Registration::provider::<dyn Authenticator>)
        .expect("local-auth registers an Authenticator")
}

#[test]
fn a_credential_verifies_against_the_password_it_was_made_from() {
    let auth = authenticator();
    let credential = auth.hash_password("correct horse").unwrap();

    assert!(auth.verify_password("correct horse", &credential).unwrap());
}

#[test]
fn a_credential_does_not_verify_against_another_password() {
    let auth = authenticator();
    let credential = auth.hash_password("correct horse").unwrap();

    assert!(!auth.verify_password("battery staple", &credential).unwrap());
}

#[test]
fn the_credential_is_an_argon2id_hash_and_never_the_password() {
    let credential = authenticator().hash_password("correct horse").unwrap();

    assert!(credential.starts_with("$argon2id$"), "{credential}");
    assert!(!credential.contains("correct horse"), "{credential}");
}

#[test]
fn a_password_shorter_than_8_characters_is_rejected() {
    // Counted in characters, not bytes: these 7 characters are 14 bytes.
    let refused = authenticator().hash_password("üüüüüüü");

    assert!(
        matches!(refused, Err(PasswordError::Rejected(_))),
        "{refused:?}"
    );
}

#[test]
fn a_password_of_8_characters_is_accepted() {
    let credential = authenticator().hash_password("üüüüüüüü");

    assert!(credential.is_ok(), "{credential:?}");
}

#[test]
fn a_password_of_1024_characters_is_accepted() {
    let credential = authenticator().hash_password(&"ü".repeat(1024));

    assert!(credential.is_ok(), "{credential:?}");
}

#[test]
fn a_password_longer_than_1024_characters_is_rejected() {
    let refused = authenticator().hash_password(&"x".repeat(1025));

    assert!(
        matches!(refused, Err(PasswordError::Rejected(_))),
        "{refused:?}"
    );
}

#[test]
fn a_rejected_password_is_not_repeated_in_the_error() {
    let refused = authenticator().hash_password("hunter2").unwrap_err();

    assert!(!refused.to_string().contains("hunter2"), "{refused}");
    assert!(!format!("{refused:?}").contains("hunter2"), "{refused:?}");
}

#[test]
fn a_credential_local_auth_did_not_make_is_an_error() {
    let checked = authenticator().verify_password("correct horse", "correct horse");

    assert!(checked.is_err(), "{checked:?}");
}

/// How long `check` takes at the least, out of a few runs, since a busy machine only slows a run.
fn fastest(check: impl Fn()) -> Duration {
    (0..3)
        .map(|_| {
            let started = Instant::now();
            check();
            started.elapsed()
        })
        .min()
        .unwrap_or_default()
}

/// Timing varies too much to show the two checks cost the same, but a busy machine only slows
/// them down, so a check that skipped the argon2 verification would be far faster than a real one.
#[test]
fn the_unknown_user_check_takes_as_long_as_a_real_verification() {
    let auth = authenticator();
    let credential = auth.hash_password("correct horse").unwrap();

    let real = fastest(|| {
        auth.verify_password("battery staple", &credential).unwrap();
    });
    let unknown = fastest(|| auth.verify_unknown_user("battery staple"));

    assert!(unknown >= real / 2, "unknown {unknown:?}, real {real:?}");
}
