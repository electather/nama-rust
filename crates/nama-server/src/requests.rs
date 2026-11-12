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
    db::{BoxFuture, Database, detached, now},
    events::{EventBus, event},
    library::contents::{self, Contents, Metadata},
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
#[path = "requests_tests.rs"]
mod tests;
