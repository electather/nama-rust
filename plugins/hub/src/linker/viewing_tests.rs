use nama_plugin_api::contract::{TitleId, TitleRef, WatchState};

use super::*;

/// When each play below started: before its Watch State changed.
const STARTED: Started = Started {
    at: 1_700_000_000,
    time_of_day_minutes: 1260,
};

fn title_id(namespace: &str, value: &str) -> TitleId {
    TitleId {
        namespace: namespace.to_owned(),
        value: value.to_owned(),
        ..Default::default()
    }
}

/// The Movie `namespace`-`value`'s Watch State, stopped 5 seconds in, changed after [`STARTED`].
fn stopped(namespace: &str, value: &str) -> TitleWatchStates {
    let title_ref = TitleRef {
        kind: TitleRefKind::TITLE_REF_KIND_MOVIE.into(),
        id: title_id(namespace, value).into(),
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

/// [`stopped`]'s Watch State, of the Episode `namespace`-`value`.
fn stopped_episode(namespace: &str, value: &str) -> TitleWatchStates {
    let mut title = stopped(namespace, value);
    (title.title_ref).modify(|title_ref| {
        title_ref.kind = TitleRefKind::TITLE_REF_KIND_EPISODE.into();
    });
    title
}

/// What `list_version_titles` answers for a Version holding `title`, whose Show is `show`.
fn listing(title: &TitleWatchStates, show: TitleId) -> VersionTitles {
    VersionTitles {
        titles: title.title_ref.as_option().cloned().into_iter().collect(),
        show_ids: vec![show],
        ..Default::default()
    }
}

#[test]
fn a_play_is_viewed_under_its_external_id() {
    let title = stopped("tmdb", "603");

    let viewed = viewed(
        &title,
        &listing(&title, TitleId::default()),
        STARTED,
        HubDeviceKind::DEVICE_KIND_TV,
    );

    let viewed = viewed.unwrap();
    assert_eq!(viewed.external_id, "tmdb-603");
    assert_eq!(viewed.show_external_id, "");
}

#[test]
fn a_play_under_an_external_id_the_hub_would_refuse_is_left_out() {
    let title = stopped("tmdb", &"1".repeat(MAX_EXTERNAL_ID_CHARS));

    let viewed = viewed(
        &title,
        &listing(&title, TitleId::default()),
        STARTED,
        HubDeviceKind::DEVICE_KIND_TV,
    );

    assert_eq!(viewed, None);
}

#[test]
fn an_episodes_play_names_its_show() {
    let title = stopped_episode("tmdb", "63056");

    let viewed = viewed(
        &title,
        &listing(&title, title_id("tmdb", "1399")),
        STARTED,
        HubDeviceKind::DEVICE_KIND_TV,
    );

    let viewed = viewed.unwrap();
    assert!(viewed.episode);
    assert_eq!(viewed.external_id, "tmdb-63056");
    assert_eq!(viewed.show_external_id, "tmdb-1399");
}

#[test]
fn an_episode_whose_show_has_no_id_is_left_out() {
    let title = stopped_episode("tmdb", "63056");

    let viewed = viewed(
        &title,
        &listing(&title, TitleId::default()),
        STARTED,
        HubDeviceKind::DEVICE_KIND_TV,
    );

    assert_eq!(viewed, None);
}

#[test]
fn an_episode_whose_show_id_the_hub_would_refuse_is_left_out() {
    let title = stopped_episode("tmdb", "63056");
    let show = title_id("tmdb", &"1".repeat(MAX_EXTERNAL_ID_CHARS));

    let viewed = viewed(
        &title,
        &listing(&title, show),
        STARTED,
        HubDeviceKind::DEVICE_KIND_TV,
    );

    assert_eq!(viewed, None);
}
