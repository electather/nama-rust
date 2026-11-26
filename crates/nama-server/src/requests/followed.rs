//! Followed Shows: Users following a Show from their Watchlist, whose later Seasons get Season
//! Requests as a Library scan Matches their Episodes.

use std::iter;

use nama_plugin_api::{
    fulfiller::Shows,
    metadata::{ExternalId, ExternalIds},
};
use nama_proto::nama::v1::{
    Event, FollowedShow, FollowedShowCancelled, FollowedShowCreated, FollowedShowRequesterAdded,
    FollowedShowRequesterRemoved, FollowedShowStateChanged, FollowedShowWithdrawn, Permission,
    Request, RequestState,
};
use sqlx::SqliteConnection;

use super::apply::Applied;
use super::asked::{self, Asked};
use super::asking::{SeasonsToAsk, WantedSeasons, kept_under, opening_state};
use super::deciding::{decide, given_reason};
use super::listing::{load_one, read_title_id};
use super::quality::{NewQuality, change_followed_quality, quality_columns, stored_quality};
use super::state::{ids_json, read_state, state_changed, stored};
use super::{Error, Requests};
use crate::{
    db::now,
    events::event,
    users::{User, may_see},
};

impl Requests {
    /// Follows the Show `show_id` for `asker`, who put it on their Watchlist: joins its open
    /// Followed Show, kept under its id as a Request is ([`kept_under`]), or opens one at the
    /// Quality [`Self::create_seasons`] would pick, in the state their Season Requests open in. It
    /// asks for none of the Seasons out already but covers each, so only a later one is asked for,
    /// by a scan; the User asks for the others from the Show's page.
    ///
    /// Fails as [`Self::create_seasons`] does, and with [`Error::NoSuchShow`] for a Show `asker`'s
    /// Maximum Age keeps from them.
    pub(crate) async fn follow_new_seasons(
        &self,
        asker: &User,
        show_id: &ExternalId,
    ) -> Result<FollowedShow, Error> {
        let quality = self.quality::<Shows>(asker, None).await?;
        let (_, show) = self.show(show_id).await?;
        if self
            .kept_from(asker, show.content_rating.as_deref())
            .await?
        {
            return Err(Error::NoSuchShow(show_id.clone()));
        }
        let latest = show.seasons.iter().copied().max();
        let wanted = WantedSeasons {
            seasons: Vec::new(),
            show_id: kept_under(show_id, &show.ids),
            show_ids: show.ids,
            show_title: show.title,
            quality,
            content_rating: show.content_rating,
        };
        let state = opening_state(asker, Permission::AutoApproveTv);
        let user = asker.id;
        self.apply(move |tx| Box::pin(follow_new_seasons(tx, user, wanted, latest, state)))
            .await
    }

    /// Takes `requester` off the open Followed Show of the Show `show_id`, as
    /// [`Self::withdraw_followed`] does, if they are one of its Requesters.
    pub(crate) async fn stop_following(
        &self,
        requester: &User,
        show_id: &ExternalId,
    ) -> Result<(), Error> {
        let user = requester.id;
        let show = ExternalIds::from_iter([show_id.clone()]);
        self.apply(move |tx| Box::pin(stop_following(tx, user, show)))
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
        let visible = requester.whose_requests();
        self.apply(move |tx| Box::pin(withdraw_followed(tx, user, visible, id)))
            .await
    }
}

/// The Approved Followed Show of the Show with the ids `show`, found by any of them, with the id it
/// is kept under and every id it stores for the Show.
pub(super) async fn approved_followed_show(
    conn: &mut SqliteConnection,
    show: &ExternalIds,
) -> Result<Option<(ExternalId, FollowedShow, ExternalIds)>, sqlx::Error> {
    let show_ids = ids_json(show);
    // ponytail: the first if the Show's ids name two Approved Followed Shows, each kept under
    // another of them.
    let row = sqlx::query!(
        r#"SELECT followed_shows.id AS "id!", show_id FROM followed_shows
           JOIN followed_show_ids AS known ON known.followed_show_id = followed_shows.id
           JOIN json_each(?) AS wanted
               ON known.namespace = wanted.key AND known.value = wanted.value
           WHERE state = 'approved'
           ORDER BY followed_shows.id LIMIT 1"#,
        show_ids
    )
    .fetch_optional(&mut *conn)
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let kept_under =
        (row.show_id.parse::<ExternalId>()).map_err(|err| sqlx::Error::Decode(err.into()))?;
    let ids = followed_show_ids(conn, row.id).await?;
    let followed = load_followed(conn, Some(row.id), None).await?.pop();
    Ok(followed.map(|followed| (kept_under, followed, ids)))
}

/// Every id the Followed Show `id` stores for its Show.
pub(super) async fn followed_show_ids(
    conn: &mut SqliteConnection,
    id: i64,
) -> Result<ExternalIds, sqlx::Error> {
    let rows = sqlx::query!(
        "SELECT namespace, value FROM followed_show_ids WHERE followed_show_id = ?",
        id
    )
    .fetch_all(&mut *conn)
    .await?;
    rows.into_iter()
        .map(|row| {
            ExternalId::new(&row.namespace, row.value)
                .map_err(|err| sqlx::Error::Decode(err.into()))
        })
        .collect()
}

/// Records that the Followed Show `id` covers the Season `season` from then on, so a scan doesn't
/// open it, nor open it again once someone declined, cancelled or withdrew from it.
pub(super) async fn cover(
    conn: &mut SqliteConnection,
    id: i64,
    season: u32,
) -> Result<(), sqlx::Error> {
    // Joining may ask for a Season before the latest one covered, which stays covered.
    sqlx::query!(
        // `.sqlx/` keys this query's data by its text, indent included.
        "UPDATE followed_shows SET latest_season = ?
                 WHERE id = ? AND (latest_season IS NULL OR latest_season < ?)",
        season,
        id,
        season
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Whether the Followed Show `id` covers the Season `season`: it is numbered no later than the
/// latest one the Followed Show asked for or skipped.
pub(super) async fn covers(
    conn: &mut SqliteConnection,
    id: i64,
    season: u32,
) -> Result<bool, sqlx::Error> {
    // ponytail: a Season that came out but was skipped, such as one numbered before the latest
    // that came out later, gets no Request from a scan; ask for it on its own.
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM followed_shows WHERE id = ? AND latest_season >= ?)
           AS "is!: bool""#,
        id,
        season
    )
    .fetch_one(&mut *conn)
    .await
}

/// Adds `user` as a Requester of the open Followed Show for `show`, opening one in `state` if there
/// is none, and covers the Season `latest` with it, so a scan asks only for later ones. Returns the
/// Followed Show, with its event if it opened or `user` joined it.
pub(super) async fn follow_new_seasons(
    conn: &mut SqliteConnection,
    user: i64,
    show: WantedSeasons,
    latest: Option<u32>,
    state: RequestState,
) -> Result<Applied<FollowedShow>, Error> {
    let asking = SeasonsToAsk::read(conn, show).await?;
    let (followed, happened) = open_or_join(conn, user, &asking, state).await?;
    if let Some(latest) = latest {
        cover(conn, followed.id, latest).await?;
    }
    Ok(Applied {
        events: happened,
        ..Applied::quiet(followed)
    })
}

/// Adds `user` as a Requester of the open Followed Show for `show`, opening one in `state` if there
/// is none. Returns it with its event if it opened or `user` joined it.
async fn open_or_join(
    conn: &mut SqliteConnection,
    user: i64,
    show: &SeasonsToAsk,
    state: RequestState,
) -> Result<(FollowedShow, Vec<Event>), sqlx::Error> {
    let now = now();
    let already_open = open_followed_show_by_any(conn, &show.show_ids).await?;
    let id = if let Some(id) = already_open {
        store_ids(conn, id, &show.show_ids).await?;
        id
    } else {
        open_followed(conn, show, state, now).await?
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
    Ok((followed, happened))
}

/// The open Followed Show of the Show with the ids `show`, found by any of them, if `user` is one
/// of its Requesters.
pub(super) async fn followed_by(
    conn: &mut SqliteConnection,
    show: &ExternalIds,
    user: i64,
) -> Result<Option<FollowedShow>, sqlx::Error> {
    let Some(id) = open_followed_show_by_any(conn, show).await? else {
        return Ok(None);
    };
    Ok(load_followed(conn, Some(id), Some(user)).await?.pop())
}

/// Takes `user` off the open Followed Show of the Show with the ids `show`, as
/// [`withdraw_followed`] does, if they are one of its Requesters.
async fn stop_following(
    conn: &mut SqliteConnection,
    user: i64,
    show: ExternalIds,
) -> Result<Applied<()>, Error> {
    match followed_by(conn, &show, user).await? {
        Some(followed) => withdraw_followed(conn, user, Some(user), followed.id).await,
        None => Ok(Applied::quiet(())),
    }
}

/// Opens a Followed Show of `show` at the Quality its Seasons take, in `state`, storing every id of
/// the Show, and returns its id.
async fn open_followed(
    conn: &mut SqliteConnection,
    show: &SeasonsToAsk,
    state: RequestState,
    now: i64,
) -> Result<i64, sqlx::Error> {
    let (quality_id, quality_name, quality_profile_id) = quality_columns(show.quality.taken());
    let show_id = show.show_id.to_string();
    let id = sqlx::query_scalar!(
        "INSERT INTO followed_shows (show_id, title, state, latest_season, created_at, quality_id,
                                     quality_name, quality_profile_id, content_rating)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
         RETURNING id",
        show_id,
        show.show_title,
        stored(state),
        show.seasons.iter().map(|season| season.number).max(),
        now,
        quality_id,
        quality_name,
        quality_profile_id,
        show.content_rating
    )
    .fetch_one(&mut *conn)
    .await?;
    store_ids(conn, id, &show.show_ids).await?;
    Ok(id)
}

/// Stores each of `ids` whose namespace the Followed Show `id` has no id in yet.
async fn store_ids(
    conn: &mut SqliteConnection,
    id: i64,
    ids: &ExternalIds,
) -> Result<(), sqlx::Error> {
    // ponytail: two values for one namespace keep the first; a Show re-keyed by its source keeps
    // the stale one until something rewrites them.
    for known in ids.iter() {
        sqlx::query!(
            "INSERT INTO followed_show_ids (followed_show_id, namespace, value) VALUES (?, ?, ?)
             ON CONFLICT (followed_show_id, namespace) DO NOTHING",
            id,
            known.namespace(),
            known.value()
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// The open Followed Show of the Show `show`, found by any id it stores.
pub(super) async fn open_followed_show(
    conn: &mut SqliteConnection,
    show: &ExternalId,
) -> Result<Option<i64>, sqlx::Error> {
    open_followed_show_by_any(conn, &ExternalIds::from_iter([show.clone()])).await
}

/// The open Followed Show of the Show with the ids `show`, found by any of them.
pub(super) async fn open_followed_show_by_any(
    conn: &mut SqliteConnection,
    show: &ExternalIds,
) -> Result<Option<i64>, sqlx::Error> {
    let show_ids = ids_json(show);
    macro_rules! find {
        ($open:literal) => {
            sqlx::query_scalar!(
                r#"SELECT DISTINCT followed_shows.id AS "id!" FROM followed_shows
                   JOIN followed_show_ids AS known ON known.followed_show_id = followed_shows.id
                   JOIN json_each(?) AS wanted
                       ON known.namespace = wanted.key AND known.value = wanted.value
                   WHERE "#
                    + $open,
                show_ids
            )
        };
    }
    // ponytail: the first if the Show's ids name two open Followed Shows.
    super::state::open_followed_show_sql!(find)
        .fetch_optional(&mut *conn)
        .await
}

/// Moves the Pending Followed Show `id`, and each Pending Season Request it opened, to `state`
/// with `reason`, first changing its Show's Quality to `new_quality` if given, and names those
/// Requests for a Hand-off if it approved them. Returns the Followed Show as it is now, with its
/// event first.
pub(super) async fn decide_followed(
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
        removed: Vec::new(),
    })
}

/// `request` as its Fulfiller is handed it: naming its Followed Show only while that is Approved,
/// which asks the Fulfiller to follow the Show.
// ponytail: only a hand-off asks to follow, so a Followed Show that hands nothing off, one whose
// Requesters asked for no Season through it, as when its aired Seasons are all in a Library or
// already have open Requests, isn't followed until a Fulfiller contract call asks to follow
// without a hand-off.
pub(super) async fn as_handed_off(
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
    let removed = asked::remove(conn, Asked::FollowedShow, id).await?;
    let cancelled = event(FollowedShowCancelled {
        followed_show: followed.into(),
        ..Default::default()
    });
    Ok(Applied {
        unfollow: followed_season,
        removed: vec![removed],
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
    // ponytail: once every Season Request it opened is cancelled its Fulfiller isn't told to stop;
    // build one from its `followed_show_ids` if that bites.
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
    if let Some(removed) = asked::leave(conn, Asked::FollowedShow, id, user).await? {
        let withdrawn = event(FollowedShowWithdrawn {
            followed_show: followed.into(),
            ..Default::default()
        });
        return Ok(Applied {
            unfollow: followed_season,
            removed: vec![removed],
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
pub(super) async fn load_followed(
    conn: &mut SqliteConnection,
    id: Option<i64>,
    requester: Option<i64>,
) -> Result<Vec<FollowedShow>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT id AS "id!", show_id, title, state, created_at, decline_reason, quality_id,
                  quality_name, quality_profile_id
           FROM followed_shows
           WHERE ?1 IS NULL OR id = ?1
           ORDER BY id DESC"#,
        id
    )
    .fetch_all(&mut *conn)
    .await?;
    let mut requesters = asked::requesters(conn, Asked::FollowedShow, id).await?;
    let mut followed = Vec::with_capacity(rows.len());
    for row in rows {
        followed.push(FollowedShow {
            id: row.id,
            show_id: read_title_id(&row.show_id)?.into(),
            show_title: row.title,
            state: read_state(&row.state).into(),
            requesters: requesters.remove(&row.id).unwrap_or_default(),
            created_at: row.created_at,
            decline_reason: row.decline_reason,
            quality: stored_quality(row.quality_id, row.quality_name, row.quality_profile_id)
                .into(),
            ..Default::default()
        });
    }
    followed.retain(|followed| may_see(requester, &followed.requesters));
    Ok(followed)
}

#[cfg(test)]
#[path = "followed_tests.rs"]
mod tests;
