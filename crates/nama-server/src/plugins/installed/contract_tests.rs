use buffa::EnumValue;
use nama_plugin_api::{
    fulfiller::{Download, DownloadStatus},
    settings::{SettingKind, SettingValue},
};
use nama_proto::nama::{
    plugin::v1::{self as wire, setting_value::Value},
    v1::{self as api, request::MovieOrSeason},
};

use super::{download, event, observed, request, request_state, setting_kind, setting_value};

#[test]
fn each_kind_becomes_its_twin_and_unspecified_none() {
    let kinds = [
        wire::SettingKind::SETTING_KIND_UNSPECIFIED,
        wire::SettingKind::SETTING_KIND_TEXT,
        wire::SettingKind::SETTING_KIND_SECRET,
        wire::SettingKind::SETTING_KIND_BOOL,
        wire::SettingKind::SETTING_KIND_INTEGER,
        wire::SettingKind::SETTING_KIND_CHOICE,
        wire::SettingKind::SETTING_KIND_URL,
    ];

    assert_eq!(
        kinds.map(setting_kind),
        [
            None,
            Some(SettingKind::Text),
            Some(SettingKind::Secret),
            Some(SettingKind::Bool),
            Some(SettingKind::Integer),
            Some(SettingKind::Choice),
            Some(SettingKind::Url),
        ]
    );
}

#[test]
fn each_value_comes_back_as_it_went_and_an_empty_one_as_none() {
    let values = [
        Some(Value::TextValue("en".to_owned())),
        Some(Value::BoolValue(true)),
        Some(Value::IntegerValue(-3)),
        None,
    ];

    let back = values.map(|value| {
        setting_value(&wire::SettingValue {
            value,
            ..Default::default()
        })
    });

    assert_eq!(
        back,
        [
            Some(SettingValue::Text("en".to_owned())),
            Some(SettingValue::Bool(true)),
            Some(SettingValue::Integer(-3)),
            None,
        ]
    );
}

#[test]
fn a_download_keeps_its_fields_and_an_unknown_status_becomes_unspecified() {
    let reported = |status| wire::Download {
        title: "Dune".to_owned(),
        percent: 42.5,
        size_bytes: Some(1_000),
        eta_seconds: Some(90),
        status,
        attention_reason: Some("stalled".to_owned()),
        ..Default::default()
    };
    let kept = |status: DownloadStatus| Download {
        title: "Dune".to_owned(),
        percent: 42.5,
        size_bytes: Some(1_000),
        eta_seconds: Some(90),
        status: status.into(),
        attention_reason: Some("stalled".to_owned()),
        ..Default::default()
    };
    let statuses = [
        (
            EnumValue::Unknown(99),
            DownloadStatus::DOWNLOAD_STATUS_UNSPECIFIED,
        ),
        (
            wire::DownloadStatus::DOWNLOAD_STATUS_UNSPECIFIED.into(),
            DownloadStatus::DOWNLOAD_STATUS_UNSPECIFIED,
        ),
        (
            wire::DownloadStatus::DOWNLOAD_STATUS_QUEUED.into(),
            DownloadStatus::DOWNLOAD_STATUS_QUEUED,
        ),
        (
            wire::DownloadStatus::DOWNLOAD_STATUS_DOWNLOADING.into(),
            DownloadStatus::DOWNLOAD_STATUS_DOWNLOADING,
        ),
        (
            wire::DownloadStatus::DOWNLOAD_STATUS_IMPORTING.into(),
            DownloadStatus::DOWNLOAD_STATUS_IMPORTING,
        ),
        (
            wire::DownloadStatus::DOWNLOAD_STATUS_NEEDS_ATTENTION.into(),
            DownloadStatus::DOWNLOAD_STATUS_NEEDS_ATTENTION,
        ),
    ];

    for (status, expected) in statuses {
        assert_eq!(download(reported(status)), kept(expected));
    }
}

#[test]
fn a_movie_request_keeps_what_a_fulfiller_reads() {
    let asked = api::Request {
        id: 7,
        movie_or_season: Some(MovieOrSeason::Movie(Box::new(api::RequestedMovie {
            tmdb_id: 438_631,
            title: "Dune".to_owned(),
            release_date: Some("2021-09-15".to_owned()),
            ..Default::default()
        }))),
        state: api::RequestState::REQUEST_STATE_APPROVED.into(),
        requesters: vec![api::Requester {
            user_id: 2,
            name: "ana".to_owned(),
            requested_at: 100,
            ..Default::default()
        }],
        created_at: 100,
        decline_reason: Some("no".to_owned()),
        failure_reason: Some("full".to_owned()),
        quality: Some(api::RequestedQuality {
            id: 3,
            name: "HD".to_owned(),
            profile_id: "4".to_owned(),
            ..Default::default()
        })
        .into(),
        quality_change: Some(api::QualityChange {
            changed_by: Some("admin".to_owned()),
            previous_name: Some("SD".to_owned()),
            changed_at: 200,
            ..Default::default()
        })
        .into(),
        downloads: vec![Download::default()],
        fulfiller_plugin_id: Some("radarr".to_owned()),
        ..Default::default()
    };

    assert_eq!(
        request(&asked),
        wire::Request {
            id: 7,
            movie_or_season: Some(wire::request::MovieOrSeason::Movie(Box::new(
                wire::RequestedMovie {
                    tmdb_id: 438_631,
                    title: "Dune".to_owned(),
                    release_date: Some("2021-09-15".to_owned()),
                    ..Default::default()
                }
            ))),
            state: wire::RequestState::REQUEST_STATE_APPROVED.into(),
            requesters: vec![wire::Requester {
                user_id: 2,
                name: "ana".to_owned(),
                requested_at: 100,
                ..Default::default()
            }],
            created_at: 100,
            decline_reason: Some("no".to_owned()),
            failure_reason: Some("full".to_owned()),
            quality: Some(wire::RequestedQuality {
                id: 3,
                name: "HD".to_owned(),
                profile_id: "4".to_owned(),
                ..Default::default()
            })
            .into(),
            quality_change: Some(wire::QualityChange {
                changed_by: Some("admin".to_owned()),
                previous_name: Some("SD".to_owned()),
                changed_at: 200,
                ..Default::default()
            })
            .into(),
            ..Default::default()
        }
    );
}

#[test]
fn a_season_request_keeps_its_show_and_followed_show() {
    let asked = api::Request {
        id: 8,
        movie_or_season: Some(MovieOrSeason::Season(Box::new(api::RequestedSeason {
            show_tmdb_id: 1_399,
            season_number: 2,
            show_title: "Severance".to_owned(),
            air_date: Some("2025-01-17".to_owned()),
            show_imdb_id: Some("tt11280740".to_owned()),
            show_tvdb_id: Some(371_980),
            ..Default::default()
        }))),
        followed_show_id: Some(5),
        ..Default::default()
    };

    assert_eq!(
        request(&asked),
        wire::Request {
            id: 8,
            movie_or_season: Some(wire::request::MovieOrSeason::Season(Box::new(
                wire::RequestedSeason {
                    show_tmdb_id: 1_399,
                    season_number: 2,
                    show_title: "Severance".to_owned(),
                    air_date: Some("2025-01-17".to_owned()),
                    show_imdb_id: Some("tt11280740".to_owned()),
                    show_tvdb_id: Some(371_980),
                    ..Default::default()
                }
            ))),
            followed_show_id: Some(5),
            ..Default::default()
        }
    );
}

#[test]
fn each_request_state_becomes_its_twin_and_an_unknown_one_unspecified() {
    let states = [
        (
            EnumValue::Unknown(99),
            wire::RequestState::REQUEST_STATE_UNSPECIFIED,
        ),
        (
            api::RequestState::REQUEST_STATE_UNSPECIFIED.into(),
            wire::RequestState::REQUEST_STATE_UNSPECIFIED,
        ),
        (
            api::RequestState::REQUEST_STATE_PENDING.into(),
            wire::RequestState::REQUEST_STATE_PENDING,
        ),
        (
            api::RequestState::REQUEST_STATE_APPROVED.into(),
            wire::RequestState::REQUEST_STATE_APPROVED,
        ),
        (
            api::RequestState::REQUEST_STATE_DECLINED.into(),
            wire::RequestState::REQUEST_STATE_DECLINED,
        ),
        (
            api::RequestState::REQUEST_STATE_PROCESSING.into(),
            wire::RequestState::REQUEST_STATE_PROCESSING,
        ),
        (
            api::RequestState::REQUEST_STATE_PARTIALLY_AVAILABLE.into(),
            wire::RequestState::REQUEST_STATE_PARTIALLY_AVAILABLE,
        ),
        (
            api::RequestState::REQUEST_STATE_AVAILABLE.into(),
            wire::RequestState::REQUEST_STATE_AVAILABLE,
        ),
        (
            api::RequestState::REQUEST_STATE_FAILED.into(),
            wire::RequestState::REQUEST_STATE_FAILED,
        ),
    ];

    for (state, expected) in states {
        assert_eq!(request_state(state), expected);
    }
}

#[test]
fn a_library_event_keeps_its_fields() {
    let finished = api_event(api::ScanFinished {
        library_id: 4,
        added: 1,
        updated: 2,
        removed: 3,
        unchanged: 5,
        ..Default::default()
    });

    assert_eq!(
        event(&finished),
        Some(wire_event(wire::ScanFinished {
            library_id: 4,
            added: 1,
            updated: 2,
            removed: 3,
            unchanged: 5,
            ..Default::default()
        }))
    );
}

#[test]
fn a_request_event_carries_the_contracts_request() {
    let asked = api::Request {
        id: 7,
        ..Default::default()
    };
    let cancelled = api_event(api::RequestCancelled {
        request: Some(asked.clone()).into(),
        deleted_by: Some("ada".to_owned()),
        ..Default::default()
    });

    assert_eq!(
        event(&cancelled),
        Some(wire_event(wire::RequestCancelled {
            request: Some(request(&asked)).into(),
            deleted_by: Some("ada".to_owned()),
            ..Default::default()
        }))
    );
}

#[test]
fn request_progress_and_playback_events_are_left_out() {
    let progress = api_event(api::RequestProgress::default());
    let playback = api_event(api::PlaybackStarted::default());

    assert_eq!(event(&progress), None);
    assert_eq!(event(&playback), None);
}

#[test]
fn a_mixed_batch_keeps_the_events_that_convert_in_order() {
    let batch = [
        api_event(api::ScanStarted {
            library_id: 1,
            ..Default::default()
        }),
        api_event(api::PlaybackStarted::default()),
        api_event(api::ScanFinished {
            library_id: 1,
            ..Default::default()
        }),
    ];

    assert_eq!(
        observed(&batch).map(|batch| batch.events),
        Some(vec![
            wire_event(wire::ScanStarted {
                library_id: 1,
                ..Default::default()
            }),
            wire_event(wire::ScanFinished {
                library_id: 1,
                ..Default::default()
            }),
        ])
    );
}

#[test]
fn a_batch_of_only_left_out_kinds_is_not_observed() {
    let batch = [
        api_event(api::RequestProgress::default()),
        api_event(api::PlaybackStarted::default()),
    ];

    assert_eq!(observed(&batch), None);
}

fn api_event(event: impl Into<Option<api::event::Event>>) -> api::Event {
    api::Event {
        event: event.into(),
        ..Default::default()
    }
}

fn wire_event(event: impl Into<Option<wire::event::Event>>) -> wire::Event {
    wire::Event {
        event: event.into(),
        ..Default::default()
    }
}
