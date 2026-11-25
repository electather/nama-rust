use super::*;

fn watched(ids: &[&str], changed_at_ms: i64) -> WatchStateChange {
    WatchStateChange {
        episode: false,
        ids: ids.iter().map(|&id| id.to_owned()).collect(),
        played: true,
        position_ms: 0,
        duration_ms: 0,
        changed_at_ms,
    }
}

#[test]
fn ids_the_hub_wouldnt_take_are_left_out_and_a_change_with_none_stays_behind() {
    let too_long = "x".repeat(101);
    let many: Vec<String> = (0..25).map(|n| format!("tmdb-{n}")).collect();
    let many: Vec<&str> = many.iter().map(String::as_str).collect();

    let kept = watch_state_to_proto(7, &watched(&[&too_long, "tmdb-348"], 1000));
    let none = watch_state_to_proto(7, &watched(&[&too_long, ""], 1000));
    let capped = watch_state_to_proto(7, &watched(&many, 1000));

    assert_eq!(
        kept.map(|change| change.ids),
        Some(vec!["tmdb-348".to_owned()])
    );
    assert_eq!(none, None);
    assert_eq!(capped.map(|change| change.ids.len()), Some(20));
}

#[test]
fn a_change_dated_where_the_hub_takes_none_stays_behind() {
    assert_eq!(watch_state_to_proto(7, &watched(&["tmdb-348"], -1)), None);
    let preference = TrackPreference {
        changed_at_ms: 253_402_300_800_000,
        ..TrackPreference::default()
    };
    assert_eq!(preference_to_proto(7, &preference), Vec::new());
}

#[test]
fn a_shows_preference_goes_once_under_each_id_the_hub_takes_and_comes_back_as_described() {
    let preference = TrackPreference {
        audio: false,
        kept_for: TrackPreferenceFor::Show(vec!["tmdb-1399".to_owned(), String::new()]),
        like: Some(TrackDescription {
            language: Some("eng".to_owned()),
            sdh: true,
            ..TrackDescription::default()
        }),
        foreign_audio_only: false,
        changed_at_ms: 1000,
    };

    let sent = preference_to_proto(7, &preference);
    let back: Vec<_> = sent
        .iter()
        .cloned()
        .filter_map(preference_from_proto)
        .collect();

    let under_its_id = TrackPreference {
        kept_for: TrackPreferenceFor::Show(vec!["tmdb-1399".to_owned()]),
        ..preference
    };
    assert_eq!(sent.len(), 1);
    assert_eq!(back, [under_its_id]);
}
