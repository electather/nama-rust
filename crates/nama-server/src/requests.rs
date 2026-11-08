//! Requests: Users asking for a Movie, or a Season of a Show, that isn't on the Server to be added
//! to it (ADR 0012).
//!
//! This module is the only code that reads or writes `requests` and `requesters`. It hands
//! Requests out as their proto messages, since those are also what its events carry.

use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use nama_plugin_api::{
    PluginId,
    fulfiller::{Fulfiller, Movies},
    metadata::{Candidate, Episode, ExternalId, SearchQuery, Season, Show, TitleKind},
};
use nama_proto::nama::v1::{
    Event, FoundSeason, Permission, Request, RequestCancelled, RequestCreated, RequestState,
    RequestStateChanged, RequestWithdrawn, RequestedMovie, RequestedSeason, Requester,
    RequesterAdded, RequesterRemoved, event::Event as Happened,
};
use sqlx::SqliteConnection;
use tracing::{error, warn};

use crate::{
    contents::{self, Contents, Metadata},
    db::{Database, now},
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

/// Why a Request couldn't be made, found or changed.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    /// There is no such Request, or none the caller may see.
    #[error("there is no Request {0}")]
    NoSuchRequest(i64),
    #[error("the Request {0} isn't Pending")]
    NotPending(i64),
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
    /// Holds the Movie Fulfiller, if one is enabled.
    plugins: Registry,
    /// The ids of the Requests being handed to a Fulfiller now.
    handing: Arc<Mutex<HashSet<i64>>>,
}

/// A hand-off's hold on one Request in [`Requests::handing`], let go however the hand-off ends, a
/// panic included, so the Request can be handed off again.
struct Claim {
    handing: Arc<Mutex<HashSet<i64>>>,
    id: i64,
}

impl Claim {
    /// Claims the Request `id`, unless another hand-off has it.
    fn take(requests: &Requests, id: i64) -> Option<Self> {
        let claimed = lock(&requests.handing).insert(id);
        claimed.then(|| Self {
            handing: Arc::clone(&requests.handing),
            id,
        })
    }
}

impl Drop for Claim {
    fn drop(&mut self) {
        lock(&self.handing).remove(&self.id);
    }
}

// Nothing panics while it is held, so a poisoned set is still whole.
fn lock(handing: &Mutex<HashSet<i64>>) -> MutexGuard<'_, HashSet<i64>> {
    handing.lock().unwrap_or_else(PoisonError::into_inner)
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
}

/// Seasons of a Show to ask for, as the metadata source gave them.
#[derive(Debug)]
struct WantedSeasons {
    show_tmdb_id: i64,
    show_title: String,
    seasons: Vec<Season>,
}

/// Whose Requests `viewer` may see: everyone's (`None`), or only those they are a Requester of.
pub(crate) fn visible_to(viewer: &User) -> Option<i64> {
    (!viewer.permissions.allow(Permission::ViewRequests)).then_some(viewer.id)
}

impl Requests {
    /// The Requests in `db`, looking Movies up through `matcher`, publishing on `events` and
    /// handing Approved ones to the Movie Fulfiller in `plugins`.
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
        }
    }

    /// Asks for the Movie with the TMDB id `tmdb_id` for `asker`, who joins its open Request, or
    /// opens one: Approved, and handed to the Movie Fulfiller, if `asker` may have Movies approved
    /// as they ask, else Pending. Publishes what changed, if anything did.
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
        };
        let state = opening_state(asker, Permission::AutoApproveMovie);
        let user = asker.id;
        let (request, happened) = self
            .db
            .write(move |tx| Box::pin(ask_movie(tx, user, wanted, state)))
            .await?;
        // Only the asker who opens the Request hands it off, so joining it can't hand it twice.
        let opened = happened
            .as_ref()
            .is_some_and(|happened| matches!(happened.event, Some(Happened::RequestCreated(_))));
        self.events.publish(happened.into_iter().collect());
        if opened && request.state == RequestState::Approved {
            return self.hand_off(request).await;
        }
        Ok(request)
    }

    /// Asks for the Seasons `numbers` of the Show with the TMDB id `tmdb_id` for `asker`, one
    /// Request per Season in Season order, each as [`Self::create`] asks for a Movie but with
    /// `AUTO_APPROVE_TV`. Asks for none if a Library has any of them: every aired Episode Matched.
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
            show_title: show.title,
            seasons,
        };
        let state = opening_state(asker, Permission::AutoApproveTv);
        let user = asker.id;
        let (requests, happened) = self
            .db
            .write(move |tx| Box::pin(ask_seasons(tx, user, wanted, state)))
            .await?;
        self.events.publish(happened);
        Ok(requests)
    }

    /// Approves the Pending Request `id` and hands it to the Movie Fulfiller, publishing each
    /// change of its state.
    pub(crate) async fn approve(&self, id: i64) -> Result<Request, Error> {
        let approved = self.decide(id, RequestState::Approved, None).await?;
        self.hand_off(approved).await
    }

    /// Hands the Approved `request` to the Movie Fulfiller and moves it to Processing, publishing
    /// that its state changed. Returns the Request as it is now. Without a Movie Fulfiller, or
    /// when it doesn't take the Request, the Request waits in Approved. A Season Request always
    /// waits in Approved, and one already being handed off is left to that hand-off.
    ///
    /// Runs in a task of its own: once the Fulfiller has taken the Request, its reference must be
    /// stored even if the caller stops waiting.
    async fn hand_off(&self, request: Request) -> Result<Request, Error> {
        // ponytail: no Show Fulfiller yet, so a Season Request isn't handed to anyone.
        if request.movie.is_unset() {
            return Ok(request);
        }
        let requests = self.clone();
        let handed = tokio::spawn(async move {
            // An approval and enabling the Fulfiller may hand the same Request off at once, which
            // would hand it to the Fulfiller twice.
            let Some(_claim) = Claim::take(&requests, request.id) else {
                return Ok(request);
            };
            requests.hand_off_claimed(request).await
        });
        // The task is never aborted, so it only fails if it panicked.
        handed
            .await
            .unwrap_or_else(|_| Err(sqlx::Error::WorkerCrashed.into()))
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
        drop(conn);
        let Some(reference) = self.fulfill(&current).await else {
            return Ok(current);
        };
        let processing = self
            .db
            .write(move |tx| Box::pin(start_processing(tx, id, reference)))
            .await?;
        let Some(processing) = processing else {
            warn!(
                request = id,
                "the Movie Fulfiller took a Request that was changed meanwhile, and keeps it"
            );
            return Ok(current);
        };
        self.events.publish(vec![event(RequestStateChanged {
            request: processing.clone().into(),
            ..Default::default()
        })]);
        Ok(processing)
    }

    /// Hands each Approved Request, oldest first, to the enabled Plugin `plugin`'s Movie Fulfiller,
    /// if it provides one, as [`Self::hand_off_waiting`] does.
    pub(crate) async fn hand_off_waiting_to(&self, plugin: &PluginId) {
        if self.plugins.provides::<dyn Fulfiller<Movies>>(plugin) {
            self.hand_off_waiting().await;
        }
    }

    /// Hands each Approved Request, oldest first, to the Movie Fulfiller as [`Self::hand_off`]
    /// does, such as those that waited for one to be enabled. A failure is only logged, and leaves
    /// the rest waiting.
    pub(crate) async fn hand_off_waiting(&self) {
        if let Err(err) = self.try_hand_off_waiting().await {
            error!("handing the waiting Requests to the Movie Fulfiller failed: {err}");
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

    /// The reference the Movie Fulfiller gives for `request` once it takes it, if one is enabled
    /// and takes it.
    async fn fulfill(&self, request: &Request) -> Option<String> {
        let fulfilled = self
            .plugins
            .call_provider::<dyn Fulfiller<Movies>, _, _>(|fulfiller| async move {
                fulfiller.fulfill(request).await
            })
            .await?;
        // ponytail: the error is only logged and the Request waits in Approved; #176 makes it
        // Failed with the error as its reason.
        fulfilled
            .inspect_err(|error| {
                warn!(request = request.id, %error, "the Movie Fulfiller didn't take the Request");
            })
            .ok()
    }

    /// Declines the Pending Request `id`, giving its Requesters `reason`, trimmed, unless it is
    /// blank, and publishes that its state changed.
    pub(crate) async fn decline(&self, id: i64, reason: &str) -> Result<Request, Error> {
        let reason = reason.trim();
        let reason = (!reason.is_empty()).then(|| reason.to_owned());
        self.decide(id, RequestState::Declined, reason).await
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
        self.events.publish(vec![event(RequestStateChanged {
            request: request.clone().into(),
            ..Default::default()
        })]);
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

    /// The Requests `viewer` may see, newest first.
    pub(crate) async fn list(&self, viewer: &User) -> Result<Vec<Request>, sqlx::Error> {
        let mut conn = self.db.reader().acquire().await?;
        load(&mut conn, None, visible_to(viewer)).await
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
            found.push(look_around(&mut conn, viewer, tmdb_id, plugin, candidate).await?);
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
        let id = ExternalId::Tmdb(tmdb_id);
        let no_such_show = |err| looking_up(err, Error::NoSuchShow(tmdb_id));
        let (plugin, show) = self.matcher.show(&id).await.map_err(no_such_show)?;
        let mut seasons = Vec::with_capacity(show.seasons.len());
        // ponytail: one metadata lookup per Season, one after another.
        for &number in &show.seasons {
            let season = self.matcher.show_season(&id, number).await;
            seasons.push(season.map_err(no_such_show)?);
        }
        let mut conn = self.db.reader().acquire().await?;
        let seasons = look_around_seasons(&mut conn, viewer, tmdb_id, seasons).await?;
        Ok(FoundShow {
            tmdb_id,
            plugin,
            show,
            seasons,
        })
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
        Ok(look_around(&mut conn, viewer, tmdb_id, plugin, movie).await?)
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
        found.into_iter().next().ok_or(Error::NoSuchRequest(id))
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
            in_library: in_library(&season.episodes, &matched, &today),
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
        show_title,
        seasons,
    } = show;
    let today = today(conn).await?;
    let mut requests = Vec::with_capacity(seasons.len());
    let mut happened = Vec::new();
    for season in seasons {
        let matched = contents::matched_episodes(conn, show_tmdb_id, season.number.into()).await?;
        if in_library(&season.episodes, &matched, &today) {
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
        };
        let (request, event) = ask(conn, user, wanted, state).await?;
        requests.push(request);
        happened.extend(event);
    }
    Ok((requests, happened))
}

/// Whether a Library has the Season with `episodes`, of which those numbered in `matched` are
/// Matched: one has aired by `today` (`YYYY-MM-DD`, as air dates are, so they sort as text), and
/// every one that has is Matched. A Season yet to air isn't in a Library, so it can be Requested.
fn in_library(episodes: &[Episode], matched: &HashSet<i64>, today: &str) -> bool {
    let mut aired = episodes
        .iter()
        .filter(|episode| {
            episode
                .air_date
                .as_deref()
                .is_some_and(|aired| aired <= today)
        })
        .peekable();
    aired.peek().is_some() && aired.all(|episode| matched.contains(&i64::from(episode.number)))
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
        None => {
            sqlx::query_scalar!(
                "INSERT INTO requests (tmdb_id, season, title, release_date, state, created_at)
                 VALUES (?, ?, ?, ?, ?, ?)
                 RETURNING id",
                wanted.tmdb_id,
                wanted.season,
                wanted.title,
                wanted.release_date,
                state,
                now
            )
            .fetch_one(&mut *conn)
            .await?
        }
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
        sqlx::query!("UPDATE requests SET state = 'available' WHERE id = ?", id)
            .execute(&mut *conn)
            .await?;
        let request = load_one(conn, id).await?.ok_or(sqlx::Error::RowNotFound)?;
        happened.push(event(RequestStateChanged {
            request: request.into(),
            ..Default::default()
        }));
    }
    Ok(happened)
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

/// Moves the Approved Request `id` to Processing, keeping the Fulfiller's `reference` with it, and
/// returns it as it is now, or `None` if it isn't Approved any more.
async fn start_processing(
    conn: &mut SqliteConnection,
    id: i64,
    reference: String,
) -> Result<Option<Request>, sqlx::Error> {
    let started = sqlx::query!(
        "UPDATE requests SET state = 'processing', fulfiller_reference = ?
         WHERE id = ? AND state = 'approved'",
        reference,
        id
    )
    .execute(&mut *conn)
    .await?
    .rows_affected()
        > 0;
    if !started {
        return Ok(None);
    }
    load_one(conn, id).await
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
        r#"SELECT id AS "id!", tmdb_id, season AS "season: u32", title, release_date, state,
                  created_at, decline_reason
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
                ..Default::default()
            };
            match row.season {
                Some(season_number) => {
                    request.season = RequestedSeason {
                        show_tmdb_id: row.tmdb_id,
                        season_number,
                        show_title: row.title,
                        air_date: row.release_date,
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
        BoxFuture, Plugin, PluginError, PluginId, Priority, Registrar,
        fulfiller::{Download, Fulfiller, Movies},
        metadata::{Episode, ExternalIds},
    };
    use nama_proto::nama::v1::{Request, RequestState};
    use sqlx::SqliteConnection;
    use tokio::sync::Notify;

    use super::{Requests, Wanted, ask, in_library};
    use crate::{db::Database, events::EventBus, matching::Matcher, plugins::Registry};

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
    fn a_season_is_in_a_library_once_every_aired_episode_is_matched() {
        let airing = [
            episode(1, Some("2026-11-01")),
            episode(2, Some(TODAY)),
            episode(3, Some("2026-11-15")),
            episode(4, None),
        ];

        assert!(in_library(&airing, &HashSet::from([1, 2]), TODAY));
        assert!(!in_library(&airing, &HashSet::from([1, 3, 4]), TODAY));
    }

    #[test]
    fn a_season_yet_to_air_is_not_in_a_library() {
        let announced = [episode(1, Some("2027-01-01")), episode(2, None)];

        assert!(!in_library(&announced, &HashSet::from([1, 2]), TODAY));
        assert!(!in_library(&[], &HashSet::new(), TODAY));
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

    struct GatedPlugin(Arc<Gated>);

    impl Plugin for GatedPlugin {
        fn id(&self) -> PluginId {
            PluginId::from_static("gated")
        }

        fn version(&self) -> &'static str {
            "0.0.0"
        }

        fn register(&self, registrar: &mut Registrar) {
            let gated: Arc<Gated> = Arc::clone(&self.0);
            registrar.provide::<dyn Fulfiller<Movies>>(Priority::DEFAULT, gated);
        }
    }

    #[tokio::test]
    async fn a_request_being_handed_off_is_not_handed_to_the_fulfiller_again() {
        let data = tempfile::tempdir().unwrap();
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let wanted = vec![Wanted {
            tmdb_id: 949,
            season: None,
            title: "Heat".to_owned(),
            release_date: None,
        }];
        db.write(move |tx| Box::pin(ask_each(tx, wanted, RequestState::Approved)))
            .await
            .unwrap();
        let gated = Arc::new(Gated::default());
        let plugins = Registry::new();
        plugins
            .enable(Arc::new(GatedPlugin(Arc::clone(&gated))))
            .unwrap();
        let events = EventBus::new(plugins.clone());
        let matcher = Matcher::new(Arc::clone(&db), plugins.clone(), events.clone());
        let requests = Requests::new(db, matcher, events, plugins);
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
}
