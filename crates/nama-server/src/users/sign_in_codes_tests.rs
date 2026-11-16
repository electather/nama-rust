use super::*;

/// Ten minutes, counted independently of the code under test.
const TEN_MINUTES: Duration = Duration::from_mins(10);

/// Ada's Device 2.
fn ada() -> ApprovingDevice {
    ApprovingDevice {
        user_id: 1,
        device_id: 2,
    }
}

/// A sign-in started at `now` for the Device "Living room TV", wanting a token.
fn start(codes: &SignInCodes, now: Instant) -> Started {
    codes
        .start("Living room TV".to_owned(), Delivery::Token, now)
        .unwrap()
}

#[test]
fn a_code_is_eight_crockford_characters_shown_in_two_groups() {
    let started = start(&SignInCodes::default(), Instant::now());

    let (first, second) = started.code.split_once('-').unwrap();
    assert_eq!((first.len(), second.len()), (4, 4), "{}", started.code);
    assert!(
        started
            .code
            .chars()
            .all(|c| c == '-' || "0123456789ABCDEFGHJKMNPQRSTVWXYZ".contains(c)),
        "{}",
        started.code
    );
}

#[test]
fn approving_then_polling_signs_in_as_the_approver_once() {
    let codes = SignInCodes::default();
    let now = Instant::now();
    let started = start(&codes, now);

    codes.approve(&started.code, ada(), now).unwrap();
    let first = codes.poll(&started.handle, now);
    let again = codes.poll(&started.handle, now);

    let Ok(Poll::Approved {
        approving,
        device_name,
        delivery,
    }) = first
    else {
        panic!("not approved: {first:?}");
    };
    assert_eq!(
        (
            approving.user_id,
            approving.device_id,
            device_name.as_str(),
            delivery
        ),
        (1, 2, "Living room TV", Delivery::Token)
    );
    assert!(matches!(again, Err(Error::HandleNotFound)), "{again:?}");
}

#[test]
fn looking_up_a_code_shows_the_device_name_without_approving_it() {
    let codes = SignInCodes::default();
    let now = Instant::now();
    let started = start(&codes, now);

    let device_name = codes.look_up(&started.code, now).unwrap();

    assert_eq!(device_name, "Living room TV");
    assert!(matches!(
        codes.poll(&started.handle, now),
        Ok(Poll::Waiting)
    ));
}

#[test]
fn a_code_matches_in_lower_case_and_without_its_dash() {
    let codes = SignInCodes::default();
    let now = Instant::now();
    let started = start(&codes, now);
    let typed = started.code.replace('-', "").to_lowercase();

    codes.approve(&typed, ada(), now).unwrap();

    assert!(matches!(
        codes.poll(&started.handle, now),
        Ok(Poll::Approved { .. })
    ));
}

#[test]
fn an_expired_code_cant_be_looked_up_or_approved() {
    let codes = SignInCodes::default();
    let started_at = Instant::now();
    let started = start(&codes, started_at);
    let expired_at = started_at + TEN_MINUTES;

    let looked_up = codes.look_up(&started.code, expired_at);
    let approved = codes.approve(&started.code, ada(), expired_at);

    assert!(matches!(looked_up, Err(Error::NotFound)), "{looked_up:?}");
    assert!(matches!(approved, Err(Error::NotFound)), "{approved:?}");
}

#[test]
fn an_unknown_code_cant_be_looked_up_or_approved() {
    let codes = SignInCodes::default();
    let now = Instant::now();
    let started = start(&codes, now);
    // Differs from the real code wherever it has its last character.
    let last = started.code.chars().last().unwrap();
    let unknown = started
        .code
        .replace(last, if last == '0' { "1" } else { "0" });

    let looked_up = codes.look_up(&unknown, now);
    let approved = codes.approve(&unknown, ada(), now);

    assert!(matches!(looked_up, Err(Error::NotFound)), "{looked_up:?}");
    assert!(matches!(approved, Err(Error::NotFound)), "{approved:?}");
}

#[test]
fn an_approved_code_cant_be_approved_again() {
    let codes = SignInCodes::default();
    let now = Instant::now();
    let started = start(&codes, now);
    codes.approve(&started.code, ada(), now).unwrap();

    let again = codes.approve(&started.code, ada(), now);

    assert!(matches!(again, Err(Error::AlreadyApproved)), "{again:?}");
}

#[test]
fn an_unapproved_handle_stays_waiting_until_it_expires() {
    let codes = SignInCodes::default();
    let started_at = Instant::now();
    let started = start(&codes, started_at);
    let just_before = started_at + Duration::from_secs(9 * 60 + 59);

    let waiting = codes.poll(&started.handle, just_before);
    let expired = codes.poll(&started.handle, started_at + TEN_MINUTES);

    assert!(matches!(waiting, Ok(Poll::Waiting)), "{waiting:?}");
    assert!(matches!(expired, Ok(Poll::Expired)), "{expired:?}");
}

#[test]
fn an_approved_handle_polled_too_late_reports_expired() {
    let codes = SignInCodes::default();
    let started_at = Instant::now();
    let started = start(&codes, started_at);
    codes.approve(&started.code, ada(), started_at).unwrap();

    let polled = codes.poll(&started.handle, started_at + TEN_MINUTES);

    assert!(matches!(polled, Ok(Poll::Expired)), "{polled:?}");
}

#[test]
fn an_unknown_handle_is_refused_when_polled() {
    let codes = SignInCodes::default();

    let polled = codes.poll("made-up", Instant::now());

    assert!(matches!(polled, Err(Error::HandleNotFound)), "{polled:?}");
}

#[test]
fn starting_forgets_sign_ins_a_lifetime_after_they_expired() {
    let codes = SignInCodes::default();
    let started_at = Instant::now();
    let old = start(&codes, started_at);
    let much_later = started_at + TEN_MINUTES * 2;

    start(&codes, much_later);

    let polled = codes.poll(&old.handle, much_later);

    assert!(matches!(polled, Err(Error::HandleNotFound)), "{polled:?}");
}
