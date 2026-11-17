use std::net::{Ipv4Addr, Ipv6Addr};

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

/// An address in the /64 `2001:db8:0:<subnet>::/64`.
fn in_subnet(subnet: u16, host: u16) -> IpAddr {
    IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, subnet, 0, 0, 0, host))
}

#[test]
fn failures_from_one_ipv6_64_are_counted_together() {
    let limits = SignInLimits::default();
    let now = Instant::now();
    for host in 0..50 {
        limits
            .admit(in_subnet(1, host), None, now)
            .expect("admitted");
    }

    let same_64 = limits.admit(in_subnet(1, 0xffff), None, now);
    let other_64 = limits.admit(in_subnet(2, 1), None, now);

    assert!(same_64.is_err(), "another address in the /64 was admitted");
    assert!(
        other_64.is_ok(),
        "another /64's failures are counted together"
    );
}

#[test]
fn a_succeeded_attempt_frees_its_slot_for_any_address_in_its_ipv6_64() {
    let limits = SignInLimits::default();
    let now = Instant::now();
    for host in 0..49 {
        limits
            .admit(in_subnet(1, host), None, now)
            .expect("admitted");
    }
    let attempt = limits.admit(in_subnet(1, 49), None, now).expect("admitted");

    limits.succeeded(&attempt);

    limits
        .admit(in_subnet(1, 0xffff), None, now)
        .expect("admitted after the success was un-counted");
}

#[test]
fn a_name_users_may_have_is_counted_whole_even_if_lowercasing_lengthens_it() {
    // "İ" lowercases to two characters.
    let longest = "İ".repeat(*NAME_LENGTH.end());

    assert_eq!(counted_name_key(&longest), name_key(&longest));
}

#[test]
fn a_name_users_may_have_is_counted_whole_even_if_typed_decomposed() {
    let longest = "e\u{301}".repeat(*NAME_LENGTH.end());

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

#[test]
fn code_starts_are_counted_per_ip_and_admitted_again_once_old_starts_leave_the_window() {
    let limits = SignInCodeStartLimits::default();
    let other_ip = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 7));
    let start = Instant::now();
    for half_minute in 0..20 {
        limits
            .admit(IP, start + Duration::from_secs(half_minute * 30))
            .expect("admitted");
    }

    let blocked = limits
        .admit(IP, start + Duration::from_mins(14))
        .expect_err("admitted at the limit");
    let other = limits.admit(other_ip, start + Duration::from_mins(14));
    let admitted = limits.admit(IP, start + WINDOW);

    assert_eq!(blocked.retry_after_secs(), 60);
    assert!(other.is_ok(), "another IP's starts are counted together");
    assert!(admitted.is_ok(), "the first start is still counted");
}

#[test]
fn code_starts_from_one_ipv6_64_are_counted_together() {
    let limits = SignInCodeStartLimits::default();
    let now = Instant::now();
    for host in 0..20 {
        limits.admit(in_subnet(1, host), now).expect("admitted");
    }

    let same_64 = limits.admit(in_subnet(1, 0xffff), now);
    let other_64 = limits.admit(in_subnet(2, 1), now);

    assert!(same_64.is_err(), "another address in the /64 was admitted");
    assert!(
        other_64.is_ok(),
        "another /64's starts are counted together"
    );
}
