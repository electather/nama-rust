//! What a Request manager or a Requester decides about a Request, and cancelling the Requests
//! that deleted media leaves waiting.

use nama_proto::nama::v1::{
    Event, Request, RequestCancelled, RequestState, RequestWithdrawn, RequesterRemoved,
};
use sqlx::SqliteConnection;

use super::apply::Applied;
use super::asked::{self, Asked};
use super::listing::{load, load_one, visible_to};
use super::quality::{NewQuality, change_quality};
use super::{Error, Requests};
use crate::{events::event, users::User};

/// A Movie, or a Season of a Show, as a Request names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct RequestTitle {
    /// The Movie's TMDB id, or the Show's.
    pub(crate) tmdb_id: i64,
    /// The Season's number, `None` for a Movie.
    pub(crate) season: Option<u32>,
}

impl Requests {
    /// Approves the Pending Request `id` as `manager`, first changing its Quality to the one with
    /// the id `quality` if given, and hands it to its Fulfiller, publishing each change of its
    /// state.
    pub(crate) async fn approve(
        &self,
        manager: &User,
        id: i64,
        quality: Option<i64>,
    ) -> Result<Request, Error> {
        let new_quality = self.new_quality(manager, id, quality).await?;
        self.apply(move |tx| Box::pin(approve(tx, id, new_quality)))
            .await
    }

    /// Moves the Failed Request `id` back to Approved as `manager`, changing its Quality as
    /// [`Self::approve`] does, and hands it to its Fulfiller again, publishing each change of its
    /// state.
    pub(crate) async fn retry(
        &self,
        manager: &User,
        id: i64,
        quality: Option<i64>,
    ) -> Result<Request, Error> {
        let new_quality = self.new_quality(manager, id, quality).await?;
        self.apply(move |tx| Box::pin(retry(tx, id, new_quality)))
            .await
    }

    /// Declines the Pending Request `id`, giving its Requesters `reason`, trimmed, unless it is
    /// blank, and publishes that its state changed.
    pub(crate) async fn decline(&self, id: i64, reason: &str) -> Result<Request, Error> {
        let reason = given_reason(reason);
        self.apply(move |tx| Box::pin(decline(tx, id, reason)))
            .await
    }

    /// Removes the Request `id`, whatever its state, and publishes that it was cancelled.
    pub(crate) async fn cancel(&self, id: i64) -> Result<(), Error> {
        self.apply(move |tx| Box::pin(cancel(tx, id))).await
    }

    /// The Requests [`Self::cancel_deleted`] would cancel for `titles`.
    pub(crate) async fn waiting_on(&self, titles: &[RequestTitle]) -> Result<Vec<Request>, Error> {
        let mut conn = self.db.reader().acquire().await?;
        let mut waiting = Vec::new();
        for &title in titles {
            if let Some(id) = waiting_on(&mut conn, title).await? {
                waiting.extend(load_one(&mut conn, id).await?);
            }
        }
        Ok(waiting)
    }

    /// Cancels the Requests still waiting on media of `titles` that `by` deleted, telling their
    /// Requesters who. Available ones stay, so a Followed Show doesn't ask for them again, and no
    /// Followed Show ends.
    pub(crate) async fn cancel_deleted(
        &self,
        titles: Vec<RequestTitle>,
        by: String,
    ) -> Result<(), Error> {
        self.apply(move |tx| Box::pin(cancel_deleted(tx, titles, by)))
            .await
    }

    /// Takes `requester` off the Pending Request `id`, removing it once nobody is left on it, and
    /// publishes which of the two happened.
    pub(crate) async fn withdraw(&self, requester: &User, id: i64) -> Result<(), Error> {
        let user = requester.id;
        let visible = visible_to(requester);
        self.apply(move |tx| Box::pin(withdraw(tx, user, visible, id)))
            .await
    }
}

/// `reason`, trimmed, unless it is blank.
pub(super) fn given_reason(reason: &str) -> Option<String> {
    let reason = reason.trim();
    (!reason.is_empty()).then(|| reason.to_owned())
}

/// Moves the Pending Request `id` to `state` with `reason`, and returns it as it is now.
pub(super) async fn decide(
    conn: &mut SqliteConnection,
    id: i64,
    state: RequestState,
    reason: Option<String>,
) -> Result<Request, Error> {
    let decided = asked::decide(conn, Asked::Request, id, state, reason.as_deref()).await?;
    let request = load_one(conn, id)
        .await?
        .ok_or(Error::NoSuch(Asked::Request, id))?;
    if !decided {
        return Err(Error::NotPending(Asked::Request, id));
    }
    Ok(request)
}

/// Approves the Pending Request `id`, first changing its Quality to `new_quality` if given.
pub(super) async fn approve(
    conn: &mut SqliteConnection,
    id: i64,
    new_quality: Option<NewQuality>,
) -> Result<Applied<Request>, Error> {
    // Undone with the rest if the Request isn't Pending.
    change_quality(conn, id, new_quality).await?;
    let approved = decide(conn, id, RequestState::Approved, None).await?;
    Ok(Applied::approved(approved))
}

/// Declines the Pending Request `id`, giving its Requesters `reason`.
pub(super) async fn decline(
    conn: &mut SqliteConnection,
    id: i64,
    reason: Option<String>,
) -> Result<Applied<Request>, Error> {
    let declined = decide(conn, id, RequestState::Declined, reason).await?;
    Ok(Applied::moved(declined))
}

/// Moves the Failed Request `id` back to Approved at `new_quality`, if given, forgetting why it
/// failed, so it is handed off again.
async fn retry(
    conn: &mut SqliteConnection,
    id: i64,
    new_quality: Option<NewQuality>,
) -> Result<Applied<Request>, Error> {
    // Undone with the rest if the Request isn't Failed.
    change_quality(conn, id, new_quality).await?;
    let retried = sqlx::query!(
        "UPDATE requests SET state = 'approved', failure_reason = NULL
         WHERE id = ? AND state = 'failed'",
        id
    )
    .execute(&mut *conn)
    .await?
    .rows_affected()
        > 0;
    let request = load_one(conn, id)
        .await?
        .ok_or(Error::NoSuch(Asked::Request, id))?;
    if !retried {
        return Err(Error::NotFailed(id));
    }
    Ok(Applied::approved(request))
}

/// Removes the Request `id`, and its Requesters with it.
pub(super) async fn cancel(conn: &mut SqliteConnection, id: i64) -> Result<Applied<()>, Error> {
    let cancelled = remove(conn, id, None).await?;
    Ok(Applied::saying((), cancelled))
}

/// Removes the Request waiting on each of `titles`, which `by` deleted media of.
async fn cancel_deleted(
    conn: &mut SqliteConnection,
    titles: Vec<RequestTitle>,
    by: String,
) -> Result<Applied<()>, Error> {
    let mut events = Vec::new();
    for title in titles {
        if let Some(id) = waiting_on(conn, title).await? {
            events.push(remove(conn, id, Some(by.clone())).await?);
        }
    }
    Ok(Applied {
        events,
        ..Applied::quiet(())
    })
}

/// Removes the Request `id`, and its Requesters with it, and returns the event saying so, which
/// names who `deleted_by` if deleting media cancelled it.
async fn remove(
    conn: &mut SqliteConnection,
    id: i64,
    deleted_by: Option<String>,
) -> Result<Event, Error> {
    let request = load_one(conn, id)
        .await?
        .ok_or(Error::NoSuch(Asked::Request, id))?;
    sqlx::query!("DELETE FROM requests WHERE id = ?", id)
        .execute(&mut *conn)
        .await?;
    Ok(event(RequestCancelled {
        request: request.into(),
        deleted_by,
        ..Default::default()
    }))
}

/// The Request still waiting on `title`'s media, which deleting any of it cancels: its open one,
/// unless Failed, which waits on a Request manager retrying it rather than on the media.
async fn waiting_on(
    conn: &mut SqliteConnection,
    RequestTitle { tmdb_id, season }: RequestTitle,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar!(
        "SELECT id FROM requests WHERE tmdb_id = ? AND season IS ?
         AND state IN ('pending', 'approved', 'processing', 'partially_available')",
        tmdb_id,
        season
    )
    .fetch_optional(&mut *conn)
    .await
}

/// Takes `user` off the Pending Request `id`, among those `visible` lets them see, removing the
/// Request if they were its last Requester.
async fn withdraw(
    conn: &mut SqliteConnection,
    user: i64,
    visible: Option<i64>,
    id: i64,
) -> Result<Applied<()>, Error> {
    let found = load(conn, Some(id), visible).await?.pop();
    let request = asked::requested_by(Asked::Request, id, user, found, |request| {
        &request.requesters
    })?;
    if request.state != RequestState::Pending {
        return Err(Error::NotPending(Asked::Request, id));
    }
    let was_last = asked::leave(conn, Asked::Request, id, user).await?;
    let left = if was_last {
        event(RequestWithdrawn {
            request: request.into(),
            ..Default::default()
        })
    } else {
        event(RequesterRemoved {
            request: request.into(),
            user_id: user,
            ..Default::default()
        })
    };
    Ok(Applied::saying((), left))
}

#[cfg(test)]
#[path = "deciding_tests.rs"]
mod tests;
