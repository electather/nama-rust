//! Everything Requests does with a Fulfiller: the Hand-off of each Approved Request, asking how the
//! Processing ones are progressing, and telling the Shows Fulfiller to stop following a Show.
//!
//! It holds no SQL, since [`super`] alone owns its tables: it calls Requests' functions to read
//! Requests and to settle a Hand-off.

use std::{
    collections::{HashMap, hash_map::Entry},
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use nama_plugin_api::{
    PluginId,
    fulfiller::{Download, Fulfilled, Fulfiller, Movies, Shows},
};
use nama_proto::nama::v1::{Request, RequestProgress, RequestState};
use sqlx::{Sqlite, Transaction};
use tokio::{
    sync::{Notify, mpsc::UnboundedReceiver},
    time::error::Elapsed,
};
use tracing::{error, warn};

use super::{
    Applied, Error, HandedBack, Processing, as_handed_off, detached, load, load_one, processing,
    settle,
};
use crate::{
    db::{BoxFuture, Database},
    events::{EventBus, event},
    plugins::Registry,
};

/// How often the Fulfillers are asked how their Processing Requests are progressing, as Seerr asks
/// (ADR 0012).
pub(super) const PROGRESS_EVERY: Duration = Duration::from_secs(60);

/// Requests' way to the Fulfillers for Movies and for Shows, where they are enabled, which also
/// publishes the changes its writes make.
#[derive(Debug, Clone)]
pub(super) struct HandOff {
    db: Arc<Database>,
    events: EventBus,
    pub(super) plugins: Registry,
    /// The ids of the Requests being handed to a Fulfiller now, each with whether another hand-off
    /// skipped it meanwhile.
    handing: Arc<Mutex<HashMap<i64, bool>>>,
    /// What each Processing Request's Fulfiller said it is downloading when last asked, by Request
    /// id. Kept in memory only (ADR 0012).
    progress: Arc<Mutex<HashMap<i64, Vec<Download>>>>,
    /// Wakes [`HandOff::follow_progress`] when a Request becomes Processing.
    became_processing: Arc<Notify>,
}

impl HandOff {
    /// Hands Requests in `db` to the Fulfillers in `plugins`, publishing on `events`.
    pub(super) fn new(db: Arc<Database>, events: EventBus, plugins: Registry) -> Self {
        Self {
            db,
            events,
            plugins,
            handing: Arc::default(),
            progress: Arc::default(),
            became_processing: Arc::default(),
        }
    }

    /// The bus it publishes on, for a test to subscribe to.
    #[cfg(test)]
    pub(super) const fn events(&self) -> &EventBus {
        &self.events
    }

    /// The enabled Plugin providing the Fulfiller of `Kind`, if any.
    pub(super) fn fulfiller<Kind: 'static>(&self) -> Option<PluginId> {
        self.plugins
            .enabled()
            .into_iter()
            .find(|plugin| self.plugins.provides::<dyn Fulfiller<Kind>>(plugin))
    }

    /// The enabled Plugin providing the Fulfiller for `request`'s kind, Movies or Shows, if any.
    pub(super) fn fulfiller_of(&self, request: &Request) -> Option<PluginId> {
        if request.movie.is_unset() {
            self.fulfiller::<Shows>()
        } else {
            self.fulfiller::<Movies>()
        }
    }

    /// Hands the Approved `request` to the Fulfiller for its kind, Movies or Shows, publishing that
    /// its state changed: to Processing once it takes the Request, or to Failed, with its reason,
    /// if it doesn't. Returns the Request as it is now. Without a Fulfiller for its kind the
    /// Request waits in Approved, and one already being handed off is left to that hand-off.
    ///
    /// Runs in a task of its own: once the Fulfiller has taken the Request, its reference must be
    /// stored even if the caller stops waiting.
    // Boxed, since the Hand-off settles through `apply`, which hands off in turn.
    pub(super) fn request(&self, request: Request) -> BoxFuture<'static, Result<Request, Error>> {
        let hand_off = self.clone();
        Box::pin(detached(async move {
            // An approval and enabling the Fulfiller may hand the same Request off at once, which
            // would hand it to the Fulfiller twice.
            let Some(mut claim) = Claim::take(&hand_off.handing, request.id) else {
                return Ok(request);
            };
            let mut request = request;
            loop {
                request = hand_off.claimed(request).await?;
                match claim.release() {
                    Some(kept) => claim = kept,
                    None => return Ok(request),
                }
            }
        }))
    }

    /// [`Self::request`] for `request`, which no other hand-off has, as it is in the database now:
    /// one that finished since `request` was read has moved it on.
    async fn claimed(&self, request: Request) -> Result<Request, Error> {
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
        let took = fulfilled.is_ok();
        let asked = current.quality.as_option().cloned();
        let plugin = self.fulfiller_of(&current);
        // Not detached again: this Hand-off already is.
        let settled = self
            .apply(move |tx| Box::pin(settle(tx, id, fulfilled, asked, plugin)))
            .await?;
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

    /// [`super::Requests::apply`], in the task the caller runs it in.
    pub(super) async fn apply<T, W>(&self, write: W) -> Result<T, Error>
    where
        T: HandedBack + Send + 'static,
        W: for<'t> FnOnce(
                &'t mut Transaction<'_, Sqlite>,
            ) -> BoxFuture<'t, Result<Applied<T>, Error>>
            + Send
            + 'static,
    {
        let Applied {
            value,
            events,
            hand_off,
            unfollow,
        } = self.db.write(write).await?;
        self.events.publish(events);
        if let Some(season) = unfollow {
            self.unfollow(&season).await;
        }
        let mut handed = Vec::with_capacity(hand_off.len());
        // ponytail: one Hand-off at a time, so a slow Fulfiller takes as long for each Season.
        for request in hand_off {
            handed.push(self.request(request).await?);
        }
        Ok(value.handed_back(&handed))
    }

    /// Hands each Approved Request, oldest first, to its Fulfiller as [`Self::request`] does
    /// whenever a Plugin `came_up` names provides a Fulfiller, which keeps the promise
    /// [`Fulfiller`]'s docs make. A failure is only logged, and leaves the rest waiting for the
    /// next. Runs until `came_up` closes or it is dropped.
    pub(super) async fn as_fulfillers_come_up(&self, mut came_up: UnboundedReceiver<PluginId>) {
        while let Some(plugin) = came_up.recv().await {
            // One disabled since it came up provides nothing.
            if !self.plugins.provides::<dyn Fulfiller<Movies>>(&plugin)
                && !self.plugins.provides::<dyn Fulfiller<Shows>>(&plugin)
            {
                continue;
            }
            if let Err(err) = self.hand_off_waiting().await {
                error!("handing the waiting Requests to their Fulfillers failed: {err}");
            }
        }
    }

    async fn hand_off_waiting(&self) -> Result<(), Error> {
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
            self.request(request).await?;
        }
        Ok(())
    }

    /// What the Fulfiller for `request`'s kind says to it, if one is enabled: what it reports once
    /// it takes it, or why it doesn't, with no secret setting's value in it.
    async fn fulfill(&self, request: &Request) -> Option<Result<Fulfilled, String>> {
        let fulfilled = if request.movie.is_unset() {
            self.fulfill_as::<Shows>(request).await
        } else {
            self.fulfill_as::<Movies>(request).await
        }?;
        Some(fulfilled.inspect_err(|reason| {
            warn!(
                request = request.id,
                reason, "the Fulfiller didn't take the Request"
            );
        }))
    }

    /// [`Self::fulfill`] by the Fulfiller of `Kind`.
    async fn fulfill_as<Kind: 'static>(
        &self,
        request: &Request,
    ) -> Option<Result<Fulfilled, String>> {
        self.plugins
            .call_provider_blanking_secrets::<dyn Fulfiller<Kind>, _, _>(|fulfiller| async move {
                // Inside the call, so formatting and dropping the Plugin's error are contained.
                fulfiller
                    .fulfill(request)
                    .await
                    .map_err(|error| error.to_string())
            })
            .await
    }

    /// Tells the Shows Fulfiller, if one is enabled, to stop following the Show of `season`, the
    /// Season Request of a Followed Show that ended. A failure is only logged: the Followed Show
    /// ended either way.
    async fn unfollow(&self, season: &Request) {
        let unfollowed = self
            .plugins
            .call_provider_blanking_secrets::<dyn Fulfiller<Shows>, _, _>(|fulfiller| async move {
                fulfiller
                    .unfollow(season)
                    .await
                    .map_err(|error| error.to_string())
            })
            .await;
        if let Some(Err(reason)) = unfollowed {
            warn!(
                show = season.season.show_tmdb_id,
                reason, "the Shows Fulfiller didn't stop following the Show"
            );
        }
    }

    /// Asks the Fulfillers how their Processing Requests are progressing, every `interval` while any
    /// Request is Processing, and at once when one becomes Processing. Runs until dropped.
    #[expect(
        clippy::infinite_loop,
        reason = "it follows progress for as long as the Server serves, which aborts it"
    )]
    pub(super) async fn follow_progress(&self, interval: Duration) {
        loop {
            let processing = self.poll_progress().await.unwrap_or_else(|err| {
                error!("reading the Processing Requests failed: {err}");
                true
            });
            // A Request that became Processing while the Fulfillers were asked left a permit, so
            // this wakes at once for it.
            let woken = self.became_processing.notified();
            if processing {
                let _: Result<(), Elapsed> = tokio::time::timeout(interval, woken).await;
            } else {
                woken.await;
            }
        }
    }

    /// Asks each enabled Fulfiller once what it is downloading for the Processing Requests it took,
    /// keeps the answers in place of the last ones, and publishes each Request whose downloads
    /// changed. Returns whether any Request is Processing.
    pub(super) async fn poll_progress(&self) -> Result<bool, sqlx::Error> {
        let processing = processing(&mut *self.db.reader().acquire().await?).await?;
        let (movies, seasons): (Vec<_>, Vec<_>) =
            processing.into_iter().partition(|request| request.is_movie);
        let any = !(movies.is_empty() && seasons.is_empty());
        let mut polled = self.progress_of::<Movies>(movies).await;
        polled.extend(self.progress_of::<Shows>(seasons).await);
        let mut changed = self.changed_progress(&polled);
        if changed.is_empty() {
            *lock(&self.progress) = polled;
            return Ok(any);
        }
        // Read before keeping `polled`, so if this fails the next poll sees the change again and
        // publishes it then. ponytail: reads every Request to publish the changed ones; load them
        // by id if there get many.
        let all = load(&mut *self.db.reader().acquire().await?, None, None).await?;
        *lock(&self.progress) = polled;
        let events = all
            .into_iter()
            // One that left Processing since it was asked about has no progress any more.
            .filter(|request| request.state == RequestState::Processing)
            .filter_map(|mut request| {
                request.downloads = changed.remove(&request.id)?;
                Some(event(RequestProgress {
                    request: request.into(),
                    ..Default::default()
                }))
            })
            .collect();
        self.events.publish(events);
        Ok(any)
    }

    /// What `Kind`'s Fulfiller, if one is enabled, is downloading for each of `processing`, by
    /// Request id, asked in one batch. Each has nothing downloading if it couldn't say.
    async fn progress_of<Kind: 'static>(
        &self,
        processing: Vec<Processing>,
    ) -> HashMap<i64, Vec<Download>> {
        if processing.is_empty() {
            return HashMap::new();
        }
        let references: Vec<String> = processing
            .iter()
            .map(|request| request.reference.clone())
            .collect();
        let references = references.as_slice();
        let answered = self
            .plugins
            .call_provider_blanking_secrets::<dyn Fulfiller<Kind>, _, _>(|fulfiller| async move {
                // Bounded, so a Fulfiller that never answers can't stop every later poll. Inside
                // the call, so formatting and dropping the Plugin's error are contained.
                let asked = tokio::time::timeout(PROGRESS_EVERY, fulfiller.progress(references));
                match asked.await {
                    Ok(answered) => answered.map_err(|error| error.to_string()),
                    Err(_) => Err("it didn't answer in time".to_owned()),
                }
            })
            .await;
        let by_reference = match answered {
            Some(Ok(by_reference)) => by_reference,
            Some(Err(reason)) => {
                warn!(
                    reason,
                    "a Fulfiller couldn't say how its Requests are progressing"
                );
                HashMap::new()
            }
            None => HashMap::new(),
        };
        processing
            .into_iter()
            .map(|request| {
                let downloads = by_reference.get(&request.reference).cloned();
                (request.id, downloads.unwrap_or_default())
            })
            .collect()
    }

    /// Those of `polled`, what each Processing Request is downloading now, that differ from what
    /// was kept.
    fn changed_progress(
        &self,
        polled: &HashMap<i64, Vec<Download>>,
    ) -> HashMap<i64, Vec<Download>> {
        let kept = lock(&self.progress);
        polled
            .iter()
            .filter(|&(id, now)| kept.get(id).map_or(&[][..], Vec::as_slice) != now.as_slice())
            .map(|(&id, now)| (id, now.clone()))
            .collect()
    }

    /// `request` with what its Fulfiller said it is downloading when last asked, if it is
    /// Processing.
    pub(super) fn with_progress(&self, mut request: Request) -> Request {
        if request.state == RequestState::Processing {
            let kept = lock(&self.progress).get(&request.id).cloned();
            request.downloads = kept.unwrap_or_default();
        }
        request
    }
}

/// A hand-off's hold on one Request in [`HandOff::handing`], let go however the hand-off ends, a
/// panic included, so the Request can be handed off again.
struct Claim {
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

// Nothing panics while one is held, so a poisoned lock's value is still whole.
pub(super) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
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
}
