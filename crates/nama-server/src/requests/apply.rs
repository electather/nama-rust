//! Applying a write to Requests: the one path every change to a Request or Followed Show takes
//! (ADR 0029).

use nama_proto::nama::v1::{Event, FollowedShow, Request};
use sqlx::{Sqlite, Transaction};

use super::state::state_changed;
use super::{Error, Requests};
use crate::db::{BoxFuture, detached};
use crate::events::Removed;

/// What a write to Requests did: its `value`, the events it built for what happened, the Requests
/// it approved, which [`Requests::apply`] hands off, the Season Request of a Followed Show it
/// ended, whose Show the Shows Fulfiller stops following, and the Requests and Followed Shows it
/// removed.
#[derive(Debug)]
pub(super) struct Applied<T> {
    pub(super) value: T,
    pub(super) events: Vec<Event>,
    pub(super) hand_off: Vec<Request>,
    pub(super) unfollow: Option<Request>,
    pub(super) removed: Vec<Removed>,
}

impl<T> Applied<T> {
    /// `value`, from a write that changed nothing worth an event.
    pub(super) fn quiet(value: T) -> Self {
        Self {
            value,
            events: Vec::new(),
            hand_off: Vec::new(),
            unfollow: None,
            removed: Vec::new(),
        }
    }

    /// `value`, from a write that `happened` says what it did.
    pub(super) fn saying(value: T, happened: Event) -> Self {
        Self {
            events: vec![happened],
            ..Self::quiet(value)
        }
    }
}

impl Applied<Request> {
    /// `request`, which the write moved to the state it is in now.
    pub(super) fn moved(request: Request) -> Self {
        Self::saying(request.clone(), state_changed(request))
    }

    /// `request`, which the write approved, so it is handed off.
    pub(super) fn approved(request: Request) -> Self {
        Self {
            hand_off: vec![request.clone()],
            ..Self::moved(request)
        }
    }
}

/// A write's value that may hold Requests the write hands off, which [`Requests::apply`] swaps
/// for those Requests as the Hand-off left them.
pub(super) trait HandedBack: Sized {
    /// `self` with any of `handed` in place of the Request with its id.
    fn handed_back(self, _handed: &[Request]) -> Self {
        self
    }
}

impl HandedBack for () {}

impl HandedBack for FollowedShow {}

impl<A: HandedBack, B: HandedBack> HandedBack for (A, B) {
    fn handed_back(self, handed: &[Request]) -> Self {
        (self.0.handed_back(handed), self.1.handed_back(handed))
    }
}

impl HandedBack for Request {
    fn handed_back(self, handed: &[Request]) -> Self {
        let now = handed.iter().find(|request| request.id == self.id);
        now.cloned().unwrap_or(self)
    }
}

impl HandedBack for Option<Request> {
    fn handed_back(self, handed: &[Request]) -> Self {
        self.map(|request| request.handed_back(handed))
    }
}

impl HandedBack for Vec<Request> {
    fn handed_back(self, handed: &[Request]) -> Self {
        self.into_iter()
            .map(|request| request.handed_back(handed))
            .collect()
    }
}

impl Requests {
    /// Runs `write`, publishes the events it built, has the Shows Fulfiller stop following the
    /// Show of a Followed Show it ended, then hands off the Requests it approved, and returns its
    /// value with those Requests as they are after the Hand-off. A failure to stop following is
    /// only logged: the Followed Show ended either way.
    ///
    /// The one path for Request changes (`src/requests/AGENTS.md`).
    ///
    /// Runs in a task of its own, since the write commits even if the caller stops waiting, so
    /// what follows it must too (ADR 0029).
    pub(super) async fn apply<T, W>(&self, write: W) -> Result<T, Error>
    where
        T: HandedBack + Send + 'static,
        W: for<'t> FnOnce(
                &'t mut Transaction<'_, Sqlite>,
            ) -> BoxFuture<'t, Result<Applied<T>, Error>>
            + Send
            + 'static,
    {
        let requests = self.clone();
        detached(async move { requests.applied(write).await }).await
    }

    /// [`Self::apply`], in the task the caller runs it in.
    pub(super) async fn applied<T, W>(&self, write: W) -> Result<T, Error>
    where
        T: HandedBack + Send + 'static,
        W: for<'t> FnOnce(
                &'t mut Transaction<'_, Sqlite>,
            ) -> BoxFuture<'t, Result<Applied<T>, Error>>
            + Send
            + 'static,
    {
        let claiming = self.hand_off.clone();
        let bus = self.events.clone();
        let (applied, claims) = self
            .db
            .write(move |tx| {
                Box::pin(async move {
                    let mut applied = write(tx).await?;
                    // Before the commit, so a subscriber reading an event about one never finds it
                    // gone and its Content Rating unknown.
                    bus.removing(std::mem::take(&mut applied.removed));
                    // The came-up sweep reads only committed Requests, so claiming them before the
                    // commit keeps it off this write's. A failed commit drops the claims.
                    let claims: Vec<_> = applied
                        .hand_off
                        .iter()
                        .map(|request| claiming.claim(request.id))
                        .collect();
                    Ok::<_, Error>((applied, claims))
                })
            })
            .await?;
        let Applied {
            value,
            events,
            hand_off,
            unfollow,
            ..
        } = applied;
        self.events.publish(events);
        if let Some(season) = unfollow {
            self.hand_off.unfollow(&season).await;
        }
        let mut handed = Vec::with_capacity(hand_off.len());
        let mut failed = None;
        // ponytail: one Hand-off at a time, so a slow Fulfiller takes as long for each Season.
        for (request, claim) in hand_off.into_iter().zip(claims) {
            let Some(claim) = claim else {
                // Another hand-off has it, and looks at it again once it is done.
                handed.push(request);
                continue;
            };
            // On past a failure: a came-up sweep that found the rest claimed left them to this.
            match self.hand_off.request(self, request, claim).await {
                Ok(request) => handed.push(request),
                Err(err) => _ = failed.get_or_insert(err),
            }
        }
        if let Some(err) = failed {
            return Err(err);
        }
        Ok(value.handed_back(&handed))
    }
}

#[cfg(test)]
#[path = "apply_tests.rs"]
mod tests;
