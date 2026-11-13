//! Reading Requests back from the database, and listing them for a viewer.

use std::cmp::Reverse;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use nama_proto::nama::v1::{
    FollowedShow, ListRequestsResponse, Permission, QualityChange, Request, RequestState,
    RequestedMovie, RequestedSeason,
};
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;

use super::asked::{self, Asked, may_see};
use super::state::{open_request, read_state, stored};
use super::{Error, Requests, load_followed, stored_quality};
use crate::users::User;

/// Whose Requests `viewer` may see: everyone's (`None`), or only those they are a Requester of.
pub(crate) fn visible_to(viewer: &User) -> Option<i64> {
    (!viewer.permissions.allow(Permission::ViewRequests)).then_some(viewer.id)
}

impl Requests {
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

    /// The Request `id`, if `viewer` may see it.
    pub(crate) async fn get(&self, viewer: &User, id: i64) -> Result<Request, Error> {
        let mut conn = self.db.reader().acquire().await?;
        let found = self.visible(&mut conn, Some(id), viewer).await?;
        found
            .into_iter()
            .next()
            .ok_or(Error::NoSuch(Asked::Request, id))
    }

    /// The open Request for the Movie `tmdb_id`, or for its Season `season` if `tmdb_id` is a Show's,
    /// if `viewer` may see it.
    pub(super) async fn visible_open_request(
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

pub(super) async fn load_one(
    conn: &mut SqliteConnection,
    id: i64,
) -> Result<Option<Request>, sqlx::Error> {
    Ok(load(conn, Some(id), None).await?.into_iter().next())
}

/// The Requests, newest first: only the one with the ID `id` if given, and only those the User
/// `requester` is a Requester of if given.
pub(super) async fn load(
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

#[cfg(test)]
#[path = "listing_tests.rs"]
mod tests;
