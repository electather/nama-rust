use std::time::Instant;

use super::{
    CREATIONS_PER_IP, Caller, CreationLimits, FAILURES_PER_EMAIL, FAILURES_PER_IP, LINKS_PER_EMAIL,
    LINKS_PER_IP, LinkLimits, SignInLimits, WINDOW,
};

fn caller(ip: &str, now: Instant) -> Caller {
    Caller {
        ip: ip.parse().unwrap(),
        now,
    }
}

/// Fails `times` sign-ins at `now`, the `i`th from `from(i)` with `email(i)`.
fn fail(
    limits: &SignInLimits,
    times: usize,
    now: Instant,
    from: impl Fn(usize) -> String,
    email: impl Fn(usize) -> String,
) {
    for i in 0..times {
        limits.admit(caller(&from(i), now), &email(i)).unwrap();
    }
}

#[test]
fn an_email_that_failed_too_often_is_refused_in_any_case_from_anywhere_until_the_window_passes() {
    let limits = SignInLimits::default();
    let now = Instant::now();
    fail(
        &limits,
        FAILURES_PER_EMAIL,
        now,
        |i| format!("203.0.113.{i}"),
        |_| "Ada@example.com".to_owned(),
    );

    let blocked = limits
        .admit(caller("198.51.100.1", now), "ada@EXAMPLE.com")
        .unwrap_err();

    assert_eq!(blocked.retry_after_secs(), WINDOW.as_secs());
    limits
        .admit(caller("198.51.100.1", now + WINDOW), "ada@example.com")
        .unwrap();
}

#[test]
fn an_ipv6_client_is_counted_by_its_64() {
    let limits = SignInLimits::default();
    let now = Instant::now();
    fail(
        &limits,
        FAILURES_PER_IP,
        now,
        |i| format!("2001:db8::{i:x}"),
        |i| format!("{i}@example.com"),
    );

    limits
        .admit(caller("2001:db8::ffff", now), "new@example.com")
        .unwrap_err();
    limits
        .admit(caller("2001:db8:0:1::1", now), "new@example.com")
        .unwrap();
}

#[test]
fn signing_in_forgets_the_emails_failures() {
    let limits = SignInLimits::default();
    let now = Instant::now();
    fail(
        &limits,
        FAILURES_PER_EMAIL - 1,
        now,
        |_| "203.0.113.1".to_owned(),
        |_| "ada@example.com".to_owned(),
    );

    let attempt = limits
        .admit(caller("203.0.113.1", now), "ada@example.com")
        .unwrap();
    limits.succeeded(&attempt);

    fail(
        &limits,
        FAILURES_PER_EMAIL,
        now,
        |_| "203.0.113.1".to_owned(),
        |_| "ada@example.com".to_owned(),
    );
}

#[test]
fn an_email_asked_a_link_for_too_often_is_refused_in_any_case_from_anywhere_until_the_window_passes()
 {
    let limits = LinkLimits::default();
    let now = Instant::now();
    for i in 0..LINKS_PER_EMAIL {
        limits
            .admit(caller(&format!("203.0.113.{i}"), now), "Nobody@example.com")
            .unwrap();
    }

    let blocked = limits
        .admit(caller("198.51.100.1", now), "nobody@EXAMPLE.com")
        .unwrap_err();

    assert_eq!(blocked.retry_after_secs(), WINDOW.as_secs());
    limits
        .admit(caller("198.51.100.1", now + WINDOW), "nobody@example.com")
        .unwrap();
}

#[test]
fn a_client_asking_for_too_many_links_is_refused_counting_ipv6_by_its_64() {
    let limits = LinkLimits::default();
    let now = Instant::now();
    for i in 0..LINKS_PER_IP {
        limits
            .admit(
                caller(&format!("2001:db8::{i:x}"), now),
                &format!("{i}@example.com"),
            )
            .unwrap();
    }

    limits
        .admit(caller("2001:db8::ffff", now), "new@example.com")
        .unwrap_err();
    limits
        .admit(caller("2001:db8:0:1::1", now), "new@example.com")
        .unwrap();
}

#[test]
fn a_client_that_created_too_many_accounts_is_refused_until_the_window_passes() {
    let limits = CreationLimits::default();
    let now = Instant::now();
    for _ in 0..CREATIONS_PER_IP {
        limits.admit(caller("203.0.113.1", now)).unwrap();
    }

    let blocked = limits.admit(caller("203.0.113.1", now)).unwrap_err();

    assert_eq!(blocked.retry_after_secs(), WINDOW.as_secs());
    limits.admit(caller("198.51.100.1", now)).unwrap();
    limits.admit(caller("203.0.113.1", now + WINDOW)).unwrap();
}
