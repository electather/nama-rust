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
    collections::{BTreeSet, HashSet},
    iter,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use nama_plugin_api::{
    PluginId,
    fulfiller::{Movies, Shows},
    metadata::{Episode, ExternalId, ExternalIds, Season},
};
use nama_proto::nama::v1::{
    Event, FollowedShow, FollowedShowCancelled, FollowedShowCreated, FollowedShowRequesterAdded,
    FollowedShowRequesterRemoved, FollowedShowStateChanged, FollowedShowWithdrawn, Permission,
    Request, RequestCancelled, RequestCreated, RequestState, RequestWithdrawn, Requester,
    RequesterAdded, RequesterRemoved,
};
use sqlx::SqliteConnection;
use tokio::{sync::mpsc::UnboundedReceiver, task::JoinHandle};

use crate::{
    db::{Database, now},
    events::{EventBus, event},
    library::contents::{self, Contents},
    metadata::MetadataSource,
    plugins::ExtensionPoints,
    users::User,
};

mod apply;
mod asked;
mod hand_off;
mod listing;
mod lookup;
mod progress;
mod quality;
mod state;
// Neighbours address it as `requests::qualities`: its names (`list`, `Refused`) read only with it.
pub(crate) mod qualities;

use apply::Applied;
pub(crate) use asked::{Asked, may_see};
use hand_off::HandOff;
pub(crate) use listing::visible_to;
use listing::{load, load_one};
use lookup::looking_up;
pub(crate) use lookup::{Found, FoundShow, Hit};
use progress::Progress;
use quality::{
    NewQuality, change_followed_quality, change_quality, quality_columns, show_requests,
    stored_quality,
};
use state::{change_state, open_request, read_state, state_changed, stored};

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

/// A Movie, or a Season of a Show, as a Request names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct RequestTitle {
    /// The Movie's TMDB id, or the Show's.
    pub(crate) tmdb_id: i64,
    /// The Season's number, `None` for a Movie.
    pub(crate) season: Option<u32>,
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
    state::open_followed_show_sql!(find)
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

/// Locks `mutex`, taking its value even if poisoned: nothing panics while one is held, so it is
/// still whole.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod testing;
#[cfg(test)]
#[path = "requests_tests.rs"]
mod tests;
