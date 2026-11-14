//! How much of a title a Library has, and what a Library's write does to Requests with it
//! (ADR 0012).

use std::collections::HashSet;

use nama_plugin_api::metadata::{Episode, ExternalId, ExternalIds, Season};
use nama_proto::nama::v1::{Event, FollowedShow, Request, RequestState};
use sqlx::SqliteConnection;

use super::apply::Applied;
use super::asking::{Asker, SeasonsToAsk, Wanted, ask_as};
use super::followed::{approved_followed_show, covers};
use super::state::{change_state, ids_json, open_requests};
use crate::{db::now, library::contents};

/// Today as `YYYY-MM-DD`, by SQLite's calendar, since `std` has none.
pub(super) async fn today(conn: &mut SqliteConnection) -> Result<String, sqlx::Error> {
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
    /// Every id the Match gave the Show, any of which a Request for it may be kept under.
    show_ids: ExternalIds,
}

impl Asker for Scanning {
    type Error = sqlx::Error;

    fn followed_show(&self) -> Option<&FollowedShow> {
        Some(&self.followed_show)
    }

    async fn opens_in(
        &self,
        conn: &mut SqliteConnection,
        _: &ExternalId,
        season: u32,
        has: Option<RequestState>,
    ) -> Result<Option<RequestState>, sqlx::Error> {
        let skips = has.is_none()
            || covers(conn, self.followed_show.id, season).await?
            || was_available(conn, &self.show_ids, season).await?;
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
pub(super) fn availability(
    episodes: &[Episode],
    matched: &HashSet<i64>,
    today: &str,
) -> Option<RequestState> {
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

/// Makes Available the open Request for each of the Movies `movies`, each by every id its Match
/// gave it, that a Library now has, and returns an event for each (ADR 0012). A Request sharing
/// any id with the Match is for it, whichever id each prefers (ADR 0042). Declined Requests are
/// closed, so they stay as they are.
pub(crate) async fn make_available(
    conn: &mut SqliteConnection,
    movies: Vec<ExternalIds>,
) -> Result<Vec<Event>, sqlx::Error> {
    let mut happened = Vec::new();
    for ids in movies {
        let Some(any) = ids.iter().next() else {
            continue;
        };
        let open = open_requests(conn, &ids, None).await?;
        // Not every Matched id is a Movie in a Library: the Match isn't stored if its Item went
        // since it was decided about, and an Episode's id can equal an unrelated Movie's.
        if open.is_empty() || contents::library_with_movie(conn, &any).await?.is_none() {
            continue;
        }
        for id in open {
            happened.extend(change_state(conn, id, RequestState::Available).await?);
        }
    }
    Ok(happened)
}

/// Makes the open Request for each of `seasons`, as the metadata source gave them just now with
/// every id their Show's Match gave it, Partially Available (if Processing) or Available by how
/// many of its aired Episodes a Library has, and returns an event for each Request that changed
/// (ADR 0012). A Request sharing any id with the Show is for it, as [`make_available`] says.
/// Declined and Available Requests are closed, so Episodes that air later don't reopen them. A
/// Season with no open Request is asked for by a scan, as [`Scanning`] says.
pub(crate) async fn make_seasons_available(
    conn: &mut SqliteConnection,
    seasons: Vec<(ExternalIds, Season)>,
) -> Result<Vec<Event>, sqlx::Error> {
    let today = today(conn).await?;
    let mut happened = Vec::new();
    for (show, season) in seasons {
        let Some(any) = show.iter().next() else {
            continue;
        };
        let matched = contents::matched_episodes(conn, &any, season.number.into()).await?;
        let Some(state) = availability(&season.episodes, &matched, &today) else {
            continue;
        };
        let open = open_requests(conn, &show, Some(season.number)).await?;
        if open.is_empty() {
            happened.extend(scan_season(conn, show, season).await?);
        }
        for id in open {
            happened.extend(change_state(conn, id, state).await?);
        }
    }
    Ok(happened)
}

/// Asks for `season` of the Show with the ids `show`, which a Library's write found no open
/// Request for, by a scan, under the id its Approved Followed Show is kept under and with every id
/// that Followed Show stores and `show` gives. Returns the events for it.
async fn scan_season(
    conn: &mut SqliteConnection,
    show: ExternalIds,
    season: Season,
) -> Result<Vec<Event>, sqlx::Error> {
    // A Show with no Approved Followed Show has no one a scan could ask for.
    let Some((kept_under, followed_show, followed_ids)) =
        approved_followed_show(conn, &show).await?
    else {
        return Ok(Vec::new());
    };
    // The ids the Followed Show stores win over the Match's: its Requests are kept under them.
    let ids = show.iter().chain(followed_ids.iter()).collect();
    let asking = SeasonsToAsk::for_scan(conn, &kept_under, ids, &followed_show, season).await?;
    let scanning = Scanning {
        followed_show,
        show_ids: show,
    };
    let scanned = asking.ask(conn, scanning).await?;
    Ok(scanned.events)
}

/// Whether the Season `season` of the Show with any of the ids `show` ever had an Available
/// Request, so its Followed Show doesn't ask for it again.
async fn was_available(
    conn: &mut SqliteConnection,
    show: &ExternalIds,
    season: u32,
) -> Result<bool, sqlx::Error> {
    let ids = ids_json(show);
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM requests
                          JOIN request_title_ids AS title ON title.request_id = requests.id
                          JOIN json_each(?) AS wanted
                              ON title.namespace = wanted.key AND title.value = wanted.value
                          WHERE requests.season = ? AND requests.state = 'available')
           AS "is!: bool""#,
        ids,
        season
    )
    .fetch_one(&mut *conn)
    .await
}

#[cfg(test)]
#[path = "availability_tests.rs"]
mod tests;
