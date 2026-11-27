//! Watch States for a Linked Server: `LinkedServerService.PushWatchStates` and `PullWatchStates`,
//! and the conversions and refusals they use.

use connectrpc::{ConnectError, RequestContext, Response, ServiceResult};
use nama_hub_proto::nama::hub::v1::{
    self as proto, PullWatchStatesRequest, PullWatchStatesResponse, PushWatchStatesRequest,
    PushWatchStatesResponse, RefusalReason,
};

use super::{
    hub_failed,
    links::{LinkedServerServiceHandler, linked_server},
    refuse, sync_refused,
};
use crate::sync::{MAX_CHANGES, MAX_ID_LENGTH, MAX_IDS, WatchStateChange};

pub(super) async fn push_watch_states(
    handler: &LinkedServerServiceHandler,
    ctx: &RequestContext,
    request: PushWatchStatesRequest,
) -> ServiceResult<PushWatchStatesResponse> {
    let server = linked_server(ctx)?;
    let changes = changes_from_proto(request.changes)?;
    (handler.watch_states.push(&handler.links, server, changes))
        .await
        .map_err(|err| sync_refused(&err, "changes", invalid_change))?;
    Response::ok(PushWatchStatesResponse::default())
}

pub(super) async fn pull_watch_states(
    handler: &LinkedServerServiceHandler,
    ctx: &RequestContext,
    request: PullWatchStatesRequest,
) -> ServiceResult<PullWatchStatesResponse> {
    let pulled = handler
        .watch_states
        .pull(&handler.links, linked_server(ctx)?, request.user_id)
        .await
        .map_err(|err| hub_failed(&err))?;
    Response::ok(PullWatchStatesResponse {
        watch_states: pulled.into_iter().map(change_to_proto).collect(),
        ..Default::default()
    })
}

/// `changes` as `sync` takes them, or the refusal of the first that a `WatchStateChange` can't
/// hold. `sync` checks the rest.
pub(super) fn changes_from_proto(
    changes: Vec<proto::WatchStateChange>,
) -> Result<Vec<WatchStateChange>, ConnectError> {
    (changes.into_iter().enumerate())
        .map(|(nth, change)| change_from_proto(nth, change))
        .collect()
}

/// The `nth` change of a push, as `sync` takes it.
fn change_from_proto(
    nth: usize,
    change: proto::WatchStateChange,
) -> Result<WatchStateChange, ConnectError> {
    let field = |name: &str| format!("changes[{nth}].{name}");
    let duration_ms = i64::try_from(change.duration_ms)
        .map_err(|_too_long| invalid_change(&field("duration_ms"), "the duration is too long"))?;
    // Too long for a duration, so past any.
    let position_ms = i64::try_from(change.position_ms).map_err(|_too_long| {
        invalid_change(&field("position_ms"), "the position is past the duration")
    })?;
    Ok(WatchStateChange {
        user_id: change.user_id,
        episode: change.episode,
        ids: change.ids,
        played: change.played,
        position_ms,
        duration_ms,
        changed_at_ms: change.changed_at_ms,
    })
}

/// `change`, as a Linked Server takes it.
pub(super) fn change_to_proto(change: WatchStateChange) -> proto::WatchStateChange {
    proto::WatchStateChange {
        user_id: change.user_id,
        episode: change.episode,
        ids: change.ids,
        played: change.played,
        // Never negative: `watch_states` checks it.
        position_ms: u64::try_from(change.position_ms).unwrap_or_default(),
        duration_ms: u64::try_from(change.duration_ms).unwrap_or_default(),
        changed_at_ms: change.changed_at_ms,
        ..Default::default()
    }
}

/// Refuses a push for a change at `field` that breaks a limit, which `message` names.
fn invalid_change(field: &str, message: impl Into<String>) -> ConnectError {
    refuse(RefusalReason::REFUSAL_REASON_WATCH_STATE_INVALID, message)
        .field(field)
        .param("maxChanges", &MAX_CHANGES)
        .param("maxIds", &MAX_IDS)
        .param("maxIdLength", &MAX_ID_LENGTH)
        .into()
}
