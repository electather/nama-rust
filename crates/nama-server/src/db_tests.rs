#[cfg(feature = "testing")]
#[test]
fn advance_clock_moves_now_and_now_ms_ahead_by_what_it_adds() {
    use std::time::Duration;

    use super::{advance_clock, now, now_ms};

    const DAY_SECS: i64 = 24 * 60 * 60;
    let (secs_before, ms_before) = (now(), now_ms());

    advance_clock(Duration::from_hours(2 * 24));

    let (secs_after, ms_after) = (now(), now_ms());
    // Two days ahead, plus however long the test took, which is well under a day.
    assert!(
        (secs_before + 2 * DAY_SECS..secs_before + 3 * DAY_SECS).contains(&secs_after),
        "{secs_before} then {secs_after}"
    );
    assert!(
        (ms_before + 2 * DAY_SECS * 1000..ms_before + 3 * DAY_SECS * 1000).contains(&ms_after),
        "{ms_before} then {ms_after}"
    );
}
