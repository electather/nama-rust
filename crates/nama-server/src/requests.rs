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
    cmp::Reverse,
    collections::{BTreeSet, HashSet},
    iter,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use nama_plugin_api::{
    PluginId,
    fulfiller::{Fulfilled, Fulfiller, Movies, Shows},
    metadata::{Candidate, Episode, ExternalId, ExternalIds, SearchQuery, Season, Show, TitleKind},
};
use nama_proto::nama::v1::{
    DefaultQuality, Event, FollowedShow, FollowedShowCancelled, FollowedShowCreated,
    FollowedShowRequesterAdded, FollowedShowRequesterRemoved, FollowedShowStateChanged,
    FollowedShowWithdrawn, FoundSeason, ListRequestsResponse, Permission, Quality, QualityChange,
    Request, RequestCancelled, RequestCreated, RequestState, RequestStateChanged, RequestWithdrawn,
    RequestedMovie, RequestedQuality, RequestedSeason, Requester, RequesterAdded, RequesterRemoved,
    request::MovieOrSeason,
};
use serde::{Deserialize, Serialize};
use sqlx::{Sqlite, SqliteConnection, Transaction};
use tokio::{sync::mpsc::UnboundedReceiver, task::JoinHandle};

use crate::{
    contents::{self, Contents, Metadata},
    db::{BoxFuture, Database, detached, now},
    events::{EventBus, event},
    metadata::{self, MetadataSource},
    plugins::{Answer, ExtensionPoints},
    qualities,
    users::User,
};

mod asked;
mod hand_off;
mod progress;

pub(crate) use asked::{Asked, may_see};
use hand_off::HandOff;
use progress::Progress;

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

/// Why a Request couldn't be made, found or changed.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    /// There is no such Request or Followed Show, or none the caller may see.
    #[error("there is no {0} {1}")]
    NoSuch(Asked, i64),
    #[error("the {0} {1} isn't Pending")]
    NotPending(Asked, i64),
    #[error("the Request {0} isn't Failed")]
    NotFailed(i64),
    #[error("you aren't a Requester of the {0} {1}")]
    NotARequester(Asked, i64),
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
    #[error(transparent)]
    Quality(#[from] qualities::Refused),
    #[error("the page token isn't one this Server gave for listing in this state")]
    PageTokenInvalid,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// The Server's Requests, which publishes what changes about them.
#[derive(Debug, Clone)]
pub(crate) struct Requests {
    db: Arc<Database>,
    contents: Contents,
    /// Looks up what a new Request asks for: a Movie, or a Show's Seasons.
    metadata: MetadataSource,
    /// What [`Self::apply`] publishes the changes its writes make on.
    events: EventBus,
    /// Where the Fulfillers are.
    plugins: ExtensionPoints,
    /// Its way to hand Requests to the Fulfillers.
    hand_off: HandOff,
    /// What the Fulfillers say they are downloading for the Requests they have.
    progress: Progress,
    /// The loop [`Requests::start`] spawns to hand off as Fulfillers come up.
    handing_off: Arc<Mutex<Option<JoinHandle<()>>>>,
    /// The loop [`Requests::start`] spawns to ask the Fulfillers about progress.
    following: Arc<Mutex<Option<JoinHandle<()>>>>,
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

/// A Movie, or a Season of a Show, as a Request names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct RequestTitle {
    /// The Movie's TMDB id, or the Show's.
    pub(crate) tmdb_id: i64,
    /// The Season's number, `None` for a Movie.
    pub(crate) season: Option<u32>,
}

/// A title a search found, which a Request can name by its TMDB id.
#[derive(Debug)]
pub(crate) struct Hit {
    pub(crate) tmdb_id: i64,
    /// The Plugin whose metadata Provider found it, which its image paths are for.
    pub(crate) plugin: PluginId,
    pub(crate) candidate: Candidate,
}

/// A Show looked up in the metadata source, and how close it is to being on the Server.
#[derive(Debug)]
pub(crate) struct FoundShow {
    pub(crate) tmdb_id: i64,
    /// The Plugin whose metadata Provider gave the Show, which its image paths are for.
    pub(crate) plugin: PluginId,
    pub(crate) show: Show,
    /// Its Seasons, in order, as the viewer sees them.
    pub(crate) seasons: Vec<FoundSeason>,
    /// Its open Followed Show, if it has one the viewer may see.
    pub(crate) followed_show: Option<FollowedShow>,
    /// Its Quality, which each of its Season Requests takes, if it has one.
    pub(crate) quality: Option<RequestedQuality>,
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
    /// For a Season, its Show's ids if its asker knows them; none for a Movie.
    show_ids: ExternalIds,
    /// The Followed Show a new Request for it is opened for, if any.
    followed_show: Option<i64>,
    /// What its asker's ask says about Quality.
    quality: qualities::Ask,
}

/// A User changing a Request's Quality: a Request manager as they approve or retry it, or an
/// auto-approved Requester whose ask approves it.
#[derive(Debug)]
struct NewQuality {
    quality: RequestedQuality,
    /// The User id of whoever changes it.
    by: i64,
}

/// Seasons of a Show to ask for, as the metadata source gave them.
#[derive(Debug)]
struct WantedSeasons {
    show_tmdb_id: i64,
    show_ids: ExternalIds,
    show_title: String,
    seasons: Vec<Season>,
    /// What the asker's ask says about Quality, which gives way to the Show's if it has one.
    quality: qualities::Ask,
}

/// What a write to Requests did: its `value`, the events it built for what happened, the Requests
/// it approved, which [`Requests::apply`] hands off, and the Season Request of a Followed Show it
/// ended, whose Show the Shows Fulfiller stops following.
#[derive(Debug)]
struct Applied<T> {
    value: T,
    events: Vec<Event>,
    hand_off: Vec<Request>,
    unfollow: Option<Request>,
}

impl<T> Applied<T> {
    /// `value`, from a write that changed nothing worth an event.
    fn quiet(value: T) -> Self {
        Self {
            value,
            events: Vec::new(),
            hand_off: Vec::new(),
            unfollow: None,
        }
    }

    /// `value`, from a write that `happened` says what it did.
    fn saying(value: T, happened: Event) -> Self {
        Self {
            events: vec![happened],
            ..Self::quiet(value)
        }
    }
}

impl Applied<Request> {
    /// `request`, which the write moved to the state it is in now.
    fn moved(request: Request) -> Self {
        Self::saying(request.clone(), state_changed(request))
    }

    /// `request`, which the write approved, so it is handed off.
    fn approved(request: Request) -> Self {
        Self {
            hand_off: vec![request.clone()],
            ..Self::moved(request)
        }
    }
}

/// A write's value that may hold Requests the write hands off, which [`Requests::apply`] swaps
/// for those Requests as the Hand-off left them.
trait HandedBack: Sized {
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

/// Whose Requests `viewer` may see: everyone's (`None`), or only those they are a Requester of.
pub(crate) fn visible_to(viewer: &User) -> Option<i64> {
    (!viewer.permissions.allow(Permission::ViewRequests)).then_some(viewer.id)
}

impl Requests {
    /// The Requests in `db`, looking titles up through the metadata Provider and handing Approved
    /// ones to the Fulfillers in `plugins`, publishing on `events`.
    pub(crate) fn new(db: Arc<Database>, events: EventBus, plugins: ExtensionPoints) -> Self {
        let progress = Progress::new(Arc::clone(&db), events.clone(), plugins.clone());
        Self {
            contents: Contents::new(Arc::clone(&db)),
            metadata: MetadataSource::new(plugins.clone()),
            hand_off: HandOff::new(
                Arc::clone(&db),
                plugins.clone(),
                progress.became_processing(),
            ),
            progress,
            events,
            plugins,
            db,
            handing_off: Arc::default(),
            following: Arc::default(),
        }
    }

    /// Starts, in the background so a slow Fulfiller doesn't hold up serving, handing each
    /// Approved Request, oldest first, to its Fulfiller whenever a Plugin `came_up` names provides
    /// one, and asking the Fulfillers how the Requests they have that aren't Available yet are
    /// progressing, every `progress_every` while there are any and at once when one becomes
    /// Processing. A failed hand-off is only logged, and leaves the rest waiting. Call it once.
    pub(crate) fn start(&self, came_up: UnboundedReceiver<PluginId>, progress_every: Duration) {
        let requests = self.clone();
        let handing_off =
            tokio::spawn(async move { requests.as_fulfillers_come_up(came_up).await });
        let asking = self.progress.clone();
        let following = tokio::spawn(async move { asking.follow(progress_every).await });
        *lock(&self.handing_off) = Some(handing_off);
        *lock(&self.following) = Some(following);
    }

    /// Hands off the waiting Requests whenever a Plugin `came_up` names provides a Fulfiller, as
    /// [`Self::start`] describes, until `came_up` closes.
    async fn as_fulfillers_come_up(&self, came_up: UnboundedReceiver<PluginId>) {
        self.hand_off.as_fulfillers_come_up(self, came_up).await;
    }

    /// Stops what [`Self::start`] started, logging a panic from either loop. Does nothing before
    /// it.
    ///
    /// The Requests not handed off yet stay Approved, so the next start hands them off. The one
    /// being handed off now runs on in a task of its own (`HandOff::request`), which nothing waits
    /// for: if the runtime stops first, a Request the Fulfiller took stays Approved and the next
    /// start hands it over again, as for an approval in flight (ADR 0029). Progress is kept in
    /// memory only, so stopping mid-poll loses nothing.
    pub(crate) async fn shutdown(&self) {
        stop(&self.handing_off, "handing the waiting Requests off").await;
        stop(&self.following, "asking the Fulfillers about progress").await;
    }

    /// Asks for the Movie with the TMDB id `tmdb_id` for `asker`, who joins its open Request, or
    /// opens one at the Quality `chosen`, else as [`Self::movie_quality`] picks: Approved, and handed
    /// to the Fulfiller for Movies, if `asker` may have Movies approved as they ask, else Pending.
    /// Such an `asker` joining a Pending Request approves it and hands it off the same way.
    /// Publishes what changed, if anything did.
    ///
    /// Refuses a `chosen` Quality from an `asker` who may not choose one, or that the Movie
    /// Fulfiller doesn't offer, even when joining.
    pub(crate) async fn create(
        &self,
        asker: &User,
        tmdb_id: i64,
        chosen: Option<i64>,
    ) -> Result<Request, Error> {
        // Refuses before asking TMDB, which may be down or slow; `ask` checks again for a scan
        // that Matched the Movie meanwhile.
        if self.contents.library_with_movie(tmdb_id).await?.is_some() {
            return Err(Error::InLibrary(tmdb_id));
        }
        let quality = self.quality::<Movies>(asker, chosen).await?;
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
            quality,
        };
        let state = opening_state(asker, Permission::AutoApproveMovie);
        let user = asker.id;
        self.apply(move |tx| Box::pin(ask_movie(tx, user, wanted, state)))
            .await
    }

    /// Runs `write`, publishes the events it built, has the Shows Fulfiller stop following the
    /// Show of a Followed Show it ended, then hands off the Requests it approved, and returns its
    /// value with those Requests as they are after the Hand-off. A failure to stop following is
    /// only logged: the Followed Show ended either way.
    ///
    /// The one path for Request changes (`AGENTS.md`, "Data").
    ///
    /// Runs in a task of its own, since the write commits even if the caller stops waiting, so
    /// what follows it must too (ADR 0029).
    async fn apply<T, W>(&self, write: W) -> Result<T, Error>
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
    async fn applied<T, W>(&self, write: W) -> Result<T, Error>
    where
        T: HandedBack + Send + 'static,
        W: for<'t> FnOnce(
                &'t mut Transaction<'_, Sqlite>,
            ) -> BoxFuture<'t, Result<Applied<T>, Error>>
            + Send
            + 'static,
    {
        let claiming = self.hand_off.clone();
        let (applied, claims) = self
            .db
            .write(move |tx| {
                Box::pin(async move {
                    let applied = write(tx).await?;
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

    /// Settles the Hand-off of the Request `id`, asked for at the Quality `asked`, with what its
    /// Fulfiller answered, through [`Self::apply`]'s path in the Hand-off's own task. Returns the
    /// Request as it is now, or `None` if it was changed meanwhile.
    async fn settle_hand_off(
        &self,
        id: i64,
        fulfilled: Answer<Fulfilled>,
        asked: Option<RequestedQuality>,
    ) -> Result<Option<Request>, Error> {
        // Not detached again: the Hand-off already is.
        self.applied(move |tx| Box::pin(settle(tx, id, fulfilled, asked)))
            .await
    }

    /// The enabled Plugin providing the Fulfiller of `Kind`, if any.
    fn fulfiller<Kind: 'static>(&self) -> Option<PluginId> {
        self.plugins.providers::<dyn Fulfiller<Kind>>().pop_first()
    }

    /// The enabled Plugin providing the Fulfiller for `request`'s kind, Movies or Shows, if any.
    fn fulfiller_of(&self, request: &Request) -> Option<PluginId> {
        match request.movie_or_season.as_ref()? {
            MovieOrSeason::Movie(_) => self.fulfiller::<Movies>(),
            MovieOrSeason::Season(_) => self.fulfiller::<Shows>(),
        }
    }

    /// What the ask of `asker`, naming the Quality `chosen` if they did, says about Quality on the
    /// enabled Fulfiller of `Kind`, as [`qualities::for_request`] decides.
    async fn quality<Kind: 'static>(
        &self,
        asker: &User,
        chosen: Option<i64>,
    ) -> Result<qualities::Ask, Error> {
        let fulfiller = self.fulfiller::<Kind>();
        let mut conn = self.db.reader().acquire().await?;
        Ok(qualities::for_request(&mut conn, asker, fulfiller.as_ref(), chosen).await??)
    }

    /// The Qualities of the enabled Fulfiller of `Kind`, in the order an admin listed them, with
    /// `asker`'s personal default among them if [`Self::quality`] would take it. Empty if no
    /// Fulfiller of `Kind` is enabled.
    pub(crate) async fn qualities<Kind: 'static>(
        &self,
        asker: &User,
    ) -> Result<(Vec<Quality>, Option<i64>), sqlx::Error> {
        let Some(fulfiller) = self.fulfiller::<Kind>() else {
            return Ok((Vec::new(), None));
        };
        let mut conn = self.db.reader().acquire().await?;
        qualities::offered_to(&mut conn, asker, &fulfiller).await
    }

    /// `user`'s default Quality for each enabled Fulfiller, as [`qualities::personal_defaults`]
    /// lists them.
    pub(crate) async fn personal_defaults(
        &self,
        user: i64,
    ) -> Result<Vec<DefaultQuality>, sqlx::Error> {
        let movies = self.fulfiller::<Movies>();
        let shows = self.fulfiller::<Shows>();
        let mut conn = self.db.reader().acquire().await?;
        qualities::personal_defaults(&mut conn, user, movies.as_ref(), shows.as_ref()).await
    }

    /// Sets `user`'s default Quality for the Fulfiller from the Plugin `plugin` to `quality`, or
    /// clears it. Refuses a Quality that isn't on its list.
    pub(crate) async fn set_personal_default(
        &self,
        user: i64,
        plugin: PluginId,
        quality: Option<i64>,
    ) -> Result<(), Error> {
        Ok(qualities::set_personal_default(&self.db, user, plugin, quality).await??)
    }

    /// The Qualities the enabled Fulfiller for titles of `kind` offers, none if there is no such
    /// Fulfiller.
    pub(crate) async fn offered_qualities(&self, kind: TitleKind) -> Result<Vec<Quality>, Error> {
        let fulfiller = match kind {
            TitleKind::Movie => self.fulfiller::<Movies>(),
            TitleKind::Show => self.fulfiller::<Shows>(),
        };
        let Some(fulfiller) = fulfiller else {
            return Ok(Vec::new());
        };
        let mut conn = self.db.reader().acquire().await?;
        Ok(qualities::list(&mut conn, &fulfiller).await?)
    }

    /// `manager` changing the Request `id`'s Quality to the one with the id `quality`, if given,
    /// which must be on the list of the Fulfiller the Request goes to.
    async fn new_quality(
        &self,
        manager: &User,
        id: i64,
        quality: Option<i64>,
    ) -> Result<Option<NewQuality>, Error> {
        let Some(quality_id) = quality else {
            return Ok(None);
        };
        let request = load_one(&mut *self.db.reader().acquire().await?, id)
            .await?
            .ok_or(Error::NoSuch(Asked::Request, id))?;
        let fulfiller = self.fulfiller_of(&request);
        self.offered_change(manager, fulfiller, quality_id)
            .await
            .map(Some)
    }

    /// `manager` changing a Quality to the one with the id `quality_id`, which must be on the list
    /// of the Fulfiller from the Plugin `fulfiller`.
    async fn offered_change(
        &self,
        manager: &User,
        fulfiller: Option<PluginId>,
        quality_id: i64,
    ) -> Result<NewQuality, Error> {
        let mut conn = self.db.reader().acquire().await?;
        let quality = qualities::offered(&mut conn, fulfiller.as_ref(), quality_id).await??;
        Ok(NewQuality {
            quality,
            by: manager.id,
        })
    }

    /// Asks for the Seasons `numbers` of the Show with the TMDB id `tmdb_id` for `asker`, one
    /// Request per Season in Season order, each as [`Self::create`] asks for a Movie but with
    /// `AUTO_APPROVE_TV` and the Fulfiller for Shows. Asks for none if a Library has any of them:
    /// every aired Episode Matched. New Requests take the Show's Quality if it has one, refusing
    /// another `chosen` one, else as [`Self::create`] picks one.
    pub(crate) async fn create_seasons(
        &self,
        asker: &User,
        tmdb_id: i64,
        numbers: &BTreeSet<u32>,
        chosen: Option<i64>,
    ) -> Result<Vec<Request>, Error> {
        let quality = self.quality::<Shows>(asker, chosen).await?;
        let id = ExternalId::Tmdb(tmdb_id);
        let (_, show) = self
            .metadata
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
                .metadata
                .season(&id, number)
                .await
                .map_err(|err| looking_up(err, no_such_season()))?;
            seasons.push(season);
        }
        let wanted = WantedSeasons {
            show_tmdb_id: tmdb_id,
            show_ids: show.ids,
            show_title: show.title,
            seasons,
            quality,
        };
        let state = opening_state(asker, Permission::AutoApproveTv);
        let user = asker.id;
        self.apply(move |tx| Box::pin(ask_seasons(tx, user, wanted, state)))
            .await
    }

    /// Asks for the whole Show with the TMDB id `tmdb_id` for `asker`: each current Season not in
    /// a Library as [`Self::create_seasons`] asks for it, at the Quality it picks, and later ones
    /// through the Show's open Followed Show, which `asker` joins or opens at that Quality, in the
    /// state their Season Requests open in. Returns the Followed Show and the Season Requests in
    /// Season order.
    ///
    /// Refuses a `chosen` Quality as [`Self::create_seasons`] does.
    pub(crate) async fn follow(
        &self,
        asker: &User,
        tmdb_id: i64,
        chosen: Option<i64>,
    ) -> Result<(FollowedShow, Vec<Request>), Error> {
        let quality = self.quality::<Shows>(asker, chosen).await?;
        let id = ExternalId::Tmdb(tmdb_id);
        let (_, show) = self
            .metadata
            .show(&id)
            .await
            .map_err(|err| looking_up(err, Error::NoSuchShow(tmdb_id)))?;
        let wanted = WantedSeasons {
            show_tmdb_id: tmdb_id,
            seasons: self.seasons(tmdb_id, &show).await?,
            show_ids: show.ids,
            show_title: show.title,
            quality,
        };
        let state = opening_state(asker, Permission::AutoApproveTv);
        let user = asker.id;
        self.apply(move |tx| Box::pin(follow(tx, user, wanted, state)))
            .await
    }

    /// Approves the Pending Followed Show `id` as `manager`, first changing its Show's Quality to
    /// the one with the id `quality` if given, and each Pending Season Request it opened, handing
    /// those to the Shows Fulfiller, publishing the Followed Show's change of state, then each
    /// Request's.
    pub(crate) async fn approve_followed(
        &self,
        manager: &User,
        id: i64,
        quality: Option<i64>,
    ) -> Result<FollowedShow, Error> {
        let new_quality = match quality {
            Some(quality) => {
                let fulfiller = self.fulfiller::<Shows>();
                Some(self.offered_change(manager, fulfiller, quality).await?)
            }
            None => None,
        };
        self.apply(move |tx| {
            Box::pin(decide_followed(
                tx,
                id,
                RequestState::Approved,
                None,
                new_quality,
            ))
        })
        .await
    }

    /// Declines the Pending Followed Show `id`, and each Pending Season Request it opened, as
    /// [`Self::decline`] declines a Request.
    pub(crate) async fn decline_followed(
        &self,
        id: i64,
        reason: &str,
    ) -> Result<FollowedShow, Error> {
        let reason = given_reason(reason);
        self.apply(move |tx| {
            Box::pin(decide_followed(
                tx,
                id,
                RequestState::Declined,
                reason,
                None,
            ))
        })
        .await
    }

    /// Removes the Followed Show `id`, whatever its state, leaving the Season Requests it opened,
    /// publishes that it was cancelled, and has the Shows Fulfiller stop following the Show.
    pub(crate) async fn cancel_followed(&self, id: i64) -> Result<(), Error> {
        self.apply(move |tx| Box::pin(cancel_followed(tx, id)))
            .await
    }

    /// Takes `requester` off the Followed Show `id`, removing it once nobody is left on it, as
    /// [`Self::cancel_followed`] does, and publishes which of the two happened.
    pub(crate) async fn withdraw_followed(&self, requester: &User, id: i64) -> Result<(), Error> {
        let user = requester.id;
        let visible = visible_to(requester);
        self.apply(move |tx| Box::pin(withdraw_followed(tx, user, visible, id)))
            .await
    }

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

    /// A page of at most `size` of the Requests and Followed Shows `viewer` may see, newest first:
    /// only those in `state` if given, each matched on its own. `token` is the page before's
    /// `next_page_token`, or empty for the first page.
    pub(crate) async fn list(
        &self,
        viewer: &User,
        state: Option<RequestState>,
        size: usize,
        token: &str,
    ) -> Result<ListRequestsResponse, Error> {
        let after = read_page_token(token, state)?;
        // ponytail: reads every Request and Followed Show, whoever `viewer` is, to list one page;
        // page and filter in the queries once a Server's Requests number in the thousands.
        let mut rows: Vec<Listed> = self
            .listed(viewer)
            .await?
            .into_iter()
            .filter(|row| state.is_none_or(|state| row.state() == state))
            .filter(|row| after.is_none_or(|after| row.place() < after))
            .collect();
        rows.sort_unstable_by_key(|row| Reverse(row.place()));
        let more = rows.len() > size;
        rows.truncate(size);
        let next_page_token = match rows.last() {
            Some(last) if more => page_token(state, last.place()),
            _ => String::new(),
        };
        let mut page = ListRequestsResponse {
            next_page_token,
            ..Default::default()
        };
        for row in rows {
            match row {
                Listed::Request(request) => page.requests.push(request),
                Listed::Followed(show) => page.followed_shows.push(show),
            }
        }
        Ok(page)
    }

    /// How many of the Requests and Followed Shows `viewer` may see are Pending, as many as
    /// [`Self::list`] shows them, counting a Followed Show apart from its Season Requests.
    pub(crate) async fn count_pending(&self, viewer: &User) -> Result<u32, sqlx::Error> {
        // ponytail: reads every Request and Followed Show to count the Pending ones, so the count
        // is what `list` shows; count in SQL once a Server's Requests number in the thousands.
        let rows = self.listed(viewer).await?;
        let pending = rows
            .iter()
            .filter(|row| row.state() == RequestState::Pending)
            .count();
        Ok(u32::try_from(pending).unwrap_or(u32::MAX))
    }

    /// Every Request and Followed Show `viewer` may see, unordered.
    async fn listed(&self, viewer: &User) -> Result<Vec<Listed>, sqlx::Error> {
        let mut conn = self.db.reader().acquire().await?;
        let requests = self.visible(&mut conn, None, viewer).await?;
        let followed = load_followed(&mut conn, None, visible_to(viewer)).await?;
        Ok(requests
            .into_iter()
            .map(Listed::Request)
            .chain(followed.into_iter().map(Listed::Followed))
            .collect())
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
        for Hit {
            tmdb_id,
            plugin,
            candidate,
        } in candidates
        {
            let movie = self
                .look_around(&mut conn, viewer, tmdb_id, plugin, candidate)
                .await?;
            found.push(movie);
        }
        Ok(found)
    }

    /// The Shows the metadata source finds titled like `title`, best fit first.
    pub(crate) async fn search_shows(&self, title: String) -> Result<Vec<Hit>, Error> {
        self.find(TitleKind::Show, title).await
    }

    /// The titles of `kind` the metadata source finds titled like `title`, best fit first.
    async fn find(&self, kind: TitleKind, title: String) -> Result<Vec<Hit>, Error> {
        let query = SearchQuery {
            kind,
            title,
            year: None,
        };
        let (plugin, candidates) = match self.metadata.search(&query).await {
            Ok(found) => found,
            Err(metadata::Error::NotFound) => return Ok(Vec::new()),
            Err(metadata::Error::Unavailable) => return Err(Error::Unavailable),
        };
        // Requests name titles by TMDB id, so a title without one can't be asked for.
        Ok(candidates
            .into_iter()
            .filter_map(|candidate| match candidate.id {
                ExternalId::Tmdb(tmdb_id) => Some(Hit {
                    tmdb_id,
                    plugin: plugin.clone(),
                    candidate,
                }),
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
    ) -> Result<FoundShow, Error> {
        let (plugin, show) = self
            .metadata
            .show(&ExternalId::Tmdb(tmdb_id))
            .await
            .map_err(|err| looking_up(err, Error::NoSuchShow(tmdb_id)))?;
        let seasons = self.seasons(tmdb_id, &show).await?;
        let mut conn = self.db.reader().acquire().await?;
        let seasons = self
            .look_around_seasons(&mut conn, viewer, tmdb_id, seasons)
            .await?;
        let followed_show = match open_followed_show(&mut conn, tmdb_id).await? {
            Some(id) => load_followed(&mut conn, Some(id), visible_to(viewer))
                .await?
                .pop(),
            None => None,
        };
        let quality = show_requests(&mut conn, tmdb_id).await?.quality().cloned();
        Ok(FoundShow {
            tmdb_id,
            plugin,
            show,
            seasons,
            followed_show,
            quality,
        })
    }

    /// Each of the Seasons `show`, the Show `tmdb_id`, lists, fetched now. One it lists but can't
    /// give fails the lookup as an unknown Show.
    async fn seasons(&self, tmdb_id: i64, show: &Show) -> Result<Vec<Season>, Error> {
        let id = ExternalId::Tmdb(tmdb_id);
        let mut seasons = Vec::with_capacity(show.seasons.len());
        // ponytail: one metadata lookup per Season, one after another.
        for &number in &show.seasons {
            let season = self.metadata.season(&id, number).await;
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
        self.look_around(&mut conn, viewer, tmdb_id, plugin, movie)
            .await
            .map_err(Error::from)
    }

    async fn fetch(&self, tmdb_id: i64) -> Result<Metadata, Error> {
        self.metadata
            .movie(&ExternalId::Tmdb(tmdb_id))
            .await
            .map_err(|err| looking_up(err, Error::NoSuchMovie(tmdb_id)))
    }

    /// The Request `id`, if `viewer` may see it.
    pub(crate) async fn get(&self, viewer: &User, id: i64) -> Result<Request, Error> {
        let mut conn = self.db.reader().acquire().await?;
        let found = self.visible(&mut conn, Some(id), viewer).await?;
        found
            .into_iter()
            .next()
            .ok_or(Error::NoSuch(Asked::Request, id))
    }

    /// `movie`, the Movie `tmdb_id` as `plugin`'s Provider gave it, with the Library that has it and
    /// its open Request, if `viewer` may see it.
    #[expect(
        clippy::too_many_arguments,
        reason = "`Found` takes the Movie as a search or lookup gives it, in three parts"
    )]
    async fn look_around<M>(
        &self,
        conn: &mut SqliteConnection,
        viewer: &User,
        tmdb_id: i64,
        plugin: PluginId,
        movie: M,
    ) -> Result<Found<M>, sqlx::Error> {
        let library = contents::library_with_movie(conn, tmdb_id).await?;
        let request = self
            .visible_open_request(conn, viewer, tmdb_id, None)
            .await?;
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
        &self,
        conn: &mut SqliteConnection,
        viewer: &User,
        show_tmdb_id: i64,
        seasons: Vec<Season>,
    ) -> Result<Vec<FoundSeason>, sqlx::Error> {
        let today = today(conn).await?;
        let mut found = Vec::with_capacity(seasons.len());
        // ponytail: two or three queries per Season, and a Show has a few dozen at most.
        for season in seasons {
            let matched =
                contents::matched_episodes(conn, show_tmdb_id, season.number.into()).await?;
            let request = self
                .visible_open_request(conn, viewer, show_tmdb_id, Some(season.number))
                .await?;
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
        &self,
        conn: &mut SqliteConnection,
        viewer: &User,
        tmdb_id: i64,
        season: Option<u32>,
    ) -> Result<Option<Request>, sqlx::Error> {
        let Some(id) = open_request(conn, tmdb_id, season).await? else {
            return Ok(None);
        };
        let visible = self.visible(conn, Some(id), viewer).await?;
        Ok(visible.into_iter().next())
    }

    /// The Requests `viewer` may see, newest first, only the one with the ID `id` if given, each
    /// with what its Fulfiller is downloading for it.
    ///
    /// The one read of a Request for a viewer, so none misses its downloads.
    async fn visible(
        &self,
        conn: &mut SqliteConnection,
        id: Option<i64>,
        viewer: &User,
    ) -> Result<Vec<Request>, sqlx::Error> {
        let requests = load(conn, id, visible_to(viewer)).await?;
        Ok(requests
            .into_iter()
            .map(|request| self.progress.attach(request))
            .collect())
    }
}

/// A row of [`Requests::list`].
#[expect(
    clippy::large_enum_variant,
    reason = "most rows are Requests, so boxing them would save nothing"
)]
enum Listed {
    Request(Request),
    Followed(FollowedShow),
}

impl Listed {
    fn state(&self) -> RequestState {
        match self {
            Self::Request(request) => request.state,
            Self::Followed(followed) => followed.state,
        }
        .as_known()
        .unwrap_or_default()
    }

    fn place(&self) -> Place {
        match self {
            Self::Request(request) => Place {
                created_at: request.created_at,
                is_request: true,
                id: request.id,
            },
            Self::Followed(followed) => Place {
                created_at: followed.created_at,
                is_request: false,
                id: followed.id,
            },
        }
    }
}

/// Aborts the loop `task` holds, if any, and waits for it, logging that `doing` panicked if it did.
async fn stop(task: &Mutex<Option<JoinHandle<()>>>, doing: &str) {
    let Some(handle) = lock(task).take() else {
        return;
    };
    handle.abort();
    if let Err(err) = handle.await
        && err.is_panic()
    {
        tracing::error!("{doing} panicked: {err}");
    }
}

/// Where a row sits in [`Requests::list`]'s order, newer sorting greater. A Request sorts as
/// newer than a Followed Show opened in the same second, so it lists first.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct Place {
    created_at: i64,
    is_request: bool,
    id: i64,
}

/// The token for the page after the one ending at `last`, tied to the filter `state`. Encoded so
/// clients treat it as opaque and don't build their own.
#[expect(
    clippy::expect_used,
    reason = "a string and plain numbers always serialize to JSON"
)]
fn page_token(state: Option<RequestState>, last: Place) -> String {
    URL_SAFE_NO_PAD.encode(serde_json::to_vec(&(state.map(stored), last)).expect("serializes"))
}

/// Where the page `token` continues from, or `None` for the first page.
fn read_page_token(token: &str, state: Option<RequestState>) -> Result<Option<Place>, Error> {
    if token.is_empty() {
        return Ok(None);
    }
    let json = URL_SAFE_NO_PAD.decode(token).unwrap_or_default();
    match serde_json::from_slice::<(Option<String>, Place)>(&json) {
        Ok((filter, last)) if filter.as_deref() == state.map(stored) => Ok(Some(last)),
        _ => Err(Error::PageTokenInvalid),
    }
}

/// `err` from looking a title up, which is `missing` if the metadata source doesn't have it.
fn looking_up(err: metadata::Error, missing: Error) -> Error {
    match err {
        metadata::Error::NotFound => missing,
        metadata::Error::Unavailable => Error::Unavailable,
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

/// Today as `YYYY-MM-DD`, by SQLite's calendar, since `std` has none.
async fn today(conn: &mut SqliteConnection) -> Result<String, sqlx::Error> {
    sqlx::query_scalar!(r#"SELECT date('now') AS "today!: String""#)
        .fetch_one(&mut *conn)
        .await
}

/// [`ask_approving`]s for the Movie `wanted`, unless a Library has it.
async fn ask_movie(
    conn: &mut SqliteConnection,
    user: i64,
    wanted: Wanted,
    state: RequestState,
) -> Result<Applied<Request>, Error> {
    if contents::library_with_movie(conn, wanted.tmdb_id)
        .await?
        .is_some()
    {
        return Err(Error::InLibrary(wanted.tmdb_id));
    }
    ask_approving(conn, user, wanted, state).await
}

/// Asks for each of the Seasons `show` as `user`, whose new Requests open in `state`, unless a
/// Library has any of them. Returns the Requests in the order of `show.seasons`.
async fn ask_seasons(
    conn: &mut SqliteConnection,
    user: i64,
    show: WantedSeasons,
    state: RequestState,
) -> Result<Applied<Vec<Request>>, Error> {
    let asking = SeasonsToAsk::read(conn, show).await?;
    asking.ask(conn, Way::Asking { user, state }).await
}

/// How Seasons of a Show are asked for, which decides what is done with each:
///
/// | Library has | asking | following | a scan |
/// |---|---|---|---|
/// | all of it | refuse | skip | open as Available |
/// | some of it | open as asked | open as asked | open as Partially Available |
/// | none of it | open as asked | open as asked | skip |
///
/// A User asks a [`Way`]; a Library's write asks [`Scanning`].
trait Asker {
    /// How asking this way can fail.
    type Error: From<sqlx::Error>;

    /// The Followed Show new Requests open for, if any.
    fn followed_show(&self) -> Option<&FollowedShow>;

    /// The state a new Request for the Season `season` of the Show `show` opens in, given how
    /// much of it a Library `has`, or `None` if this way skips it. Refuses a Season a Library has
    /// in full if this way refuses one.
    async fn opens_in(
        &self,
        conn: &mut SqliteConnection,
        show: i64,
        season: u32,
        has: Option<RequestState>,
    ) -> Result<Option<RequestState>, Self::Error>;

    /// Opens or joins the Request for `wanted` this way, a new one opening in `opens_in`.
    async fn ask(
        &self,
        conn: &mut SqliteConnection,
        wanted: Wanted,
        opens_in: RequestState,
    ) -> Result<Applied<Request>, Self::Error>;

    /// Records that the Followed Show this way asks for covers the Season `season` from then on,
    /// so one it opened or joined and someone declined, cancelled or withdrew from isn't opened
    /// again.
    async fn cover(&self, conn: &mut SqliteConnection, season: u32) -> Result<(), sqlx::Error> {
        if let Some(followed_show) = self.followed_show() {
            // Joining may ask for a Season before the latest one covered, which stays covered.
            sqlx::query!(
                "UPDATE followed_shows SET latest_season = ?
                 WHERE id = ? AND (latest_season IS NULL OR latest_season < ?)",
                season,
                followed_show.id,
                season
            )
            .execute(&mut *conn)
            .await?;
        }
        Ok(())
    }
}

/// A User asking for Seasons of a Show, which can be refused.
#[derive(Debug, Clone, Copy)]
enum Way<'a> {
    /// `user` asking for Seasons on their own, apart from the Show's Followed Show. Their new
    /// Requests open in `state`.
    Asking { user: i64, state: RequestState },
    /// `user`, whose new Requests would open in `state`, following the Show through
    /// `followed_show`, which new Requests open for and in the state of.
    Following {
        user: i64,
        state: RequestState,
        followed_show: &'a FollowedShow,
    },
}

impl Asker for Way<'_> {
    type Error = Error;

    fn followed_show(&self) -> Option<&FollowedShow> {
        match *self {
            Self::Asking { .. } => None,
            Self::Following { followed_show, .. } => Some(followed_show),
        }
    }

    #[expect(
        clippy::unused_async_trait_impl,
        reason = "a User's way decides without the database, which a scan reads"
    )]
    async fn opens_in(
        &self,
        _: &mut SqliteConnection,
        show: i64,
        season: u32,
        has: Option<RequestState>,
    ) -> Result<Option<RequestState>, Error> {
        let in_full = has == Some(RequestState::Available);
        match *self {
            Self::Asking { .. } if in_full => Err(Error::SeasonInLibrary { show, season }),
            Self::Asking { state, .. } => Ok(Some(state)),
            Self::Following { .. } if in_full => Ok(None),
            // An Approved Followed Show was approved once for every Season, so needs no approving
            // again.
            Self::Following {
                state,
                followed_show,
                ..
            } => Ok(Some(followed_show.state.as_known().unwrap_or(state))),
        }
    }

    async fn ask(
        &self,
        conn: &mut SqliteConnection,
        wanted: Wanted,
        opens_in: RequestState,
    ) -> Result<Applied<Request>, Error> {
        let (Self::Asking { user, state } | Self::Following { user, state, .. }) = *self;
        // Only `user`'s own auto-approve approves a Pending Season Request they join, whichever
        // way they ask: the state of a Followed Show they join doesn't.
        if state == RequestState::Approved {
            ask_approving(conn, user, wanted, opens_in).await
        } else {
            Ok(ask(conn, user, wanted, opens_in).await?)
        }
    }
}

/// A Library's write, which has no Request open for the Seasons, asking for them for the
/// Requesters of the Show's Approved Followed Show, `followed_show`, in the order they asked.
/// Never hands off, as new Requests open already in a Library, and is never refused.
#[derive(Debug)]
struct Scanning {
    followed_show: FollowedShow,
}

impl Asker for Scanning {
    type Error = sqlx::Error;

    fn followed_show(&self) -> Option<&FollowedShow> {
        Some(&self.followed_show)
    }

    async fn opens_in(
        &self,
        conn: &mut SqliteConnection,
        show: i64,
        season: u32,
        has: Option<RequestState>,
    ) -> Result<Option<RequestState>, sqlx::Error> {
        let skips = has.is_none()
            || covers(conn, self.followed_show.id, season).await?
            || was_available(conn, show, season).await?;
        Ok(if skips { None } else { has })
    }

    async fn ask(
        &self,
        conn: &mut SqliteConnection,
        wanted: Wanted,
        opens_in: RequestState,
    ) -> Result<Applied<Request>, sqlx::Error> {
        let requesters = &self.followed_show.requesters;
        ask_as(conn, requesters, wanted, opens_in, now()).await
    }
}

/// Seasons of a Show to ask for, at the Quality the Show's Quality rules give their new Requests.
#[derive(Debug)]
struct SeasonsToAsk {
    show_tmdb_id: i64,
    /// The Show's ids, or `None` for a scan.
    // ponytail: a Followed Show stores no Show ids, and a Season a scan asks for opens already in a
    // Library.
    show_ids: Option<ExternalIds>,
    show_title: String,
    seasons: Vec<Season>,
    quality: qualities::Ask,
}

impl SeasonsToAsk {
    /// `show`'s Seasons at the Show's one Quality, refusing another Quality the asker named.
    async fn read(conn: &mut SqliteConnection, show: WantedSeasons) -> Result<Self, Error> {
        // Read in the write, so two asks for the Show's Seasons at once can't take two Qualities
        // (ADR 0025).
        let quality = show_requests(conn, show.show_tmdb_id)
            .await?
            .for_seasons(show.quality)?;
        Ok(Self {
            show_tmdb_id: show.show_tmdb_id,
            show_ids: Some(show.show_ids),
            show_title: show.show_title,
            seasons: show.seasons,
            quality,
        })
    }

    /// The Season `season` of the Show `followed_show` follows, at the Show's Quality, as a scan
    /// asks for it. A scan names no Quality, so unlike [`Self::read`] it refuses none.
    async fn for_scan(
        conn: &mut SqliteConnection,
        followed_show: &FollowedShow,
        season: Season,
    ) -> Result<Self, sqlx::Error> {
        let show_tmdb_id = followed_show.show_tmdb_id;
        let quality = show_requests(conn, show_tmdb_id).await?.for_scan();
        Ok(Self {
            show_tmdb_id,
            show_ids: None,
            show_title: followed_show.show_title.clone(),
            seasons: vec![season],
            quality,
        })
    }

    /// Opens or joins a Request for each Season, the way `asking` asks for it. Returns the
    /// Requests in Season order.
    async fn ask<A: Asker>(
        self,
        conn: &mut SqliteConnection,
        asking: A,
    ) -> Result<Applied<Vec<Request>>, A::Error> {
        let show = self.show_tmdb_id;
        let today = today(conn).await?;
        let mut asked = Applied::quiet(Vec::with_capacity(self.seasons.len()));
        for season in &self.seasons {
            let matched = contents::matched_episodes(conn, show, season.number.into()).await?;
            let has = availability(&season.episodes, &matched, &today);
            let Some(opens_in) = asking.opens_in(conn, show, season.number, has).await? else {
                continue;
            };
            let wanted = self.wanted(season, asking.followed_show().map(|followed| followed.id));
            let one = asking.ask(conn, wanted, opens_in).await?;
            asking.cover(conn, season.number).await?;
            asked.value.push(one.value);
            asked.events.extend(one.events);
            asked.hand_off.extend(one.hand_off);
        }
        Ok(asked)
    }

    /// What a new Request for `season` stores, opened for `followed_show` if given.
    fn wanted(&self, season: &Season, followed_show: Option<i64>) -> Wanted {
        Wanted {
            tmdb_id: self.show_tmdb_id,
            season: Some(season.number),
            title: self.show_title.clone(),
            release_date: season.air_date.clone(),
            show_ids: self.show_ids.clone().unwrap_or_default(),
            followed_show,
            quality: self.quality.clone(),
        }
    }
}

/// [`ask`]s for `wanted`, then approves the open Request `user` joined if it is Pending and `state`
/// is Approved, since their ask would have opened it Approved, at the Quality
/// [`qualities::Ask::joiner_approves_at`] gives.
async fn ask_approving(
    conn: &mut SqliteConnection,
    user: i64,
    wanted: Wanted,
    state: RequestState,
) -> Result<Applied<Request>, Error> {
    let new_quality = wanted
        .quality
        .joiner_approves_at()
        .map(|quality| NewQuality { quality, by: user });
    let asked = ask(conn, user, wanted, state).await?;
    if state != RequestState::Approved || asked.value.state != RequestState::Pending {
        return Ok(asked);
    }
    let approved = approve(conn, asked.value.id, new_quality).await?;
    Ok(Applied {
        events: [asked.events, approved.events].concat(),
        ..approved
    })
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
    let (quality_id, quality_name, quality_profile_id) = quality_columns(wanted.quality.taken());
    sqlx::query_scalar!(
        "INSERT INTO requests (tmdb_id, season, title, release_date, show_imdb_id, show_tvdb_id,
                               state, created_at, followed_show_id, quality_id, quality_name,
                               quality_profile_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         RETURNING id",
        wanted.tmdb_id,
        wanted.season,
        wanted.title,
        wanted.release_date,
        wanted.show_ids.imdb,
        wanted.show_ids.tvdb,
        state,
        now,
        wanted.followed_show,
        quality_id,
        quality_name,
        quality_profile_id
    )
    .fetch_one(&mut *conn)
    .await
}

/// [`ask_as`] for `user`, asking now.
async fn ask(
    conn: &mut SqliteConnection,
    user: i64,
    wanted: Wanted,
    state: RequestState,
) -> Result<Applied<Request>, sqlx::Error> {
    let now = now();
    let asker = Requester {
        user_id: user,
        requested_at: now,
        ..Default::default()
    };
    ask_as(conn, &[asker], wanted, state, now).await
}

/// Adds each of `requesters`, in order and as of when they asked, as a Requester of the open
/// Request for `wanted`, opening one in `state` at `now` if there is none, which is handed off if
/// that is Approved. Joining one hands nothing off, so a Request isn't handed off twice.
async fn ask_as(
    conn: &mut SqliteConnection,
    requesters: &[Requester],
    wanted: Wanted,
    state: RequestState,
    now: i64,
) -> Result<Applied<Request>, sqlx::Error> {
    let open = open_request(conn, wanted.tmdb_id, wanted.season).await?;
    let opened = open.is_none();
    let id = match open {
        Some(id) => id,
        None => insert_request(conn, &wanted, stored(state), now).await?,
    };
    let mut joined = Vec::new();
    for requester in requesters {
        let added = asked::join(
            conn,
            Asked::Request,
            id,
            requester.user_id,
            requester.requested_at,
        )
        .await?;
        if added {
            joined.push(requester.user_id);
        }
    }
    let request = load_one(conn, id).await?.ok_or(sqlx::Error::RowNotFound)?;
    if opened {
        let created = event(RequestCreated {
            request: request.clone().into(),
            ..Default::default()
        });
        let hand_off = Vec::from_iter((state == RequestState::Approved).then(|| request.clone()));
        return Ok(Applied {
            hand_off,
            ..Applied::saying(request, created)
        });
    }
    let added = joined.into_iter().map(|user| {
        event(RequesterAdded {
            request: request.clone().into(),
            user_id: user,
            ..Default::default()
        })
    });
    Ok(Applied {
        events: added.collect(),
        ..Applied::quiet(request)
    })
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
/// their Show's TMDB id, Partially Available (if Processing) or Available by how many of its
/// aired Episodes a Library has, and returns an event for each Request that changed (ADR 0012).
/// Declined and Available Requests are closed, so Episodes that air later don't reopen them. A
/// Season with no open Request is asked for by a scan, as [`Scanning`] says.
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
        match open_request(conn, show_tmdb_id, Some(season.number)).await? {
            Some(id) => happened.extend(change_state(conn, id, state).await?),
            None => happened.extend(scan_season(conn, show_tmdb_id, season).await?),
        }
    }
    Ok(happened)
}

/// Asks for `season` of the Show `show_tmdb_id`, which a Library's write found no open Request
/// for, by a scan. Returns the events for it.
async fn scan_season(
    conn: &mut SqliteConnection,
    show_tmdb_id: i64,
    season: Season,
) -> Result<Vec<Event>, sqlx::Error> {
    // A Show with no Approved Followed Show has no one a scan could ask for.
    let Some(followed_show) = approved_followed_show(conn, show_tmdb_id).await? else {
        return Ok(Vec::new());
    };
    let asking = SeasonsToAsk::for_scan(conn, &followed_show, season).await?;
    let scanned = asking.ask(conn, Scanning { followed_show }).await?;
    Ok(scanned.events)
}

async fn approved_followed_show(
    conn: &mut SqliteConnection,
    tmdb_id: i64,
) -> Result<Option<FollowedShow>, sqlx::Error> {
    let id = sqlx::query_scalar!(
        "SELECT id FROM followed_shows WHERE tmdb_id = ? AND state = 'approved'",
        tmdb_id
    )
    .fetch_optional(&mut *conn)
    .await?;
    let Some(id) = id else {
        return Ok(None);
    };
    Ok(load_followed(conn, Some(id), None).await?.pop())
}

/// Whether the Followed Show `id` covers the Season `season`: it is numbered no later than the
/// latest one the Followed Show asked for.
async fn covers(conn: &mut SqliteConnection, id: i64, season: u32) -> Result<bool, sqlx::Error> {
    // ponytail: a Season that came out but was skipped, such as a Season 0 added later, gets no
    // Request from a scan; ask for it on its own.
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM followed_shows WHERE id = ? AND latest_season >= ?)
           AS "is!: bool""#,
        id,
        season
    )
    .fetch_one(&mut *conn)
    .await
}

/// Whether the Season `season` of the Show `show_tmdb_id` ever had an Available Request, so its
/// Followed Show doesn't ask for it again.
async fn was_available(
    conn: &mut SqliteConnection,
    show_tmdb_id: i64,
    season: u32,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM requests
                          WHERE tmdb_id = ? AND season = ? AND state = 'available') AS "is!: bool""#,
        show_tmdb_id,
        season
    )
    .fetch_one(&mut *conn)
    .await
}

/// Moves the Request `id` to `state`, Available or Partially Available, and returns the event for
/// it, or `None` if it didn't move. Only a Request its Fulfiller has becomes Partially Available:
/// one not yet handed off keeps its state, so it can still be decided, handed off or retried
/// (ADR 0012).
async fn change_state(
    conn: &mut SqliteConnection,
    id: i64,
    state: RequestState,
) -> Result<Option<Event>, sqlx::Error> {
    let state = stored(state);
    macro_rules! change {
        ($fulfilling:literal) => {
            sqlx::query!(
                "UPDATE requests SET state = ?1, failure_reason = NULL
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

/// Whether a Request in `state` is open: Declined and Available ones are closed, and Cancelled or
/// Withdrawn ones are gone.
pub(crate) fn is_open(state: RequestState) -> bool {
    !matches!(state, RequestState::Declined | RequestState::Available)
}

/// The open Request for the Movie `tmdb_id`, or for its Season `season` if `tmdb_id` is a Show's.
async fn open_request(
    conn: &mut SqliteConnection,
    tmdb_id: i64,
    season: Option<u32>,
) -> Result<Option<i64>, sqlx::Error> {
    macro_rules! find {
        ($open:literal) => {
            sqlx::query_scalar!(
                "SELECT id FROM requests WHERE tmdb_id = ? AND season IS ? AND " + $open,
                tmdb_id,
                season
            )
        };
    }
    open_request_sql!(find).fetch_optional(&mut *conn).await
}

/// Moves the Pending Request `id` to `state` with `reason`, and returns it as it is now.
async fn decide(
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
async fn approve(
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
async fn decline(
    conn: &mut SqliteConnection,
    id: i64,
    reason: Option<String>,
) -> Result<Applied<Request>, Error> {
    let declined = decide(conn, id, RequestState::Declined, reason).await?;
    Ok(Applied::moved(declined))
}

/// Changes the Request `id`'s Quality to `new_quality`, if given, as [`requality`] does.
async fn change_quality(
    conn: &mut SqliteConnection,
    id: i64,
    new_quality: Option<NewQuality>,
) -> Result<(), Error> {
    let Some(new_quality) = new_quality else {
        return Ok(());
    };
    let request = load_one(conn, id)
        .await?
        .ok_or(Error::NoSuch(Asked::Request, id))?;
    let show = match &request.movie_or_season {
        Some(MovieOrSeason::Season(season)) => Some(season.show_tmdb_id),
        _ => None,
    };
    requality(conn, Some(id), show, new_quality).await
}

/// Changes the Quality of the Followed Show `id`'s Show to `new_quality`, if given, as
/// [`requality`] does.
async fn change_followed_quality(
    conn: &mut SqliteConnection,
    id: i64,
    new_quality: Option<NewQuality>,
) -> Result<(), Error> {
    let Some(new_quality) = new_quality else {
        return Ok(());
    };
    let show = sqlx::query_scalar!("SELECT tmdb_id FROM followed_shows WHERE id = ?", id)
        .fetch_optional(&mut *conn)
        .await?
        .ok_or(Error::NoSuch(Asked::FollowedShow, id))?;
    requality(conn, None, Some(show), new_quality).await
}

/// Changes the Quality of the Request `id`, if given, and of the Show `show`, if given, to
/// `quality`, where it isn't that one, keeping each change as that Request's latest in place of
/// any earlier one. What the Show's change moves, or why it is refused, is
/// [`qualities::ShowRequests::change`]'s to say.
async fn requality(
    conn: &mut SqliteConnection,
    id: Option<i64>,
    show: Option<i64>,
    NewQuality { quality, by }: NewQuality,
) -> Result<(), Error> {
    let (seasons, followed) = match show {
        Some(show) => {
            let moves = show_requests(conn, show).await?.change(&quality)?;
            (moves.seasons, moves.followed)
        }
        None => (Vec::new(), false),
    };
    let at = now();
    // ponytail: the Show's other Season Requests change without an event, so a client sees it on
    // its next read; publish one for each if a screen shows them live.
    for request in id.into_iter().chain(seasons) {
        // SQLite reads `quality_name` on the right as it was before this UPDATE.
        sqlx::query!(
            "UPDATE requests
             SET quality_id = ?1, quality_name = ?2, quality_profile_id = ?3,
                 quality_changed_by = ?4, quality_changed_from = quality_name,
                 quality_changed_at = ?5
             WHERE id = ?6 AND quality_id IS NOT ?1",
            quality.id,
            quality.name,
            quality.profile_id,
            by,
            at,
            request
        )
        .execute(&mut *conn)
        .await?;
    }
    if followed {
        macro_rules! change {
            ($open:literal) => {
                sqlx::query!(
                    "UPDATE followed_shows
                     SET quality_id = ?, quality_name = ?, quality_profile_id = ?
                     WHERE tmdb_id = ? AND "
                        + $open,
                    quality.id,
                    quality.name,
                    quality.profile_id,
                    show
                )
            };
        }
        open_followed_show_sql!(change).execute(&mut *conn).await?;
    }
    Ok(())
}

/// The Season Requests, in the order they were made, and the open Followed Show of the Show
/// `show_tmdb_id`, for its Quality rules.
async fn show_requests(
    conn: &mut SqliteConnection,
    show_tmdb_id: i64,
) -> Result<qualities::ShowRequests, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT id AS "id!", state, fulfiller_reference IS NOT NULL AS "handed_off!: bool",
                  quality_id, quality_name, quality_profile_id,
                  quality_used_id, quality_used_name, quality_used_profile_id
           FROM requests WHERE tmdb_id = ? AND season IS NOT NULL
           ORDER BY created_at, id"#,
        show_tmdb_id
    )
    .fetch_all(&mut *conn)
    .await?;
    let seasons = rows
        .into_iter()
        .map(|row| qualities::SeasonRequest {
            id: row.id,
            asked: stored_quality(row.quality_id, row.quality_name, row.quality_profile_id),
            used: stored_quality(
                row.quality_used_id,
                row.quality_used_name,
                row.quality_used_profile_id,
            ),
            state: read_state(&row.state),
            handed_off: row.handed_off,
        })
        .collect();
    macro_rules! followed {
        ($open:literal) => {
            sqlx::query!(
                "SELECT quality_id, quality_name, quality_profile_id FROM followed_shows
                 WHERE tmdb_id = ? AND "
                    + $open,
                show_tmdb_id
            )
        };
    }
    let followed = open_followed_show_sql!(followed)
        .fetch_optional(&mut *conn)
        .await?;
    Ok(qualities::ShowRequests {
        seasons,
        followed: followed.is_some(),
        followed_quality: followed.and_then(|row| {
            stored_quality(row.quality_id, row.quality_name, row.quality_profile_id)
        }),
    })
}

/// Moves the Approved Request `id`, asked at the Quality `asked`, on as its Fulfiller said: to
/// Processing, keeping its reference and the Quality used, or to Failed, keeping why. Either way
/// it keeps the Plugin that answered. Returns it as it is now, or `None` if it isn't Approved any
/// more.
async fn settle(
    conn: &mut SqliteConnection,
    id: i64,
    answered: Answer<Fulfilled>,
    asked: Option<RequestedQuality>,
) -> Result<Applied<Option<Request>>, Error> {
    let Answer { plugin, result } = answered;
    // In the same write as the reference, so a failed read can't lose a Request the Fulfiller took.
    let (state, reference, used, reason) = match result {
        Ok(fulfilled) => {
            let listed = qualities::list(conn, &plugin).await?;
            let used = qualities::used(asked.as_ref(), &listed, &fulfilled);
            (
                RequestState::Processing,
                Some(fulfilled.reference),
                used,
                None,
            )
        }
        Err(failure) => (RequestState::Failed, None, None, Some(failure.message)),
    };
    let state = stored(state);
    let (used_id, used_name, used_profile_id) = quality_columns(used.as_ref());
    let plugin = plugin.as_str();
    let settled = sqlx::query!(
        "UPDATE requests SET state = ?, fulfiller_reference = ?, failure_reason = ?,
                             fulfiller_plugin_id = ?,
                             quality_used_id = ?, quality_used_name = ?, quality_used_profile_id = ?
         WHERE id = ? AND state = 'approved'",
        state,
        reference,
        reason,
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
async fn cancel(conn: &mut SqliteConnection, id: i64) -> Result<Applied<()>, Error> {
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

/// Adds `user` as a Requester of the open Followed Show for `show`, opening one in `state` if there
/// is none, then asks for each of its Seasons not in a Library by following it. Returns the
/// Followed Show and the Season Requests, with the Followed Show's event first.
async fn follow(
    conn: &mut SqliteConnection,
    user: i64,
    show: WantedSeasons,
    state: RequestState,
) -> Result<Applied<(FollowedShow, Vec<Request>)>, Error> {
    let now = now();
    let asking = SeasonsToAsk::read(conn, show).await?;
    let already_open = open_followed_show(conn, asking.show_tmdb_id).await?;
    let id = if let Some(id) = already_open {
        id
    } else {
        open_followed(conn, &asking, state, now).await?
    };
    let joined = asked::join(conn, Asked::FollowedShow, id, user, now).await?;
    let followed = load_followed(conn, Some(id), None).await?.pop();
    let followed = followed.ok_or(sqlx::Error::RowNotFound)?;
    let mut happened = Vec::new();
    if already_open.is_none() {
        happened.push(event(FollowedShowCreated {
            followed_show: followed.clone().into(),
            ..Default::default()
        }));
    } else if joined {
        happened.push(event(FollowedShowRequesterAdded {
            followed_show: followed.clone().into(),
            user_id: user,
            ..Default::default()
        }));
    }
    let way = Way::Following {
        user,
        state,
        followed_show: &followed,
    };
    let seasons = asking.ask(conn, way).await?;
    happened.extend(seasons.events);
    Ok(Applied {
        value: (followed, seasons.value),
        events: happened,
        hand_off: seasons.hand_off,
        unfollow: None,
    })
}

/// Opens a Followed Show of `show` at the Quality its Seasons take, in `state`, returning its id.
async fn open_followed(
    conn: &mut SqliteConnection,
    show: &SeasonsToAsk,
    state: RequestState,
    now: i64,
) -> Result<i64, sqlx::Error> {
    let (quality_id, quality_name, quality_profile_id) = quality_columns(show.quality.taken());
    sqlx::query_scalar!(
        "INSERT INTO followed_shows (tmdb_id, title, state, latest_season, created_at, quality_id,
                                     quality_name, quality_profile_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)
         RETURNING id",
        show.show_tmdb_id,
        show.show_title,
        stored(state),
        show.seasons.iter().map(|season| season.number).max(),
        now,
        quality_id,
        quality_name,
        quality_profile_id
    )
    .fetch_one(&mut *conn)
    .await
}

/// The open Followed Show of the Show `tmdb_id`.
async fn open_followed_show(
    conn: &mut SqliteConnection,
    tmdb_id: i64,
) -> Result<Option<i64>, sqlx::Error> {
    macro_rules! find {
        ($open:literal) => {
            sqlx::query_scalar!(
                "SELECT id FROM followed_shows WHERE tmdb_id = ? AND " + $open,
                tmdb_id
            )
        };
    }
    open_followed_show_sql!(find)
        .fetch_optional(&mut *conn)
        .await
}

/// Moves the Pending Followed Show `id`, and each Pending Season Request it opened, to `state`
/// with `reason`, first changing its Show's Quality to `new_quality` if given, and names those
/// Requests for a Hand-off if it approved them. Returns the Followed Show as it is now, with its
/// event first.
async fn decide_followed(
    conn: &mut SqliteConnection,
    id: i64,
    state: RequestState,
    reason: Option<String>,
    new_quality: Option<NewQuality>,
) -> Result<Applied<FollowedShow>, Error> {
    // Undone with the rest if the Followed Show can't be decided.
    change_followed_quality(conn, id, new_quality).await?;
    let decided = asked::decide(conn, Asked::FollowedShow, id, state, reason.as_deref()).await?;
    let followed = load_followed(conn, Some(id), None).await?.pop();
    let followed = followed.ok_or(Error::NoSuch(Asked::FollowedShow, id))?;
    if !decided {
        return Err(Error::NotPending(Asked::FollowedShow, id));
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
    let followed_changed = event(FollowedShowStateChanged {
        followed_show: followed.clone().into(),
        ..Default::default()
    });
    let seasons_changed = requests.iter().cloned().map(state_changed);
    let events = iter::once(followed_changed)
        .chain(seasons_changed)
        .collect();
    let hand_off = if state == RequestState::Approved {
        requests
    } else {
        Vec::new()
    };
    Ok(Applied {
        value: followed,
        events,
        hand_off,
        unfollow: None,
    })
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

/// Removes the Followed Show `id`, and its Requesters with it, naming its [`followed_season`], if
/// any, as the one whose Show to stop following.
async fn cancel_followed(conn: &mut SqliteConnection, id: i64) -> Result<Applied<()>, Error> {
    let followed = load_followed(conn, Some(id), None).await?.pop();
    let followed = followed.ok_or(Error::NoSuch(Asked::FollowedShow, id))?;
    let followed_season = followed_season(conn, id).await?;
    sqlx::query!("DELETE FROM followed_shows WHERE id = ?", id)
        .execute(&mut *conn)
        .await?;
    let cancelled = event(FollowedShowCancelled {
        followed_show: followed.into(),
        ..Default::default()
    });
    Ok(Applied {
        unfollow: followed_season,
        ..Applied::saying((), cancelled)
    })
}

/// The first Season Request the Followed Show `id` opened, if it is Approved, so its Fulfiller was
/// asked to follow the Show. Removing the Followed Show unlinks it, so read it first.
async fn followed_season(
    conn: &mut SqliteConnection,
    id: i64,
) -> Result<Option<Request>, sqlx::Error> {
    if !followed_show_approved(conn, id).await? {
        return Ok(None);
    }
    // ponytail: a Followed Show stores no Show ids, so once every Season Request it opened is
    // cancelled its Fulfiller isn't told to stop; store them if that bites.
    let first = sqlx::query_scalar!(
        "SELECT MIN(id) FROM requests WHERE followed_show_id = ?",
        id
    )
    .fetch_one(&mut *conn)
    .await?;
    match first {
        Some(first) => load_one(conn, first).await,
        None => Ok(None),
    }
}

/// Takes `user` off the Followed Show `id`, among those `visible` lets them see. If they were its
/// last Requester, removes it instead, naming its Season Request as [`cancel_followed`] does.
async fn withdraw_followed(
    conn: &mut SqliteConnection,
    user: i64,
    visible: Option<i64>,
    id: i64,
) -> Result<Applied<()>, Error> {
    let found = load_followed(conn, Some(id), visible).await?.pop();
    let followed = asked::requested_by(Asked::FollowedShow, id, user, found, |followed| {
        &followed.requesters
    })?;
    let followed_season = followed_season(conn, id).await?;
    let was_last = asked::leave(conn, Asked::FollowedShow, id, user).await?;
    if was_last {
        let withdrawn = event(FollowedShowWithdrawn {
            followed_show: followed.into(),
            ..Default::default()
        });
        return Ok(Applied {
            unfollow: followed_season,
            ..Applied::saying((), withdrawn)
        });
    }
    let removed = event(FollowedShowRequesterRemoved {
        followed_show: followed.into(),
        user_id: user,
        ..Default::default()
    });
    Ok(Applied::saying((), removed))
}

/// The Followed Shows, newest first: only the one with the ID `id` if given, and only those the
/// User `requester` is a Requester of if given.
async fn load_followed(
    conn: &mut SqliteConnection,
    id: Option<i64>,
    requester: Option<i64>,
) -> Result<Vec<FollowedShow>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT id AS "id!", tmdb_id, title, state, created_at, decline_reason, quality_id,
                  quality_name, quality_profile_id
           FROM followed_shows
           WHERE ?1 IS NULL OR id = ?1
           ORDER BY id DESC"#,
        id
    )
    .fetch_all(&mut *conn)
    .await?;
    let mut requesters = asked::requesters(conn, Asked::FollowedShow, id).await?;
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
            quality: stored_quality(row.quality_id, row.quality_name, row.quality_profile_id)
                .into(),
            ..Default::default()
        })
        .filter(|followed| may_see(requester, &followed.requesters))
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
                  show_tvdb_id, state, created_at, decline_reason, failure_reason, followed_show_id,
                  fulfiller_plugin_id,
                  quality_id, quality_name, quality_profile_id, quality_changed_from,
                  quality_changed_at, quality_used_id, quality_used_name,
                  quality_used_profile_id,
                  (SELECT display_username FROM users WHERE id = quality_changed_by)
                      AS quality_changed_by
           FROM requests
           WHERE ?1 IS NULL OR id = ?1
           ORDER BY id DESC"#,
        id
    )
    .fetch_all(&mut *conn)
    .await?;
    let mut requesters = asked::requesters(conn, Asked::Request, id).await?;
    Ok(rows
        .into_iter()
        .map(|row| Request {
            id: row.id,
            movie_or_season: match row.season {
                Some(season_number) => RequestedSeason {
                    show_tmdb_id: row.tmdb_id,
                    season_number,
                    show_title: row.title,
                    air_date: row.release_date,
                    show_imdb_id: row.show_imdb_id,
                    show_tvdb_id: row.show_tvdb_id,
                    ..Default::default()
                }
                .into(),
                None => RequestedMovie {
                    tmdb_id: row.tmdb_id,
                    title: row.title,
                    release_date: row.release_date,
                    ..Default::default()
                }
                .into(),
            },
            state: read_state(&row.state).into(),
            requesters: requesters.remove(&row.id).unwrap_or_default(),
            created_at: row.created_at,
            decline_reason: row.decline_reason,
            failure_reason: row.failure_reason,
            followed_show_id: row.followed_show_id,
            fulfiller_plugin_id: row.fulfiller_plugin_id,
            quality: stored_quality(row.quality_id, row.quality_name, row.quality_profile_id)
                .into(),
            quality_change: row
                .quality_changed_at
                .map(|changed_at| QualityChange {
                    changed_by: row.quality_changed_by,
                    previous_name: row.quality_changed_from,
                    changed_at,
                    ..Default::default()
                })
                .into(),
            quality_used: stored_quality(
                row.quality_used_id,
                row.quality_used_name,
                row.quality_used_profile_id,
            )
            .into(),
            ..Default::default()
        })
        .filter(|request| may_see(requester, &request.requesters))
        .collect())
}

/// The three columns a Request stores `quality` in, all set or all `NULL`, as [`stored_quality`]
/// reads them back.
fn quality_columns(
    quality: Option<&RequestedQuality>,
) -> (Option<i64>, Option<&str>, Option<&str>) {
    (
        quality.map(|quality| quality.id),
        quality.map(|quality| quality.name.as_str()),
        quality.map(|quality| quality.profile_id.as_str()),
    )
}

/// A Quality a Request stored, if it has one, from the columns [`quality_columns`] gives.
fn stored_quality(
    id: Option<i64>,
    name: Option<String>,
    profile_id: Option<String>,
) -> Option<RequestedQuality> {
    Some(RequestedQuality {
        id: id?,
        name: name?,
        profile_id: profile_id?,
        ..Default::default()
    })
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

/// Locks `mutex`, taking its value even if poisoned: nothing panics while one is held, so it is
/// still whole.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, HashMap, HashSet},
        pin::Pin,
        sync::Arc,
        time::Duration,
    };

    use futures_util::{Stream, StreamExt};
    use nama_plugin_api::{
        BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
        fulfiller::{Download, Fulfilled, Fulfiller, Movies, Shows},
        metadata::{Episode, ExternalId, ExternalIds, Season},
        settings::{SettingValue, Settings},
    };
    use nama_plugin_tmdb::{
        Tmdb,
        testing::{API_KEY, FakeTmdb},
    };
    use nama_proto::nama::v1::{
        Event, FollowedShow, ListRequestsResponse, Permission, Request, RequestState,
        SubscribeResponse, event::Event as Happened, subscribe_response,
    };
    use sqlx::SqliteConnection;
    use tokio::sync::Notify;

    use super::{
        Applied, Error, MovieOrSeason, RequestedMovie, RequestedSeason, Requests, STATES, Wanted,
        WantedSeasons, approve, ask, ask_movie, ask_seasons, asked, availability, cancel,
        cancel_followed, decide_followed, decline, follow, is_open, load, load_one,
        make_seasons_available, opening_state, progress, retry, settle, withdraw,
        withdraw_followed,
    };
    use crate::{
        artwork::Artwork,
        contents::{
            Change, Contents, Decided, Decisions, Item, Metadata,
            fixture::{MatchedMovie, movies_library, tmdb_movie},
        },
        custom_images::CustomImages,
        db::Database,
        events::{EventBus, Filter},
        images::ImageStore,
        libraries::{Kind, Libraries},
        plugins::{Answer, ExtensionPoints, Failure, Registry},
        qualities,
        scanner::Scanner,
        server_settings::ServerUrl,
        users::{Permissions, User},
    };

    const TODAY: &str = "2026-11-08";

    /// The Movie `request` asks for, panicking if it asks for a Season.
    fn asked_movie(request: &Request) -> &RequestedMovie {
        match &request.movie_or_season {
            Some(MovieOrSeason::Movie(movie)) => movie,
            other => panic!("not a Movie Request: {other:?}"),
        }
    }

    /// The Season `request` asks for, panicking if it asks for a Movie.
    fn asked_season(request: &Request) -> &RequestedSeason {
        match &request.movie_or_season {
            Some(MovieOrSeason::Season(season)) => season,
            other => panic!("not a Season Request: {other:?}"),
        }
    }

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
        let user = insert_ada(conn).await?;
        let mut ids = Vec::new();
        for one in wanted {
            ids.push(ask(conn, user, one, state).await?.value.id);
        }
        Ok(ids)
    }

    /// Adds the User [`ada`] stands for, and returns the id.
    async fn insert_ada(conn: &mut SqliteConnection) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar(
            "INSERT INTO users (username, display_username, permissions, created_at, updated_at)
             VALUES ('ada', 'Ada', 0, 0, 0) RETURNING id",
        )
        .fetch_one(&mut *conn)
        .await
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
            quality: qualities::Ask::default(),
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

    /// The interval a test polls progress at: how long a poll waits for the fake Fulfillers, which
    /// answer at once.
    const INTERVAL: Duration = Duration::from_secs(5);

    /// A Fulfiller for Movies and Shows that keeps the ids of the Requests it is handed, in order,
    /// and takes each only once `open` lets it.
    #[derive(Default)]
    struct Gated {
        handed: std::sync::Mutex<Vec<i64>>,
        open: Notify,
    }

    impl Gated {
        fn handed(&self) -> Vec<i64> {
            self.handed.lock().unwrap().clone()
        }

        fn take<'a>(&'a self, request: &Request) -> BoxFuture<'a, Result<Fulfilled, PluginError>> {
            self.handed.lock().unwrap().push(request.id);
            Box::pin(async {
                self.open.notified().await;
                Ok(Fulfilled {
                    reference: "taken".to_owned(),
                    ..Fulfilled::default()
                })
            })
        }
    }

    impl Fulfiller<Movies> for Gated {
        fn fulfill<'a>(
            &'a self,
            request: &'a Request,
        ) -> BoxFuture<'a, Result<Fulfilled, PluginError>> {
            self.take(request)
        }

        fn progress<'a>(
            &'a self,
            _: &'a [String],
        ) -> BoxFuture<'a, Result<HashMap<String, Vec<Download>>, PluginError>> {
            Box::pin(async { Ok(HashMap::new()) })
        }
    }

    impl Fulfiller<Shows> for Gated {
        fn fulfill<'a>(
            &'a self,
            request: &'a Request,
        ) -> BoxFuture<'a, Result<Fulfilled, PluginError>> {
            self.take(request)
        }

        fn progress<'a>(
            &'a self,
            _: &'a [String],
        ) -> BoxFuture<'a, Result<HashMap<String, Vec<Download>>, PluginError>> {
            Box::pin(async { Ok(HashMap::new()) })
        }
    }

    /// A Fulfiller for Movies and Shows that takes every Request at once, giving a Movie's TMDB id
    /// or `season-<number>` as the reference, keeps each batch of references it is asked about,
    /// and says it is downloading [`downloading`] for each.
    #[derive(Default)]
    struct Asked(std::sync::Mutex<Vec<Vec<String>>>);

    impl Fulfiller<Movies> for Asked {
        fn fulfill<'a>(
            &'a self,
            request: &'a Request,
        ) -> BoxFuture<'a, Result<Fulfilled, PluginError>> {
            took(asked_movie(request).tmdb_id.to_string())
        }

        fn progress<'a>(
            &'a self,
            references: &'a [String],
        ) -> BoxFuture<'a, Result<HashMap<String, Vec<Download>>, PluginError>> {
            self.answer(references)
        }
    }

    impl Fulfiller<Shows> for Asked {
        fn fulfill<'a>(
            &'a self,
            request: &'a Request,
        ) -> BoxFuture<'a, Result<Fulfilled, PluginError>> {
            took(format!("season-{}", asked_season(request).season_number))
        }

        fn progress<'a>(
            &'a self,
            references: &'a [String],
        ) -> BoxFuture<'a, Result<HashMap<String, Vec<Download>>, PluginError>> {
            self.answer(references)
        }
    }

    /// A Fulfiller's answer that it took a Request, with `reference`.
    fn took(reference: String) -> BoxFuture<'static, Result<Fulfilled, PluginError>> {
        Box::pin(async {
            Ok(Fulfilled {
                reference,
                ..Fulfilled::default()
            })
        })
    }

    /// What [`Asked`] says it is downloading for `reference`.
    fn downloading(reference: &str) -> Vec<Download> {
        vec![Download {
            title: reference.to_owned(),
            ..Download::default()
        }]
    }

    impl Asked {
        fn answer(
            &self,
            references: &[String],
        ) -> BoxFuture<'static, Result<HashMap<String, Vec<Download>>, PluginError>> {
            self.0.lock().unwrap().push(references.to_vec());
            let answered = references
                .iter()
                .map(|reference| (reference.clone(), downloading(reference)))
                .collect();
            Box::pin(async { Ok(answered) })
        }

        fn batches(&self) -> Vec<Vec<String>> {
            self.0.lock().unwrap().clone()
        }

        /// The references it was last asked about, sorted.
        fn last_asked_about(&self) -> Vec<String> {
            let mut batch = self.0.lock().unwrap().last().cloned().unwrap_or_default();
            batch.sort();
            batch
        }
    }

    /// A Plugin with the id `.0`, providing `.1` as the Fulfiller of `Kind`, Movies or Shows.
    struct FulfillerPlugin<Kind>(&'static str, Arc<dyn Fulfiller<Kind>>);

    impl<Kind: 'static> Plugin for FulfillerPlugin<Kind> {
        fn manifest(&self) -> Manifest {
            Manifest {
                version: "0.0.0".to_owned(),
                extension_points: vec![PointKey::provide::<dyn Fulfiller<Kind>>()],
                ..Manifest::new(PluginId::from_static(self.0))
            }
        }

        fn register(&self, registrar: &mut Registrar) {
            registrar.provide::<dyn Fulfiller<Kind>>(Priority::DEFAULT, Arc::clone(&self.1));
        }
    }

    /// The ids of a title with the TMDB id `tmdb_id`.
    fn tmdb(tmdb_id: i64) -> ExternalIds {
        ExternalIds {
            tmdb: Some(tmdb_id),
            ..ExternalIds::default()
        }
    }

    /// A Match to the title with `ids`, or for an Episode to its Show's `show_ids`.
    fn matched(ids: ExternalIds, show_ids: ExternalIds) -> Decided {
        Decided::Matched(Box::new(Metadata {
            plugin: PluginId::from_static("tmdb"),
            ids,
            show_ids,
            title: "Matched".to_owned(),
            original_title: None,
            show_title: None,
            overview: None,
            release_date: None,
            runtime_minutes: None,
            poster: None,
            backdrop: None,
            still: None,
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

    /// The Movie with the TMDB id `tmdb_id`, as the metadata source gives it.
    fn movie(tmdb_id: i64) -> Wanted {
        Wanted {
            tmdb_id,
            season: None,
            title: format!("Movie {tmdb_id}"),
            release_date: None,
            show_ids: ExternalIds::default(),
            followed_show: None,
            quality: qualities::Ask::default(),
        }
    }

    /// Requests in a new database, which holds an Approved Movie Request for each of `tmdb_ids`,
    /// and whose one Plugin provides `fulfiller` as the Movie Fulfiller.
    async fn approved_for(
        tmdb_ids: &[i64],
        fulfiller: Arc<dyn Fulfiller<Movies>>,
    ) -> (tempfile::TempDir, Requests) {
        approved_on(&Registry::new(), tmdb_ids, fulfiller).await
    }

    /// [`approved_for`], with `plugins` enabled too, and `fulfiller` as the Fulfiller of `Kind`.
    async fn approved_on<Kind: 'static>(
        plugins: &Registry,
        tmdb_ids: &[i64],
        fulfiller: Arc<dyn Fulfiller<Kind>>,
    ) -> (tempfile::TempDir, Requests) {
        let data = tempfile::tempdir().unwrap();
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        db.write(|tx| Box::pin(insert_ada(tx))).await.unwrap();
        plugins
            .enable(Arc::new(FulfillerPlugin("gated", fulfiller)))
            .unwrap();
        let requests = requests_on(db, plugins.extension_points());
        for &tmdb_id in tmdb_ids {
            ask_as(&requests, Permissions::ADMIN, tmdb_id).await;
        }
        (data, requests)
    }

    /// Requests in `db`, handing off to the Fulfillers in `plugins`.
    fn requests_on(db: Arc<Database>, plugins: ExtensionPoints) -> Requests {
        let events = EventBus::new(Arc::clone(&db), plugins.clone(), ServerUrl::default());
        Requests::new(db, events, plugins)
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

    /// [`ada`] holding `permissions` asks for the Movie `tmdb_id`, as [`Requests::create`] does
    /// once the metadata source has answered. Returns the Request's id.
    async fn ask_as(requests: &Requests, permissions: Permissions, tmdb_id: i64) -> i64 {
        let asker = ada(permissions);
        let state = opening_state(&asker, Permission::AutoApproveMovie);
        let applied = requests
            .db
            .write(move |tx| Box::pin(ask_movie(tx, asker.id, movie(tmdb_id), state)))
            .await
            .unwrap();
        applied.value.id
    }

    /// A new Library Matches a file to each of the Movies `tmdb_ids` in turn, which makes their
    /// open Requests Available.
    async fn match_in_a_library(db: &Arc<Database>, tmdb_ids: &[i64]) {
        let folder = tempfile::tempdir().unwrap();
        let library = movies_library(db, folder.path()).await;
        let contents = Contents::new(Arc::clone(db));
        for &tmdb_id in tmdb_ids {
            let path = format!("/media/{tmdb_id}.mkv");
            let movie = tmdb_movie(tmdb_id, &format!("Movie {tmdb_id}"));
            MatchedMovie::store(&contents, library, &path, movie).await;
        }
    }

    #[tokio::test]
    async fn one_poll_asks_the_fulfiller_once_about_every_processing_request() {
        let asked = Arc::new(Asked::default());
        let fulfiller = Arc::clone(&asked);
        let (_data, requests) = approved_for(&[949, 438_631, 603], fulfiller).await;
        come_up(&requests, "gated").await;
        match_in_a_library(&requests.db, &[603]).await;

        let any = requests.progress.poll(INTERVAL).await.unwrap();

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
        let pending = ask_as(&requests, Permissions::NEW_USER, 603).await;
        let following = requests.clone();
        let follow = tokio::spawn(async move { following.progress.follow(EVERY).await });
        let nothing_processing = asked_while(&asked, EVERY * 10).await;

        come_up(&requests, "gated").await;
        until_asked_about(&asked, &["438631", "949"]).await;
        match_in_a_library(&requests.db, &[949, 438_631]).await;
        // Asked once more at most, if it was already asking as they became Available.
        asked_while(&asked, EVERY * 5).await;
        let nothing_processing_again = asked_while(&asked, EVERY * 10).await;
        requests
            .approve(&ada(Permissions::ADMIN), pending, None)
            .await
            .unwrap();

        until_asked_about(&asked, &["603"]).await;
        follow.abort();
        assert_eq!(nothing_processing, 0);
        assert_eq!(nothing_processing_again, 0);
    }

    #[tokio::test]
    async fn only_a_request_its_fulfiller_has_that_isnt_available_yet_is_asked_about_and_gets_downloads()
     {
        let asked = Arc::new(Asked::default());
        let fulfiller = Arc::clone(&asked);
        let (_data, requests) = approved_for(&[], fulfiller).await;
        let inserted = requests
            .db
            .write(|tx| Box::pin(insert_in_each_state(tx)))
            .await
            .unwrap();

        requests.progress.poll(INTERVAL).await.unwrap();

        let admin = ada(Permissions::ADMIN);
        let mut has_it = Vec::new();
        for (id, state, reference) in inserted {
            let not_available_yet = matches!(
                state,
                RequestState::Processing | RequestState::PartiallyAvailable
            );
            let expected = reference.is_some() && not_available_yet;
            let request = requests.get(&admin, id).await.unwrap();
            assert_eq!(
                !request.downloads.is_empty(),
                expected,
                "{state:?} with the reference {reference:?}"
            );
            has_it.extend(reference.filter(|_| expected));
        }
        has_it.sort();
        assert_eq!(asked.last_asked_about(), has_it);
    }

    /// A Movie Request in each state, without and with a Fulfiller's reference. Returns each one's
    /// id, state and reference.
    async fn insert_in_each_state(
        conn: &mut SqliteConnection,
    ) -> Result<Vec<(i64, RequestState, Option<String>)>, sqlx::Error> {
        let mut inserted = Vec::new();
        for (tmdb_id, &(state, stored)) in (1..).zip(&STATES) {
            for reference in [None, Some(format!("movie-{tmdb_id}"))] {
                let id = sqlx::query_scalar(
                    "INSERT INTO requests (tmdb_id, title, state, created_at, fulfiller_reference)
                     VALUES (?, 'Movie', ?, 0, ?)
                     RETURNING id",
                )
                // A Movie of its own each, as only one open Request per Movie may be stored.
                .bind(tmdb_id * 2 + i64::from(reference.is_some()))
                .bind(stored)
                .bind(&reference)
                .fetch_one(&mut *conn)
                .await?;
                inserted.push((id, state, reference));
            }
        }
        Ok(inserted)
    }

    #[tokio::test]
    async fn sql_and_rust_agree_on_which_requests_are_open() {
        let data = tempfile::tempdir().unwrap();
        let db = Database::open(data.path()).await.unwrap();
        let inserted = db
            .write(|tx| Box::pin(insert_in_each_state(tx)))
            .await
            .unwrap();
        macro_rules! read {
            ($open:literal) => {
                sqlx::query_scalar!(r#"SELECT id AS "id!" FROM requests WHERE "# + $open)
            };
        }
        let open: HashSet<i64> = open_request_sql!(read)
            .fetch_all(db.reader())
            .await
            .unwrap()
            .into_iter()
            .collect();

        for (id, state, reference) in inserted {
            let expected = is_open(state);
            assert_eq!(open.contains(&id), expected, "{state:?} with {reference:?}");
        }
    }

    #[tokio::test]
    async fn sql_and_rust_agree_on_which_requests_their_fulfiller_has() {
        let data = tempfile::tempdir().unwrap();
        let db = Database::open(data.path()).await.unwrap();
        let inserted = db
            .write(|tx| Box::pin(insert_in_each_state(tx)))
            .await
            .unwrap();
        macro_rules! read {
            ($fulfilling:literal) => {
                sqlx::query_scalar!(r#"SELECT id AS "id!" FROM requests WHERE "# + $fulfilling)
            };
        }
        let fulfilling: HashSet<i64> = progress::fulfilling_sql!(read)
            .fetch_all(db.reader())
            .await
            .unwrap()
            .into_iter()
            .collect();

        for (id, state, reference) in inserted {
            let request = Request {
                state: state.into(),
                ..Default::default()
            };
            let expected = progress::is_fulfilling(&request);
            assert_eq!(
                fulfilling.contains(&id),
                expected,
                "{state:?} with {reference:?}"
            );
        }
    }

    #[tokio::test]
    async fn only_a_declined_followed_show_is_closed() {
        let data = tempfile::tempdir().unwrap();
        let db = Database::open(data.path()).await.unwrap();
        db.write(|tx| {
            Box::pin(
                sqlx::query(
                    "INSERT INTO followed_shows (id, tmdb_id, title, state, created_at)
                     VALUES (1, 1, 'Show', 'pending', 0), (2, 2, 'Show', 'approved', 0),
                            (3, 3, 'Show', 'declined', 0)",
                )
                .execute(&mut **tx),
            )
        })
        .await
        .unwrap();
        macro_rules! read {
            ($open:literal) => {
                sqlx::query_scalar!(r#"SELECT id AS "id!" FROM followed_shows WHERE "# + $open)
            };
        }
        let open: HashSet<i64> = open_followed_show_sql!(read)
            .fetch_all(db.reader())
            .await
            .unwrap()
            .into_iter()
            .collect();

        assert_eq!(open, HashSet::from([1, 2]));
    }

    #[tokio::test]
    async fn a_handed_off_season_gets_progress_while_partially_available_but_not_once_available() {
        let asked = Arc::new(Asked::default());
        let (_data, db) = ada_and_bob().await;
        let plugins = Registry::new();
        let fulfiller: Arc<dyn Fulfiller<Shows>> = Arc::<Asked>::clone(&asked);
        plugins
            .enable(Arc::new(FulfillerPlugin("sonarr", fulfiller)))
            .unwrap();
        let plugins = plugins.extension_points();
        let bus = EventBus::new(Arc::clone(&db), plugins.clone(), ServerUrl::default());
        let requests = Requests::new(Arc::clone(&db), bus.clone(), plugins);
        let season_1 = WantedSeasons {
            seasons: vec![dark_season(1)],
            ..dark()
        };
        let id = db
            .write(|tx| Box::pin(ask_seasons(tx, ADA, season_1, RequestState::Approved)))
            .await
            .unwrap()
            .value[0]
            .id;
        come_up(&requests, "sonarr").await;
        match_dark_episodes(&db, 1, &[1]).await;
        scan_dark(&db, &[1]).await;
        let mut events = Box::pin(bus.subscribe_as_admin(Filter::default()));

        let any = requests.progress.poll(INTERVAL).await.unwrap();
        let Happened::RequestProgress(progress) = next_happened(&mut events).await else {
            panic!("its progress was not published");
        };
        let partly = requests.get(&ada(Permissions::ADMIN), id).await.unwrap();
        match_dark_episodes(&db, 1, &[2]).await;
        scan_dark(&db, &[1]).await;
        let batches = asked.batches().len();
        let any_once_available = requests.progress.poll(INTERVAL).await.unwrap();
        // `poll` publishes before it returns, so anything it published is waiting here.
        let published_once_available =
            tokio::time::timeout(Duration::from_millis(100), events.next()).await;
        let available = requests.get(&ada(Permissions::ADMIN), id).await.unwrap();

        assert!(any);
        assert_eq!(asked.last_asked_about(), ["season-1"]);
        let published = progress.request.into_option().unwrap();
        assert_eq!(
            (published.id, published.state),
            (id, RequestState::PartiallyAvailable.into())
        );
        assert_eq!(published.downloads, downloading("season-1"));
        assert_eq!(partly.downloads, downloading("season-1"));
        assert!(!any_once_available);
        assert_eq!(asked.batches().len(), batches);
        assert_eq!(published_once_available.ok(), None);
        assert_eq!(available.state, RequestState::Available);
        assert_eq!(available.downloads, []);
    }

    #[tokio::test]
    async fn a_request_being_handed_off_is_not_handed_to_the_fulfiller_again() {
        let (_data, requests, gated) = approved_for_gated(&[949]).await;
        let handing_first = requests.clone();
        let first = tokio::spawn(async move { come_up(&handing_first, "gated").await });
        while gated.handed().is_empty() {
            tokio::task::yield_now().await;
        }

        tokio::time::timeout(Duration::from_secs(1), come_up(&requests, "gated"))
            .await
            .expect("the second hand-off waited on the Fulfiller");
        gated.open.notify_one();
        first.await.unwrap();

        assert_eq!(gated.handed().len(), 1);
    }

    #[tokio::test]
    async fn a_fulfiller_coming_up_leaves_the_requests_an_ask_is_handing_off_to_it() {
        let gated = Arc::new(Gated::default());
        let fulfiller: Arc<dyn Fulfiller<Shows>> = Arc::<Gated>::clone(&gated);
        let (_data, requests) = approved_on(&Registry::new(), &[], fulfiller).await;
        let asking = requests.clone();
        let ask = tokio::spawn(async move {
            asking
                .apply(|tx| Box::pin(ask_seasons(tx, ADA, dark(), RequestState::Approved)))
                .await
        });
        while gated.handed().is_empty() {
            tokio::task::yield_now().await;
        }

        tokio::time::timeout(Duration::from_secs(1), come_up(&requests, "gated"))
            .await
            .expect("the came-up hand-off waited on the Fulfiller");
        assert_eq!(
            gated.handed().len(),
            1,
            "the came-up hand-off took the second Request"
        );
        gated.open.notify_one();
        while gated.handed().len() < 2 {
            tokio::task::yield_now().await;
        }
        gated.open.notify_one();
        let asked = ask.await.unwrap().unwrap();

        let ids: Vec<i64> = asked.iter().map(|request| request.id).collect();
        assert_eq!(gated.handed(), ids);
        assert!(
            asked
                .iter()
                .all(|request| request.state == RequestState::Processing),
            "{asked:?}"
        );
    }

    #[tokio::test]
    async fn a_plugin_that_provides_no_fulfiller_coming_up_hands_nothing_off() {
        let (_data, requests, gated) = approved_for_gated(&[949]).await;

        come_up(&requests, "tmdb").await;

        assert_eq!(gated.handed(), NOTHING);
    }

    /// Tells `requests` the Plugin `plugin` came up, and waits until it has handed off what that
    /// asks for.
    async fn come_up(requests: &Requests, plugin: &'static str) {
        let (came_up, queue) = tokio::sync::mpsc::unbounded_channel();
        came_up.send(PluginId::from_static(plugin)).unwrap();
        drop(came_up);
        requests.as_fulfillers_come_up(queue).await;
    }

    /// Ada, the User the tests ask as, holding `permissions`.
    fn ada(permissions: Permissions) -> User {
        User {
            id: 1,
            name: "ada".to_owned(),
            permissions,
            created_at: 0,
        }
    }

    /// Runs `change` for a caller that stops waiting once its write is queued, as hyper drops a
    /// handler's future when its client goes away, then lets the write run.
    async fn leave_once_queued<T: Send + 'static>(
        requests: &Requests,
        change: impl Future<Output = T> + Send + 'static,
    ) {
        let busy = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let db = Arc::clone(&requests.db);
        let (writing, held) = (Arc::clone(&busy), Arc::clone(&release));
        let holding = tokio::spawn(async move {
            db.write(move |_| Box::pin(hold_writer(writing, held)))
                .await
        });
        busy.notified().await;
        let caller = tokio::spawn(change);
        let queued = async {
            while requests.db.queued_writes() == 0 {
                tokio::task::yield_now().await;
            }
        };
        tokio::time::timeout(Duration::from_secs(5), queued)
            .await
            .expect("the change never queued its write");

        caller.abort();
        assert!(caller.await.is_err_and(|ended| ended.is_cancelled()));
        release.notify_one();
        holding.await.unwrap().unwrap();
    }

    /// Enables `tmdb` as the metadata Provider in `plugins`.
    fn enable_tmdb(plugins: &Registry, tmdb: &FakeTmdb) {
        let settings = Settings::new(BTreeMap::from([(
            "api_key".to_owned(),
            SettingValue::Text(API_KEY.to_owned()),
        )]));
        plugins
            .enable_with_settings(Arc::new(Tmdb::with_endpoints(tmdb.endpoints())), settings)
            .unwrap();
    }

    /// A write that tells `writing` it runs, then holds the writer until `released`.
    async fn hold_writer(writing: Arc<Notify>, released: Arc<Notify>) -> Result<(), sqlx::Error> {
        writing.notify_one();
        released.notified().await;
        Ok(())
    }

    /// Every event `requests` publishes from now on.
    fn subscribe(requests: &Requests) -> Pin<Box<impl Stream<Item = SubscribeResponse> + use<>>> {
        Box::pin(requests.events.subscribe_as_admin(Filter::default()))
    }

    /// What the next event `published` lets through says happened, waiting a second at most.
    async fn next_happened(
        published: &mut (impl Stream<Item = SubscribeResponse> + Unpin),
    ) -> Happened {
        let response = tokio::time::timeout(Duration::from_secs(1), published.next())
            .await
            .expect("nothing was published")
            .expect("the bus closed");
        match response.message {
            Some(subscribe_response::Message::Event(event)) => event.event.unwrap(),
            other => panic!("not an event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn an_auto_approved_movie_asked_for_by_a_caller_who_left_is_published_and_handed_off() {
        let tmdb = FakeTmdb::start().await;
        let plugins = Registry::new();
        enable_tmdb(&plugins, &tmdb);
        let (_data, requests) =
            approved_on::<Movies>(&plugins, &[], Arc::new(Asked::default())).await;
        let mut published = subscribe(&requests);
        let asking = requests.clone();

        leave_once_queued(&requests, async move {
            asking.create(&ada(Permissions::ADMIN), 348, None).await
        })
        .await;

        let created = next_happened(&mut published).await;
        assert!(
            matches!(created, Happened::RequestCreated(_)),
            "{created:?}"
        );
        let Happened::RequestStateChanged(handed_off) = next_happened(&mut published).await else {
            panic!("the Request was not handed off");
        };
        assert_eq!(handed_off.request.state, RequestState::Processing);
    }

    #[tokio::test]
    async fn a_request_declined_by_a_caller_who_left_is_published() {
        let (_data, requests) = approved_for(&[], Arc::new(Asked::default())).await;
        ask_as(&requests, Permissions::NEW_USER, 949).await;
        let mut published = subscribe(&requests);
        let declining = requests.clone();

        leave_once_queued(&requests, async move { declining.decline(1, "").await }).await;

        let Happened::RequestStateChanged(declined) = next_happened(&mut published).await else {
            panic!("not a change of state");
        };
        assert_eq!(declined.request.state, RequestState::Declined);
    }

    #[tokio::test]
    async fn a_request_cancelled_by_a_caller_who_left_is_published() {
        let (_data, requests) = approved_for(&[949], Arc::new(Asked::default())).await;
        let mut published = subscribe(&requests);
        let cancelling = requests.clone();

        leave_once_queued(&requests, async move { cancelling.cancel(1).await }).await;

        let cancelled = next_happened(&mut published).await;
        assert!(
            matches!(cancelled, Happened::RequestCancelled(_)),
            "{cancelled:?}"
        );
    }

    #[tokio::test]
    async fn a_request_withdrawn_by_a_caller_who_left_is_published() {
        let (_data, requests) = approved_for(&[], Arc::new(Asked::default())).await;
        ask_as(&requests, Permissions::NEW_USER, 949).await;
        let mut published = subscribe(&requests);
        let withdrawing = requests.clone();

        leave_once_queued(&requests, async move {
            withdrawing.withdraw(&ada(Permissions::NEW_USER), 1).await
        })
        .await;

        let withdrawn = next_happened(&mut published).await;
        assert!(
            matches!(withdrawn, Happened::RequestWithdrawn(_)),
            "{withdrawn:?}"
        );
    }

    #[tokio::test]
    async fn a_match_pinned_by_a_caller_who_left_publishes_it_and_the_request_it_made_available() {
        let tmdb = FakeTmdb::start().await;
        let folder = tempfile::tempdir().unwrap();
        let plugins = Registry::new();
        enable_tmdb(&plugins, &tmdb);
        let (data, requests) =
            approved_on::<Movies>(&plugins, &[949], Arc::new(Asked::default())).await;
        let root = folder.path().canonicalize().unwrap();
        let library = Libraries::new(Arc::clone(&requests.db))
            .create(
                "Movies",
                Kind::Movies,
                vec![root.to_str().unwrap().to_owned()],
            )
            .await
            .unwrap()
            .id;
        let heat = Item::Movie {
            title: "Heat".to_owned(),
            year: Some(1995),
            hint: None,
        };
        let store = Change::Store {
            path: "/media/Heat (1995)/Heat (1995).mkv".to_owned(),
            size: 1,
            modified_ns: 0,
            items: vec![heat],
        };
        requests.contents.apply(library, vec![store]).await.unwrap();
        let item = requests.contents.files(library).await.unwrap()[0].items[0].id;
        let images = ImageStore::new(data.path(), Arc::clone(&requests.db));
        let scanner = Scanner::new(
            Arc::clone(&requests.db),
            requests.plugins.clone(),
            Artwork::new(images.clone(), requests.plugins.clone()),
            CustomImages::new(Arc::clone(&requests.db), images),
            requests.events.clone(),
            None,
            None,
            None,
        );
        let mut published = subscribe(&requests);
        let pinning = scanner.matcher().clone();

        leave_once_queued(&requests, async move {
            pinning.set_match(item, &ExternalId::Tmdb(949)).await
        })
        .await;

        let Happened::ItemMatched(matched) = next_happened(&mut published).await else {
            panic!("the Match was not published");
        };
        assert_eq!(matched.item_id, item);
        let Happened::RequestStateChanged(available) = next_happened(&mut published).await else {
            panic!("the Request was not made Available");
        };
        assert_eq!(available.request.state, RequestState::Available);
    }

    #[tokio::test]
    async fn an_available_season_request_stays_available_when_a_later_episode_airs() {
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
            hint: Some(ExternalId::Tmdb(DARK)),
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
            quality: qualities::Ask::default(),
        };
        let request = db
            .write(move |tx| Box::pin(ask_each(tx, vec![wanted], RequestState::Pending)))
            .await
            .unwrap()[0];
        let scan = |episode_2_airs| Decisions {
            items: vec![(item, matched(ExternalIds::default(), tmdb(DARK)))],
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

    /// What each event in `applied` says happened, as [`said`] tells it.
    fn happened<T>(applied: &Applied<T>) -> Vec<(&'static str, RequestState)> {
        said(&applied.events)
    }

    /// What each of `events` says happened, and the state its Request or Followed Show is in by
    /// then.
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "the writes under test publish only Request and Followed Show events"
    )]
    fn said(events: &[Event]) -> Vec<(&'static str, RequestState)> {
        events
            .iter()
            .map(|event| {
                let (what, state) = match event.event.as_ref().expect("an event") {
                    Happened::RequestCreated(created) => ("created", created.request.state),
                    Happened::RequesterAdded(added) => ("joined", added.request.state),
                    Happened::RequestStateChanged(changed) => ("moved", changed.request.state),
                    Happened::RequestCancelled(cancelled) => ("cancelled", cancelled.request.state),
                    Happened::RequestWithdrawn(withdrawn) => ("withdrawn", withdrawn.request.state),
                    Happened::RequesterRemoved(removed) => ("left", removed.request.state),
                    Happened::FollowedShowCreated(created) => {
                        ("followed", created.followed_show.state)
                    }
                    Happened::FollowedShowRequesterAdded(added) => {
                        ("joined the Show", added.followed_show.state)
                    }
                    Happened::FollowedShowStateChanged(changed) => {
                        ("moved the Show", changed.followed_show.state)
                    }
                    Happened::FollowedShowCancelled(cancelled) => {
                        ("cancelled the Show", cancelled.followed_show.state)
                    }
                    Happened::FollowedShowWithdrawn(withdrawn) => {
                        ("withdrew the Show", withdrawn.followed_show.state)
                    }
                    Happened::FollowedShowRequesterRemoved(removed) => {
                        ("left the Show", removed.followed_show.state)
                    }
                    other => panic!("not a Request or Followed Show event: {other:?}"),
                };
                (what, state.as_known().unwrap_or_default())
            })
            .collect()
    }

    const NOTHING: [i64; 0] = [];

    /// The ids of the Requests `applied` hands off.
    fn handed_off<T>(applied: &Applied<T>) -> Vec<i64> {
        applied.hand_off.iter().map(|request| request.id).collect()
    }

    /// The id of the Season Request whose Show `applied` stops following, if any.
    fn unfollowed<T>(applied: &Applied<T>) -> Option<i64> {
        applied.unfollow.as_ref().map(|season| season.id)
    }

    const ADA: i64 = 1;
    const BOB: i64 = 2;

    /// A new database holding the Users [`ADA`] and [`BOB`].
    async fn ada_and_bob() -> (tempfile::TempDir, Arc<Database>) {
        let data = tempfile::tempdir().unwrap();
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        db.write(|tx| {
            Box::pin(async move {
                insert_ada(tx).await?;
                sqlx::query(
                    "INSERT INTO users (username, display_username, permissions, created_at,
                                        updated_at)
                     VALUES ('bob', 'Bob', 0, 0, 0)",
                )
                .execute(&mut **tx)
                .await
            })
        })
        .await
        .unwrap();
        (data, db)
    }

    /// `user` asks for the Movie `tmdb_id`, opening a Request in `state` if it has none.
    async fn ask_for(
        db: &Database,
        user: i64,
        tmdb_id: i64,
        state: RequestState,
    ) -> Applied<Request> {
        db.write(move |tx| Box::pin(ask_movie(tx, user, movie(tmdb_id), state)))
            .await
            .unwrap()
    }

    /// The Fulfiller of the Plugin `fulfiller` took the Approved Request `id`, or didn't, and why.
    async fn settled(
        db: &Database,
        id: i64,
        fulfilled: Result<Fulfilled, String>,
    ) -> Applied<Option<Request>> {
        let fulfilled = answered("fulfiller", fulfilled);
        db.write(move |tx| Box::pin(settle(tx, id, fulfilled, None)))
            .await
            .unwrap()
    }

    /// What the Plugin `plugin`'s Fulfiller answered, as the Registry hands it back.
    fn answered(plugin: &'static str, fulfilled: Result<Fulfilled, String>) -> Answer<Fulfilled> {
        Answer {
            plugin: PluginId::from_static(plugin),
            result: fulfilled.map_err(|message| Failure { kind: (), message }),
        }
    }

    /// What a Fulfiller says when it takes a Request.
    fn fulfilled() -> Fulfilled {
        Fulfilled {
            reference: "taken".to_owned(),
            ..Fulfilled::default()
        }
    }

    #[tokio::test]
    async fn asking_for_a_movie_opens_its_request_and_hands_it_off_only_if_approved() {
        let (_data, db) = ada_and_bob().await;

        let pending = ask_for(&db, ADA, 949, RequestState::Pending).await;
        let approved = ask_for(&db, ADA, 603, RequestState::Approved).await;

        assert_eq!(happened(&pending), [("created", RequestState::Pending)]);
        assert_eq!(handed_off(&pending), NOTHING);
        assert_eq!(happened(&approved), [("created", RequestState::Approved)]);
        assert_eq!(handed_off(&approved), [approved.value.id]);
    }

    #[tokio::test]
    async fn joining_a_pending_request_approves_and_hands_it_off_only_for_an_auto_approver() {
        let (_data, db) = ada_and_bob().await;
        ask_for(&db, ADA, 949, RequestState::Pending).await;

        let joined = ask_for(&db, BOB, 949, RequestState::Pending).await;
        let asked_again = ask_for(&db, BOB, 949, RequestState::Approved).await;

        assert_eq!(happened(&joined), [("joined", RequestState::Pending)]);
        assert_eq!(handed_off(&joined), NOTHING);
        assert_eq!(happened(&asked_again), [("moved", RequestState::Approved)]);
        assert_eq!(handed_off(&asked_again), [joined.value.id]);
    }

    #[tokio::test]
    async fn joining_a_request_already_approved_hands_nothing_off() {
        let (_data, db) = ada_and_bob().await;
        ask_for(&db, ADA, 949, RequestState::Approved).await;
        let processing = ask_for(&db, ADA, 603, RequestState::Approved)
            .await
            .value
            .id;
        settled(&db, processing, Ok(fulfilled())).await;
        let failed = ask_for(&db, ADA, 348, RequestState::Approved)
            .await
            .value
            .id;
        settled(&db, failed, Err("no space".to_owned())).await;

        let mut joined = Vec::new();
        for tmdb_id in [949, 603, 348] {
            let applied = ask_for(&db, BOB, tmdb_id, RequestState::Approved).await;
            assert_eq!(handed_off(&applied), NOTHING);
            joined.extend(happened(&applied));
        }

        assert_eq!(
            joined,
            [
                ("joined", RequestState::Approved),
                ("joined", RequestState::Processing),
                ("joined", RequestState::Failed)
            ]
        );
    }

    #[tokio::test]
    async fn asking_for_a_declined_movie_opens_a_new_request_but_one_in_a_library_is_refused() {
        let (_data, db) = ada_and_bob().await;
        let declined = ask_for(&db, ADA, 949, RequestState::Pending).await.value.id;
        db.write(move |tx| Box::pin(decline(tx, declined, None)))
            .await
            .unwrap();
        ask_for(&db, ADA, 603, RequestState::Pending).await;
        match_in_a_library(&db, &[603]).await;

        let asked_again = ask_for(&db, BOB, 949, RequestState::Pending).await;
        let in_a_library = db
            .write(|tx| Box::pin(ask_movie(tx, BOB, movie(603), RequestState::Pending)))
            .await;

        assert_eq!(happened(&asked_again), [("created", RequestState::Pending)]);
        assert_ne!(asked_again.value.id, declined);
        assert!(
            matches!(in_a_library, Err(Error::InLibrary(603))),
            "{in_a_library:?}"
        );
    }

    /// Dark's TMDB id.
    const DARK: i64 = 70523;

    /// Seasons 1 and 2 of Dark, as the metadata source gives them.
    fn dark() -> WantedSeasons {
        WantedSeasons {
            show_tmdb_id: DARK,
            show_ids: ExternalIds::default(),
            show_title: "Dark".to_owned(),
            seasons: vec![
                season_1("2017-12-08"),
                Season {
                    number: 2,
                    ..season_1("2019-06-21")
                },
            ],
            quality: qualities::Ask::default(),
        }
    }

    /// Season `number` of Dark, as the metadata source gives it: both its Episodes have aired.
    fn dark_season(number: u32) -> Season {
        Season {
            number,
            title: format!("Season {number}"),
            ..season_1("2017-12-08")
        }
    }

    /// A new Library Matches a file to each of `episodes` of Season `season` of Dark, as a scan
    /// does, without Requests hearing of the Season yet: [`scan_dark`] tells them.
    async fn match_dark_episodes(db: &Arc<Database>, season: i64, episodes: &[i64]) {
        let folder = tempfile::tempdir().unwrap();
        let root = folder.path().canonicalize().unwrap();
        let contents = Contents::new(Arc::clone(db));
        let library = Libraries::new(Arc::clone(db))
            .create(
                "Shows",
                Kind::Shows,
                vec![root.to_str().unwrap().to_owned()],
            )
            .await
            .unwrap()
            .id;
        let stores = episodes
            .iter()
            .map(|&episode| Change::Store {
                path: format!("/media/Dark/Season {season}/Dark - S{season}E{episode}.mkv"),
                size: 1,
                modified_ns: 0,
                items: vec![Item::Episode {
                    show: "Dark".to_owned(),
                    year: None,
                    hint: Some(ExternalId::Tmdb(DARK)),
                    season,
                    episode,
                }],
            })
            .collect();
        contents.apply(library, stores).await.unwrap();
        let decided: Vec<_> = contents
            .files(library)
            .await
            .unwrap()
            .into_iter()
            .flat_map(|file| file.items)
            .map(|stored| (stored.id, matched(ExternalIds::default(), tmdb(DARK))))
            .collect();
        contents.record(Decisions::from(decided)).await.unwrap();
    }

    /// Tells Requests a scan fetched each of the Seasons `numbers` of Dark, as [`dark_season`]
    /// gives them, as a Library's write does. Returns the events for what it changed.
    async fn scan_dark(db: &Database, numbers: &[u32]) -> Vec<Event> {
        let seasons = numbers
            .iter()
            .map(|&number| (DARK, dark_season(number)))
            .collect();
        db.write(move |tx| Box::pin(make_seasons_available(tx, seasons)))
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn asking_for_seasons_opens_a_request_for_each_and_hands_off_the_approved_ones() {
        let (_data, db) = ada_and_bob().await;
        let pending = db
            .write(|tx| Box::pin(ask_seasons(tx, ADA, dark(), RequestState::Pending)))
            .await
            .unwrap();
        let first = pending.value[0].id;
        db.write(move |tx| Box::pin(cancel(tx, first)))
            .await
            .unwrap();

        let approved = db
            .write(|tx| Box::pin(ask_seasons(tx, BOB, dark(), RequestState::Approved)))
            .await
            .unwrap();

        assert_eq!(
            happened(&pending),
            [
                ("created", RequestState::Pending),
                ("created", RequestState::Pending)
            ]
        );
        assert_eq!(handed_off(&pending), NOTHING);
        assert_eq!(
            happened(&approved),
            [
                ("created", RequestState::Approved),
                ("joined", RequestState::Pending),
                ("moved", RequestState::Approved)
            ]
        );
        let ids: Vec<i64> = approved.value.iter().map(|request| request.id).collect();
        assert_eq!(handed_off(&approved), ids);
    }

    #[tokio::test]
    async fn approving_a_pending_request_hands_it_off() {
        let (_data, db) = ada_and_bob().await;
        let id = ask_for(&db, ADA, 949, RequestState::Pending).await.value.id;

        let approved = db
            .write(move |tx| Box::pin(approve(tx, id, None)))
            .await
            .unwrap();
        let again = db.write(move |tx| Box::pin(approve(tx, id, None))).await;

        assert_eq!(happened(&approved), [("moved", RequestState::Approved)]);
        assert_eq!(handed_off(&approved), [id]);
        assert!(matches!(again, Err(Error::NotPending(..))), "{again:?}");
    }

    #[tokio::test]
    async fn declining_a_pending_request_hands_nothing_off() {
        let (_data, db) = ada_and_bob().await;
        let id = ask_for(&db, ADA, 949, RequestState::Pending).await.value.id;

        let declined = db
            .write(move |tx| Box::pin(decline(tx, id, Some("No.".to_owned()))))
            .await
            .unwrap();
        let again = db.write(move |tx| Box::pin(decline(tx, id, None))).await;

        assert_eq!(happened(&declined), [("moved", RequestState::Declined)]);
        assert_eq!(handed_off(&declined), NOTHING);
        assert_eq!(declined.value.decline_reason.as_deref(), Some("No."));
        assert!(matches!(again, Err(Error::NotPending(..))), "{again:?}");
    }

    #[tokio::test]
    async fn settling_an_approved_request_moves_it_to_processing_or_failed_once() {
        let (_data, db) = ada_and_bob().await;
        let taken = ask_for(&db, ADA, 949, RequestState::Approved)
            .await
            .value
            .id;
        let refused = ask_for(&db, ADA, 603, RequestState::Approved)
            .await
            .value
            .id;

        let processing = settled(&db, taken, Ok(fulfilled())).await;
        let failed = settled(&db, refused, Err("no space".to_owned())).await;
        let again = settled(&db, taken, Ok(fulfilled())).await;

        assert_eq!(happened(&processing), [("moved", RequestState::Processing)]);
        assert_eq!(happened(&failed), [("moved", RequestState::Failed)]);
        assert_eq!(handed_off(&processing), NOTHING);
        assert_eq!(handed_off(&failed), NOTHING);
        assert!(again.value.is_none());
        assert_eq!(again.events.len(), 0);
    }

    #[tokio::test]
    async fn settling_a_request_names_the_plugin_whose_fulfiller_failed_it() {
        let (_data, db) = ada_and_bob().await;
        let id = ask_for(&db, ADA, 949, RequestState::Approved)
            .await
            .value
            .id;
        let failed_by_radarr = answered("radarr", Err("no space".to_owned()));

        let failed = db
            .write(move |tx| Box::pin(settle(tx, id, failed_by_radarr, None)))
            .await
            .unwrap()
            .value
            .unwrap();

        assert_eq!(failed.fulfiller_plugin_id.as_deref(), Some("radarr"));
    }

    #[tokio::test]
    async fn retrying_a_failed_request_approves_and_hands_it_off() {
        let (_data, db) = ada_and_bob().await;
        let id = ask_for(&db, ADA, 949, RequestState::Approved)
            .await
            .value
            .id;
        let pending = ask_for(&db, ADA, 603, RequestState::Pending).await.value.id;
        settled(&db, id, Err("no space".to_owned())).await;

        let retried = db
            .write(move |tx| Box::pin(retry(tx, id, None)))
            .await
            .unwrap();
        let not_failed = db.write(move |tx| Box::pin(retry(tx, pending, None))).await;

        assert_eq!(happened(&retried), [("moved", RequestState::Approved)]);
        assert_eq!(handed_off(&retried), [id]);
        assert_eq!(retried.value.failure_reason, None);
        assert!(
            matches!(not_failed, Err(Error::NotFailed(_))),
            "{not_failed:?}"
        );
    }

    #[tokio::test]
    async fn cancelling_a_request_says_so_whatever_its_state() {
        let (_data, db) = ada_and_bob().await;
        let pending = ask_for(&db, ADA, 949, RequestState::Pending).await.value.id;
        let processing = ask_for(&db, ADA, 603, RequestState::Approved)
            .await
            .value
            .id;
        settled(&db, processing, Ok(fulfilled())).await;
        let declined = ask_for(&db, ADA, 348, RequestState::Pending).await.value.id;
        db.write(move |tx| Box::pin(decline(tx, declined, None)))
            .await
            .unwrap();
        let approved = ask_for(&db, ADA, 680, RequestState::Approved)
            .await
            .value
            .id;
        let failed = ask_for(&db, ADA, 155, RequestState::Approved)
            .await
            .value
            .id;
        settled(&db, failed, Err("no space".to_owned())).await;
        let available = ask_for(&db, ADA, 157, RequestState::Pending).await.value.id;
        match_in_a_library(&db, &[157]).await;
        let partly = db
            .write(|tx| Box::pin(ask_seasons(tx, ADA, dark(), RequestState::Approved)))
            .await
            .unwrap()
            .value[0]
            .id;
        settled(&db, partly, Ok(fulfilled())).await;
        match_dark_episodes(&db, 1, &[1]).await;
        scan_dark(&db, &[1]).await;

        let mut cancelled = Vec::new();
        for id in [
            pending, processing, declined, approved, failed, available, partly,
        ] {
            let applied = db.write(move |tx| Box::pin(cancel(tx, id))).await.unwrap();
            assert_eq!(handed_off(&applied), NOTHING);
            cancelled.extend(happened(&applied));
        }

        assert_eq!(
            cancelled,
            [
                ("cancelled", RequestState::Pending),
                ("cancelled", RequestState::Processing),
                ("cancelled", RequestState::Declined),
                ("cancelled", RequestState::Approved),
                ("cancelled", RequestState::Failed),
                ("cancelled", RequestState::Available),
                ("cancelled", RequestState::PartiallyAvailable)
            ]
        );
    }

    #[tokio::test]
    async fn withdrawing_from_a_request_that_is_not_pending_is_refused() {
        let (_data, db) = ada_and_bob().await;
        let id = ask_for(&db, ADA, 603, RequestState::Approved)
            .await
            .value
            .id;

        let refused = db
            .write(move |tx| Box::pin(withdraw(tx, ADA, None, id)))
            .await;

        assert!(
            matches!(refused, Err(Error::NotPending(asked::Asked::Request, refused_id)) if refused_id == id),
            "{refused:?}"
        );
    }

    /// `user` follows `show`, opening its Followed Show in `state` if it has none.
    async fn follow_as(
        db: &Database,
        user: i64,
        show: WantedSeasons,
        state: RequestState,
    ) -> Applied<(FollowedShow, Vec<Request>)> {
        db.write(move |tx| Box::pin(follow(tx, user, show, state)))
            .await
            .unwrap()
    }

    /// The ids of the Season Requests `followed` asked for.
    fn season_ids(followed: &Applied<(FollowedShow, Vec<Request>)>) -> Vec<i64> {
        followed.value.1.iter().map(|request| request.id).collect()
    }

    /// Breaking Bad, as [`dark`] gives Dark.
    fn breaking_bad() -> WantedSeasons {
        WantedSeasons {
            show_tmdb_id: 1396,
            show_title: "Breaking Bad".to_owned(),
            ..dark()
        }
    }

    #[tokio::test]
    async fn following_a_show_asks_for_each_season_and_hands_them_off_only_if_approved() {
        let (_data, db) = ada_and_bob().await;

        let pending = follow_as(&db, ADA, dark(), RequestState::Pending).await;
        let joined = follow_as(&db, BOB, dark(), RequestState::Pending).await;
        let approved = follow_as(&db, ADA, breaking_bad(), RequestState::Approved).await;

        assert_eq!(
            happened(&pending),
            [
                ("followed", RequestState::Pending),
                ("created", RequestState::Pending),
                ("created", RequestState::Pending)
            ]
        );
        assert_eq!(handed_off(&pending), NOTHING);
        assert_eq!(
            happened(&joined),
            [
                ("joined the Show", RequestState::Pending),
                ("joined", RequestState::Pending),
                ("joined", RequestState::Pending)
            ]
        );
        assert_eq!(handed_off(&joined), NOTHING);
        assert_eq!(
            happened(&approved),
            [
                ("followed", RequestState::Approved),
                ("created", RequestState::Approved),
                ("created", RequestState::Approved)
            ]
        );
        assert_eq!(handed_off(&approved), season_ids(&approved));
        for followed in [pending, joined, approved] {
            assert_eq!(unfollowed(&followed), None);
        }
    }

    #[tokio::test]
    async fn approving_a_pending_followed_show_approves_and_hands_off_its_pending_seasons() {
        let (_data, db) = ada_and_bob().await;
        let followed = follow_as(&db, ADA, dark(), RequestState::Pending).await;
        let id = followed.value.0.id;

        let approved = db
            .write(move |tx| Box::pin(decide_followed(tx, id, RequestState::Approved, None, None)))
            .await
            .unwrap();
        let again = db
            .write(move |tx| Box::pin(decide_followed(tx, id, RequestState::Approved, None, None)))
            .await;

        assert_eq!(
            happened(&approved),
            [
                ("moved the Show", RequestState::Approved),
                ("moved", RequestState::Approved),
                ("moved", RequestState::Approved)
            ]
        );
        assert_eq!(handed_off(&approved), season_ids(&followed));
        assert_eq!(unfollowed(&approved), None);
        assert!(matches!(again, Err(Error::NotPending(..))), "{again:?}");
    }

    #[tokio::test]
    async fn declining_a_pending_followed_show_declines_its_pending_seasons_and_hands_nothing_off()
    {
        let (_data, db) = ada_and_bob().await;
        let id = follow_as(&db, ADA, dark(), RequestState::Pending)
            .await
            .value
            .0
            .id;

        let declined = db
            .write(move |tx| {
                let reason = Some("No.".to_owned());
                Box::pin(decide_followed(
                    tx,
                    id,
                    RequestState::Declined,
                    reason,
                    None,
                ))
            })
            .await
            .unwrap();

        assert_eq!(
            happened(&declined),
            [
                ("moved the Show", RequestState::Declined),
                ("moved", RequestState::Declined),
                ("moved", RequestState::Declined)
            ]
        );
        assert_eq!(handed_off(&declined), NOTHING);
        assert_eq!(unfollowed(&declined), None);
        assert_eq!(declined.value.decline_reason.as_deref(), Some("No."));
    }

    #[tokio::test]
    async fn cancelling_a_followed_show_unfollows_it_only_if_it_was_approved() {
        let (_data, db) = ada_and_bob().await;
        let approved = follow_as(&db, ADA, dark(), RequestState::Approved).await;
        let pending = follow_as(&db, ADA, breaking_bad(), RequestState::Pending).await;
        let (approved_id, pending_id) = (approved.value.0.id, pending.value.0.id);

        let cancelled_approved = db
            .write(move |tx| Box::pin(cancel_followed(tx, approved_id)))
            .await
            .unwrap();
        let cancelled_pending = db
            .write(move |tx| Box::pin(cancel_followed(tx, pending_id)))
            .await
            .unwrap();
        let again = db
            .write(move |tx| Box::pin(cancel_followed(tx, approved_id)))
            .await;

        assert_eq!(
            happened(&cancelled_approved),
            [("cancelled the Show", RequestState::Approved)]
        );
        assert_eq!(
            unfollowed(&cancelled_approved),
            season_ids(&approved).first().copied()
        );
        assert_eq!(
            happened(&cancelled_pending),
            [("cancelled the Show", RequestState::Pending)]
        );
        assert_eq!(unfollowed(&cancelled_pending), None);
        assert_eq!(handed_off(&cancelled_approved), NOTHING);
        assert_eq!(handed_off(&cancelled_pending), NOTHING);
        assert!(matches!(again, Err(Error::NoSuch(..))), "{again:?}");
    }

    #[tokio::test]
    async fn withdrawing_from_a_followed_show_that_is_not_pending_is_allowed() {
        let (_data, db) = ada_and_bob().await;
        let id = follow_as(&db, ADA, dark(), RequestState::Approved)
            .await
            .value
            .0
            .id;
        follow_as(&db, BOB, dark(), RequestState::Pending).await;

        let bob_left = db
            .write(move |tx| Box::pin(withdraw_followed(tx, BOB, None, id)))
            .await
            .unwrap();

        assert_eq!(
            happened(&bob_left),
            [("left the Show", RequestState::Approved)]
        );
        assert_eq!(unfollowed(&bob_left), None);
    }

    #[tokio::test]
    async fn the_last_requester_withdrawing_from_an_approved_followed_show_unfollows_it() {
        let (_data, db) = ada_and_bob().await;
        let followed = follow_as(&db, ADA, dark(), RequestState::Approved).await;
        let id = followed.value.0.id;

        let ada_left = db
            .write(move |tx| Box::pin(withdraw_followed(tx, ADA, None, id)))
            .await
            .unwrap();

        assert_eq!(
            happened(&ada_left),
            [("withdrew the Show", RequestState::Approved)]
        );
        assert_eq!(
            unfollowed(&ada_left),
            season_ids(&followed).first().copied()
        );
    }

    const KINDS: [asked::Asked; 2] = [asked::Asked::Request, asked::Asked::FollowedShow];

    fn user(id: i64, permissions: Permissions) -> User {
        User {
            id,
            ..ada(permissions)
        }
    }

    /// `user` asks for the Movie 949 or follows Dark, as `kind` says, joining it if it is open.
    /// Returns what that applied, with the id of the one asked for as its value.
    async fn ask_kind(db: &Database, kind: asked::Asked, user: i64) -> Applied<i64> {
        match kind {
            asked::Asked::Request => {
                let Applied {
                    value,
                    events,
                    hand_off,
                    unfollow,
                } = ask_for(db, user, 949, RequestState::Pending).await;
                Applied {
                    value: value.id,
                    events,
                    hand_off,
                    unfollow,
                }
            }
            asked::Asked::FollowedShow => {
                let Applied {
                    value: (followed, _),
                    events,
                    hand_off,
                    unfollow,
                } = follow_as(db, user, dark(), RequestState::Pending).await;
                Applied {
                    value: followed.id,
                    events,
                    hand_off,
                    unfollow,
                }
            }
        }
    }

    /// The ids of the Requesters `page` lists for the `kind` with the ID `id`, if it lists it.
    fn listed_requesters(
        page: &ListRequestsResponse,
        kind: asked::Asked,
        id: i64,
    ) -> Option<Vec<i64>> {
        let requesters = match kind {
            asked::Asked::Request => page
                .requests
                .iter()
                .find(|request| request.id == id)
                .map(|request| &request.requesters),
            asked::Asked::FollowedShow => page
                .followed_shows
                .iter()
                .find(|followed| followed.id == id)
                .map(|followed| &followed.requesters),
        }?;
        Some(
            requesters
                .iter()
                .map(|requester| requester.user_id)
                .collect(),
        )
    }

    #[tokio::test]
    async fn requesters_are_listed_in_the_order_they_asked() {
        let (_data, db) = ada_and_bob().await;
        let requests = requests_on(Arc::clone(&db), ExtensionPoints::default());

        for kind in KINDS {
            let id = ask_kind(&db, kind, BOB).await.value;
            ask_kind(&db, kind, ADA).await;
            let page = requests
                .list(&ada(Permissions::ADMIN), None, 100, "")
                .await
                .unwrap();

            assert_eq!(
                listed_requesters(&page, kind, id),
                Some(vec![BOB, ADA]),
                "{kind:?}"
            );
        }
    }

    #[tokio::test]
    async fn asking_for_an_open_one_joins_it_and_says_so() {
        let (_data, db) = ada_and_bob().await;

        for kind in KINDS {
            let opened = ask_kind(&db, kind, ADA).await.value;
            let joined = ask_kind(&db, kind, BOB).await;
            let says = match kind {
                asked::Asked::Request => "joined",
                asked::Asked::FollowedShow => "joined the Show",
            };

            assert_eq!(joined.value, opened, "{kind:?}");
            assert_eq!(
                said(&joined.events).first(),
                Some(&(says, RequestState::Pending)),
                "{kind:?}"
            );
        }
    }

    /// `requester` withdraws from the `kind` with the ID `id`.
    async fn withdraw_kind(
        requests: &Requests,
        kind: asked::Asked,
        requester: &User,
        id: i64,
    ) -> Result<(), Error> {
        match kind {
            asked::Asked::Request => requests.withdraw(requester, id).await,
            asked::Asked::FollowedShow => requests.withdraw_followed(requester, id).await,
        }
    }

    #[tokio::test]
    async fn joining_one_twice_changes_nothing() {
        let (_data, db) = ada_and_bob().await;
        let requests = requests_on(Arc::clone(&db), ExtensionPoints::default());

        for kind in KINDS {
            let opened = ask_kind(&db, kind, ADA).await.value;
            let again = ask_kind(&db, kind, ADA).await;
            let page = requests
                .list(&ada(Permissions::ADMIN), None, 100, "")
                .await
                .unwrap();

            assert_eq!(again.value, opened, "{kind:?}");
            assert_eq!(said(&again.events), [], "{kind:?}");
            assert_eq!(handed_off(&again), NOTHING, "{kind:?}");
            assert_eq!(
                listed_requesters(&page, kind, again.value),
                Some(vec![ADA]),
                "{kind:?}"
            );
        }
    }

    #[tokio::test]
    async fn a_requester_leaving_takes_only_them_off() {
        let (_data, db) = ada_and_bob().await;
        let requests = requests_on(Arc::clone(&db), ExtensionPoints::default());
        let bob = user(BOB, Permissions::from_iter([Permission::ViewRequests]));

        for kind in KINDS {
            let id = ask_kind(&db, kind, ADA).await.value;
            ask_kind(&db, kind, BOB).await;
            withdraw_kind(&requests, kind, &bob, id).await.unwrap();
            let again = withdraw_kind(&requests, kind, &bob, id).await;
            let page = requests
                .list(&ada(Permissions::ADMIN), None, 100, "")
                .await
                .unwrap();

            assert_eq!(
                listed_requesters(&page, kind, id),
                Some(vec![ADA]),
                "{kind:?}"
            );
            assert!(
                matches!(again, Err(Error::NotARequester(k, i)) if k == kind && i == id),
                "{again:?}"
            );
        }
    }

    #[tokio::test]
    async fn the_last_requester_leaving_removes_it() {
        let (_data, db) = ada_and_bob().await;
        let requests = requests_on(Arc::clone(&db), ExtensionPoints::default());

        for kind in KINDS {
            let id = ask_kind(&db, kind, ADA).await.value;
            withdraw_kind(&requests, kind, &ada(Permissions::NEW_USER), id)
                .await
                .unwrap();
            let page = requests
                .list(&ada(Permissions::ADMIN), None, 100, "")
                .await
                .unwrap();

            assert_eq!(listed_requesters(&page, kind, id), None, "{kind:?}");
        }
    }

    #[tokio::test]
    async fn a_non_requester_withdrawing_is_not_found_unless_they_may_see_it_even_once_approved() {
        let (_data, db) = ada_and_bob().await;
        let requests = requests_on(Arc::clone(&db), ExtensionPoints::default());
        let viewer = user(BOB, Permissions::from_iter([Permission::ViewRequests]));
        let non_viewer = user(BOB, Permissions::from_iter([]));

        for kind in KINDS {
            let id = ask_kind(&db, kind, ADA).await.value;
            approve_kind(&requests, kind, id).await.unwrap();
            let unseen = withdraw_kind(&requests, kind, &non_viewer, id).await;
            let seen = withdraw_kind(&requests, kind, &viewer, id).await;

            assert!(
                matches!(unseen, Err(Error::NoSuch(k, i)) if k == kind && i == id),
                "{unseen:?}"
            );
            assert!(
                matches!(seen, Err(Error::NotARequester(k, i)) if k == kind && i == id),
                "{seen:?}"
            );
        }
    }

    /// A Request manager approves the `kind` with the ID `id`.
    async fn approve_kind(requests: &Requests, kind: asked::Asked, id: i64) -> Result<(), Error> {
        let manager = ada(Permissions::ADMIN);
        match kind {
            asked::Asked::Request => requests.approve(&manager, id, None).await.map(drop),
            asked::Asked::FollowedShow => requests
                .approve_followed(&manager, id, None)
                .await
                .map(drop),
        }
    }

    /// Declines the `kind` with the ID `id`, giving `reason`. Returns the reason it keeps.
    async fn decline_kind(
        requests: &Requests,
        kind: asked::Asked,
        id: i64,
        reason: &str,
    ) -> Result<Option<String>, Error> {
        match kind {
            asked::Asked::Request => requests
                .decline(id, reason)
                .await
                .map(|request| request.decline_reason),
            asked::Asked::FollowedShow => requests
                .decline_followed(id, reason)
                .await
                .map(|followed| followed.decline_reason),
        }
    }

    #[tokio::test]
    async fn only_a_pending_one_is_decided_and_declining_keeps_its_reason() {
        let (_data, db) = ada_and_bob().await;
        let requests = requests_on(Arc::clone(&db), ExtensionPoints::default());

        for kind in KINDS {
            let declined = ask_kind(&db, kind, ADA).await.value;
            let reason = decline_kind(&requests, kind, declined, " No. ")
                .await
                .unwrap();
            let approved = ask_kind(&db, kind, ADA).await.value;
            approve_kind(&requests, kind, approved).await.unwrap();
            let approved_again = approve_kind(&requests, kind, declined).await;
            let declined_again = decline_kind(&requests, kind, approved, "").await;

            assert_eq!(reason.as_deref(), Some("No."), "{kind:?}");
            assert!(
                matches!(approved_again, Err(Error::NotPending(k, id)) if k == kind && id == declined),
                "{approved_again:?}"
            );
            assert!(
                matches!(declined_again, Err(Error::NotPending(k, id)) if k == kind && id == approved),
                "{declined_again:?}"
            );
        }
    }

    /// Whether `viewer` sees the `kind` with the ID `id` among the Requests listed.
    async fn sees(requests: &Requests, viewer: &User, kind: asked::Asked, id: i64) -> bool {
        let page = requests.list(viewer, None, 100, "").await.unwrap();
        listed_requesters(&page, kind, id).is_some()
    }

    #[tokio::test]
    async fn only_its_requesters_and_those_who_may_view_requests_see_one() {
        let (_data, db) = ada_and_bob().await;
        let requests = requests_on(Arc::clone(&db), ExtensionPoints::default());
        let nothing = Permissions::from_iter([]);
        let view_requests = Permissions::from_iter([Permission::ViewRequests]);

        for kind in KINDS {
            let id = ask_kind(&db, kind, ADA).await.value;
            let seen = [
                sees(&requests, &ada(nothing), kind, id).await,
                sees(&requests, &user(BOB, nothing), kind, id).await,
                sees(&requests, &user(BOB, view_requests), kind, id).await,
            ];

            assert_eq!(seen, [true, false, true], "{kind:?}");
        }
    }

    #[tokio::test]
    async fn the_pending_count_is_as_many_as_the_pending_list_shows() {
        let (_data, db) = ada_and_bob().await;
        let requests = requests_on(Arc::clone(&db), ExtensionPoints::default());
        ask_for(&db, ADA, 949, RequestState::Pending).await;
        ask_for(&db, ADA, 603, RequestState::Approved).await;
        follow_as(&db, ADA, dark(), RequestState::Pending).await;
        follow_as(&db, ADA, breaking_bad(), RequestState::Approved).await;
        let nothing = Permissions::from_iter([]);
        let viewers = [
            user(BOB, Permissions::from_iter([Permission::ViewRequests])),
            ada(nothing),
            user(BOB, nothing),
        ];

        let mut counted_and_listed = Vec::new();
        for viewer in &viewers {
            let counted = requests.count_pending(viewer).await.unwrap();
            let page = requests
                .list(viewer, Some(RequestState::Pending), 100, "")
                .await
                .unwrap();
            let listed = page.requests.len() + page.followed_shows.len();
            counted_and_listed.push((counted, u32::try_from(listed).unwrap()));
        }

        // Ada's Pending Movie, Followed Show and its two Season Requests.
        assert_eq!(counted_and_listed, [(4, 4), (4, 4), (0, 0)]);
    }

    /// A Shows Fulfiller that takes every Request, and keeps the TMDB id of each Show it is told
    /// to stop following.
    #[derive(Default)]
    struct Unfollowing(std::sync::Mutex<Vec<i64>>);

    impl Fulfiller<Shows> for Unfollowing {
        fn fulfill<'a>(&'a self, _: &'a Request) -> BoxFuture<'a, Result<Fulfilled, PluginError>> {
            Box::pin(async { Ok(fulfilled()) })
        }

        fn unfollow<'a>(&'a self, season: &'a Request) -> BoxFuture<'a, Result<(), PluginError>> {
            self.0
                .lock()
                .unwrap()
                .push(asked_season(season).show_tmdb_id);
            Box::pin(async { Ok(()) })
        }

        fn progress<'a>(
            &'a self,
            _: &'a [String],
        ) -> BoxFuture<'a, Result<HashMap<String, Vec<Download>>, PluginError>> {
            Box::pin(async { Ok(HashMap::new()) })
        }
    }

    /// The TMDB ids of the Shows `unfollowing` was told to stop following, once it was told any,
    /// waiting a second at most.
    async fn unfollowed_shows(unfollowing: &Unfollowing) -> Vec<i64> {
        let told = async {
            while unfollowing.0.lock().unwrap().is_empty() {
                tokio::task::yield_now().await;
            }
        };
        tokio::time::timeout(Duration::from_secs(1), told)
            .await
            .expect("the Shows Fulfiller was never told to stop following a Show");
        unfollowing.0.lock().unwrap().clone()
    }

    /// Requests whose Shows Fulfiller is the returned [`Unfollowing`], with Ada following Dark,
    /// Approved. Returns the Followed Show's id too.
    async fn following_dark() -> (tempfile::TempDir, Requests, Arc<Unfollowing>, i64) {
        let unfollowing = Arc::new(Unfollowing::default());
        let fulfiller = Arc::clone(&unfollowing);
        let plugins = Registry::new();
        plugins
            .enable(Arc::new(FulfillerPlugin("shows", fulfiller)))
            .unwrap();
        let (data, requests) =
            approved_on::<Movies>(&plugins, &[], Arc::new(Asked::default())).await;
        let followed = follow_as(&requests.db, ADA, dark(), RequestState::Approved).await;
        let id = followed.value.0.id;
        (data, requests, unfollowing, id)
    }

    #[tokio::test]
    async fn a_followed_show_cancelled_by_a_caller_who_left_is_published_and_unfollowed() {
        let (_data, requests, unfollowing, id) = following_dark().await;
        let mut published = subscribe(&requests);
        let cancelling = requests.clone();

        leave_once_queued(
            &requests,
            async move { cancelling.cancel_followed(id).await },
        )
        .await;

        let cancelled = next_happened(&mut published).await;
        assert!(
            matches!(cancelled, Happened::FollowedShowCancelled(_)),
            "{cancelled:?}"
        );
        assert_eq!(unfollowed_shows(&unfollowing).await, [dark().show_tmdb_id]);
    }

    #[tokio::test]
    async fn a_followed_show_withdrawn_from_by_its_last_requester_who_left_is_published_and_unfollowed()
     {
        let (_data, requests, unfollowing, id) = following_dark().await;
        let mut published = subscribe(&requests);
        let withdrawing = requests.clone();

        leave_once_queued(&requests, async move {
            withdrawing
                .withdraw_followed(&ada(Permissions::NEW_USER), id)
                .await
        })
        .await;

        let withdrawn = next_happened(&mut published).await;
        assert!(
            matches!(withdrawn, Happened::FollowedShowWithdrawn(_)),
            "{withdrawn:?}"
        );
        assert_eq!(unfollowed_shows(&unfollowing).await, [dark().show_tmdb_id]);
    }

    #[tokio::test]
    async fn upgrading_makes_a_season_a_scan_moved_before_hand_off_pending_and_leaves_handed_off_and_followed_shows_seasons()
     {
        let data_dir = tempfile::tempdir().unwrap();
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(data_dir.path().join(crate::db::FILE_NAME))
            .create_if_missing(true);
        // A raw connection, not `Database::write`: `Database::open` would run every migration.
        let mut before = sqlx::ConnectOptions::connect(&options).await.unwrap();
        // The last migration before stuck Requests went back to Pending.
        sqlx::migrate!().run_to(38, &mut before).await.unwrap();
        sqlx::query(
            "INSERT INTO followed_shows (id, tmdb_id, title, state, created_at)
             VALUES (1, 70523, 'Dark', 'approved', 0);
             INSERT INTO requests (id, tmdb_id, season, title, state, created_at,
                                   fulfiller_reference, followed_show_id)
             VALUES (1, 70523, 1, 'Dark', 'partially_available', 0, NULL, NULL),
                    (2, 70523, 2, 'Dark', 'partially_available', 0, 'show-70523-season-2', NULL),
                    (3, 70523, 3, 'Dark', 'partially_available', 0, NULL, 1)",
        )
        .execute(&mut before)
        .await
        .unwrap();
        sqlx::Connection::close(before).await.unwrap();

        let db = Database::open(data_dir.path()).await.unwrap();
        let states: Vec<(i64, String)> =
            sqlx::query_as("SELECT id, state FROM requests ORDER BY id")
                .fetch_all(db.reader())
                .await
                .unwrap();

        assert_eq!(
            states,
            [
                (1, "pending".to_owned()),
                (2, "partially_available".to_owned()),
                (3, "partially_available".to_owned())
            ]
        );
    }

    /// Dark's Season Requests as a later read shows them, oldest first: each one's Season, state
    /// and Requesters, first asker first.
    async fn dark_seasons(db: &Database) -> Vec<(u32, RequestState, Vec<i64>)> {
        let newest_first = db.write(|tx| Box::pin(load(tx, None, None))).await.unwrap();
        newest_first
            .into_iter()
            .rev()
            .map(|request| {
                let requesters = request.requesters.iter().map(|one| one.user_id).collect();
                let state = request.state.as_known().unwrap_or_default();
                (asked_season(&request).season_number, state, requesters)
            })
            .collect()
    }

    /// The Season Requests for Seasons 1 and 2 of Dark that Ada following it in `state` opens, as
    /// [`dark_seasons`] reads them.
    fn opened_by_ada_following_dark(state: RequestState) -> [(u32, RequestState, Vec<i64>); 2] {
        [1, 2].map(|season| (season, state, vec![ADA]))
    }

    #[tokio::test]
    async fn a_scan_opens_a_new_season_of_an_approved_followed_show_for_its_requesters_in_the_order_they_asked()
     {
        let (_data, db) = ada_and_bob().await;
        follow_as(&db, BOB, dark(), RequestState::Approved).await;
        follow_as(&db, ADA, dark(), RequestState::Pending).await;
        match_dark_episodes(&db, 3, &[1, 2]).await;

        let scanned = scan_dark(&db, &[3]).await;

        assert_eq!(said(&scanned), [("created", RequestState::Available)]);
        assert_eq!(
            dark_seasons(&db).await.last(),
            Some(&(3, RequestState::Available, vec![BOB, ADA]))
        );
    }

    #[tokio::test]
    async fn a_season_a_scan_opens_is_partially_available_or_available_by_its_matched_aired_episodes()
     {
        let (_data, db) = ada_and_bob().await;
        follow_as(&db, ADA, dark(), RequestState::Approved).await;
        match_dark_episodes(&db, 3, &[1]).await;
        match_dark_episodes(&db, 4, &[1, 2]).await;

        let scanned = scan_dark(&db, &[3, 4]).await;

        assert_eq!(
            said(&scanned),
            [
                ("created", RequestState::PartiallyAvailable),
                ("created", RequestState::Available)
            ]
        );
    }

    #[tokio::test]
    async fn a_scan_opens_no_season_without_matched_episodes() {
        let (_data, db) = ada_and_bob().await;
        follow_as(&db, ADA, dark(), RequestState::Approved).await;

        let scanned = scan_dark(&db, &[3]).await;

        assert_eq!(said(&scanned), []);
        assert_eq!(
            dark_seasons(&db).await,
            opened_by_ada_following_dark(RequestState::Approved)
        );
    }

    #[tokio::test]
    async fn a_scan_opens_no_season_for_a_followed_show_not_yet_approved() {
        let (_data, db) = ada_and_bob().await;
        follow_as(&db, ADA, dark(), RequestState::Pending).await;
        match_dark_episodes(&db, 3, &[1, 2]).await;

        let scanned = scan_dark(&db, &[3]).await;

        assert_eq!(said(&scanned), []);
        assert_eq!(
            dark_seasons(&db).await,
            opened_by_ada_following_dark(RequestState::Pending)
        );
    }

    #[tokio::test]
    async fn a_scan_opens_no_season_the_followed_show_already_covers() {
        let (_data, db) = ada_and_bob().await;
        let followed = follow_as(&db, ADA, dark(), RequestState::Approved).await;
        for season in season_ids(&followed) {
            db.write(move |tx| Box::pin(cancel(tx, season)))
                .await
                .unwrap();
        }
        match_dark_episodes(&db, 1, &[1, 2]).await;
        match_dark_episodes(&db, 2, &[1, 2]).await;

        let scanned = scan_dark(&db, &[1, 2]).await;

        assert_eq!(said(&scanned), []);
        assert_eq!(dark_seasons(&db).await, []);
    }

    #[tokio::test]
    async fn a_scan_opens_no_season_that_was_once_available() {
        let (_data, db) = ada_and_bob().await;
        follow_as(&db, ADA, dark(), RequestState::Approved).await;
        let season_3 = WantedSeasons {
            seasons: vec![dark_season(3)],
            ..dark()
        };
        db.write(|tx| Box::pin(ask_seasons(tx, BOB, season_3, RequestState::Pending)))
            .await
            .unwrap();
        match_dark_episodes(&db, 3, &[1, 2]).await;
        scan_dark(&db, &[3]).await;

        let scanned_again = scan_dark(&db, &[3]).await;

        assert_eq!(said(&scanned_again), []);
        assert_eq!(
            dark_seasons(&db).await.last(),
            Some(&(3, RequestState::Available, vec![BOB]))
        );
    }

    #[tokio::test]
    async fn a_season_a_scan_opened_does_not_open_again_once_cancelled() {
        let (_data, db) = ada_and_bob().await;
        follow_as(&db, ADA, dark(), RequestState::Approved).await;
        match_dark_episodes(&db, 3, &[1]).await;
        let opened = scan_dark(&db, &[3]).await;
        let Some(Happened::RequestCreated(created)) =
            opened.first().and_then(|one| one.event.as_ref())
        else {
            panic!("the scan opened no Request: {opened:?}");
        };
        let season_3 = created.request.id;
        db.write(move |tx| Box::pin(cancel(tx, season_3)))
            .await
            .unwrap();

        let scanned_again = scan_dark(&db, &[3]).await;

        assert_eq!(said(&scanned_again), []);
        assert_eq!(
            dark_seasons(&db).await,
            opened_by_ada_following_dark(RequestState::Approved)
        );
    }

    /// Ada follows Dark while Seasons 1 and 2 are out, Pending, then Bob joins her Followed Show
    /// once Season 3 has aired. Returns the Followed Show's id and Season 3's Request id.
    async fn bob_joins_following_dark_after_season_3_aired(db: &Database) -> (i64, i64) {
        follow_as(db, ADA, dark(), RequestState::Pending).await;
        let mut to_season_3 = dark();
        to_season_3.seasons.push(dark_season(3));
        let joined = follow_as(db, BOB, to_season_3, RequestState::Pending).await;
        let season_3 = joined.value.1.last().expect("Season 3's Request");
        assert_eq!(asked_season(season_3).season_number, 3);
        (joined.value.0.id, season_3.id)
    }

    /// Approves the Pending Followed Show `id`, and with it its Pending Season Requests.
    async fn approve_followed(db: &Database, id: i64) {
        db.write(move |tx| Box::pin(decide_followed(tx, id, RequestState::Approved, None, None)))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn a_scan_opens_no_season_asked_for_by_joining_a_followed_show_and_declined() {
        let (_data, db) = ada_and_bob().await;
        let (followed, season_3) = bob_joins_following_dark_after_season_3_aired(&db).await;
        db.write(move |tx| Box::pin(decline(tx, season_3, None)))
            .await
            .unwrap();
        approve_followed(&db, followed).await;
        match_dark_episodes(&db, 3, &[1, 2]).await;

        let scanned = scan_dark(&db, &[3]).await;

        assert_eq!(said(&scanned), []);
        assert_eq!(
            dark_seasons(&db).await.last(),
            Some(&(3, RequestState::Declined, vec![BOB]))
        );
    }

    #[tokio::test]
    async fn a_scan_opens_no_season_asked_for_by_joining_a_followed_show_and_cancelled_once_approved()
     {
        let (_data, db) = ada_and_bob().await;
        let (followed, season_3) = bob_joins_following_dark_after_season_3_aired(&db).await;
        approve_followed(&db, followed).await;
        db.write(move |tx| Box::pin(cancel(tx, season_3)))
            .await
            .unwrap();
        match_dark_episodes(&db, 3, &[1, 2]).await;

        let scanned = scan_dark(&db, &[3]).await;

        assert_eq!(said(&scanned), []);
        assert_eq!(
            dark_seasons(&db).await.last(),
            Some(&(2, RequestState::Approved, vec![ADA, BOB]))
        );
    }

    #[tokio::test]
    async fn asking_for_a_season_on_its_own_leaves_which_seasons_the_followed_show_covers() {
        let (_data, db) = ada_and_bob().await;
        follow_as(&db, ADA, dark(), RequestState::Approved).await;
        let season_3 = WantedSeasons {
            seasons: vec![dark_season(3)],
            ..dark()
        };
        let asked = db
            .write(|tx| Box::pin(ask_seasons(tx, BOB, season_3, RequestState::Pending)))
            .await
            .unwrap();
        let asked_id = asked.value[0].id;
        db.write(move |tx| Box::pin(decline(tx, asked_id, None)))
            .await
            .unwrap();
        match_dark_episodes(&db, 3, &[1, 2]).await;

        let scanned = scan_dark(&db, &[3]).await;

        assert_eq!(said(&scanned), [("created", RequestState::Available)]);
        assert_eq!(
            dark_seasons(&db).await.last(),
            Some(&(3, RequestState::Available, vec![ADA]))
        );
    }
}
