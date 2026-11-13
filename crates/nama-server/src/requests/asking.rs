//! A User asking for a Movie or for Seasons of a Show, which opens or joins Requests.

use std::collections::BTreeSet;

use nama_plugin_api::{
    fulfiller::{Movies, Shows},
    metadata::{ExternalId, ExternalIds, IMDB, Season, TVDB},
};
use nama_proto::nama::v1::{
    FollowedShow, Permission, Request, RequestCreated, RequestState, Requester, RequesterAdded,
};
use sqlx::SqliteConnection;

use super::apply::Applied;
use super::asked::{self, Asked};
use super::availability::{availability, today};
use super::deciding::approve;
use super::listing::load_one;
use super::lookup::looking_up;
use super::quality::{NewQuality, quality_columns, show_requests};
use super::state::{open_request, stored};
use super::{Error, Requests, qualities};
use crate::{db::now, events::event, library::contents, users::User};

/// What a new Request asks for, as the metadata source gave it.
#[derive(Debug)]
pub(super) struct Wanted {
    /// The Movie's TMDB id, or for a Season its Show's.
    pub(super) tmdb_id: i64,
    /// The Season's number, or `None` for a Movie.
    pub(super) season: Option<u32>,
    /// The Movie's title, or for a Season its Show's.
    pub(super) title: String,
    /// The Movie's `YYYY-MM-DD` release date, or the Season's first air date.
    pub(super) release_date: Option<String>,
    /// For a Season, its Show's ids if its asker knows them; none for a Movie.
    pub(super) show_ids: ExternalIds,
    /// The Followed Show a new Request for it is opened for, if any.
    pub(super) followed_show: Option<i64>,
    /// What its asker's ask says about Quality.
    pub(super) quality: qualities::Ask,
}

/// Seasons of a Show to ask for, as the metadata source gave them.
#[derive(Debug)]
pub(super) struct WantedSeasons {
    pub(super) show_tmdb_id: i64,
    pub(super) show_ids: ExternalIds,
    pub(super) show_title: String,
    pub(super) seasons: Vec<Season>,
    /// What the asker's ask says about Quality, which gives way to the Show's if it has one.
    pub(super) quality: qualities::Ask,
}

impl Requests {
    /// Asks for the Movie with the TMDB id `tmdb_id` for `asker`, who joins its open Request, or
    /// opens one at the Quality `chosen`, else as [`Self::quality`] picks: Approved, and handed
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
        if self
            .contents
            .library_with_movie(&ExternalId::tmdb(tmdb_id))
            .await?
            .is_some()
        {
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
        let id = ExternalId::tmdb(tmdb_id);
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
}

/// The state `asker`'s new Request opens in: Approved if they pass a check for `auto_approve`,
/// else Pending.
pub(super) fn opening_state(asker: &User, auto_approve: Permission) -> RequestState {
    if asker.permissions.allow(auto_approve) {
        RequestState::Approved
    } else {
        RequestState::Pending
    }
}

/// [`ask_approving`]s for the Movie `wanted`, unless a Library has it.
pub(super) async fn ask_movie(
    conn: &mut SqliteConnection,
    user: i64,
    wanted: Wanted,
    state: RequestState,
) -> Result<Applied<Request>, Error> {
    if contents::library_with_movie(conn, &ExternalId::tmdb(wanted.tmdb_id))
        .await?
        .is_some()
    {
        return Err(Error::InLibrary(wanted.tmdb_id));
    }
    ask_approving(conn, user, wanted, state).await
}

/// Asks for each of the Seasons `show` as `user`, whose new Requests open in `state`, unless a
/// Library has any of them. Returns the Requests in the order of `show.seasons`.
pub(super) async fn ask_seasons(
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
/// A User asks a [`Way`]; a Library's write asks `availability::Scanning`.
pub(super) trait Asker {
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
pub(super) enum Way<'a> {
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

/// Seasons of a Show to ask for, at the Quality the Show's Quality rules give their new Requests.
#[derive(Debug)]
pub(super) struct SeasonsToAsk {
    pub(super) show_tmdb_id: i64,
    /// The Show's ids, or `None` for a scan.
    // ponytail: a Followed Show stores no Show ids, and a Season a scan asks for opens already in a
    // Library.
    pub(super) show_ids: Option<ExternalIds>,
    pub(super) show_title: String,
    pub(super) seasons: Vec<Season>,
    pub(super) quality: qualities::Ask,
}

impl SeasonsToAsk {
    /// `show`'s Seasons at the Show's one Quality, refusing another Quality the asker named.
    pub(super) async fn read(
        conn: &mut SqliteConnection,
        show: WantedSeasons,
    ) -> Result<Self, Error> {
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
    pub(super) async fn for_scan(
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
    pub(super) async fn ask<A: Asker>(
        self,
        conn: &mut SqliteConnection,
        asking: A,
    ) -> Result<Applied<Vec<Request>>, A::Error> {
        let show = self.show_tmdb_id;
        let today = today(conn).await?;
        let mut asked = Applied::quiet(Vec::with_capacity(self.seasons.len()));
        for season in &self.seasons {
            let matched =
                contents::matched_episodes(conn, &ExternalId::tmdb(show), season.number.into())
                    .await?;
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
        wanted.show_ids.get(IMDB).map(|id| id.value().to_owned()),
        wanted
            .show_ids
            .get(TVDB)
            .and_then(|id| id.value().parse::<i64>().ok()),
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
pub(super) async fn ask(
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
pub(super) async fn ask_as(
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

#[cfg(test)]
#[path = "asking_tests.rs"]
mod tests;
