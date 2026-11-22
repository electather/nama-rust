use nama_plugin_api::contract::{TitleId, TitleRef, WatchState};

use super::*;

/// When each play below started: before its Watch State changed.
const STARTED: Started = Started {
    at: 1_700_000_000,
    time_of_day_minutes: 1260,
};

/// The Movie `namespace`-`value`'s Watch State, stopped 5 seconds in, changed after [`STARTED`].
fn stopped(namespace: &str, value: &str) -> TitleWatchStates {
    let title_ref = TitleRef {
        kind: TitleRefKind::TITLE_REF_KIND_MOVIE.into(),
        id: TitleId {
            namespace: namespace.to_owned(),
            value: value.to_owned(),
            ..Default::default()
        }
        .into(),
        ..Default::default()
    };
    let state = WatchState {
        position_ms: 5_000,
        duration_ms: 8_160_000,
        updated_at_ms: (STARTED.at + 60) * 1000,
        ..Default::default()
    };
    TitleWatchStates {
        title_ref: title_ref.into(),
        states: vec![state],
        ..Default::default()
    }
}

#[test]
fn a_play_is_viewed_under_its_external_id() {
    let viewed = viewed(
        &stopped("tmdb", "603"),
        STARTED,
        HubDeviceKind::DEVICE_KIND_TV,
    );

    assert_eq!(viewed.unwrap().external_id, "tmdb-603");
}

#[test]
fn a_play_under_an_external_id_the_hub_would_refuse_is_left_out() {
    let long = "1".repeat(MAX_EXTERNAL_ID_CHARS);

    let viewed = viewed(
        &stopped("tmdb", &long),
        STARTED,
        HubDeviceKind::DEVICE_KIND_TV,
    );

    assert_eq!(viewed, None);
}
