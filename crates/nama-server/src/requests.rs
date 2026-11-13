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
    collections::HashSet,
    iter,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use nama_plugin_api::{
    PluginId,
    fulfiller::Shows,
    metadata::{Episode, ExternalId, Season},
};
use nama_proto::nama::v1::{
    Event, FollowedShow, FollowedShowCancelled, FollowedShowCreated, FollowedShowRequesterAdded,
    FollowedShowRequesterRemoved, FollowedShowStateChanged, FollowedShowWithdrawn, Permission,
    Request, RequestCancelled, RequestState, RequestWithdrawn, RequesterRemoved,
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
mod asking;
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
use asking::{Asker, SeasonsToAsk, Wanted, WantedSeasons, Way, ask_as, opening_state};
use hand_off::HandOff;
pub(crate) use listing::visible_to;
use listing::{load, load_one};
use lookup::looking_up;
pub(crate) use lookup::{Found, FoundShow, Hit};
use progress::Progress;
use quality::{
    NewQuality, change_followed_quality, change_quality, quality_columns, stored_quality,
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

/// Today as `YYYY-MM-DD`, by SQLite's calendar, since `std` has none.
async fn today(conn: &mut SqliteConnection) -> Result<String, sqlx::Error> {
    sqlx::query_scalar!(r#"SELECT date('now') AS "today!: String""#)
        .fetch_one(&mut *conn)
        .await
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
