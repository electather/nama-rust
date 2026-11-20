use jiff::{
    civil::DateTime,
    tz::{Offset, TimeZone},
};

use super::Cron;

/// Seconds since the Unix epoch of `at`, read in UTC, such as "2026-11-20 04:00".
fn utc(at: &str) -> i64 {
    let at: DateTime = at.parse().unwrap();
    at.to_zoned(TimeZone::UTC).unwrap().timestamp().as_second()
}

fn fires(cron: &str, after: &str, until: &str) -> bool {
    let cron = Cron::parse(cron).unwrap();
    cron.fires_between(&TimeZone::UTC, utc(after), utc(until))
}

#[test]
fn a_daily_schedule_fires_once_its_minute_starts_and_not_before() {
    assert!(!fires(
        "0 4 * * *",
        "2026-11-20 03:00",
        "2026-11-20 03:59:59"
    ));
    assert!(fires(
        "0 4 * * *",
        "2026-11-20 03:59:59",
        "2026-11-20 04:00"
    ));
}

#[test]
fn a_minute_that_started_at_or_before_after_is_not_fired_again() {
    assert!(!fires(
        "0 4 * * *",
        "2026-11-20 04:00",
        "2026-11-20 04:00:59"
    ));
    assert!(!fires(
        "0 4 * * *",
        "2026-11-20 04:00:30",
        "2026-11-21 03:59"
    ));
}

#[test]
fn a_window_spanning_a_day_finds_its_minute() {
    assert!(fires("10 4 * * *", "2026-11-20 05:00", "2026-11-21 05:00"));
}

#[test]
fn lists_ranges_and_steps_name_the_minutes_they_cover() {
    let cron = Cron::parse("*/15 9-17/4 * * *").unwrap();
    let at = |time: &str| cron.matches(format!("2026-11-20 {time}").parse().unwrap());

    assert!(at("09:00") && at("09:45") && at("13:15") && at("17:30"));
    assert!(!at("09:10") && !at("10:00") && !at("18:00"));

    let listed = Cron::parse("5,10-12 * * * *").unwrap();
    let at_minute =
        |minute: &str| listed.matches(format!("2026-11-20 08:{minute}").parse().unwrap());
    assert!(at_minute("05") && at_minute("10") && at_minute("12"));
    assert!(!at_minute("06") && !at_minute("13"));
}

#[test]
fn months_and_weekdays_may_be_named_and_sunday_is_0_or_7() {
    // 2026-11-22 is a Sunday, 2026-11-23 a Monday.
    let sunday: DateTime = "2026-11-22 04:00".parse().unwrap();
    let monday: DateTime = "2026-11-23 04:00".parse().unwrap();

    for weekly in ["0 4 * * 0", "0 4 * * 7", "0 4 * * sun", "0 4 * NOV SUN"] {
        let cron = Cron::parse(weekly).unwrap();
        assert!(cron.matches(sunday), "{weekly}");
        assert!(!cron.matches(monday), "{weekly}");
    }
    assert!(!Cron::parse("0 4 * dec sun").unwrap().matches(sunday));
}

#[test]
fn a_restricted_day_of_month_and_day_of_week_fire_on_either() {
    let cron = Cron::parse("0 4 1 * mon").unwrap();

    assert!(
        cron.matches("2026-11-23 04:00".parse().unwrap()),
        "a Monday"
    );
    assert!(cron.matches("2026-12-01 04:00".parse().unwrap()), "the 1st");
    assert!(!cron.matches("2026-11-24 04:00".parse().unwrap()));
}

#[test]
fn a_day_of_month_with_any_weekday_fires_on_that_day_only() {
    let cron = Cron::parse("0 4 1 * *").unwrap();

    assert!(cron.matches("2026-12-01 04:00".parse().unwrap()));
    assert!(!cron.matches("2026-11-23 04:00".parse().unwrap()));
}

#[test]
fn the_schedule_is_read_in_the_zone_it_is_given() {
    let cron = Cron::parse("0 4 * * *").unwrap();
    // A fixed offset, since a test on Windows has no time zone database.
    let tehran = TimeZone::fixed(Offset::from_seconds(3 * 3600 + 30 * 60).unwrap());
    // 04:00 at +03:30 is 00:30 UTC.
    let at = |time: &str| utc(&format!("2026-11-20 {time}"));

    assert!(cron.fires_between(&tehran, at("00:29"), at("00:30")));
    assert!(!cron.fires_between(&tehran, at("03:59"), at("04:00")));
}

#[test]
fn a_schedule_with_a_seconds_field_is_refused_saying_so() {
    let refused = Cron::parse("0 0 4 * * *").unwrap_err();

    assert_eq!(
        refused,
        "\"0 0 4 * * *\" has a seconds field; a schedule has 5 fields and runs to the minute"
    );
}

#[test]
fn a_schedule_that_doesnt_parse_is_refused_saying_why() {
    let refused = |cron: &str| Cron::parse(cron).unwrap_err();

    assert_eq!(
        refused("0 4 * *"),
        "\"0 4 * *\" has 4 fields; a schedule has 5: minute, hour, day of month, month and day of \
         week"
    );
    assert_eq!(
        refused("60 4 * * *"),
        "\"60 4 * * *\" has minute 60, outside 0-59"
    );
    assert_eq!(
        refused("0 4 0 * *"),
        "\"0 4 0 * *\" has day of month 0, outside 1-31"
    );
    assert_eq!(
        refused("0 4 * * fri-mon"),
        "\"0 4 * * fri-mon\" has a range from 5 down to 1"
    );
    assert_eq!(refused("*/0 4 * * *"), "\"*/0 4 * * *\" has a step of 0");
    assert_eq!(
        refused("0 4 * * often"),
        "\"0 4 * * often\" has day of week often, which isn't one"
    );
    assert_eq!(
        refused("@daily"),
        "\"@daily\" has 1 field; a schedule has 5: minute, hour, day of month, month and day of week"
    );
}
