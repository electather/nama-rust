//! Requests: Users asking for a Movie, or a Season of a Show, that isn't on the Server to be added
//! to it (ADR 0012).
//!
//! Followed Shows sit beside them: Users asking for a whole Show, whose later Seasons get Season
//! Requests as a Library scan Matches their Episodes, never by asking a metadata source.
//!
//! This module is the only code that reads or writes `requests`, `requesters`, `followed_shows`
//! and `followed_show_requesters`. It hands Requests out as their proto messages, since those are
//! also what its events carry.

use std::{
    collections::{BTreeSet, HashMap, HashSet, hash_map::Entry},
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use nama_plugin_api::{
    PluginId,
    fulfiller::{Download, Fulfiller, Movies, Shows},
    metadata::{Candidate, Episode, ExternalId, ExternalIds, SearchQuery, Season, Show, TitleKind},
};
use nama_proto::nama::v1::{
    Event, FollowedShow, FoundSeason, Permission, Request, RequestCancelled, RequestCreated,
    RequestProgress, RequestState, RequestStateChanged, RequestWithdrawn, RequestedMovie,
    RequestedSeason, Requester, RequesterAdded, RequesterRemoved, event::Event as Happened,
};
use sqlx::{Sqlite, SqliteConnection, Transaction};
use tokio::{sync::Notify, time::error::Elapsed};
use tracing::{error, warn};

use crate::{
    contents::{self, Contents, Metadata},
    db::{BoxFuture, Database, now},
    events::{EventBus, event},
    matching::{ManualError, Matcher},
    plugins::Registry,
    users::User,
};

/// Each Request state as `requests.state` stores it.
const STATES: [(RequestState, &str); 7] = [
    (RequestState::Pending, "pending"),
    (RequestState::Approved, "approved"),
    (RequestState::Declined, "declined"),
    (RequestState::Processing, "processing"),
    (RequestState::PartiallyAvailable, "partially_available"),
    (RequestState::Available, "available"),
    (RequestState::Failed, "failed"),
];

/// How often the Fulfillers are asked how their Processing Requests are progressing, as Seerr asks
/// (ADR 0012).
pub(crate) const PROGRESS_EVERY: Duration = Duration::from_secs(60);

/// Why a Request couldn't be made, found or changed.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    /// There is no such Request, or none the caller may see.
    #[error("there is no Request {0}")]
    NoSuchRequest(i64),
    #[error("the Request {0} isn't Pending")]
    NotPending(i64),
    #[error("the Request {0} isn't Failed")]
    NotFailed(i64),
    #[error("you aren't a Requester of the Request {0}")]
    NotARequester(i64),
    #[error("a Library already has the Movie tmdb-{0}")]
    InLibrary(i64),
    #[error("every aired Episode of Season {season} of the Show tmdb-{show} is in a Library")]
    SeasonInLibrary { show: i64, season: u32 },
    #[error("the metadata source has no Movie tmdb-{0}")]
    NoSuchMovie(i64),
    #[error("the metadata source has no Show tmdb-{0}")]
    NoSuchShow(i64),
    #[error("the metadata source has no Season {season} of the Show tmdb-{show}")]
    NoSuchSeason { show: i64, season: u32 },
    /// No metadata Provider is enabled, or it failed.
    #[error("no metadata Provider could answer")]
    Unavailable,
    /// There is no such Followed Show, or none the caller may see.
    #[error("there is no Followed Show {0}")]
    NoSuchFollowedShow(i64),
    #[error("the Followed Show {0} isn't Pending")]
    FollowedShowNotPending(i64),
    #[error("you aren't a Requester of the Followed Show {0}")]
    NotAFollowedShowRequester(i64),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// The Server's Requests, which publishes what changes about them.
#[derive(Debug, Clone)]
pub(crate) struct Requests {
    db: Arc<Database>,
    contents: Contents,
    /// Looks up what a new Request asks for: a Movie, or a Show's Seasons.
    matcher: Matcher,
    events: EventBus,
    /// Holds the Fulfillers for Movies and for Shows, where they are enabled.
    plugins: Registry,
    /// The ids of the Requests being handed to a Fulfiller now, each with whether another hand-off
    /// skipped it meanwhile.
    handing: Arc<Mutex<HashMap<i64, bool>>>,
    /// What each Processing Request's Fulfiller said it is downloading when last asked, by Request
    /// id. Kept in memory only (ADR 0012).
    progress: Arc<Mutex<HashMap<i64, Vec<Download>>>>,
    /// Wakes [`Requests::follow_progress`] when a Request becomes Processing.
    became_processing: Arc<Notify>,
}

/// A hand-off's hold on one Request in [`Requests::handing`], let go however the hand-off ends, a
/// panic included, so the Request can be handed off again.
struct Claim {
    handing: Arc<Mutex<HashMap<i64, bool>>>,
    id: i64,
    /// Whether [`Self::release`] has let go already, so dropping it mustn't.
    released: bool,
}

impl Claim {
    /// Claims the Request `id`, unless another hand-off has it, which then has to look again.
    fn take(requests: &Requests, id: i64) -> Option<Self> {
        match lock(&requests.handing).entry(id) {
            Entry::Occupied(mut skipped) => {
                skipped.insert(true);
                None
            }
            Entry::Vacant(free) => {
                free.insert(false);
                Some(Self {
                    handing: Arc::clone(&requests.handing),
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

// Nothing panics while one is held, so a poisoned map is still whole.
fn lock<T>(map: &Mutex<T>) -> MutexGuard<'_, T> {
    map.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A Movie the metadata source has, and how close it is to being on the Server.
#[derive(Debug)]
pub(crate) struct Found<M> {
    pub(crate) tmdb_id: i64,
    /// The Plugin whose metadata Provider gave the Movie, which its image paths are for.
    pub(crate) plugin: PluginId,
    pub(crate) movie: M,
    /// The oldest Library with the Movie Matched, if any.
    pub(crate) library: Option<i64>,
    /// The Movie's open Request, if it has one the viewer may see.
    pub(crate) request: Option<Request>,
}

/// A Show the metadata source has, as `S` holds it.
#[derive(Debug)]
pub(crate) struct FoundShow<S> {
    pub(crate) tmdb_id: i64,
    /// The Plugin whose metadata Provider gave the Show, which its image paths are for.
    pub(crate) plugin: PluginId,
    pub(crate) show: S,
    /// Its Seasons, in order, as the viewer sees them. Only a lookup fills them.
    pub(crate) seasons: Vec<FoundSeason>,
    /// Its open Followed Show, if it has one the viewer may see. Only a lookup fills it.
    pub(crate) followed_show: Option<FollowedShow>,
}

/// What a new Request asks for, as the metadata source gave it.
#[derive(Debug)]
struct Wanted {
    /// The Movie's TMDB id, or for a Season its Show's.
    tmdb_id: i64,
    /// The Season's number, or `None` for a Movie.
    season: Option<u32>,
    /// The Movie's title, or for a Season its Show's.
    title: String,
    /// The Movie's `YYYY-MM-DD` release date, or the Season's first air date.
    release_date: Option<String>,
    /// For a Season, its Show's ids; none for a Movie.
    show_ids: ExternalIds,
    /// The Followed Show a new Request for it is opened for, if any.
    followed_show: Option<i64>,
}

/// Seasons of a Show to ask for, as the metadata source gave them.
#[derive(Debug)]
struct WantedSeasons {
    show_tmdb_id: i64,
    show_ids: ExternalIds,
    show_title: String,
    seasons: Vec<Season>,
}

/// Whose Requests `viewer` may see: everyone's (`None`), or only those they are a Requester of.
pub(crate) fn visible_to(viewer: &User) -> Option<i64> {
    (!viewer.permissions.allow(Permission::ViewRequests)).then_some(viewer.id)
}

impl Requests {
    /// The Requests in `db`, looking Movies up through `matcher`, publishing on `events` and
    /// handing Approved ones to the Fulfillers in `plugins`.
    pub(crate) fn new(
        db: Arc<Database>,
        matcher: Matcher,
        events: EventBus,
        plugins: Registry,
    ) -> Self {
        Self {
            contents: Contents::new(Arc::clone(&db)),
            db,
            matcher,
            events,
            plugins,
            handing: Arc::default(),
            progress: Arc::default(),
            became_processing: Arc::default(),
        }
    }

    /// Asks for the Movie with the TMDB id `tmdb_id` for `asker`, who joins its open Request, or
    /// opens one: Approved, and handed to the Fulfiller for Movies, if `asker` may have Movies
    /// approved as they ask, else Pending. Publishes what changed, if anything did.
    pub(crate) async fn create(&self, asker: &User, tmdb_id: i64) -> Result<Request, Error> {
        // Refuses before asking TMDB, which may be down or slow; `ask` checks again for a scan
        // that Matched the Movie meanwhile.
        if self.contents.library_with_movie(tmdb_id).await?.is_some() {
            return Err(Error::InLibrary(tmdb_id));
        }
        // ponytail: looked up even to join an open Request, so the write has it if that Request
        // closes meanwhile. Look up only when opening one if TMDB's rate limit starts to bite.
        let movie = self.fetch(tmdb_id).await?;
        let wanted = Wanted {
            tmdb_id,
            season: None,
            title: movie.title,
            release_date: movie.release_date,
            show_ids: ExternalIds::default(),
            followed_show: None,
        };
        let state = opening_state(asker, Permission::AutoApproveMovie);
        let user = asker.id;
        let (request, happened) = self
            .db
            .write(move |tx| Box::pin(ask_movie(tx, user, wanted, state)))
            .await?;
        // Only the asker who opens the Request hands it off, so joining it can't hand it twice.
        let opened = happened.as_ref().and_then(opened_request_id).is_some();
        self.events.publish(happened.into_iter().collect());
        if opened && request.state == RequestState::Approved {
            return self.hand_off(request).await;
        }
        Ok(request)
    }

    /// Asks for the Seasons `numbers` of the Show with the TMDB id `tmdb_id` for `asker`, one
    /// Request per Season in Season order, each as [`Self::create`] asks for a Movie but with
    /// `AUTO_APPROVE_TV` and the Fulfiller for Shows. Asks for none if a Library has any of them:
    /// every aired Episode Matched.
    pub(crate) async fn create_seasons(
        &self,
        asker: &User,
        tmdb_id: i64,
        numbers: &BTreeSet<u32>,
    ) -> Result<Vec<Request>, Error> {
        let id = ExternalId::Tmdb(tmdb_id);
        let (_, show) = self
            .matcher
            .show(&id)
            .await
            .map_err(|err| looking_up(err, Error::NoSuchShow(tmdb_id)))?;
        let mut seasons = Vec::with_capacity(numbers.len());
        // ponytail: one metadata lookup per Season, one after another.
        for &number in numbers {
            let no_such_season = || Error::NoSuchSeason {
                show: tmdb_id,
                season: number,
            };
            if !show.seasons.contains(&number) {
                return Err(no_such_season());
            }
            let season = self
                .matcher
                .show_season(&id, number)
                .await
                .map_err(|err| looking_up(err, no_such_season()))?;
            seasons.push(season);
        }
        let wanted = WantedSeasons {
            show_tmdb_id: tmdb_id,
            show_ids: show.ids,
            show_title: show.title,
            seasons,
        };
        let state = opening_state(asker, Permission::AutoApproveTv);
        let user = asker.id;
        let this = self.clone();
        // The write too, so a caller that stops waiting leaves no Season Approved behind.
        detached(async move {
            let (requests, happened) = this
                .db
                .write(move |tx| Box::pin(ask_seasons(tx, user, wanted, state)))
                .await?;
            let opened: HashSet<i64> = happened.iter().filter_map(opened_request_id).collect();
            this.events.publish(happened);
            this.hand_off_opened(requests, &opened).await
        })
        .await
    }

    /// Each of `requests`, handed off first if it is Approved and its id is in `opened`, as it is
    /// after that.
    async fn hand_off_opened(
        &self,
        requests: Vec<Request>,
        opened: &HashSet<i64>,
    ) -> Result<Vec<Request>, Error> {
        let mut after_hand_off = Vec::with_capacity(requests.len());
        // ponytail: one hand-off at a time, so a slow Fulfiller takes as long for each Season.
        for request in requests {
            let request = if opened.contains(&request.id) && request.state == RequestState::Approved
            {
                self.hand_off(request).await?
            } else {
                request
            };
            after_hand_off.push(request);
        }
        Ok(after_hand_off)
    }

    /// Asks for the whole Show with the TMDB id `tmdb_id` for `asker`: each current Season not in
    /// a Library as [`Self::create_seasons`] asks for it, and later ones through the Show's open
    /// Followed Show, which `asker` joins or opens, in the state their Season Requests open in.
    /// Returns the Followed Show and the Season Requests in Season order.
    pub(crate) async fn follow(
        &self,
        asker: &User,
        tmdb_id: i64,
    ) -> Result<(FollowedShow, Vec<Request>), Error> {
        let id = ExternalId::Tmdb(tmdb_id);
        let (_, show) = self
            .matcher
            .show(&id)
            .await
            .map_err(|err| looking_up(err, Error::NoSuchShow(tmdb_id)))?;
        let wanted = WantedSeasons {
            show_tmdb_id: tmdb_id,
            seasons: self.seasons(tmdb_id, &show).await?,
            show_ids: show.ids,
            show_title: show.title,
        };
        let state = opening_state(asker, Permission::AutoApproveTv);
        let user = asker.id;
        let this = self.clone();
        // The write too, as in `create_seasons`.
        detached(async move {
            let (followed, requests, happened) = this
                .db
                .write(move |tx| Box::pin(follow(tx, user, wanted, state)))
                .await?;
            let opened: HashSet<i64> = happened.iter().filter_map(opened_request_id).collect();
            this.events.publish(happened);
            let requests = this.hand_off_opened(requests, &opened).await?;
            Ok((followed, requests))
        })
        .await
    }

    /// Approves the Pending Followed Show `id`, and each Pending Season Request it opened, handing
    /// those to the Shows Fulfiller, publishing each Request's change of state.
    pub(crate) async fn approve_followed(&self, id: i64) -> Result<FollowedShow, Error> {
        self.decide_followed(id, RequestState::Approved, None).await
    }

    /// Declines the Pending Followed Show `id`, and each Pending Season Request it opened, as
    /// [`Self::decline`] declines a Request.
    pub(crate) async fn decline_followed(
        &self,
        id: i64,
        reason: &str,
    ) -> Result<FollowedShow, Error> {
        self.decide_followed(id, RequestState::Declined, given_reason(reason))
            .await
    }

    async fn decide_followed(
        &self,
        id: i64,
        state: RequestState,
        reason: Option<String>,
    ) -> Result<FollowedShow, Error> {
        let this = self.clone();
        // The write too, as in `approve`.
        detached(async move {
            let (followed, decided) = this
                .db
                .write(move |tx| Box::pin(decide_followed(tx, id, state, reason)))
                .await?;
            this.events
                .publish(decided.iter().cloned().map(state_changed).collect());
            let ids: HashSet<i64> = decided.iter().map(|request| request.id).collect();
            this.hand_off_opened(decided, &ids).await?;
            Ok(followed)
        })
        .await
    }

    /// Removes the Followed Show `id`, whatever its state, leaving the Season Requests it opened,
    /// and has the Shows Fulfiller stop following the Show.
    pub(crate) async fn cancel_followed(&self, id: i64) -> Result<(), Error> {
        self.end_followed(move |tx| Box::pin(cancel_followed(tx, id)))
            .await
    }

    /// Takes `requester` off the Followed Show `id`, removing it once nobody is left on it, as
    /// [`Self::cancel_followed`] does.
    pub(crate) async fn withdraw_followed(&self, requester: &User, id: i64) -> Result<(), Error> {
        let user = requester.id;
        let visible = visible_to(requester);
        self.end_followed(move |tx| Box::pin(withdraw_followed(tx, user, visible, id)))
            .await
    }

    /// Writes `end`, then, if it returns the Season Request of a Followed Show it ended, tells the
    /// Shows Fulfiller, if one is enabled, to stop following that Show. A failure to tell it is
    /// only logged: the Followed Show ended either way.
    ///
    /// Runs in a task of its own, so a caller that stops waiting leaves the Fulfiller following no
    /// ended Followed Show.
    async fn end_followed<F>(&self, end: F) -> Result<(), Error>
    where
        F: for<'t> FnOnce(
                &'t mut Transaction<'_, Sqlite>,
            ) -> BoxFuture<'t, Result<Option<Request>, Error>>
            + Send
            + 'static,
    {
        let this = self.clone();
        detached(async move {
            if let Some(followed_season) = this.db.write(end).await? {
                this.unfollow(&followed_season).await;
            }
            Ok(())
        })
        .await
    }

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

    /// Approves the Pending Request `id` and hands it to its Fulfiller, publishing each change of
    /// its state.
    pub(crate) async fn approve(&self, id: i64) -> Result<Request, Error> {
        let requests = self.clone();
        detached(async move {
            let approved = requests.decide(id, RequestState::Approved, None).await?;
            requests.hand_off(approved).await
        })
        .await
    }

    /// Moves the Failed Request `id` back to Approved and hands it to its Fulfiller again,
    /// publishing each change of its state.
    pub(crate) async fn retry(&self, id: i64) -> Result<Request, Error> {
        let requests = self.clone();
        detached(async move {
            let approved = requests.db.write(move |tx| Box::pin(retry(tx, id))).await?;
            requests
                .events
                .publish(vec![state_changed(approved.clone())]);
            requests.hand_off(approved).await
        })
        .await
    }

    /// Hands the Approved `request` to the Fulfiller for its kind, Movies or Shows, publishing that
    /// its state changed: to Processing once it takes the Request, or to Failed, with its reason,
    /// if it doesn't. Returns the Request as it is now. Without a Fulfiller for its kind the
    /// Request waits in Approved, and one already being handed off is left to that hand-off.
    ///
    /// Runs in a task of its own: once the Fulfiller has taken the Request, its reference must be
    /// stored even if the caller stops waiting.
    async fn hand_off(&self, request: Request) -> Result<Request, Error> {
        let requests = self.clone();
        detached(async move {
            // An approval and enabling the Fulfiller may hand the same Request off at once, which
            // would hand it to the Fulfiller twice.
            let Some(mut claim) = Claim::take(&requests, request.id) else {
                return Ok(request);
            };
            let mut request = request;
            loop {
                request = requests.hand_off_claimed(request).await?;
                match claim.release() {
                    Some(kept) => claim = kept,
                    None => return Ok(request),
                }
            }
        })
        .await
    }

    /// [`Self::hand_off`] for `request`, which no other hand-off has, as it is in the database now:
    /// one that finished since `request` was read has moved it on.
    async fn hand_off_claimed(&self, request: Request) -> Result<Request, Error> {
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
        let settled = self
            .db
            .write(move |tx| Box::pin(settle(tx, id, fulfilled)))
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
        self.events.publish(vec![state_changed(settled.clone())]);
        if settled.state == RequestState::Processing {
            self.became_processing.notify_one();
        }
        Ok(settled)
    }

    /// Hands each Approved Request, oldest first, to its Fulfiller, as [`Self::hand_off_waiting`]
    /// does, if the enabled Plugin `plugin` provides a Fulfiller.
    ///
    /// Runs in a task of its own, so a caller that stops waiting leaves none of them waiting.
    pub(crate) async fn hand_off_waiting_to(&self, plugin: &PluginId) {
        if !self.plugins.provides::<dyn Fulfiller<Movies>>(plugin)
            && !self.plugins.provides::<dyn Fulfiller<Shows>>(plugin)
        {
            return;
        }
        let requests = self.clone();
        if let Err(err) = detached(async move { requests.try_hand_off_waiting().await }).await {
            error!("handing the waiting Requests to their Fulfillers failed: {err}");
        }
    }

    /// Hands each Approved Request, oldest first, to its Fulfiller as [`Self::hand_off`] does, such
    /// as those that waited for one to be enabled. A failure is only logged, and leaves the rest
    /// waiting.
    pub(crate) async fn hand_off_waiting(&self) {
        if let Err(err) = self.try_hand_off_waiting().await {
            error!("handing the waiting Requests to their Fulfillers failed: {err}");
        }
    }

    async fn try_hand_off_waiting(&self) -> Result<(), Error> {
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
            self.hand_off(request).await?;
        }
        Ok(())
    }

    /// What the Fulfiller for `request`'s kind says to it, if one is enabled: the reference it
    /// gives once it takes it, or why it doesn't, with no secret setting's value in it.
    async fn fulfill(&self, request: &Request) -> Option<Result<String, String>> {
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
    async fn fulfill_as<Kind: 'static>(&self, request: &Request) -> Option<Result<String, String>> {
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

    /// Asks the Fulfillers how their Processing Requests are progressing, every `interval` while any
    /// Request is Processing, and at once when one becomes Processing. Runs until dropped.
    #[expect(
        clippy::infinite_loop,
        reason = "it follows progress for as long as the Server serves, which aborts it"
    )]
    pub(crate) async fn follow_progress(&self, interval: Duration) {
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
    async fn poll_progress(&self) -> Result<bool, sqlx::Error> {
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
    fn with_progress(&self, mut request: Request) -> Request {
        if request.state == RequestState::Processing {
            let kept = lock(&self.progress).get(&request.id).cloned();
            request.downloads = kept.unwrap_or_default();
        }
        request
    }

    /// Declines the Pending Request `id`, giving its Requesters `reason`, trimmed, unless it is
    /// blank, and publishes that its state changed.
    pub(crate) async fn decline(&self, id: i64, reason: &str) -> Result<Request, Error> {
        self.decide(id, RequestState::Declined, given_reason(reason))
            .await
    }

    async fn decide(
        &self,
        id: i64,
        state: RequestState,
        reason: Option<String>,
    ) -> Result<Request, Error> {
        let request = self
            .db
            .write(move |tx| Box::pin(decide(tx, id, state, reason)))
            .await?;
        self.events.publish(vec![state_changed(request.clone())]);
        Ok(request)
    }

    /// Removes the Request `id`, whatever its state, and publishes that it was cancelled.
    pub(crate) async fn cancel(&self, id: i64) -> Result<(), Error> {
        let request = self.db.write(move |tx| Box::pin(cancel(tx, id))).await?;
        self.events.publish(vec![event(RequestCancelled {
            request: request.into(),
            ..Default::default()
        })]);
        Ok(())
    }

    /// Takes `requester` off the Pending Request `id`, removing it once nobody is left on it, and
    /// publishes which of the two happened.
    pub(crate) async fn withdraw(&self, requester: &User, id: i64) -> Result<(), Error> {
        let user = requester.id;
        let visible = visible_to(requester);
        let happened = self
            .db
            .write(move |tx| Box::pin(withdraw(tx, user, visible, id)))
            .await?;
        self.events.publish(vec![happened]);
        Ok(())
    }

    /// The Requests and Followed Shows `viewer` may see, each newest first.
    pub(crate) async fn list(
        &self,
        viewer: &User,
    ) -> Result<(Vec<Request>, Vec<FollowedShow>), sqlx::Error> {
        let mut conn = self.db.reader().acquire().await?;
        let requests = load(&mut conn, None, visible_to(viewer)).await?;
        let followed = load_followed(&mut conn, None, visible_to(viewer)).await?;
        let requests = requests
            .into_iter()
            .map(|request| self.with_progress(request))
            .collect();
        Ok((requests, followed))
    }

    /// The Movies the metadata source finds titled like `title`, best fit first, as `viewer`
    /// sees them.
    pub(crate) async fn search(
        &self,
        viewer: &User,
        title: String,
    ) -> Result<Vec<Found<Candidate>>, Error> {
        let candidates = self.find(TitleKind::Movie, title).await?;
        let mut conn = self.db.reader().acquire().await?;
        let mut found = Vec::with_capacity(candidates.len());
        // ponytail: two queries per Movie found, which a search caps at a page of 20.
        for (tmdb_id, plugin, candidate) in candidates {
            let mut movie = look_around(&mut conn, viewer, tmdb_id, plugin, candidate).await?;
            movie.request = movie.request.map(|request| self.with_progress(request));
            found.push(movie);
        }
        Ok(found)
    }

    /// The Shows the metadata source finds titled like `title`, best fit first.
    pub(crate) async fn search_shows(
        &self,
        title: String,
    ) -> Result<Vec<FoundShow<Candidate>>, Error> {
        let candidates = self.find(TitleKind::Show, title).await?;
        Ok(candidates
            .into_iter()
            .map(|(tmdb_id, plugin, show)| FoundShow {
                tmdb_id,
                plugin,
                show,
                seasons: Vec::new(),
                followed_show: None,
            })
            .collect())
    }

    /// The titles of `kind` the metadata source finds titled like `title`, best fit first, each
    /// with its TMDB id and the Plugin whose Provider found it.
    async fn find(
        &self,
        kind: TitleKind,
        title: String,
    ) -> Result<Vec<(i64, PluginId, Candidate)>, Error> {
        let query = SearchQuery {
            kind,
            title,
            year: None,
        };
        let candidates = self
            .matcher
            .search_metadata(&query)
            .await
            .map_err(|err| match err {
                ManualError::Database(err) => Error::Database(err),
                // A search names no item or title, so it fails only when no Provider answers.
                ManualError::NoSuchItem(_)
                | ManualError::NoSuchTitle(_)
                | ManualError::Unavailable => Error::Unavailable,
            })?;
        // Requests name titles by TMDB id, so a title without one can't be asked for.
        Ok(candidates
            .into_iter()
            .filter_map(|(plugin, candidate)| match candidate.id {
                ExternalId::Tmdb(tmdb_id) => Some((tmdb_id, plugin, candidate)),
                ExternalId::Imdb(_) | ExternalId::Tvdb(_) => None,
            })
            .collect())
    }

    /// The Show with the TMDB id `tmdb_id` and each of its Seasons, fetched now, as `viewer` sees
    /// them.
    pub(crate) async fn look_up_show(
        &self,
        viewer: &User,
        tmdb_id: i64,
    ) -> Result<FoundShow<Show>, Error> {
        let (plugin, show) = self
            .matcher
            .show(&ExternalId::Tmdb(tmdb_id))
            .await
            .map_err(|err| looking_up(err, Error::NoSuchShow(tmdb_id)))?;
        let seasons = self.seasons(tmdb_id, &show).await?;
        let mut conn = self.db.reader().acquire().await?;
        let mut seasons = look_around_seasons(&mut conn, viewer, tmdb_id, seasons).await?;
        for season in &mut seasons {
            let request = season.request.take();
            season.request = request.map(|request| self.with_progress(request)).into();
        }
        let followed_show = match open_followed_show(&mut conn, tmdb_id).await? {
            Some(id) => load_followed(&mut conn, Some(id), visible_to(viewer))
                .await?
                .pop(),
            None => None,
        };
        Ok(FoundShow {
            tmdb_id,
            plugin,
            show,
            seasons,
            followed_show,
        })
    }

    /// Each of the Seasons `show`, the Show `tmdb_id`, lists, fetched now. One it lists but can't
    /// give fails the lookup as an unknown Show.
    async fn seasons(&self, tmdb_id: i64, show: &Show) -> Result<Vec<Season>, Error> {
        let id = ExternalId::Tmdb(tmdb_id);
        let mut seasons = Vec::with_capacity(show.seasons.len());
        // ponytail: one metadata lookup per Season, one after another.
        for &number in &show.seasons {
            let season = self.matcher.show_season(&id, number).await;
            seasons.push(season.map_err(|err| looking_up(err, Error::NoSuchShow(tmdb_id)))?);
        }
        Ok(seasons)
    }

    /// The Movie with the TMDB id `tmdb_id`, fetched now, as `viewer` sees it.
    pub(crate) async fn look_up(
        &self,
        viewer: &User,
        tmdb_id: i64,
    ) -> Result<Found<Metadata>, Error> {
        let movie = self.fetch(tmdb_id).await?;
        let mut conn = self.db.reader().acquire().await?;
        let plugin = movie.plugin.clone();
        let mut found = look_around(&mut conn, viewer, tmdb_id, plugin, movie).await?;
        found.request = found.request.map(|request| self.with_progress(request));
        Ok(found)
    }

    async fn fetch(&self, tmdb_id: i64) -> Result<Metadata, Error> {
        self.matcher
            .movie(&ExternalId::Tmdb(tmdb_id))
            .await
            .map_err(|err| looking_up(err, Error::NoSuchMovie(tmdb_id)))
    }

    /// The Request `id`, if `viewer` may see it.
    pub(crate) async fn get(&self, viewer: &User, id: i64) -> Result<Request, Error> {
        let mut conn = self.db.reader().acquire().await?;
        let found = load(&mut conn, Some(id), visible_to(viewer)).await?;
        let request = found.into_iter().next().ok_or(Error::NoSuchRequest(id))?;
        Ok(self.with_progress(request))
    }
}

/// `err` from looking a title up, which is `missing` if the metadata source doesn't have it.
fn looking_up(err: ManualError, missing: Error) -> Error {
    match err {
        // A look-up names no item, so its only refusal is the title's.
        ManualError::NoSuchTitle(_) | ManualError::NoSuchItem(_) => missing,
        ManualError::Unavailable => Error::Unavailable,
        ManualError::Database(err) => Error::Database(err),
    }
}

/// `reason`, trimmed, unless it is blank.
fn given_reason(reason: &str) -> Option<String> {
    let reason = reason.trim();
    (!reason.is_empty()).then(|| reason.to_owned())
}

/// The state `asker`'s new Request opens in: Approved if they pass a check for `auto_approve`,
/// else Pending.
fn opening_state(asker: &User, auto_approve: Permission) -> RequestState {
    if asker.permissions.allow(auto_approve) {
        RequestState::Approved
    } else {
        RequestState::Pending
    }
}

/// `movie`, the Movie `tmdb_id` as `plugin`'s Provider gave it, with the Library that has it and
/// its open Request, if `viewer` may see it.
async fn look_around<M>(
    conn: &mut SqliteConnection,
    viewer: &User,
    tmdb_id: i64,
    plugin: PluginId,
    movie: M,
) -> Result<Found<M>, sqlx::Error> {
    let library = contents::library_with_movie(conn, tmdb_id).await?;
    let request = visible_open_request(conn, viewer, tmdb_id, None).await?;
    Ok(Found {
        tmdb_id,
        plugin,
        movie,
        library,
        request,
    })
}

/// Each of `seasons`, of the Show `show_tmdb_id`, as `viewer` sees it: whether a Library has it,
/// and its open Request.
async fn look_around_seasons(
    conn: &mut SqliteConnection,
    viewer: &User,
    show_tmdb_id: i64,
    seasons: Vec<Season>,
) -> Result<Vec<FoundSeason>, sqlx::Error> {
    let today = today(conn).await?;
    let mut found = Vec::with_capacity(seasons.len());
    // ponytail: two or three queries per Season, and a Show has a few dozen at most.
    for season in seasons {
        let matched = contents::matched_episodes(conn, show_tmdb_id, season.number.into()).await?;
        let request = visible_open_request(conn, viewer, show_tmdb_id, Some(season.number)).await?;
        found.push(FoundSeason {
            season_number: season.number,
            in_library: availability(&season.episodes, &matched, &today)
                == Some(RequestState::Available),
            air_date: season.air_date,
            request: request.into(),
            ..Default::default()
        });
    }
    Ok(found)
}

/// The open Request for the Movie `tmdb_id`, or for its Season `season` if `tmdb_id` is a Show's,
/// if `viewer` may see it.
async fn visible_open_request(
    conn: &mut SqliteConnection,
    viewer: &User,
    tmdb_id: i64,
    season: Option<u32>,
) -> Result<Option<Request>, sqlx::Error> {
    let Some(id) = open_request(conn, tmdb_id, season).await? else {
        return Ok(None);
    };
    let visible = load(conn, Some(id), visible_to(viewer)).await?;
    Ok(visible.into_iter().next())
}

/// Today as `YYYY-MM-DD`, by SQLite's calendar, since `std` has none.
async fn today(conn: &mut SqliteConnection) -> Result<String, sqlx::Error> {
    sqlx::query_scalar!(r#"SELECT date('now') AS "today!: String""#)
        .fetch_one(&mut *conn)
        .await
}

/// [`ask`]s for the Movie `wanted`, unless a Library has it.
async fn ask_movie(
    conn: &mut SqliteConnection,
    user: i64,
    wanted: Wanted,
    state: RequestState,
) -> Result<(Request, Option<Event>), Error> {
    if contents::library_with_movie(conn, wanted.tmdb_id)
        .await?
        .is_some()
    {
        return Err(Error::InLibrary(wanted.tmdb_id));
    }
    Ok(ask(conn, user, wanted, state).await?)
}

/// [`ask`]s for each of the Seasons `show`, unless a Library has any of them. Returns the Requests
/// in the order of `show.seasons`, and the events for what changed.
async fn ask_seasons(
    conn: &mut SqliteConnection,
    user: i64,
    show: WantedSeasons,
    state: RequestState,
) -> Result<(Vec<Request>, Vec<Event>), Error> {
    let WantedSeasons {
        show_tmdb_id,
        show_ids,
        show_title,
        seasons,
    } = show;
    let today = today(conn).await?;
    let mut requests = Vec::with_capacity(seasons.len());
    let mut happened = Vec::new();
    for season in seasons {
        let matched = contents::matched_episodes(conn, show_tmdb_id, season.number.into()).await?;
        if availability(&season.episodes, &matched, &today) == Some(RequestState::Available) {
            return Err(Error::SeasonInLibrary {
                show: show_tmdb_id,
                season: season.number,
            });
        }
        let wanted = Wanted {
            tmdb_id: show_tmdb_id,
            season: Some(season.number),
            title: show_title.clone(),
            release_date: season.air_date,
            show_ids: show_ids.clone(),
            followed_show: None,
        };
        let (request, event) = ask(conn, user, wanted, state).await?;
        requests.push(request);
        happened.extend(event);
    }
    Ok((requests, happened))
}

/// How much a Library has of the Season with `episodes`, of which those numbered in `matched` are
/// Matched, counting only those aired by `today` (`YYYY-MM-DD`, as air dates are, so they sort as
/// text): Available once every aired Episode is Matched, Partially Available while some are, and
/// `None` while none are. So a Season yet to air isn't in a Library, and can be Requested.
fn availability(episodes: &[Episode], matched: &HashSet<i64>, today: &str) -> Option<RequestState> {
    let aired: Vec<i64> = episodes
        .iter()
        .filter(|episode| {
            episode
                .air_date
                .as_deref()
                .is_some_and(|aired| aired <= today)
        })
        .map(|episode| episode.number.into())
        .collect();
    let have = aired
        .iter()
        .filter(|number| matched.contains(number))
        .count();
    if have == 0 {
        None
    } else if have == aired.len() {
        Some(RequestState::Available)
    } else {
        Some(RequestState::PartiallyAvailable)
    }
}

/// Opens a Request for `wanted` in `state`, as stored, with no Requesters yet. Returns its ID.
async fn insert_request(
    conn: &mut SqliteConnection,
    wanted: &Wanted,
    state: &str,
    now: i64,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar!(
        "INSERT INTO requests (tmdb_id, season, title, release_date, show_imdb_id, show_tvdb_id,
                               state, created_at, followed_show_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
         RETURNING id",
        wanted.tmdb_id,
        wanted.season,
        wanted.title,
        wanted.release_date,
        wanted.show_ids.imdb,
        wanted.show_ids.tvdb,
        state,
        now,
        wanted.followed_show
    )
    .fetch_one(&mut *conn)
    .await
}

/// Adds `user` as a Requester of the open Request for `wanted`, opening one in `state` if there
/// is none. Returns the Request, and the event for what changed, if anything did.
async fn ask(
    conn: &mut SqliteConnection,
    user: i64,
    wanted: Wanted,
    state: RequestState,
) -> Result<(Request, Option<Event>), sqlx::Error> {
    let now = now();
    let state = stored(state);
    let open = open_request(conn, wanted.tmdb_id, wanted.season).await?;
    let opened = open.is_none();
    let id = match open {
        Some(id) => id,
        None => insert_request(conn, &wanted, state, now).await?,
    };
    let joined = sqlx::query!(
        "INSERT INTO requesters (request_id, user_id, requested_at) VALUES (?, ?, ?)
         ON CONFLICT DO NOTHING",
        id,
        user,
        now
    )
    .execute(&mut *conn)
    .await?
    .rows_affected()
        > 0;
    let request = load_one(conn, id).await?.ok_or(sqlx::Error::RowNotFound)?;
    let happened = if opened {
        Some(event(RequestCreated {
            request: request.clone().into(),
            ..Default::default()
        }))
    } else if joined {
        Some(event(RequesterAdded {
            request: request.clone().into(),
            user_id: user,
            ..Default::default()
        }))
    } else {
        None
    };
    Ok((request, happened))
}

/// Makes Available the open Request for each of the Movies `tmdb_ids` that a Library now has,
/// and returns an event for each (ADR 0012). Declined Requests are closed, so they stay as they
/// are.
pub(crate) async fn make_available(
    conn: &mut SqliteConnection,
    tmdb_ids: Vec<i64>,
) -> Result<Vec<Event>, sqlx::Error> {
    let mut happened = Vec::new();
    for tmdb_id in tmdb_ids {
        let Some(id) = open_request(conn, tmdb_id, None).await? else {
            continue;
        };
        // Not every Matched id is a Movie in a Library: the Match isn't stored if its Item went
        // since it was decided about, and an Episode's TMDB id can equal an unrelated Movie's.
        if contents::library_with_movie(conn, tmdb_id).await?.is_none() {
            continue;
        }
        happened.extend(change_state(conn, id, RequestState::Available).await?);
    }
    Ok(happened)
}

/// Makes the open Request for each of `seasons`, as the metadata source gave them just now with
/// their Show's TMDB id, Partially Available or Available by how many of its aired Episodes a
/// Library has, and returns an event for each Request that changed (ADR 0012). Declined and
/// Available Requests are closed, so Episodes that air later don't reopen them. A Season that came
/// out after the Show's Approved Followed Show was opened gets a Request in that state.
pub(crate) async fn make_seasons_available(
    conn: &mut SqliteConnection,
    seasons: Vec<(i64, Season)>,
) -> Result<Vec<Event>, sqlx::Error> {
    let today = today(conn).await?;
    let mut happened = Vec::new();
    for (show_tmdb_id, season) in seasons {
        let matched = contents::matched_episodes(conn, show_tmdb_id, season.number.into()).await?;
        let Some(state) = availability(&season.episodes, &matched, &today) else {
            continue;
        };
        let changed = match open_request(conn, show_tmdb_id, Some(season.number)).await? {
            Some(id) => change_state(conn, id, state).await?,
            None => open_followed_season(conn, show_tmdb_id, &season, state).await?,
        };
        happened.extend(changed);
    }
    Ok(happened)
}

/// Opens a Request in `state` for `season` of the Show `show_tmdb_id`, which has no open one, for
/// the Requesters of the Show's Approved Followed Show, if the Season is numbered after every one
/// the Followed Show covers and has no Available Request, so a Season once Available isn't asked
/// for again. The Followed Show covers the Season from then on, so one it opened and someone
/// declined, cancelled or withdrew from isn't opened again. Returns the event for it.
async fn open_followed_season(
    conn: &mut SqliteConnection,
    show_tmdb_id: i64,
    season: &Season,
    state: RequestState,
) -> Result<Option<Event>, sqlx::Error> {
    // ponytail: a Season that came out but was skipped, such as a Season 0 added later, gets no
    // Request; ask for it on its own.
    let Some(followed) = sqlx::query!(
        r#"SELECT id AS "id!", title FROM followed_shows
           WHERE tmdb_id = ? AND state = 'approved'
             AND (latest_season IS NULL OR latest_season < ?)"#,
        show_tmdb_id,
        season.number
    )
    .fetch_optional(&mut *conn)
    .await?
    else {
        return Ok(None);
    };
    let available = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM requests
                          WHERE tmdb_id = ? AND season = ? AND state = 'available') AS "is!: bool""#,
        show_tmdb_id,
        season.number
    )
    .fetch_one(&mut *conn)
    .await?;
    if available {
        return Ok(None);
    }
    let wanted = Wanted {
        tmdb_id: show_tmdb_id,
        season: Some(season.number),
        title: followed.title,
        release_date: season.air_date.clone(),
        // ponytail: a Followed Show stores no Show ids, and this Season opens already in a Library.
        show_ids: ExternalIds::default(),
        followed_show: Some(followed.id),
    };
    let id = insert_request(conn, &wanted, stored(state), now()).await?;
    sqlx::query!(
        "UPDATE followed_shows SET latest_season = ? WHERE id = ?",
        season.number,
        followed.id
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!(
        "INSERT INTO requesters (request_id, user_id, requested_at)
         SELECT ?, user_id, requested_at FROM followed_show_requesters WHERE followed_show_id = ?
         ORDER BY requested_at, rowid",
        id,
        followed.id
    )
    .execute(&mut *conn)
    .await?;
    let request = load_one(conn, id).await?.ok_or(sqlx::Error::RowNotFound)?;
    Ok(Some(event(RequestCreated {
        request: request.into(),
        ..Default::default()
    })))
}

/// Moves the Request `id` to `state`, and returns the event for it, or `None` if it was in
/// `state` already.
async fn change_state(
    conn: &mut SqliteConnection,
    id: i64,
    state: RequestState,
) -> Result<Option<Event>, sqlx::Error> {
    let state = stored(state);
    let changed = sqlx::query!(
        "UPDATE requests SET state = ?1, failure_reason = NULL WHERE id = ?2 AND state != ?1",
        state,
        id
    )
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

/// The open Request for the Movie `tmdb_id`, or for its Season `season` if `tmdb_id` is a Show's.
async fn open_request(
    conn: &mut SqliteConnection,
    tmdb_id: i64,
    season: Option<u32>,
) -> Result<Option<i64>, sqlx::Error> {
    // Open as `one_open_request_per_movie_or_season` has it.
    sqlx::query_scalar!(
        "SELECT id FROM requests
         WHERE tmdb_id = ? AND season IS ? AND state NOT IN ('declined', 'available')",
        tmdb_id,
        season
    )
    .fetch_optional(&mut *conn)
    .await
}

/// Moves the Pending Request `id` to `state` with `reason`, and returns it as it is now.
async fn decide(
    conn: &mut SqliteConnection,
    id: i64,
    state: RequestState,
    reason: Option<String>,
) -> Result<Request, Error> {
    let state = stored(state);
    let decided = sqlx::query!(
        "UPDATE requests SET state = ?, decline_reason = ? WHERE id = ? AND state = 'pending'",
        state,
        reason,
        id
    )
    .execute(&mut *conn)
    .await?
    .rows_affected()
        > 0;
    let request = load_one(conn, id).await?.ok_or(Error::NoSuchRequest(id))?;
    if !decided {
        return Err(Error::NotPending(id));
    }
    Ok(request)
}

/// Moves the Approved Request `id` on as its Fulfiller said: to Processing, keeping the reference
/// it gave, or to Failed, keeping why. Returns it as it is now, or `None` if it isn't Approved any
/// more.
async fn settle(
    conn: &mut SqliteConnection,
    id: i64,
    fulfilled: Result<String, String>,
) -> Result<Option<Request>, sqlx::Error> {
    let (state, reference, reason) = match fulfilled {
        Ok(reference) => (RequestState::Processing, Some(reference), None),
        Err(reason) => (RequestState::Failed, None, Some(reason)),
    };
    let state = stored(state);
    let settled = sqlx::query!(
        "UPDATE requests SET state = ?, fulfiller_reference = ?, failure_reason = ?
         WHERE id = ? AND state = 'approved'",
        state,
        reference,
        reason,
        id
    )
    .execute(&mut *conn)
    .await?
    .rows_affected()
        > 0;
    if !settled {
        return Ok(None);
    }
    load_one(conn, id).await
}

/// Moves the Failed Request `id` back to Approved, forgetting why it failed, and returns it as it
/// is now.
async fn retry(conn: &mut SqliteConnection, id: i64) -> Result<Request, Error> {
    let retried = sqlx::query!(
        "UPDATE requests SET state = 'approved', failure_reason = NULL
         WHERE id = ? AND state = 'failed'",
        id
    )
    .execute(&mut *conn)
    .await?
    .rows_affected()
        > 0;
    let request = load_one(conn, id).await?.ok_or(Error::NoSuchRequest(id))?;
    if !retried {
        return Err(Error::NotFailed(id));
    }
    Ok(request)
}

/// Removes the Request `id`, and its Requesters with it, and returns it as it was.
async fn cancel(conn: &mut SqliteConnection, id: i64) -> Result<Request, Error> {
    let request = load_one(conn, id).await?.ok_or(Error::NoSuchRequest(id))?;
    sqlx::query!("DELETE FROM requests WHERE id = ?", id)
        .execute(&mut *conn)
        .await?;
    Ok(request)
}

/// Takes `user` off the Pending Request `id`, among those `visible` lets them see, removing the
/// Request if they were its last Requester. Returns the event for what changed.
async fn withdraw(
    conn: &mut SqliteConnection,
    user: i64,
    visible: Option<i64>,
    id: i64,
) -> Result<Event, Error> {
    let request = load(conn, Some(id), visible)
        .await?
        .into_iter()
        .next()
        .ok_or(Error::NoSuchRequest(id))?;
    if !request.requesters.iter().any(|r| r.user_id == user) {
        return Err(Error::NotARequester(id));
    }
    if request.state != RequestState::Pending {
        return Err(Error::NotPending(id));
    }
    if request.requesters.len() == 1 {
        sqlx::query!("DELETE FROM requests WHERE id = ?", id)
            .execute(&mut *conn)
            .await?;
        return Ok(event(RequestWithdrawn {
            request: request.into(),
            ..Default::default()
        }));
    }
    sqlx::query!(
        "DELETE FROM requesters WHERE request_id = ? AND user_id = ?",
        id,
        user
    )
    .execute(&mut *conn)
    .await?;
    Ok(event(RequesterRemoved {
        request: request.into(),
        user_id: user,
        ..Default::default()
    }))
}

/// Adds `user` as a Requester of the open Followed Show for `show`, opening one in `state` if there
/// is none, then [`ask`]s for each of its Seasons not in a Library, opening missing ones for the
/// Followed Show in its state. Returns the Followed Show, the Season Requests and the events for
/// what changed.
async fn follow(
    conn: &mut SqliteConnection,
    user: i64,
    show: WantedSeasons,
    state: RequestState,
) -> Result<(FollowedShow, Vec<Request>, Vec<Event>), Error> {
    let WantedSeasons {
        show_tmdb_id,
        show_ids,
        show_title,
        seasons,
    } = show;
    let now = now();
    let id = match open_followed_show(conn, show_tmdb_id).await? {
        Some(id) => id,
        None => {
            sqlx::query_scalar!(
                "INSERT INTO followed_shows (tmdb_id, title, state, latest_season, created_at)
                 VALUES (?, ?, ?, ?, ?)
                 RETURNING id",
                show_tmdb_id,
                show_title,
                stored(state),
                seasons.iter().map(|season| season.number).max(),
                now
            )
            .fetch_one(&mut *conn)
            .await?
        }
    };
    sqlx::query!(
        "INSERT INTO followed_show_requesters (followed_show_id, user_id, requested_at) VALUES (?, ?, ?)
         ON CONFLICT DO NOTHING",
        id,
        user,
        now
    )
    .execute(&mut *conn)
    .await?;
    let followed = load_followed(conn, Some(id), None).await?.pop();
    let followed = followed.ok_or(sqlx::Error::RowNotFound)?;
    // An Approved Followed Show was approved once for every Season, so needs no approving again.
    let state = followed.state.as_known().unwrap_or(state);
    let today = today(conn).await?;
    let mut requests = Vec::with_capacity(seasons.len());
    let mut happened = Vec::new();
    for season in seasons {
        let matched = contents::matched_episodes(conn, show_tmdb_id, season.number.into()).await?;
        if availability(&season.episodes, &matched, &today) == Some(RequestState::Available) {
            continue;
        }
        let wanted = Wanted {
            tmdb_id: show_tmdb_id,
            season: Some(season.number),
            title: show_title.clone(),
            release_date: season.air_date,
            show_ids: show_ids.clone(),
            followed_show: Some(id),
        };
        let (request, event) = ask(conn, user, wanted, state).await?;
        requests.push(request);
        happened.extend(event);
    }
    Ok((followed, requests, happened))
}

/// The open Followed Show of the Show `tmdb_id`.
async fn open_followed_show(
    conn: &mut SqliteConnection,
    tmdb_id: i64,
) -> Result<Option<i64>, sqlx::Error> {
    // Open as `one_open_followed_show` has it.
    sqlx::query_scalar!(
        "SELECT id FROM followed_shows WHERE tmdb_id = ? AND state != 'declined'",
        tmdb_id
    )
    .fetch_optional(&mut *conn)
    .await
}

/// Moves the Pending Followed Show `id`, and each Pending Season Request it opened, to `state`
/// with `reason`. Returns the Followed Show and those Requests as they are now.
async fn decide_followed(
    conn: &mut SqliteConnection,
    id: i64,
    state: RequestState,
    reason: Option<String>,
) -> Result<(FollowedShow, Vec<Request>), Error> {
    let stored_state = stored(state);
    let decided = sqlx::query!(
        "UPDATE followed_shows SET state = ?, decline_reason = ? WHERE id = ? AND state = 'pending'",
        stored_state,
        reason,
        id
    )
    .execute(&mut *conn)
    .await?
    .rows_affected()
        > 0;
    let followed = load_followed(conn, Some(id), None).await?.pop();
    let followed = followed.ok_or(Error::NoSuchFollowedShow(id))?;
    if !decided {
        return Err(Error::FollowedShowNotPending(id));
    }
    let pending = sqlx::query_scalar!(
        "SELECT id FROM requests WHERE followed_show_id = ? AND state = 'pending'",
        id
    )
    .fetch_all(&mut *conn)
    .await?;
    let mut requests = Vec::with_capacity(pending.len());
    for request in pending {
        requests.push(decide(conn, request, state, reason.clone()).await?);
    }
    Ok((followed, requests))
}

/// `request` as its Fulfiller is handed it: naming its Followed Show only while that is Approved,
/// which asks the Fulfiller to follow the Show.
// ponytail: only a hand-off asks to follow, so a Followed Show that hands nothing off, such as one
// whose aired Seasons are all in a Library or already have open Requests, isn't followed until a
// Fulfiller contract call asks to follow without a hand-off.
async fn as_handed_off(
    conn: &mut SqliteConnection,
    mut request: Request,
) -> Result<Request, sqlx::Error> {
    if let Some(id) = request.followed_show_id
        && !followed_show_approved(conn, id).await?
    {
        request.followed_show_id = None;
    }
    Ok(request)
}

async fn followed_show_approved(conn: &mut SqliteConnection, id: i64) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM followed_shows WHERE id = ? AND state = 'approved')
           AS "is!: bool""#,
        id
    )
    .fetch_one(&mut *conn)
    .await
}

/// Removes the Followed Show `id`, and its Requesters with it. Returns the first Season Request it
/// opened, as it was, if it was Approved, so its Fulfiller was asked to follow the Show.
async fn cancel_followed(conn: &mut SqliteConnection, id: i64) -> Result<Option<Request>, Error> {
    let followed_season = if followed_show_approved(conn, id).await? {
        // ponytail: a Followed Show stores no Show ids, so once every Season Request it opened is
        // cancelled its Fulfiller isn't told to stop; store them if that bites.
        let first = sqlx::query_scalar!(
            "SELECT MIN(id) FROM requests WHERE followed_show_id = ?",
            id
        )
        .fetch_one(&mut *conn)
        .await?;
        match first {
            Some(first) => load_one(conn, first).await?,
            None => None,
        }
    } else {
        None
    };
    let removed = sqlx::query!("DELETE FROM followed_shows WHERE id = ?", id)
        .execute(&mut *conn)
        .await?
        .rows_affected();
    if removed == 0 {
        return Err(Error::NoSuchFollowedShow(id));
    }
    Ok(followed_season)
}

/// Takes `user` off the Followed Show `id`, among those `visible` lets them see, removing it as
/// [`cancel_followed`] does if they were its last Requester.
async fn withdraw_followed(
    conn: &mut SqliteConnection,
    user: i64,
    visible: Option<i64>,
    id: i64,
) -> Result<Option<Request>, Error> {
    let followed = load_followed(conn, Some(id), visible).await?.pop();
    let followed = followed.ok_or(Error::NoSuchFollowedShow(id))?;
    if !followed.requesters.iter().any(|r| r.user_id == user) {
        return Err(Error::NotAFollowedShowRequester(id));
    }
    if followed.requesters.len() == 1 {
        return cancel_followed(conn, id).await;
    }
    sqlx::query!(
        "DELETE FROM followed_show_requesters WHERE followed_show_id = ? AND user_id = ?",
        id,
        user
    )
    .execute(&mut *conn)
    .await?;
    Ok(None)
}

/// The Followed Shows, newest first: only the one with the ID `id` if given, and only those the
/// User `requester` is a Requester of if given.
async fn load_followed(
    conn: &mut SqliteConnection,
    id: Option<i64>,
    requester: Option<i64>,
) -> Result<Vec<FollowedShow>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT id AS "id!", tmdb_id, title, state, created_at, decline_reason
           FROM followed_shows
           WHERE (?1 IS NULL OR id = ?1)
             AND (?2 IS NULL OR EXISTS (SELECT 1 FROM followed_show_requesters
                                        WHERE followed_show_id = followed_shows.id
                                          AND user_id = ?2))
           ORDER BY id DESC"#,
        id,
        requester
    )
    .fetch_all(&mut *conn)
    .await?;
    // ponytail: every Followed Show's Requesters, as `load` reads every Request's.
    let mut requesters: HashMap<i64, Vec<Requester>> = HashMap::new();
    let requester_rows = sqlx::query!(
        "SELECT r.followed_show_id, r.user_id, users.display_username, r.requested_at
         FROM followed_show_requesters AS r
         JOIN users ON users.id = r.user_id
         WHERE ?1 IS NULL OR r.followed_show_id = ?1
         ORDER BY r.requested_at, r.rowid",
        id
    )
    .fetch_all(&mut *conn)
    .await?;
    for row in requester_rows {
        requesters
            .entry(row.followed_show_id)
            .or_default()
            .push(Requester {
                user_id: row.user_id,
                name: row.display_username,
                requested_at: row.requested_at,
                ..Default::default()
            });
    }
    Ok(rows
        .into_iter()
        .map(|row| FollowedShow {
            id: row.id,
            show_tmdb_id: row.tmdb_id,
            show_title: row.title,
            state: read_state(&row.state).into(),
            requesters: requesters.remove(&row.id).unwrap_or_default(),
            created_at: row.created_at,
            decline_reason: row.decline_reason,
            ..Default::default()
        })
        .collect())
}

async fn load_one(conn: &mut SqliteConnection, id: i64) -> Result<Option<Request>, sqlx::Error> {
    Ok(load(conn, Some(id), None).await?.into_iter().next())
}

/// The Requests, newest first: only the one with the ID `id` if given, and only those the User
/// `requester` is a Requester of if given.
async fn load(
    conn: &mut SqliteConnection,
    id: Option<i64>,
    requester: Option<i64>,
) -> Result<Vec<Request>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT id AS "id!", tmdb_id, season AS "season: u32", title, release_date, show_imdb_id,
                  show_tvdb_id, state, created_at, decline_reason, failure_reason, followed_show_id
           FROM requests
           WHERE (?1 IS NULL OR id = ?1)
             AND (?2 IS NULL OR EXISTS (SELECT 1 FROM requesters
                                        WHERE request_id = requests.id AND user_id = ?2))
           ORDER BY id DESC"#,
        id,
        requester
    )
    .fetch_all(&mut *conn)
    .await?;
    // ponytail: every Request's Requesters, even for one User's Requests; filter by them in SQL if
    // the list gets big.
    let mut requesters: HashMap<i64, Vec<Requester>> = HashMap::new();
    let requester_rows = sqlx::query!(
        "SELECT requesters.request_id, requesters.user_id, users.display_username,
                requesters.requested_at
         FROM requesters
         JOIN users ON users.id = requesters.user_id
         WHERE ?1 IS NULL OR requesters.request_id = ?1
         ORDER BY requesters.requested_at, requesters.rowid",
        id
    )
    .fetch_all(&mut *conn)
    .await?;
    for row in requester_rows {
        requesters
            .entry(row.request_id)
            .or_default()
            .push(Requester {
                user_id: row.user_id,
                name: row.display_username,
                requested_at: row.requested_at,
                ..Default::default()
            });
    }
    Ok(rows
        .into_iter()
        .map(|row| {
            let mut request = Request {
                id: row.id,
                state: read_state(&row.state).into(),
                requesters: requesters.remove(&row.id).unwrap_or_default(),
                created_at: row.created_at,
                decline_reason: row.decline_reason,
                failure_reason: row.failure_reason,
                followed_show_id: row.followed_show_id,
                ..Default::default()
            };
            match row.season {
                Some(season_number) => {
                    request.season = RequestedSeason {
                        show_tmdb_id: row.tmdb_id,
                        season_number,
                        show_title: row.title,
                        air_date: row.release_date,
                        show_imdb_id: row.show_imdb_id,
                        show_tvdb_id: row.show_tvdb_id,
                        ..Default::default()
                    }
                    .into();
                }
                None => {
                    request.movie = RequestedMovie {
                        tmdb_id: row.tmdb_id,
                        title: row.title,
                        release_date: row.release_date,
                        ..Default::default()
                    }
                    .into();
                }
            }
            request
        })
        .collect())
}

/// A Processing Request, as [`Requests::poll_progress`] asks its Fulfiller about it.
struct Processing {
    id: i64,
    /// Whether it asks for a Movie, rather than a Season.
    is_movie: bool,
    /// What its Fulfiller gave when it took it.
    reference: String,
}

async fn processing(conn: &mut SqliteConnection) -> Result<Vec<Processing>, sqlx::Error> {
    sqlx::query_as!(
        Processing,
        r#"SELECT id AS "id!", season IS NULL AS "is_movie!: bool",
                  fulfiller_reference AS "reference!"
           FROM requests
           WHERE state = 'processing' AND fulfiller_reference IS NOT NULL"#
    )
    .fetch_all(&mut *conn)
    .await
}

/// Runs `work` in a task of its own, so it finishes even if the caller stops waiting: a Request
/// written Approved must still reach the Fulfiller, and its reference must still be stored.
async fn detached<T: Send + 'static>(
    work: impl Future<Output = Result<T, Error>> + Send + 'static,
) -> Result<T, Error> {
    // The task is never aborted, so it only fails if it panicked.
    tokio::spawn(work)
        .await
        .unwrap_or_else(|_| Err(sqlx::Error::WorkerCrashed.into()))
}

/// The id of the Request `happened` says was opened, if it says that.
fn opened_request_id(happened: &Event) -> Option<i64> {
    match &happened.event {
        Some(Happened::RequestCreated(created)) => Some(created.request.id),
        _ => None,
    }
}

/// The event saying `request` is now in the state it has.
fn state_changed(request: Request) -> Event {
    event(RequestStateChanged {
        request: request.into(),
        ..Default::default()
    })
}

/// `state` as `requests.state` stores it. Unspecified has no stored form: `""` fails the CHECK.
fn stored(state: RequestState) -> &'static str {
    STATES
        .iter()
        .find(|&&(known, _)| known == state)
        .map_or("", |&(_, name)| name)
}

fn read_state(stored: &str) -> RequestState {
    STATES
        .iter()
        .find(|&&(_, name)| name == stored)
        .map_or(RequestState::REQUEST_STATE_UNSPECIFIED, |&(state, _)| state)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, HashSet},
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use nama_plugin_api::{
        BoxFuture, Manifest, Plugin, PluginError, PluginId, Priority, Registrar,
        fulfiller::{Download, Fulfiller, Movies},
        metadata::{Episode, ExternalIds, Season},
        settings::SettingsSchema,
    };
    use nama_proto::nama::v1::{Request, RequestState};
    use sqlx::SqliteConnection;
    use tokio::sync::Notify;

    use super::{Claim, Requests, Wanted, ask, availability, change_state, load_one};
    use crate::{
        contents::{Change, Contents, Decided, Decisions, Item, Metadata},
        db::Database,
        events::EventBus,
        libraries::{Kind, Libraries},
        matching::Matcher,
        plugins::Registry,
        server_settings::ServerUrl,
    };

    const TODAY: &str = "2026-11-08";

    fn episode(number: u32, air_date: Option<&str>) -> Episode {
        Episode {
            ids: ExternalIds::default(),
            season: 1,
            number,
            title: format!("Episode {number}"),
            overview: None,
            air_date: air_date.map(str::to_owned),
            runtime_minutes: None,
            still: None,
        }
    }

    #[test]
    fn a_season_still_airing_follows_how_many_of_its_aired_episodes_are_matched() {
        let airing = [
            episode(1, Some("2026-11-01")),
            episode(2, Some(TODAY)),
            episode(3, Some("2026-11-15")),
            episode(4, None),
        ];

        let all_aired = availability(&airing, &HashSet::from([1, 2]), TODAY);
        let some_aired = availability(&airing, &HashSet::from([1, 3, 4]), TODAY);
        let none_aired = availability(&airing, &HashSet::from([3, 4]), TODAY);

        assert_eq!(all_aired, Some(RequestState::Available));
        assert_eq!(some_aired, Some(RequestState::PartiallyAvailable));
        assert_eq!(none_aired, None);
    }

    #[test]
    fn a_season_yet_to_air_is_not_in_a_library() {
        let announced = [episode(1, Some("2027-01-01")), episode(2, None)];

        assert_eq!(
            availability(&announced, &HashSet::from([1, 2]), TODAY),
            None
        );
        assert_eq!(availability(&[], &HashSet::new(), TODAY), None);
    }

    /// Asks for each of `wanted` in turn as a new User, opening Requests in `state`, and returns
    /// the Requests' ids.
    async fn ask_each(
        conn: &mut SqliteConnection,
        wanted: Vec<Wanted>,
        state: RequestState,
    ) -> Result<Vec<i64>, sqlx::Error> {
        let user = sqlx::query_scalar(
            "INSERT INTO users (username, display_username, permissions, created_at, updated_at)
             VALUES ('ada', 'Ada', 0, 0, 0) RETURNING id",
        )
        .fetch_one(&mut *conn)
        .await?;
        let mut ids = Vec::new();
        for one in wanted {
            let (request, _) = ask(conn, user, one, state).await?;
            ids.push(request.id);
        }
        Ok(ids)
    }

    #[tokio::test]
    async fn a_movie_and_a_show_with_the_same_tmdb_id_have_requests_of_their_own() {
        let data = tempfile::tempdir().unwrap();
        let db = Database::open(data.path()).await.unwrap();
        let wanted = [None, Some(0), Some(1), None].map(|season| Wanted {
            tmdb_id: 1,
            season,
            title: "Shared".to_owned(),
            release_date: None,
            show_ids: ExternalIds::default(),
            followed_show: None,
        });

        let ids = db
            .write(move |tx| Box::pin(ask_each(tx, wanted.into(), RequestState::Pending)))
            .await
            .unwrap();

        assert_eq!(ids.iter().collect::<HashSet<_>>().len(), 3, "{ids:?}");
        assert_eq!(
            ids.first(),
            ids.last(),
            "asking again joins the Movie's Request"
        );
    }

    /// A Movie Fulfiller that counts the Requests it is handed, and takes each only once `open`
    /// lets it.
    #[derive(Default)]
    struct Gated {
        handed: AtomicUsize,
        open: Notify,
    }

    impl Fulfiller<Movies> for Gated {
        fn fulfill<'a>(&'a self, _: &'a Request) -> BoxFuture<'a, Result<String, PluginError>> {
            self.handed.fetch_add(1, Ordering::SeqCst);
            Box::pin(async {
                self.open.notified().await;
                Ok("taken".to_owned())
            })
        }

        fn progress<'a>(
            &'a self,
            _: &'a [String],
        ) -> BoxFuture<'a, Result<HashMap<String, Vec<Download>>, PluginError>> {
            Box::pin(async { Ok(HashMap::new()) })
        }
    }

    impl Gated {
        /// Lets it take each Request it is handed until it has been handed `count`.
        async fn take_until_handed(&self, count: usize) {
            while self.handed.load(Ordering::SeqCst) < count {
                self.open.notify_one();
                tokio::task::yield_now().await;
            }
        }
    }

    /// A Movie Fulfiller that takes every Request at once, giving its TMDB id as the reference,
    /// and keeps each batch of references it is asked about.
    #[derive(Default)]
    struct Asked(std::sync::Mutex<Vec<Vec<String>>>);

    impl Fulfiller<Movies> for Asked {
        fn fulfill<'a>(
            &'a self,
            request: &'a Request,
        ) -> BoxFuture<'a, Result<String, PluginError>> {
            let reference = request.movie.tmdb_id.to_string();
            Box::pin(async { Ok(reference) })
        }

        fn progress<'a>(
            &'a self,
            references: &'a [String],
        ) -> BoxFuture<'a, Result<HashMap<String, Vec<Download>>, PluginError>> {
            super::lock(&self.0).push(references.to_vec());
            Box::pin(async { Ok(HashMap::new()) })
        }
    }

    impl Asked {
        fn batches(&self) -> Vec<Vec<String>> {
            super::lock(&self.0).clone()
        }

        /// The references it was last asked about, sorted.
        fn last_asked_about(&self) -> Vec<String> {
            let mut batch = super::lock(&self.0).last().cloned().unwrap_or_default();
            batch.sort();
            batch
        }
    }

    /// A Plugin, `gated`, providing a Movie Fulfiller.
    struct FulfillerPlugin(Arc<dyn Fulfiller<Movies>>);

    impl Plugin for FulfillerPlugin {
        fn manifest(&self) -> Manifest {
            Manifest {
                id: PluginId::from_static("gated"),
                version: "0.0.0".to_owned(),
                settings: SettingsSchema::default(),
            }
        }

        fn register(&self, registrar: &mut Registrar) {
            registrar.provide::<dyn Fulfiller<Movies>>(Priority::DEFAULT, Arc::clone(&self.0));
        }
    }

    /// A Match of an Episode of the Show `show_tmdb_id`.
    fn matched_to_show(show_tmdb_id: i64) -> Decided {
        Decided::Matched(Box::new(Metadata {
            plugin: PluginId::from_static("tmdb"),
            ids: ExternalIds::default(),
            show_ids: ExternalIds {
                tmdb: Some(show_tmdb_id),
                ..ExternalIds::default()
            },
            title: "Episode 1".to_owned(),
            original_title: None,
            show_title: None,
            overview: None,
            release_date: None,
            runtime_minutes: None,
            poster: None,
            backdrop: None,
        }))
    }

    /// Season 1, as the metadata source gives it: Episode 1 aired in 2017, and Episode 2 airs on
    /// `episode_2_airs`.
    fn season_1(episode_2_airs: &str) -> Season {
        Season {
            ids: ExternalIds::default(),
            number: 1,
            title: "Season 1".to_owned(),
            overview: None,
            air_date: None,
            poster: None,
            episodes: vec![
                episode(1, Some("2017-12-01")),
                episode(2, Some(episode_2_airs)),
            ],
        }
    }

    /// Requests in a new database, which holds an Approved Movie Request for each of `tmdb_ids`,
    /// and whose one Plugin provides the returned [`Gated`] as the Movie Fulfiller.
    async fn approved_for_gated(tmdb_ids: &[i64]) -> (tempfile::TempDir, Requests, Arc<Gated>) {
        let gated = Arc::new(Gated::default());
        let fulfiller = Arc::clone(&gated);
        let (data, requests) = approved_for(tmdb_ids, fulfiller).await;
        (data, requests, gated)
    }

    /// Requests in a new database, which holds an Approved Movie Request for each of `tmdb_ids`,
    /// and whose one Plugin provides `fulfiller` as the Movie Fulfiller.
    async fn approved_for(
        tmdb_ids: &[i64],
        fulfiller: Arc<dyn Fulfiller<Movies>>,
    ) -> (tempfile::TempDir, Requests) {
        let data = tempfile::tempdir().unwrap();
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let wanted = tmdb_ids
            .iter()
            .map(|&tmdb_id| Wanted {
                tmdb_id,
                season: None,
                title: format!("Movie {tmdb_id}"),
                release_date: None,
                show_ids: ExternalIds::default(),
                followed_show: None,
            })
            .collect();
        db.write(move |tx| Box::pin(ask_each(tx, wanted, RequestState::Approved)))
            .await
            .unwrap();
        let plugins = Registry::new();
        plugins
            .enable(Arc::new(FulfillerPlugin(fulfiller)))
            .unwrap();
        let events = EventBus::new(Arc::clone(&db), plugins.clone(), ServerUrl::default());
        let matcher = Matcher::new(Arc::clone(&db), plugins.clone(), events.clone());
        (data, Requests::new(db, matcher, events, plugins))
    }

    /// Waits until `asked` has been asked about `references`, sorted, in one batch.
    async fn until_asked_about(asked: &Asked, references: &[&str]) {
        let wait = async {
            while asked.last_asked_about() != references {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        };
        tokio::time::timeout(Duration::from_secs(5), wait)
            .await
            .expect("never asked about them in one batch");
    }

    /// How many times `asked` is asked about anything while `quiet` passes.
    async fn asked_while(asked: &Asked, quiet: Duration) -> usize {
        let before = asked.batches().len();
        tokio::time::sleep(quiet).await;
        asked.batches().len() - before
    }

    async fn move_to(requests: &Requests, id: i64, state: RequestState) {
        requests
            .db
            .write(move |tx| Box::pin(change_state(tx, id, state)))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn one_poll_asks_the_fulfiller_once_about_every_processing_request() {
        let asked = Arc::new(Asked::default());
        let fulfiller = Arc::clone(&asked);
        let (_data, requests) = approved_for(&[949, 438_631, 603], fulfiller).await;
        requests.hand_off_waiting().await;
        move_to(&requests, 3, RequestState::Available).await;

        let any = requests.poll_progress().await.unwrap();

        assert!(any);
        assert_eq!(asked.batches().len(), 1);
        assert_eq!(asked.last_asked_about(), ["438631", "949"]);
    }

    #[tokio::test]
    async fn the_fulfiller_is_asked_about_all_its_processing_requests_at_once_only_while_any_is() {
        const EVERY: Duration = Duration::from_millis(10);
        let asked = Arc::new(Asked::default());
        let fulfiller = Arc::clone(&asked);
        let (_data, requests) = approved_for(&[949, 438_631], fulfiller).await;
        let following = requests.clone();
        let follow = tokio::spawn(async move { following.follow_progress(EVERY).await });
        let nothing_processing = asked_while(&asked, EVERY * 10).await;

        requests.hand_off_waiting().await;
        until_asked_about(&asked, &["438631", "949"]).await;
        move_to(&requests, 1, RequestState::Available).await;
        move_to(&requests, 2, RequestState::Available).await;
        // Asked once more at most, if it was already asking as they became Available.
        asked_while(&asked, EVERY * 5).await;
        let nothing_processing_again = asked_while(&asked, EVERY * 10).await;
        // As if it were Approved again, to be handed off again.
        move_to(&requests, 2, RequestState::Approved).await;
        requests.hand_off_waiting().await;

        until_asked_about(&asked, &["438631"]).await;
        follow.abort();
        assert_eq!(nothing_processing, 0);
        assert_eq!(nothing_processing_again, 0);
    }

    #[tokio::test]
    async fn a_request_being_handed_off_is_not_handed_to_the_fulfiller_again() {
        let (_data, requests, gated) = approved_for_gated(&[949]).await;
        let handing_first = requests.clone();
        let first = tokio::spawn(async move { handing_first.hand_off_waiting().await });
        while gated.handed.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }

        tokio::time::timeout(Duration::from_secs(1), requests.hand_off_waiting())
            .await
            .expect("the second hand-off waited on the Fulfiller");
        gated.open.notify_one();
        first.await.unwrap();

        assert_eq!(gated.handed.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_claim_another_hand_off_skipped_is_kept_for_one_more_look() {
        let (_data, requests, _) = approved_for_gated(&[]).await;
        let claim = Claim::take(&requests, 1).unwrap();
        assert!(Claim::take(&requests, 1).is_none());

        let kept = claim.release().expect("the skipped Request was let go");

        assert!(kept.release().is_none());
        assert!(Claim::take(&requests, 1).is_some());
    }

    #[tokio::test]
    async fn enabling_a_fulfiller_hands_it_every_waiting_request_even_if_the_admin_stops_waiting() {
        let (_data, requests, gated) = approved_for_gated(&[949, 438_631]).await;
        let id = PluginId::from_static("gated");
        let enabling = tokio::spawn(async move { requests.hand_off_waiting_to(&id).await });
        while gated.handed.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }

        enabling.abort();
        tokio::time::timeout(Duration::from_secs(1), gated.take_until_handed(2))
            .await
            .expect("the second Request was never handed to the Fulfiller");
    }

    #[tokio::test]
    async fn an_available_season_request_stays_available_when_a_later_episode_airs() {
        const DARK: i64 = 70523;
        let data = tempfile::tempdir().unwrap();
        let folder = tempfile::tempdir().unwrap();
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let root = folder.path().canonicalize().unwrap();
        let library = Libraries::new(Arc::clone(&db))
            .create(
                "Shows",
                Kind::Shows,
                vec![root.to_str().unwrap().to_owned()],
            )
            .await
            .unwrap()
            .id;
        let contents = Contents::new(Arc::clone(&db));
        let pilot = Item::Episode {
            show: "Dark".to_owned(),
            year: None,
            tmdb_id: Some(DARK),
            season: 1,
            episode: 1,
        };
        let store = Change::Store {
            path: "/media/Dark/Season 01/Dark - S01E01.mkv".to_owned(),
            size: 1,
            modified_ns: 0,
            items: vec![pilot],
        };
        contents.apply(library, vec![store]).await.unwrap();
        let item = contents.files(library).await.unwrap()[0].items[0].id;
        let wanted = Wanted {
            tmdb_id: DARK,
            season: Some(1),
            title: "Dark".to_owned(),
            release_date: None,
            show_ids: ExternalIds::default(),
            followed_show: None,
        };
        let request = db
            .write(move |tx| Box::pin(ask_each(tx, vec![wanted], RequestState::Pending)))
            .await
            .unwrap()[0];
        let scan = |episode_2_airs| Decisions {
            items: vec![(item, matched_to_show(DARK))],
            seasons: vec![(DARK, season_1(episode_2_airs))],
        };
        contents.record(scan("2099-12-01")).await.unwrap();

        let episode_2_aired = contents.record(scan("2017-12-08")).await.unwrap();

        let state = db
            .write(move |tx| Box::pin(load_one(tx, request)))
            .await
            .unwrap()
            .unwrap()
            .state;
        assert_eq!(state.as_known(), Some(RequestState::Available));
        assert!(episode_2_aired.is_empty(), "{episode_2_aired:?}");
    }
}
