//! What Requests does with a Fulfiller: the Hand-off of each Approved Request, and telling the
//! Shows Fulfiller to stop following a Show. Asking how the Requests it took are progressing is
//! [`super::progress`]'s.
//!
//! It holds no SQL: it calls Requests' functions to read Requests, and settles a Hand-off through
//! [`Requests::apply`]'s path, which calls it in turn.

use std::{
    collections::{HashMap, hash_map::Entry},
    sync::{Arc, Mutex},
    time::Duration,
};

use nama_plugin_api::{
    PluginId,
    fulfiller::{Fulfilled, Fulfiller, Movies, Shows},
};
use nama_proto::nama::v1::{ExternalId, Request, RequestState, request::MovieOrSeason};
use tokio::sync::{Notify, mpsc::UnboundedReceiver};
use tracing::{error, warn};

use super::followed::as_handed_off;
use super::listing::{load, load_one};
use super::{Error, Requests, lock};
use crate::{
    db::{BoxFuture, Database, detached},
    plugins::{self, Answer, ExtensionPoints, Failure, RETRIES, Retryable, ask_again},
};

/// What a Fulfiller answered to a hand-off, and how long it said to wait if it said to try again,
/// when the answer is the failure the Request settles with once the Kernel gives up.
type Fulfilling = (Answer<Fulfilled, Vec<String>>, Option<Duration>);

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
    /// How many times in a row each Request waiting to be handed off again was, after its
    /// Fulfiller said to try again.
    retries: Arc<Mutex<HashMap<i64, u32>>>,
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
            retries: Arc::default(),
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
    /// Failed, with its reason, if it doesn't or if the title lacks every id it needs, when it
    /// isn't asked. Returns the Request as it is now. Without a
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
        let retried = lock(&self.retries).remove(&id).unwrap_or(0);
        let mut conn = self.db.reader().acquire().await?;
        let current = match load_one(&mut conn, id).await? {
            Some(current) if current.state == RequestState::Approved => current,
            Some(moved_on) => return Ok(moved_on),
            // Cancelled since.
            None => return Ok(request),
        };
        let handed = as_handed_off(&mut conn, current.clone()).await?;
        drop(conn);
        let Some((fulfilled, try_again)) = self.fulfill(&handed).await else {
            return Ok(current);
        };
        if let Some(wait) = try_again {
            if retried < RETRIES {
                lock(&self.retries).insert(id, retried + 1);
                self.hand_off_later(requests, &fulfilled.plugin, current.clone(), wait);
                return Ok(current);
            }
            warn!(
                request = id,
                retries = RETRIES,
                "the Fulfiller kept saying to try again, so the Request fails"
            );
        }
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

    /// Hands `request` off again after `wait`, as the Plugin `plugin`'s Fulfiller said to, settling
    /// through `requests`' path. The wait runs in the Plugin's scope, so disabling it ends the
    /// wait at once (ADR 0047). The hand-off itself is detached like any other, since once the
    /// Fulfiller takes the Request its reference must be stored.
    fn hand_off_later(
        &self,
        requests: &Requests,
        plugin: &PluginId,
        request: Request,
        wait: Duration,
    ) {
        let hand_off = self.clone();
        let requests = requests.clone();
        // Not spawned if the Plugin was disabled meanwhile: enabling one hands off what waits.
        self.plugins.spawn_in(plugin, async move {
            tokio::time::sleep(wait).await;
            let id = request.id;
            // Another hand-off has it, and looks at it again once done.
            let Some(claim) = hand_off.claim(id) else {
                return;
            };
            if let Err(err) = hand_off.request(&requests, request, claim).await {
                error!(request = id, "handing a Request off again failed: {err}");
            }
        });
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
    /// it takes it, or why it doesn't. A failure's `kind` names the id namespaces the Fulfiller
    /// needs when the Kernel didn't hand it `request` for lack of an id in any of them, and is
    /// empty when the Fulfiller itself refused. Also how long it said to wait, if it said to try
    /// again.
    async fn fulfill(&self, request: &Request) -> Option<Fulfilling> {
        let (fulfilled, try_again) = match request.movie_or_season.as_ref()? {
            MovieOrSeason::Movie(movie) => {
                self.fulfill_as::<Movies>(request, "Movie", &movie.ids)
                    .await
            }
            MovieOrSeason::Season(season) => {
                self.fulfill_as::<Shows>(request, "Show", &season.show_ids)
                    .await
            }
        }?;
        if let (Err(failure), None) = (&fulfilled.result, try_again) {
            warn!(
                request = request.id,
                reason = failure.message,
                "the Fulfiller didn't take the Request"
            );
        }
        Some((fulfilled, try_again))
    }

    /// [`Self::fulfill`] by the Fulfiller of `Kind`, for a `title`, `Movie` or `Show`, with the ids
    /// `ids`.
    async fn fulfill_as<Kind: 'static>(
        &self,
        request: &Request,
        title: &str,
        ids: &[ExternalId],
    ) -> Option<Fulfilling> {
        let told = &plugins::request(request);
        // Asked inside the Plugin's containment, so the failure names the Plugin that would have
        // been handed the Request.
        let Answer { plugin, result } = self
            .plugins
            .ask_first(|fulfiller: Arc<dyn Fulfiller<Kind>>| async move {
                if let Some(needs) = unmet(&*fulfiller, namespaces(ids)) {
                    return Ok(Err(needs));
                }
                fulfiller.fulfill(told).await.map(Ok).map_err(Retryable)
            })
            .await?;
        let mut try_again = None;
        let result = match result {
            Ok(Ok(fulfilled)) => Ok(fulfilled),
            Ok(Err(needs)) => Err(Failure {
                message: missing(&plugin, title, &needs),
                kind: needs,
            }),
            Err(Failure { kind, message }) => {
                try_again = kind;
                Err(Failure {
                    kind: Vec::new(),
                    message,
                })
            }
        };
        Some((Answer { plugin, result }, try_again))
    }

    /// Tells the Shows Fulfiller, if one is enabled, to stop following the Show of `season`, the
    /// Season Request of a Followed Show that ended. A failure, or a Show lacking every id it
    /// needs, which skips it, is only logged: the Followed Show ended either way. One that says to
    /// try again is asked again later, in its scope (ADR 0047).
    pub(super) async fn unfollow(&self, season: &Request) {
        let Some(MovieOrSeason::Season(asked)) = &season.movie_or_season else {
            return;
        };
        let told = &plugins::request(season);
        let unfollowed = self
            .plugins
            .ask_first(|fulfiller: Arc<dyn Fulfiller<Shows>>| async move {
                if let Some(needs) = unmet(&*fulfiller, namespaces(&asked.show_ids)) {
                    return Ok(Err(needs));
                }
                fulfiller.unfollow(told).await.map(Ok).map_err(Retryable)
            })
            .await;
        let show = asked
            .show_id
            .as_option()
            .map(|id| format!("{}-{}", id.namespace, id.value));
        let reason = match unfollowed {
            Some(Answer {
                plugin,
                result: Ok(Err(needs)),
            }) => missing(&plugin, "Show", &needs),
            Some(Answer {
                plugin,
                result: Err(Failure {
                    kind: Some(wait), ..
                }),
            }) => {
                let unfollow = <dyn Fulfiller<Shows>>::unfollow;
                let gave_up = move |reason: String| not_unfollowed(show.as_deref(), &reason);
                ask_again(&self.plugins, plugin, wait, told.clone(), unfollow, gave_up);
                return;
            }
            Some(Answer {
                result: Err(failure),
                ..
            }) => failure.message,
            _ => return,
        };
        not_unfollowed(show.as_deref(), &reason);
    }
}

/// Logs that the Shows Fulfiller didn't stop following the Show known as `show`, and why.
fn not_unfollowed(show: Option<&str>, reason: &str) {
    warn!(
        show,
        reason, "the Shows Fulfiller didn't stop following the Show"
    );
}

/// The namespaces of `ids`.
fn namespaces(ids: &[ExternalId]) -> impl Iterator<Item = &str> {
    ids.iter().map(|id| id.namespace.as_str())
}

/// The namespaces `fulfiller` needs when a title, known by ids in `namespaces`, has an id in none
/// of them, so the Kernel doesn't ask it about the title.
///
/// ponytail: each caller reads this inside its `ask`, not with `read_provider`, so an unmet need
/// clears the Fulfiller's recorded failure though its service wasn't called (`plugins/AGENTS.md`).
pub(crate) fn unmet<'a, Kind: 'static>(
    fulfiller: &dyn Fulfiller<Kind>,
    mut namespaces: impl Iterator<Item = &'a str>,
) -> Option<Vec<String>> {
    let needs = fulfiller.needs();
    let met = needs.is_empty() || namespaces.any(|namespace| needs.iter().any(|n| n == namespace));
    (!met).then_some(needs)
}

/// Why the Plugin `plugin`'s Fulfiller wasn't handed a Request for a `title`, `Movie` or `Show`,
/// which has no id in any of the namespaces it `needs`.
pub(crate) fn missing(plugin: &PluginId, title: &str, needs: &[String]) -> String {
    let ids = match needs {
        [one] => format!("the {title}'s {one} id"),
        [first @ .., last] => format!("one of the {title}'s {} or {last} ids", first.join(", ")),
        [] => format!("the {title}'s id"),
    };
    format!("{plugin} needs {ids}, and none is known for it")
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
#[path = "hand_off_tests.rs"]
mod tests;
