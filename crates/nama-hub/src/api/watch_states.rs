//! The conversions `LinkedServerService.PushWatchStates` and `PullWatchStates` use.

use connectrpc::ConnectError;
use nama_hub_proto::nama::hub::v1::{self as proto, RefusalReason};

use super::refuse;
use crate::sync::{MAX_CHANGED_AT_MS, MAX_CHANGES, MAX_ID_LENGTH, MAX_IDS, WatchStateChange};

/// `changes` as `sync` takes them, or the refusal of the first limit they break.
pub(super) fn changes_from_proto(
    changes: Vec<proto::WatchStateChange>,
) -> Result<Vec<WatchStateChange>, ConnectError> {
    if changes.len() > MAX_CHANGES {
        return Err(invalid_change(
            "changes",
            format!("a push holds up to {MAX_CHANGES} changes"),
        ));
    }
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
    let id_fits = |id: &String| !id.is_empty() && id.chars().count() <= MAX_ID_LENGTH;
    if !(1..=MAX_IDS).contains(&change.ids.len()) || !change.ids.iter().all(id_fits) {
        return Err(invalid_change(
            &field("ids"),
            format!(
                "a change names 1 to {MAX_IDS} External IDs of 1 to {MAX_ID_LENGTH} characters"
            ),
        ));
    }
    let duration_ms = i64::try_from(change.duration_ms)
        .map_err(|_too_long| invalid_change(&field("duration_ms"), "the duration is too long"))?;
    let position_ms = (i64::try_from(change.position_ms).ok())
        .filter(|&position_ms| position_ms <= duration_ms)
        .ok_or_else(|| {
            invalid_change(&field("position_ms"), "the position is past the duration")
        })?;
    if !(0..=MAX_CHANGED_AT_MS).contains(&change.changed_at_ms) {
        return Err(invalid_change(
            &field("changed_at_ms"),
            "a change is dated from 1970 to the end of 9999",
        ));
    }
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
