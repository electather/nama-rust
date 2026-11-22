use super::*;

fn played() -> ViewingEvent {
    ViewingEvent {
        kind: Kind::Movie,
        external_id: "tmdb-603".to_owned(),
        started_at: 1_700_000_000,
        time_of_day_minutes: 1260,
        stop_position_ms: 5_000,
        duration_ms: 8_160_000,
        finished: false,
        device_kind: DeviceKind::Tv,
    }
}

#[test]
fn a_play_within_the_rules_is_valid() {
    assert!(played().is_valid());
}

#[test]
fn a_play_without_an_external_id_is_invalid() {
    let event = ViewingEvent {
        external_id: String::new(),
        ..played()
    };

    assert!(!event.is_valid());
}

#[test]
fn a_time_of_day_past_the_last_minute_is_invalid() {
    let event = ViewingEvent {
        time_of_day_minutes: MINUTES_PER_DAY,
        ..played()
    };

    assert!(!event.is_valid());
}

#[test]
fn a_stop_past_the_end_is_invalid() {
    let event = ViewingEvent {
        stop_position_ms: 8_160_001,
        ..played()
    };

    assert!(!event.is_valid());
}

#[test]
fn a_key_over_its_length_is_invalid() {
    let event = UserViewingEvent {
        user_id: 1,
        key: "k".repeat(KEY_LENGTH + 1),
        event: played(),
    };

    assert!(!event.is_valid());
}

#[test]
fn each_device_kind_reads_back_as_stored() {
    for kind in DeviceKind::ALL {
        assert_eq!(DeviceKind::stored(kind.as_str()), kind);
    }
}
