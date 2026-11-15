use crate::{
    Notification,
    nama_plugin_proto::nama::plugin::v1::{
        Event, QualityChange, Request, RequestCancelled, RequestState, RequestStateChanged,
        RequestedMovie, RequestedQuality, RequestedSeason, Requester, RequesterAdded, event,
    },
};

use super::what_happened;

fn requester(user_id: i64, name: &str) -> Requester {
    Requester {
        user_id,
        name: name.to_owned(),
        ..Requester::default()
    }
}

/// Dune, released in 2021, in `state`, asked for by Sam then Ada.
fn dune(state: RequestState) -> Request {
    Request {
        movie_or_season: RequestedMovie {
            title: "Dune".to_owned(),
            release_date: Some("2021-09-15".to_owned()),
            ..RequestedMovie::default()
        }
        .into(),
        state: state.into(),
        requesters: vec![requester(1, "Sam"), requester(2, "Ada")],
        ..Request::default()
    }
}

fn notify(event: impl Into<Option<event::Event>>, request: Request) -> Option<String> {
    let event = Event {
        event: event.into(),
        ..Event::default()
    };
    what_happened(&Notification {
        event: event.into(),
        request: request.into(),
        ..Notification::default()
    })
}

#[test]
fn a_failed_request_says_why() {
    let request = Request {
        failure_reason: Some("Radarr didn't answer".to_owned()),
        ..dune(RequestState::REQUEST_STATE_FAILED)
    };

    let posted = notify(RequestStateChanged::default(), request);

    assert_eq!(
        posted.as_deref(),
        Some("Dune (2021) is now Failed. Reason: Radarr didn't answer")
    );
}

/// Dune Approved at `name`.
fn dune_approved_at(name: &str) -> Request {
    Request {
        quality: RequestedQuality {
            name: name.to_owned(),
            ..RequestedQuality::default()
        }
        .into(),
        ..dune(RequestState::REQUEST_STATE_APPROVED)
    }
}

#[test]
fn an_approved_request_names_its_quality_and_who_changed_it_from_what() {
    let changed = Request {
        quality_change: QualityChange {
            changed_by: Some("Sam".to_owned()),
            previous_name: Some("4K".to_owned()),
            ..QualityChange::default()
        }
        .into(),
        ..dune_approved_at("HD")
    };

    let unchanged = notify(RequestStateChanged::default(), dune_approved_at("HD"));
    let changed = notify(RequestStateChanged::default(), changed);

    assert_eq!(
        unchanged.as_deref(),
        Some("Dune (2021) is now Approved at HD.")
    );
    assert_eq!(
        changed.as_deref(),
        Some("Dune (2021) is now Approved at HD, changed from 4K by Sam.")
    );
}

#[test]
fn a_quality_changed_from_none_by_a_deleted_user_says_a_request_manager_changed_it() {
    let request = Request {
        quality_change: QualityChange::default().into(),
        ..dune_approved_at("HD")
    };

    let posted = notify(RequestStateChanged::default(), request);

    assert_eq!(
        posted.as_deref(),
        Some("Dune (2021) is now Approved at HD, changed by a Request manager.")
    );
}

#[test]
fn a_season_is_named_by_its_show_and_number() {
    let request = Request {
        movie_or_season: RequestedSeason {
            show_title: "Dark".to_owned(),
            season_number: 2,
            air_date: Some("2019-06-21".to_owned()),
            ..RequestedSeason::default()
        }
        .into(),
        ..dune(RequestState::REQUEST_STATE_PARTIALLY_AVAILABLE)
    };

    let posted = notify(RequestStateChanged::default(), request);

    assert_eq!(
        posted.as_deref(),
        Some("Dark Season 2 (2019) is now Partially Available.")
    );
}

#[test]
fn a_requester_joining_is_named() {
    let added = RequesterAdded {
        user_id: 2,
        ..RequesterAdded::default()
    };

    let posted = notify(added, dune(RequestState::REQUEST_STATE_PENDING));

    assert_eq!(posted.as_deref(), Some("Ada also asked for Dune (2021)."));
}

#[test]
fn a_cancelled_request_is_posted() {
    let request = dune(RequestState::REQUEST_STATE_PROCESSING);

    let cancelled = notify(RequestCancelled::default(), request);

    assert_eq!(
        cancelled.as_deref(),
        Some("The Request for Dune (2021) was cancelled.")
    );
}

#[test]
fn a_request_deleting_cancelled_names_who_deleted_it() {
    let deleted = RequestCancelled {
        deleted_by: Some("Grace".to_owned()),
        ..Default::default()
    };

    let posted = notify(deleted, dune(RequestState::REQUEST_STATE_PROCESSING));

    assert_eq!(
        posted.as_deref(),
        Some("Grace deleted Dune (2021), so its Request was cancelled.")
    );
}
