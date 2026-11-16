use std::net::Ipv4Addr;

use super::*;

const IP: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);

#[test]
fn attempts_are_admitted_again_once_old_failures_leave_the_window() {
    let limits = SignInLimits::default();
    let start = Instant::now();
    for minute in 0..10 {
        let at = start + Duration::from_mins(minute);
        limits.admit(IP, Some("Ada"), at).expect("admitted");
    }

    let blocked = limits
        .admit(IP, Some("Ada"), start + Duration::from_mins(14))
        .expect_err("admitted at the limit");
    let admitted = limits.admit(IP, Some("Ada"), start + WINDOW);

    assert_eq!(blocked.retry_after_secs(), 60);
    assert!(admitted.is_ok(), "the first failure is still counted");
}

#[test]
fn a_name_users_may_have_is_counted_whole_even_if_lowercasing_lengthens_it() {
    // "İ" lowercases to two characters.
    let longest = "İ".repeat(*NAME_LENGTH.end());

    assert_eq!(counted_name_key(&longest), name_key(&longest));
}

#[test]
fn keys_whose_failures_left_the_window_are_dropped() {
    let mut window = Window::default();
    let start = Instant::now();
    window.record("gone", start);

    window.record("new", start + WINDOW);

    assert_eq!(window.times.keys().collect::<Vec<_>>(), [&"new"]);
}

#[test]
fn code_attempts_are_admitted_again_once_old_failures_leave_the_window() {
    let limits = SignInCodeLimits::default();
    let start = Instant::now();
    for minute in 0..10 {
        limits
            .admit(1, start + Duration::from_mins(minute))
            .expect("admitted");
    }

    let blocked = limits
        .admit(1, start + Duration::from_mins(14))
        .expect_err("admitted at the limit");
    let admitted = limits.admit(1, start + Duration::from_mins(15));

    assert_eq!(blocked.retry_after_secs(), 60);
    assert!(admitted.is_ok(), "the first failure is still counted");
}

#[test]
fn a_code_attempt_that_wasnt_a_guess_isnt_counted() {
    let limits = SignInCodeLimits::default();
    let now = Instant::now();
    for _ in 0..10 {
        let attempt = limits.admit(1, now).expect("admitted");
        limits.wasnt_a_guess(&attempt);
    }

    limits
        .admit(1, now)
        .expect("admitted after 10 that weren't guesses");
}
