use buffa::EnumValue;
use nama_plugin_proto::nama::plugin::v1 as wire;
use nama_proto::nama::v1::{self as api, request::MovieOrSeason};

use super::{download, event, notification, observed, request, request_state};

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
    let kept = |status: api::DownloadStatus| api::Download {
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
            api::DownloadStatus::DOWNLOAD_STATUS_UNSPECIFIED,
        ),
        (
            wire::DownloadStatus::DOWNLOAD_STATUS_UNSPECIFIED.into(),
            api::DownloadStatus::DOWNLOAD_STATUS_UNSPECIFIED,
        ),
        (
            wire::DownloadStatus::DOWNLOAD_STATUS_QUEUED.into(),
            api::DownloadStatus::DOWNLOAD_STATUS_QUEUED,
        ),
        (
            wire::DownloadStatus::DOWNLOAD_STATUS_DOWNLOADING.into(),
            api::DownloadStatus::DOWNLOAD_STATUS_DOWNLOADING,
        ),
        (
            wire::DownloadStatus::DOWNLOAD_STATUS_IMPORTING.into(),
            api::DownloadStatus::DOWNLOAD_STATUS_IMPORTING,
        ),
        (
            wire::DownloadStatus::DOWNLOAD_STATUS_NEEDS_ATTENTION.into(),
            api::DownloadStatus::DOWNLOAD_STATUS_NEEDS_ATTENTION,
        ),
    ];

    for (status, expected) in statuses {
        assert_eq!(download(reported(status)), kept(expected));
    }
}

fn id(namespace: &str, value: &str) -> api::ExternalId {
    api::ExternalId {
        namespace: namespace.to_owned(),
        value: value.to_owned(),
        ..Default::default()
    }
}

fn title_id(namespace: &str, value: &str) -> wire::TitleId {
    wire::TitleId {
        namespace: namespace.to_owned(),
        value: value.to_owned(),
        ..Default::default()
    }
}

#[test]
fn a_movie_request_keeps_what_a_fulfiller_reads() {
    let asked = api::Request {
        id: 7,
        movie_or_season: Some(MovieOrSeason::Movie(Box::new(api::RequestedMovie {
            id: id("tmdb", "438631").into(),
            ids: vec![id("tmdb", "438631"), id("anidb", "a/b 1")],
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
        downloads: vec![api::Download::default()],
        fulfiller_plugin_id: Some("radarr".to_owned()),
        ..Default::default()
    };

    assert_eq!(
        request(&asked),
        wire::Request {
            id: 7,
            movie_or_season: Some(wire::request::MovieOrSeason::Movie(Box::new(
                wire::RequestedMovie {
                    title: "Dune".to_owned(),
                    release_date: Some("2021-09-15".to_owned()),
                    ids: vec![title_id("tmdb", "438631"), title_id("anidb", "a/b 1")],
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
            show_id: id("tmdb", "1399").into(),
            show_ids: vec![
                id("tmdb", "1399"),
                id("imdb", "tt11280740"),
                id("tvdb", "371980"),
            ],
            season_number: 2,
            show_title: "Severance".to_owned(),
            air_date: Some("2025-01-17".to_owned()),
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
                    season_number: 2,
                    show_title: "Severance".to_owned(),
                    air_date: Some("2025-01-17".to_owned()),
                    show_ids: vec![
                        title_id("tmdb", "1399"),
                        title_id("imdb", "tt11280740"),
                        title_id("tvdb", "371980"),
                    ],
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
#[expect(
    clippy::too_many_lines,
    reason = "one row for each kind the contract converts"
)]
fn every_event_kind_that_converts_keeps_each_field_in_its_place() {
    // Every field holds a value no other field holds, so a swap in any arm fails.
    let asked = |id| api::Request {
        id,
        ..Default::default()
    };
    let told = |id| Some(request(&asked(id))).into();
    let kinds = [
        (
            api_event(api::ScanStarted {
                library_id: 1,
                ..Default::default()
            }),
            wire_event(wire::ScanStarted {
                library_id: 1,
                ..Default::default()
            }),
        ),
        (
            api_event(api::ScanProgress {
                library_id: 2,
                files_done: 3,
                files_total: 4,
                ..Default::default()
            }),
            wire_event(wire::ScanProgress {
                library_id: 2,
                files_done: 3,
                files_total: 4,
                ..Default::default()
            }),
        ),
        (
            api_event(api::ScanFinished {
                library_id: 5,
                added: 6,
                updated: 7,
                removed: 8,
                unchanged: 9,
                ..Default::default()
            }),
            wire_event(wire::ScanFinished {
                library_id: 5,
                added: 6,
                updated: 7,
                removed: 8,
                unchanged: 9,
                ..Default::default()
            }),
        ),
        (
            api_event(api::LibraryItemAdded {
                library_id: 10,
                item_id: 11,
                path: "/movies/Heat (1995).mkv".to_owned(),
                ..Default::default()
            }),
            wire_event(wire::LibraryItemAdded {
                library_id: 10,
                item_id: 11,
                path: "/movies/Heat (1995).mkv".to_owned(),
                ..Default::default()
            }),
        ),
        (
            api_event(api::LibraryItemRemoved {
                library_id: 12,
                item_id: 13,
                ..Default::default()
            }),
            wire_event(wire::LibraryItemRemoved {
                library_id: 12,
                item_id: 13,
                ..Default::default()
            }),
        ),
        (
            api_event(api::ItemMatched {
                library_id: 14,
                item_id: 15,
                title: "Heat".to_owned(),
                ..Default::default()
            }),
            wire_event(wire::ItemMatched {
                library_id: 14,
                item_id: 15,
                title: "Heat".to_owned(),
                ..Default::default()
            }),
        ),
        (
            api_event(api::MatchingFinished {
                library_id: 16,
                matched: 17,
                unmatched: 18,
                undecided: 19,
                ..Default::default()
            }),
            wire_event(wire::MatchingFinished {
                library_id: 16,
                matched: 17,
                unmatched: 18,
                undecided: 19,
                ..Default::default()
            }),
        ),
        (
            api_event(api::ProbingFinished {
                library_id: 20,
                probed: 21,
                failed: 22,
                ..Default::default()
            }),
            wire_event(wire::ProbingFinished {
                library_id: 20,
                probed: 21,
                failed: 22,
                ..Default::default()
            }),
        ),
        (
            api_event(api::RequestCreated {
                request: Some(asked(23)).into(),
                ..Default::default()
            }),
            wire_event(wire::RequestCreated {
                request: told(23),
                ..Default::default()
            }),
        ),
        (
            api_event(api::RequesterAdded {
                request: Some(asked(24)).into(),
                user_id: 25,
                ..Default::default()
            }),
            wire_event(wire::RequesterAdded {
                request: told(24),
                user_id: 25,
                ..Default::default()
            }),
        ),
        (
            api_event(api::RequestStateChanged {
                request: Some(asked(26)).into(),
                ..Default::default()
            }),
            wire_event(wire::RequestStateChanged {
                request: told(26),
                ..Default::default()
            }),
        ),
        (
            api_event(api::RequestCancelled {
                request: Some(asked(27)).into(),
                deleted_by: Some("ada".to_owned()),
                ..Default::default()
            }),
            wire_event(wire::RequestCancelled {
                request: told(27),
                deleted_by: Some("ada".to_owned()),
                ..Default::default()
            }),
        ),
        (
            api_event(api::RequesterRemoved {
                request: Some(asked(28)).into(),
                user_id: 29,
                ..Default::default()
            }),
            wire_event(wire::RequesterRemoved {
                request: told(28),
                user_id: 29,
                ..Default::default()
            }),
        ),
        (
            api_event(api::RequestWithdrawn {
                request: Some(asked(30)).into(),
                ..Default::default()
            }),
            wire_event(wire::RequestWithdrawn {
                request: told(30),
                ..Default::default()
            }),
        ),
    ];

    for (sent, expected) in kinds {
        assert_eq!(event(&sent), Some(expected));
    }
}

#[test]
fn request_progress_and_followed_show_events_are_left_out() {
    let progress = api_event(api::RequestProgress::default());
    let followed = api_event(api::FollowedShowCreated::default());

    assert_eq!(event(&progress), None);
    assert_eq!(event(&followed), None);
}

#[test]
fn each_playback_event_keeps_what_an_observer_reads() {
    let playing = |method: api::PlaybackMethod| api::Playback {
        user_id: 2,
        device_id: 3,
        version_id: 4,
        method: method.into(),
        tracks: vec![api::Track::default()],
        ..Default::default()
    };
    let told = |method: wire::PlaybackMethod| {
        Some(wire::Playback {
            user_id: 2,
            device_id: 3,
            version_id: 4,
            method: method.into(),
            ..Default::default()
        })
        .into()
    };
    let kinds = [
        (
            api_event(api::PlaybackStarted {
                playback: Some(playing(api::PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY)).into(),
                ..Default::default()
            }),
            wire_event(wire::PlaybackStarted {
                playback: told(wire::PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY),
                ..Default::default()
            }),
        ),
        (
            api_event(api::PlaybackEnded {
                playback: Some(playing(api::PlaybackMethod::PLAYBACK_METHOD_REMUX)).into(),
                ..Default::default()
            }),
            wire_event(wire::PlaybackEnded {
                playback: told(wire::PlaybackMethod::PLAYBACK_METHOD_REMUX),
                ..Default::default()
            }),
        ),
        (
            api_event(api::PlaybackChanged {
                playback: Some(playing(api::PlaybackMethod::PLAYBACK_METHOD_TRANSCODE)).into(),
                ..Default::default()
            }),
            wire_event(wire::PlaybackChanged {
                playback: told(wire::PlaybackMethod::PLAYBACK_METHOD_TRANSCODE),
                ..Default::default()
            }),
        ),
    ];

    for (sent, expected) in kinds {
        assert_eq!(event(&sent), Some(expected));
    }
}

#[test]
fn a_mixed_batch_keeps_the_events_that_convert_in_order() {
    let batch = [
        api_event(api::ScanStarted {
            library_id: 1,
            ..Default::default()
        }),
        api_event(api::RequestProgress::default()),
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
        api_event(api::FollowedShowCreated::default()),
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

#[test]
fn a_request_created_notification_keeps_its_request_and_link() {
    let asked = api::Request {
        id: 7,
        ..Default::default()
    };
    let created = api::Event {
        event: Some(api::event::Event::from(api::RequestCreated {
            request: asked.clone().into(),
            ..Default::default()
        })),
        ..Default::default()
    };
    let link = Some("https://nama.example/requests/7".to_owned());

    let sent = notification(&created, &asked, link.clone()).unwrap();

    assert_eq!(sent.event.as_option(), event(&created).as_ref());
    assert_eq!(sent.request.as_option(), Some(&request(&asked)));
    assert_eq!(sent.request_url, link);
}

#[test]
fn a_request_progress_notification_gives_none() {
    let progress = api::Event {
        event: Some(api::event::Event::from(api::RequestProgress::default())),
        ..Default::default()
    };

    assert_eq!(
        notification(&progress, &api::Request::default(), None),
        None
    );
}
