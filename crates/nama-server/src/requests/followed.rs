//! Followed Shows: Users asking for a whole Show, whose later Seasons get Season Requests as a
//! Library scan Matches their Episodes.

use std::iter;

use nama_plugin_api::{
    fulfiller::Shows,
    metadata::{ExternalId, ExternalIds},
};
use nama_proto::nama::v1::{
    FollowedShow, FollowedShowCancelled, FollowedShowCreated, FollowedShowRequesterAdded,
    FollowedShowRequesterRemoved, FollowedShowStateChanged, FollowedShowWithdrawn, Permission,
    Request, RequestState,
};
use sqlx::SqliteConnection;

use super::apply::Applied;
use super::asked::{self, Asked, may_see};
use super::asking::{SeasonsToAsk, WantedSeasons, Way, kept_under, opening_state};
use super::deciding::{decide, given_reason};
use super::listing::{load_one, read_title_id, visible_to};
use super::quality::{NewQuality, change_followed_quality, quality_columns, stored_quality};
use super::state::{ids_json, read_state, state_changed, stored};
use super::{Error, Requests};
use crate::{db::now, events::event, users::User};

impl Requests {
    /// Asks for the whole Show `show_id` for `asker`, kept under its id as a Request is
    /// ([`kept_under`]): each current Season not in a Library as [`Self::create_seasons`] asks
    /// for it, at the Quality it picks, and later ones through the Show's open Followed Show,
    /// which `asker` joins or opens at that Quality, in the state their Season Requests open in.
    /// Returns the Followed Show and the Season Requests in Season order.
    ///
    /// Refuses a `chosen` Quality as [`Self::create_seasons`] does.
    pub(crate) async fn follow(
        &self,
        asker: &User,
        show_id: &ExternalId,
        chosen: Option<i64>,
    ) -> Result<(FollowedShow, Vec<Request>), Error> {
        let quality = self.quality::<Shows>(asker, chosen).await?;
        let (source, show) = self.show(show_id).await?;
        let id = kept_under(show_id, &show.ids);
        let wanted = WantedSeasons {
            seasons: self.seasons_from(&source, show_id, &show).await?,
            show_id: id,
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

/// Whether the Followed Show `id` covers the Season `season`: it is numbered no later than the
/// latest one the Followed Show asked for.
pub(super) async fn covers(
    conn: &mut SqliteConnection,
    id: i64,
    season: u32,
) -> Result<bool, sqlx::Error> {
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

/// Adds `user` as a Requester of the open Followed Show for `show`, opening one in `state` if there
/// is none, then asks for each of its Seasons not in a Library by following it. Returns the
/// Followed Show and the Season Requests, with the Followed Show's event first.
pub(super) async fn follow(
    conn: &mut SqliteConnection,
    user: i64,
    show: WantedSeasons,
    state: RequestState,
) -> Result<Applied<(FollowedShow, Vec<Request>)>, Error> {
    let now = now();
    let asking = SeasonsToAsk::read(conn, show).await?;
    let already_open = open_followed_show_by_any(conn, &asking.show_ids).await?;
    let id = if let Some(id) = already_open {
        store_ids(conn, id, &asking.show_ids).await?;
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
                                     quality_name, quality_profile_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)
         RETURNING id",
        show_id,
        show.show_title,
        stored(state),
        show.seasons.iter().map(|season| season.number).max(),
        now,
        quality_id,
        quality_name,
        quality_profile_id
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
    })
}

/// `request` as its Fulfiller is handed it: naming its Followed Show only while that is Approved,
/// which asks the Fulfiller to follow the Show.
// ponytail: only a hand-off asks to follow, so a Followed Show that hands nothing off, such as one
// whose aired Seasons are all in a Library or already have open Requests, isn't followed until a
// Fulfiller contract call asks to follow without a hand-off.
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
