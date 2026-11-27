//! Subtitle and Audio Preferences for a Linked Server: `LinkedServerService.PushTrackPreferences`
//! and `PullTrackPreferences`, and the conversions and refusals they use.

use connectrpc::{ConnectError, RequestContext, Response, ServiceResult};
use nama_hub_proto::nama::hub::v1::{
    self as proto, PullTrackPreferencesRequest, PullTrackPreferencesResponse,
    PushTrackPreferencesRequest, PushTrackPreferencesResponse, RefusalReason, TitleKind,
};

use super::{
    hub_failed,
    links::{LinkedServerServiceHandler, linked_server},
    refuse, sync_refused,
};
use crate::sync::{
    self, Like, MAX_CHANGES, MAX_ID_LENGTH, MAX_LANGUAGE_LENGTH, MAX_TITLE_LENGTH, Part,
    PreferenceFor, TrackPreferenceChange,
};

pub(super) async fn push_track_preferences(
    handler: &LinkedServerServiceHandler,
    ctx: &RequestContext,
    request: PushTrackPreferencesRequest,
) -> ServiceResult<PushTrackPreferencesResponse> {
    let server = linked_server(ctx)?;
    let changes = changes_from_proto(request.changes)?;
    (handler.track_preferences)
        .push(&handler.links, server, changes)
        .await
        .map_err(|err| sync_refused(&err, "changes", invalid))?;
    Response::ok(PushTrackPreferencesResponse::default())
}

pub(super) async fn pull_track_preferences(
    handler: &LinkedServerServiceHandler,
    ctx: &RequestContext,
    request: PullTrackPreferencesRequest,
) -> ServiceResult<PullTrackPreferencesResponse> {
    let pulled = handler
        .track_preferences
        .pull(&handler.links, linked_server(ctx)?, request.user_id)
        .await
        .map_err(|err| hub_failed(&err))?;
    Response::ok(PullTrackPreferencesResponse {
        preferences: pulled.into_iter().map(change_to_proto).collect(),
        ..Default::default()
    })
}

/// `changes` as `sync` takes them, or the refusal of the first whose kind doesn't fit its title
/// id. `sync` checks the rest.
pub(super) fn changes_from_proto(
    changes: Vec<proto::TrackPreferenceChange>,
) -> Result<Vec<TrackPreferenceChange>, ConnectError> {
    (changes.into_iter().enumerate())
        .map(|(nth, change)| change_from_proto(nth, change))
        .collect()
}

/// The `nth` change of a push, as `sync` takes it.
fn change_from_proto(
    nth: usize,
    change: proto::TrackPreferenceChange,
) -> Result<TrackPreferenceChange, ConnectError> {
    let broken = |part| sync_refused(&sync::Error::Invalid { nth, part }, "changes", invalid);
    let title_id = change.title_id;
    let kept_for = match change.title_kind.as_known() {
        Some(TitleKind::TITLE_KIND_UNSPECIFIED) if title_id.is_empty() => PreferenceFor::User,
        Some(TitleKind::TITLE_KIND_MOVIE) => PreferenceFor::Movie(title_id),
        Some(TitleKind::TITLE_KIND_SHOW) => PreferenceFor::Show(title_id),
        Some(TitleKind::TITLE_KIND_UNSPECIFIED) | None => return Err(broken(Part::TitleId)),
    };
    // A change turned off reaches `sync` without its language and title, which a Linked Server
    // still gets refused for when over-long, as it always has.
    if change.off && change.language.chars().count() > MAX_LANGUAGE_LENGTH {
        return Err(broken(Part::Language));
    }
    if change.off && change.title.chars().count() > MAX_TITLE_LENGTH {
        return Err(broken(Part::Title));
    }
    Ok(TrackPreferenceChange {
        user_id: change.user_id,
        audio: change.audio,
        kept_for,
        like: (!change.off).then_some(Like {
            language: change.language,
            forced: change.forced,
            sdh: change.sdh,
            title: change.title,
        }),
        foreign_audio_only: change.foreign_audio_only,
        changed_at_ms: change.changed_at_ms,
    })
}

/// `change`, as a Linked Server takes it.
pub(super) fn change_to_proto(change: TrackPreferenceChange) -> proto::TrackPreferenceChange {
    let (title_kind, title_id) = match change.kept_for {
        PreferenceFor::User => (TitleKind::TITLE_KIND_UNSPECIFIED, String::new()),
        PreferenceFor::Movie(id) => (TitleKind::TITLE_KIND_MOVIE, id),
        PreferenceFor::Show(id) => (TitleKind::TITLE_KIND_SHOW, id),
    };
    let off = change.like.is_none();
    let like = change.like.unwrap_or_default();
    proto::TrackPreferenceChange {
        user_id: change.user_id,
        audio: change.audio,
        title_kind: title_kind.into(),
        title_id,
        off,
        language: like.language,
        forced: like.forced,
        sdh: like.sdh,
        title: like.title,
        foreign_audio_only: change.foreign_audio_only,
        changed_at_ms: change.changed_at_ms,
        ..Default::default()
    }
}

/// Refuses a push for a change at `field` that breaks a limit, which `message` names.
fn invalid(field: &str, message: impl Into<String>) -> ConnectError {
    refuse(
        RefusalReason::REFUSAL_REASON_TRACK_PREFERENCE_INVALID,
        message,
    )
    .field(field)
    .param("maxChanges", &MAX_CHANGES)
    .param("maxIdLength", &MAX_ID_LENGTH)
    .param("maxLanguageLength", &MAX_LANGUAGE_LENGTH)
    .param("maxTitleLength", &MAX_TITLE_LENGTH)
    .into()
}
