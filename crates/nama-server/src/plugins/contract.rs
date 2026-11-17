//! The client API's values as the Plugin contract's, and back, converted once where the Kernel
//! calls a Plugin, so Built-in and Installed Plugins see the same messages.

use buffa::EnumValue;
use nama_plugin_proto::nama::plugin::v1 as wire;
use nama_proto::nama::v1::{self as api, request::MovieOrSeason};

/// The download `download` reports, as the Kernel keeps it on a Request.
pub(crate) fn download(download: wire::Download) -> api::Download {
    let status = match download.status {
        EnumValue::Known(wire::DownloadStatus::DOWNLOAD_STATUS_QUEUED) => {
            api::DownloadStatus::DOWNLOAD_STATUS_QUEUED
        }
        EnumValue::Known(wire::DownloadStatus::DOWNLOAD_STATUS_DOWNLOADING) => {
            api::DownloadStatus::DOWNLOAD_STATUS_DOWNLOADING
        }
        EnumValue::Known(wire::DownloadStatus::DOWNLOAD_STATUS_IMPORTING) => {
            api::DownloadStatus::DOWNLOAD_STATUS_IMPORTING
        }
        EnumValue::Known(wire::DownloadStatus::DOWNLOAD_STATUS_NEEDS_ATTENTION) => {
            api::DownloadStatus::DOWNLOAD_STATUS_NEEDS_ATTENTION
        }
        EnumValue::Known(wire::DownloadStatus::DOWNLOAD_STATUS_UNSPECIFIED)
        | EnumValue::Unknown(_) => api::DownloadStatus::DOWNLOAD_STATUS_UNSPECIFIED,
    };
    api::Download {
        title: download.title,
        percent: download.percent,
        size_bytes: download.size_bytes,
        eta_seconds: download.eta_seconds,
        status: status.into(),
        attention_reason: download.attention_reason,
        ..Default::default()
    }
}

/// `ids` as the contract carries a title's ids, one for one.
fn title_ids(ids: &[api::ExternalId]) -> Vec<wire::TitleId> {
    (ids.iter())
        .map(|id| wire::TitleId {
            namespace: id.namespace.clone(),
            value: id.value.clone(),
            ..Default::default()
        })
        .collect()
}

/// The Request `request` as a Plugin reads it, whether a Fulfiller, an Observer or a Notifier,
/// leaving out what only a client reads.
pub(crate) fn request(request: &api::Request) -> wire::Request {
    let movie_or_season = request.movie_or_season.as_ref().map(|asked| match asked {
        MovieOrSeason::Movie(movie) => {
            wire::request::MovieOrSeason::Movie(Box::new(wire::RequestedMovie {
                title: movie.title.clone(),
                release_date: movie.release_date.clone(),
                ids: title_ids(&movie.ids),
                ..Default::default()
            }))
        }
        MovieOrSeason::Season(season) => {
            wire::request::MovieOrSeason::Season(Box::new(wire::RequestedSeason {
                season_number: season.season_number,
                show_title: season.show_title.clone(),
                air_date: season.air_date.clone(),
                show_ids: title_ids(&season.show_ids),
                ..Default::default()
            }))
        }
    });
    let requesters = request.requesters.iter().map(|requester| wire::Requester {
        user_id: requester.user_id,
        name: requester.name.clone(),
        requested_at: requester.requested_at,
        ..Default::default()
    });
    let quality = request
        .quality
        .as_option()
        .map(|quality| wire::RequestedQuality {
            id: quality.id,
            name: quality.name.clone(),
            profile_id: quality.profile_id.clone(),
            ..Default::default()
        });
    let quality_change = request
        .quality_change
        .as_option()
        .map(|change| wire::QualityChange {
            changed_by: change.changed_by.clone(),
            previous_name: change.previous_name.clone(),
            changed_at: change.changed_at,
            ..Default::default()
        });
    wire::Request {
        id: request.id,
        movie_or_season,
        state: request_state(request.state).into(),
        requesters: requesters.collect(),
        created_at: request.created_at,
        decline_reason: request.decline_reason.clone(),
        failure_reason: request.failure_reason.clone(),
        followed_show_id: request.followed_show_id,
        quality: quality.into(),
        quality_change: quality_change.into(),
        ..Default::default()
    }
}

/// The batch an Observer is told of `events`: those [`event`] converts, in order, or
/// `None` when none does, so an Observer is never called for left-out kinds alone.
pub(crate) fn observed(events: &[api::Event]) -> Option<wire::ObservedEvents> {
    let events: Vec<_> = events.iter().filter_map(event).collect();
    (!events.is_empty()).then(|| wire::ObservedEvents {
        events,
        ..Default::default()
    })
}

/// What a Notifier is told of `happened` to `asked`, linking to it at `request_url`, or `None`
/// when the event is a kind the contract leaves out, such as Request progress, so a Notifier is
/// never called for it.
pub(crate) fn notification(
    happened: &api::Event,
    asked: &api::Request,
    request_url: Option<String>,
) -> Option<wire::Notification> {
    Some(wire::Notification {
        event: event(happened)?.into(),
        request: request(asked).into(),
        request_url,
        ..Default::default()
    })
}

/// The contract's twin of `event`, or `None` for a kind the contract leaves out.
#[expect(
    clippy::too_many_lines,
    reason = "one exhaustive match, so a new client kind must be placed in or out"
)]
fn event(event: &api::Event) -> Option<wire::Event> {
    use api::event::Event as Api;
    use wire::event::Event as Wire;
    let event = match event.event.as_ref()? {
        Api::ScanStarted(started) => Wire::from(wire::ScanStarted {
            library_id: started.library_id,
            ..Default::default()
        }),
        Api::ScanProgress(progress) => Wire::from(wire::ScanProgress {
            library_id: progress.library_id,
            files_done: progress.files_done,
            files_total: progress.files_total,
            ..Default::default()
        }),
        Api::ScanFinished(finished) => Wire::from(wire::ScanFinished {
            library_id: finished.library_id,
            added: finished.added,
            updated: finished.updated,
            removed: finished.removed,
            unchanged: finished.unchanged,
            ..Default::default()
        }),
        Api::LibraryItemAdded(added) => Wire::from(wire::LibraryItemAdded {
            library_id: added.library_id,
            item_id: added.item_id,
            path: added.path.clone(),
            ..Default::default()
        }),
        Api::LibraryItemRemoved(removed) => Wire::from(wire::LibraryItemRemoved {
            library_id: removed.library_id,
            item_id: removed.item_id,
            ..Default::default()
        }),
        Api::ItemMatched(matched) => Wire::from(wire::ItemMatched {
            library_id: matched.library_id,
            item_id: matched.item_id,
            title: matched.title.clone(),
            ..Default::default()
        }),
        Api::MatchingFinished(finished) => Wire::from(wire::MatchingFinished {
            library_id: finished.library_id,
            matched: finished.matched,
            unmatched: finished.unmatched,
            undecided: finished.undecided,
            ..Default::default()
        }),
        Api::ProbingFinished(finished) => Wire::from(wire::ProbingFinished {
            library_id: finished.library_id,
            probed: finished.probed,
            failed: finished.failed,
            ..Default::default()
        }),
        Api::RequestCreated(created) => Wire::from(wire::RequestCreated {
            request: created.request.as_option().map(request).into(),
            ..Default::default()
        }),
        Api::RequesterAdded(added) => Wire::from(wire::RequesterAdded {
            request: added.request.as_option().map(request).into(),
            user_id: added.user_id,
            ..Default::default()
        }),
        Api::RequestStateChanged(changed) => Wire::from(wire::RequestStateChanged {
            request: changed.request.as_option().map(request).into(),
            ..Default::default()
        }),
        Api::RequestCancelled(cancelled) => Wire::from(wire::RequestCancelled {
            request: cancelled.request.as_option().map(request).into(),
            deleted_by: cancelled.deleted_by.clone(),
            ..Default::default()
        }),
        Api::RequesterRemoved(removed) => Wire::from(wire::RequesterRemoved {
            request: removed.request.as_option().map(request).into(),
            user_id: removed.user_id,
            ..Default::default()
        }),
        Api::RequestWithdrawn(withdrawn) => Wire::from(wire::RequestWithdrawn {
            request: withdrawn.request.as_option().map(request).into(),
            ..Default::default()
        }),
        Api::RequestProgress(_)
        | Api::FollowedShowCreated(_)
        | Api::FollowedShowRequesterAdded(_)
        | Api::FollowedShowStateChanged(_)
        | Api::FollowedShowCancelled(_)
        | Api::FollowedShowRequesterRemoved(_)
        | Api::FollowedShowWithdrawn(_) => return None,
        Api::PlaybackStarted(started) => Wire::from(wire::PlaybackStarted {
            playback: started.playback.as_option().map(playback).into(),
            ..Default::default()
        }),
        Api::PlaybackEnded(ended) => Wire::from(wire::PlaybackEnded {
            playback: ended.playback.as_option().map(playback).into(),
            ..Default::default()
        }),
        Api::PlaybackChanged(changed) => Wire::from(wire::PlaybackChanged {
            playback: changed.playback.as_option().map(playback).into(),
            ..Default::default()
        }),
    };
    Some(wire::Event {
        event: Some(event),
        ..Default::default()
    })
}

/// The contract's twin of `playback`, leaving out its Tracks and reasons.
fn playback(playback: &api::Playback) -> wire::Playback {
    let method = match playback.method {
        EnumValue::Known(api::PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY) => {
            wire::PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY
        }
        EnumValue::Known(api::PlaybackMethod::PLAYBACK_METHOD_REMUX) => {
            wire::PlaybackMethod::PLAYBACK_METHOD_REMUX
        }
        EnumValue::Known(api::PlaybackMethod::PLAYBACK_METHOD_TRANSCODE) => {
            wire::PlaybackMethod::PLAYBACK_METHOD_TRANSCODE
        }
        EnumValue::Known(api::PlaybackMethod::PLAYBACK_METHOD_UNSPECIFIED)
        | EnumValue::Unknown(_) => wire::PlaybackMethod::PLAYBACK_METHOD_UNSPECIFIED,
    };
    wire::Playback {
        user_id: playback.user_id,
        device_id: playback.device_id,
        version_id: playback.version_id,
        method: method.into(),
        plan_id: playback.plan_id,
        ..Default::default()
    }
}

/// The contract's twin of `state`, or unspecified for one this Kernel doesn't know.
fn request_state(state: EnumValue<api::RequestState>) -> wire::RequestState {
    match state {
        EnumValue::Known(api::RequestState::REQUEST_STATE_PENDING) => {
            wire::RequestState::REQUEST_STATE_PENDING
        }
        EnumValue::Known(api::RequestState::REQUEST_STATE_APPROVED) => {
            wire::RequestState::REQUEST_STATE_APPROVED
        }
        EnumValue::Known(api::RequestState::REQUEST_STATE_DECLINED) => {
            wire::RequestState::REQUEST_STATE_DECLINED
        }
        EnumValue::Known(api::RequestState::REQUEST_STATE_PROCESSING) => {
            wire::RequestState::REQUEST_STATE_PROCESSING
        }
        EnumValue::Known(api::RequestState::REQUEST_STATE_PARTIALLY_AVAILABLE) => {
            wire::RequestState::REQUEST_STATE_PARTIALLY_AVAILABLE
        }
        EnumValue::Known(api::RequestState::REQUEST_STATE_AVAILABLE) => {
            wire::RequestState::REQUEST_STATE_AVAILABLE
        }
        EnumValue::Known(api::RequestState::REQUEST_STATE_FAILED) => {
            wire::RequestState::REQUEST_STATE_FAILED
        }
        EnumValue::Known(api::RequestState::REQUEST_STATE_UNSPECIFIED) | EnumValue::Unknown(_) => {
            wire::RequestState::REQUEST_STATE_UNSPECIFIED
        }
    }
}

#[cfg(test)]
#[path = "contract_tests.rs"]
mod tests;
