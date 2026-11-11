//! What Requests does with a Fulfiller: the Hand-off of each Approved Request, and telling the
//! Shows Fulfiller to stop following a Show. Asking how the Requests it took are progressing is
//! [`super::progress`]'s.
//!
//! It holds no SQL: it calls Requests' functions to read Requests, and settles a Hand-off through
//! [`Requests::apply`]'s path, which calls it in turn.

use std::{
    collections::{HashMap, hash_map::Entry},
    sync::{Arc, Mutex},
};

use nama_plugin_api::{
    PluginId,
    fulfiller::{Fulfilled, Fulfiller, Movies, Shows},
};
use nama_proto::nama::v1::{Request, RequestState, request::MovieOrSeason};
use tokio::sync::{Notify, mpsc::UnboundedReceiver};
use tracing::{error, warn};

use super::{Error, Requests, as_handed_off, load, load_one, lock};
use crate::{
    db::{BoxFuture, Database, detached},
    plugins::{Answer, ExtensionPoints},
};

/// Requests' way to the Fulfillers for Movies and for Shows, where they are enabled.
#[derive(Debug, Clone)]
pub(super) struct HandOff {
    db: Arc<Database>,
    plugins: ExtensionPoints,
    /// The ids of the Requests being handed to a Fulfiller now, each with whether another hand-off
    /// skipped it meanwhile.
    handing: Arc<Mutex<HashMap<i64, bool>>>,
    /// Wakes [`super::progress::Progress::follow`] when a Request becomes Processing.
    became_processing: Arc<Notify>,
}

impl HandOff {
    /// Hands Requests in `db` to the Fulfillers in `plugins`, notifying `became_processing` when one
    /// becomes Processing.
    pub(super) fn new(
        db: Arc<Database>,
        plugins: ExtensionPoints,
        became_processing: Arc<Notify>,
    ) -> Self {
        Self {
            db,
            plugins,
            handing: Arc::default(),
            became_processing,
        }
    }

    /// Claims the Request `id` for a hand-off, unless another hand-off has it, which then looks at
    /// it again once it is done.
    ///
    /// An approval and enabling the Fulfiller may hand the same Request off at once, which would
    /// hand it to the Fulfiller twice.
    pub(super) fn claim(&self, id: i64) -> Option<Claim> {
        Claim::take(&self.handing, id)
    }

    /// Hands the Approved `request`, which `claim` holds, to the Fulfiller for its kind, Movies or
    /// Shows, publishing that its state changed: to Processing once it takes the Request, or to
    /// Failed, with its reason, if it doesn't. Returns the Request as it is now. Without a
    /// Fulfiller for its kind the Request waits in Approved. Settles through `requests`' path.
    ///
    /// Runs in a task of its own: once the Fulfiller has taken the Request, its reference must be
    /// stored even if the caller stops waiting.
    // Boxed, since the Hand-off settles through the path, which hands off in turn.
    pub(super) fn request(
        &self,
        requests: &Requests,
        request: Request,
        mut claim: Claim,
    ) -> BoxFuture<'static, Result<Request, Error>> {
        let hand_off = self.clone();
        let requests = requests.clone();
        Box::pin(detached(async move {
            let mut request = request;
            loop {
                request = hand_off.claimed(&requests, request).await?;
                match claim.release() {
                    Some(kept) => claim = kept,
                    None => return Ok(request),
                }
            }
        }))
    }

    /// [`Self::request`] for `request`, which no other hand-off has, as it is in the database now:
    /// one that finished since `request` was read has moved it on.
    async fn claimed(&self, requests: &Requests, request: Request) -> Result<Request, Error> {
        let id = request.id;
        let mut conn = self.db.reader().acquire().await?;
        let current = match load_one(&mut conn, id).await? {
            Some(current) if current.state == RequestState::Approved => current,
            Some(moved_on) => return Ok(moved_on),
            // Cancelled since.
            None => return Ok(request),
        };
        let handed = as_handed_off(&mut conn, current.clone()).await?;
        drop(conn);
        let Some(fulfilled) = self.fulfill(&handed).await else {
            return Ok(current);
        };
        let took = fulfilled.result.is_ok();
        let asked = current.quality.as_option().cloned();
        let settled = requests.settle_hand_off(id, fulfilled, asked).await?;
        if took && settled.is_none() {
            warn!(
                request = id,
                "the Fulfiller took a Request that was changed meanwhile, and keeps it"
            );
        }
        let Some(settled) = settled else {
            return Ok(current);
        };
        if settled.state == RequestState::Processing {
            self.became_processing.notify_one();
        }
        Ok(settled)
    }

    /// Hands each Approved Request, oldest first, to its Fulfiller as [`Self::request`] does
    /// whenever a Plugin `came_up` names provides a Fulfiller, which keeps the promise
    /// [`Fulfiller`]'s docs make, settling through `requests`' path. A failure is only logged, and
    /// leaves the rest waiting for the next. Runs until `came_up` closes or it is dropped.
    pub(super) async fn as_fulfillers_come_up(
        &self,
        requests: &Requests,
        mut came_up: UnboundedReceiver<PluginId>,
    ) {
        while let Some(plugin) = came_up.recv().await {
            // One disabled since it came up provides nothing.
            let movies = self.plugins.providers::<dyn Fulfiller<Movies>>();
            let shows = self.plugins.providers::<dyn Fulfiller<Shows>>();
            if !movies.contains(&plugin) && !shows.contains(&plugin) {
                continue;
            }
            if let Err(err) = self.hand_off_waiting(requests).await {
                error!("handing the waiting Requests to their Fulfillers failed: {err}");
            }
        }
    }

    async fn hand_off_waiting(&self, requests: &Requests) -> Result<(), Error> {
        let mut conn = self.db.reader().acquire().await?;
        // ponytail: reads every Request to find the Approved ones; filter in SQL if there get many.
        let all = load(&mut conn, None, None).await?;
        drop(conn);
        let waiting = all
            .into_iter()
            .rev()
            .filter(|request| request.state == RequestState::Approved);
        // ponytail: one at a time, so a slow Fulfiller takes as long for each.
        for request in waiting {
            if let Some(claim) = self.claim(request.id) {
                self.request(requests, request, claim).await?;
            }
        }
        Ok(())
    }

    /// What the Fulfiller for `request`'s kind says to it, if one is enabled: what it reports once
    /// it takes it, or why it doesn't.
    async fn fulfill(&self, request: &Request) -> Option<Answer<Fulfilled>> {
        let fulfilled = match request.movie_or_season.as_ref()? {
            MovieOrSeason::Movie(_) => self.fulfill_as::<Movies>(request).await,
            MovieOrSeason::Season(_) => self.fulfill_as::<Shows>(request).await,
        }?;
        if let Err(failure) = &fulfilled.result {
            warn!(
                request = request.id,
                reason = failure.message,
                "the Fulfiller didn't take the Request"
            );
        }
        Some(fulfilled)
    }

    /// [`Self::fulfill`] by the Fulfiller of `Kind`.
    async fn fulfill_as<Kind: 'static>(&self, request: &Request) -> Option<Answer<Fulfilled>> {
        self.plugins
            .ask_first(|fulfiller: Arc<dyn Fulfiller<Kind>>| async move {
                fulfiller.fulfill(request).await
            })
            .await
    }

    /// Tells the Shows Fulfiller, if one is enabled, to stop following the Show of `season`, the
    /// Season Request of a Followed Show that ended. A failure is only logged: the Followed Show
    /// ended either way.
    pub(super) async fn unfollow(&self, season: &Request) {
        let unfollowed = self
            .plugins
            .ask_first(|fulfiller: Arc<dyn Fulfiller<Shows>>| async move {
                fulfiller.unfollow(season).await
            })
            .await;
        if let Some(Answer {
            result: Err(failure),
            ..
        }) = unfollowed
            && let Some(MovieOrSeason::Season(asked)) = &season.movie_or_season
        {
            warn!(
                show = asked.show_tmdb_id,
                reason = failure.message,
                "the Shows Fulfiller didn't stop following the Show"
            );
        }
    }
}

/// A hand-off's hold on one Request in [`HandOff::handing`], let go however the hand-off ends, a
/// panic included, so the Request can be handed off again.
pub(super) struct Claim {
    handing: Arc<Mutex<HashMap<i64, bool>>>,
    id: i64,
    /// Whether [`Self::release`] has let go already, so dropping it mustn't.
    released: bool,
}

impl Claim {
    /// Claims the Request `id` in `handing`, unless another hand-off has it, which then has to look
    /// again.
    fn take(handing: &Arc<Mutex<HashMap<i64, bool>>>, id: i64) -> Option<Self> {
        match lock(handing).entry(id) {
            Entry::Occupied(mut skipped) => {
                skipped.insert(true);
                None
            }
            Entry::Vacant(free) => {
                free.insert(false);
                Some(Self {
                    handing: Arc::clone(handing),
                    id,
                    released: false,
                })
            }
        }
    }

    /// Lets go of the Request, unless another hand-off skipped it since the claim was taken or last
    /// kept: that one may have come for a Fulfiller enabled after this one looked, so the claim is
    /// kept for one more look.
    fn release(mut self) -> Option<Self> {
        let mut handing = lock(&self.handing);
        if handing.insert(self.id, false) == Some(true) {
            drop(handing);
            return Some(self);
        }
        handing.remove(&self.id);
        drop(handing);
        self.released = true;
        None
    }
}

impl Drop for Claim {
    fn drop(&mut self) {
        if !self.released {
            lock(&self.handing).remove(&self.id);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::Claim;

    #[test]
    fn a_claim_another_hand_off_skipped_is_kept_for_one_more_look() {
        let handing = Arc::default();
        let claim = Claim::take(&handing, 1).unwrap();
        assert!(Claim::take(&handing, 1).is_none());

        let kept = claim.release().expect("the skipped Request was let go");

        assert!(kept.release().is_none());
        assert!(Claim::take(&handing, 1).is_some());
    }

    #[test]
    fn a_dropped_claim_lets_the_request_be_claimed_again() {
        let handing = Arc::default();
        let claim = Claim::take(&handing, 1).unwrap();

        drop(claim);

        assert!(Claim::take(&handing, 1).is_some());
    }
}
