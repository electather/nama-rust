use super::*;

/// When each play below started: before its Watch State changed.
const STARTED: Started = Started {
    at: 1_700_000_000,
    time_of_day_minutes: 1260,
};

/// Stopped 5 seconds in, changed after [`STARTED`].
const STOPPED: WatchState = WatchState {
    position_ms: 5_000,
    duration_ms: 8_160_000,
    played: false,
    played_at: None,
    updated_at: STARTED.at + 60,
};

fn title(kind: WatchStateKind, id: i64, show: Option<i64>) -> Title {
    Title {
        kind,
        id: ExternalId::tmdb(id),
        ids: ExternalIds::from_iter([ExternalId::tmdb(id)]),
        show: show.map(ExternalId::tmdb),
    }
}

fn viewed_at(title: Title, state: WatchState) -> Option<ViewingEvent> {
    viewed(title, state, STARTED, DeviceKind::DEVICE_KIND_TV)
}

#[test]
fn a_movies_play_is_viewed_under_its_id_naming_no_show() {
    let viewed = viewed_at(title(WatchStateKind::Movie, 603, Some(1399)), STOPPED).unwrap();

    assert_eq!(viewed.id, "tmdb-603");
    assert_eq!(viewed.show_id, None);
    assert_eq!(viewed.stop_position_ms, 5_000);
}

#[test]
fn an_episodes_play_names_its_show() {
    let viewed = viewed_at(title(WatchStateKind::Episode, 63056, Some(1399)), STOPPED);

    assert_eq!(viewed.unwrap().show_id.as_deref(), Some("tmdb-1399"));
}

#[test]
fn an_episode_whose_show_has_no_id_is_left_out() {
    let viewed = viewed_at(title(WatchStateKind::Episode, 63056, None), STOPPED);

    assert_eq!(viewed, None);
}

#[test]
fn a_title_whose_watch_state_the_play_left_alone_is_left_out() {
    let before = WatchState {
        updated_at: STARTED.at - 1,
        ..STOPPED
    };

    assert_eq!(
        viewed_at(title(WatchStateKind::Movie, 603, None), before),
        None
    );
}

#[test]
fn a_later_play_of_one_of_its_titles_plays_alike() {
    let two_episodes = [
        title(WatchStateKind::Episode, 1, Some(9)),
        title(WatchStateKind::Episode, 2, Some(9)),
    ];

    assert!(plays_alike(
        &title(WatchStateKind::Episode, 2, Some(9)),
        &two_episodes
    ));
    assert!(!plays_alike(
        &title(WatchStateKind::Movie, 2, None),
        &two_episodes
    ));
}
