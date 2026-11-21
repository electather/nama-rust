//! The states a Request is stored in, which of them are open, and the two moves no User decides:
//! a Library making a Request Available and a Fulfiller settling its Hand-off.

use nama_plugin_api::{
    fulfiller::Fulfilled,
    metadata::{ExternalId, ExternalIds},
};
use nama_proto::nama::v1::{Event, Request, RequestState, RequestStateChanged, RequestedQuality};
use sqlx::SqliteConnection;

use super::apply::Applied;
use super::asked::Asked;
use super::listing::load_one;
use super::quality::quality_columns;
use super::{Error, Requests, progress, qualities};
use crate::{events::event, plugins::Answer};

/// Each Request state as `requests.state` stores it.
pub(super) const STATES: [(RequestState, &str); 7] = [
    (RequestState::Pending, "pending"),
    (RequestState::Approved, "approved"),
    (RequestState::Declined, "declined"),
    (RequestState::Processing, "processing"),
    (RequestState::PartiallyAvailable, "partially_available"),
    (RequestState::Available, "available"),
    (RequestState::Failed, "failed"),
];

impl Requests {
    /// Settles the Hand-off of the Request `id`, asked for at the Quality `asked`, with what its
    /// Fulfiller answered, through [`Self::apply`]'s path in the Hand-off's own task. Returns the
    /// Request as it is now, or `None` if it was changed meanwhile.
    pub(super) async fn settle_hand_off(
        &self,
        id: i64,
        fulfilled: Answer<Fulfilled, Vec<String>>,
        asked: Option<RequestedQuality>,
    ) -> Result<Option<Request>, Error> {
        // Not detached again: the Hand-off already is.
        self.applied(move |tx| Box::pin(settle(tx, id, fulfilled, asked)))
            .await
    }
}

// sqlx reads a query from string literals alone, so SQL that queries share is a literal a macro
// hands to each query's local macro, as `asked::per_kind!` hands it a table's name.

/// Hands `$query!` the SQL for whether a Request is open. It must match the
/// `one_open_request_per_movie_or_season` index's.
macro_rules! open_request_sql {
    ($query:ident) => {
        $query!("state NOT IN ('declined', 'available')")
    };
}

/// Hands `$query!` the SQL for whether a Followed Show is open. It must match the
/// `one_open_followed_show` index's.
macro_rules! open_followed_show_sql {
    ($query:ident) => {
        $query!("state != 'declined'")
    };
}
pub(super) use open_followed_show_sql;

/// Whether a Request in `state` is open: Declined and Available ones are closed, and Cancelled or
/// Withdrawn ones are gone.
pub(super) fn is_open(state: RequestState) -> bool {
    !matches!(state, RequestState::Declined | RequestState::Available)
}

/// The open Request for the Movie `id`, or for its Season `season` if `id` is a Show's.
pub(super) async fn open_request(
    conn: &mut SqliteConnection,
    id: &ExternalId,
    season: Option<u32>,
) -> Result<Option<i64>, sqlx::Error> {
    let title_id = id.to_string();
    macro_rules! find {
        ($open:literal) => {
            sqlx::query_scalar!(
                "SELECT id FROM requests WHERE title_id = ? AND season IS ? AND " + $open,
                title_id,
                season
            )
        };
    }
    open_request_sql!(find).fetch_optional(&mut *conn).await
}

/// The open Requests for the Movie with any of `ids`, or for its Season `season` if `ids` are a
/// Show's: those whose `request_title_ids` share a namespace and value with `ids`, whichever id
/// each is kept under (ADR 0042).
pub(super) async fn open_requests(
    conn: &mut SqliteConnection,
    ids: &ExternalIds,
    season: Option<u32>,
) -> Result<Vec<i64>, sqlx::Error> {
    let ids = ids_json(ids);
    macro_rules! find {
        ($open:literal) => {
            sqlx::query_scalar!(
                r#"SELECT DISTINCT requests.id AS "id!" FROM requests
                 JOIN request_title_ids AS title ON title.request_id = requests.id
                 JOIN json_each(?) AS wanted
                     ON title.namespace = wanted.key AND title.value = wanted.value
                 WHERE requests.season IS ? AND "#
                    + $open,
                ids,
                season
            )
        };
    }
    open_request_sql!(find).fetch_all(&mut *conn).await
}

/// The open Requests for any Season of the Show with any of `ids`, earliest Season first, found
/// as [`open_requests`] finds them.
pub(super) async fn open_season_requests(
    conn: &mut SqliteConnection,
    ids: &ExternalIds,
) -> Result<Vec<i64>, sqlx::Error> {
    let ids = ids_json(ids);
    macro_rules! find {
        ($open:literal) => {
            sqlx::query_scalar!(
                r#"SELECT id AS "id!" FROM requests
                 WHERE season IS NOT NULL
                   AND id IN (SELECT title.request_id FROM request_title_ids AS title
                              JOIN json_each(?) AS wanted
                                  ON title.namespace = wanted.key AND title.value = wanted.value)
                   AND "#
                    + $open
                    + " ORDER BY season",
                ids
            )
        };
    }
    open_request_sql!(find).fetch_all(&mut *conn).await
}

/// `ids` as a JSON object of namespace to value, which SQLite's `json_each` reads.
pub(super) fn ids_json(ids: &ExternalIds) -> String {
    let pairs: serde_json::Map<_, _> = ids
        .iter()
        .map(|id| (id.namespace().to_owned(), id.value().into()))
        .collect();
    serde_json::Value::Object(pairs).to_string()
}

/// Moves the Request `id` to `state`, Available or Partially Available, and returns the event for
/// it, or `None` if it didn't move. Only a Request its Fulfiller has becomes Partially Available:
/// one not yet handed off keeps its state, so it can still be decided, handed off or retried
/// (ADR 0012).
pub(super) async fn change_state(
    conn: &mut SqliteConnection,
    id: i64,
    state: RequestState,
) -> Result<Option<Event>, sqlx::Error> {
    let state = stored(state);
    macro_rules! change {
        ($fulfilling:literal) => {
            sqlx::query!(
                "UPDATE requests SET state = ?1, failure_reason = NULL, failure_needs = NULL
                 WHERE id = ?2 AND state != ?1 AND (?1 = 'available' OR "
                    + $fulfilling
                    + ")",
                state,
                id
            )
        };
    }
    let changed = progress::fulfilling_sql!(change)
        .execute(&mut *conn)
        .await?
        .rows_affected()
        > 0;
    if !changed {
        return Ok(None);
    }
    let request = load_one(conn, id).await?.ok_or(sqlx::Error::RowNotFound)?;
    Ok(Some(event(RequestStateChanged {
        request: request.into(),
        ..Default::default()
    })))
}

/// Moves the Approved Request `id`, asked at the Quality `asked`, on as its Fulfiller said: to
/// Processing, keeping its reference and the Quality used, or to Failed, keeping why, with the id
/// namespaces the failure's `kind` names when the Kernel didn't hand it off for lack of an id.
/// Either way it keeps the Plugin that answered. Returns it as it is now, or `None` if it isn't
/// Approved any more.
pub(super) async fn settle(
    conn: &mut SqliteConnection,
    id: i64,
    answered: Answer<Fulfilled, Vec<String>>,
    asked: Option<RequestedQuality>,
) -> Result<Applied<Option<Request>>, Error> {
    let Answer { plugin, result } = answered;
    // In the same write as the reference, so a failed read can't lose a Request the Fulfiller took.
    let (state, reference, used, reason, needs) = match result {
        Ok(fulfilled) => {
            let listed = qualities::list(conn, &plugin).await?;
            let used = qualities::used(asked.as_ref(), &listed, &fulfilled);
            (
                RequestState::Processing,
                Some(fulfilled.reference),
                used,
                None,
                None,
            )
        }
        Err(failure) => (
            RequestState::Failed,
            None,
            None,
            Some(failure.message),
            (!failure.kind.is_empty()).then(|| failure.kind.join(",")),
        ),
    };
    let state = stored(state);
    let (used_id, used_name, used_profile_id) = quality_columns(used.as_ref());
    let plugin = plugin.as_str();
    let settled = sqlx::query!(
        "UPDATE requests SET state = ?, fulfiller_reference = ?, failure_reason = ?,
                             failure_needs = ?, fulfiller_plugin_id = ?,
                             quality_used_id = ?, quality_used_name = ?, quality_used_profile_id = ?
         WHERE id = ? AND state = 'approved'",
        state,
        reference,
        reason,
        needs,
        plugin,
        used_id,
        used_name,
        used_profile_id,
        id
    )
    .execute(&mut *conn)
    .await?
    .rows_affected()
        > 0;
    if !settled {
        return Ok(Applied::quiet(None));
    }
    let request = load_one(conn, id)
        .await?
        .ok_or(Error::NoSuch(Asked::Request, id))?;
    Ok(Applied::saying(
        Some(request.clone()),
        state_changed(request),
    ))
}

/// The event saying `request` is now in the state it has.
pub(super) fn state_changed(request: Request) -> Event {
    event(RequestStateChanged {
        request: request.into(),
        ..Default::default()
    })
}

/// `state` as `requests.state` stores it. Unspecified has no stored form: `""` fails the CHECK.
pub(super) fn stored(state: RequestState) -> &'static str {
    STATES
        .iter()
        .find(|&&(known, _)| known == state)
        .map_or("", |&(_, name)| name)
}

pub(super) fn read_state(stored: &str) -> RequestState {
    STATES
        .iter()
        .find(|&&(_, name)| name == stored)
        .map_or(RequestState::REQUEST_STATE_UNSPECIFIED, |&(state, _)| state)
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
