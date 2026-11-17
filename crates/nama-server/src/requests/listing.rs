//! Reading Requests back from the database, and listing them for a viewer.

use std::collections::BTreeMap;

use nama_plugin_api::metadata;
use nama_proto::nama::v1::{
    ExternalId, ListRequestsResponse, PageRequest, QualityChange, Refusal, RefusalReason, Request,
    RequestState, RequestedMovie, RequestedSeason,
};
use sqlx::SqliteConnection;

use super::asked::{self, Asked};
use super::followed::load_followed;
use super::quality::stored_quality;
use super::state::{open_request, read_state, stored};
use super::{Error, Requests};
use crate::page::Page;
use crate::users::{User, may_see};

/// The client API's id `value` in `namespace`.
pub(super) fn title_id(namespace: &str, value: &impl ToString) -> ExternalId {
    ExternalId {
        namespace: namespace.to_owned(),
        value: value.to_string(),
        ..Default::default()
    }
}

/// The client API's id stored as the text form `text`, such as `tmdb-603`.
pub(super) fn read_title_id(text: &str) -> Result<ExternalId, sqlx::Error> {
    let id = text
        .parse::<metadata::ExternalId>()
        .map_err(|err| sqlx::Error::Decode(err.into()))?;
    Ok(title_id(id.namespace(), &id.value()))
}

/// Hands `$query!` the SQL for the Requests and Followed Shows the User `?1` may see, or all if
/// it is NULL, in the state `?2`, or any if it is NULL: one row each of `is_request`, `id` and
/// `created_at`. It asks `may_see`'s question in SQL, so a page reads only the keys it sorts.
macro_rules! listed_sql {
    ($query:ident) => {
        $query!(
            "SELECT 1 AS is_request, id, created_at FROM requests
             WHERE (?2 IS NULL OR state = ?2)
               AND (?1 IS NULL OR id IN (SELECT request_id FROM requesters WHERE user_id = ?1))
             UNION ALL
             SELECT 0, id, created_at FROM followed_shows
             WHERE (?2 IS NULL OR state = ?2)
               AND (?1 IS NULL OR id IN (SELECT followed_show_id FROM followed_show_requesters
                                         WHERE user_id = ?1))"
        )
    };
}

/// The sort key of [`Requests::list`]'s newest-first order, tie-broken by id: when a row opened,
/// and whether it is a Request, which lists before a Followed Show opened in the same second.
type Place = (i64, bool);

impl Requests {
    /// A page of the Requests and Followed Shows `viewer` may see, newest first: only those in
    /// `state` if given, each matched on its own. Only the first page counts them all.
    pub(crate) async fn list(
        &self,
        viewer: &User,
        state: Option<RequestState>,
        asked: &PageRequest,
    ) -> Result<ListRequestsResponse, Error> {
        let state = state.map(stored);
        let list = format!("ListRequests {}", state.unwrap_or_default());
        let page = Page::<Place>::read(asked, list.as_bytes())?;
        let whose = viewer.whose_requests();
        // One snapshot, so the count, the page and its rows agree.
        let mut tx = self.db.reader().begin().await?;
        let mut rows = page_rows(&mut tx, whose, state, &page).await?;
        let mut paging = page.finish(&mut rows, |row| ((row.created_at, row.is_request), row.id));
        if page.is_first() {
            let total = count(&mut tx, whose, state).await?;
            paging.total_size = Some(i32::try_from(total).unwrap_or(i32::MAX));
        }
        let mut listed = ListRequestsResponse {
            page: paging.into(),
            ..Default::default()
        };
        // ponytail: two queries per row, for at most 100 rows; read a page's rows in one query
        // each for Requests and Followed Shows if pages get slow.
        for row in rows {
            if row.is_request {
                let request = load_one(&mut tx, row.id).await?;
                listed
                    .requests
                    .extend(request.map(|request| self.progress.attach(request)));
            } else {
                let followed = load_followed(&mut tx, Some(row.id), None).await?;
                listed.followed_shows.extend(followed);
            }
        }
        Ok(listed)
    }

    /// How many of the Requests and Followed Shows `viewer` may see are Pending, as many as
    /// [`Self::list`] shows them, counting a Followed Show apart from its Season Requests.
    pub(crate) async fn count_pending(&self, viewer: &User) -> Result<u32, sqlx::Error> {
        let mut conn = self.db.reader().acquire().await?;
        count(
            &mut conn,
            viewer.whose_requests(),
            Some(stored(RequestState::Pending)),
        )
        .await
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

    /// The open Request for the Movie `title`, or for its Season `season` if `title` is a Show's,
    /// if `viewer` may see it.
    pub(super) async fn visible_open_request(
        &self,
        conn: &mut SqliteConnection,
        viewer: &User,
        title: &metadata::ExternalId,
        season: Option<u32>,
    ) -> Result<Option<Request>, sqlx::Error> {
        let Some(id) = open_request(conn, title, season).await? else {
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
        let requests = load(conn, id, viewer.whose_requests()).await?;
        Ok(requests
            .into_iter()
            .map(|request| self.progress.attach(request))
            .collect())
    }
}

/// A row of [`Requests::list`], before it is read in full.
struct Row {
    is_request: bool,
    id: i64,
    created_at: i64,
}

/// The rows of `page` of the Requests and Followed Shows the User `whose` may see, or all if
/// `None`, in the stored state `state`, or any if `None`, newest first.
async fn page_rows(
    conn: &mut SqliteConnection,
    whose: Option<i64>,
    state: Option<&str>,
    page: &Page<Place>,
) -> Result<Vec<Row>, sqlx::Error> {
    let (opened_before, request_before, id_before) = match page.after {
        Some(((created_at, is_request), id)) => (Some(created_at), Some(is_request), Some(id)),
        None => (None, None, None),
    };
    // ponytail: SQLite sorts the key of every row the filter lets through, with no index on
    // `created_at`; add one to each table once a Server's Requests number in the tens of thousands.
    macro_rules! page_rows {
        ($listed:literal) => {
            sqlx::query_as!(
                Row,
                r#"SELECT is_request AS "is_request!: bool", id AS "id!",
                          created_at AS "created_at!"
                   FROM ("#
                    + $listed
                    + r#")
                   WHERE ?3 IS NULL OR (created_at, is_request, id) < (?3, ?4, ?5)
                   ORDER BY created_at DESC, is_request DESC, id DESC
                   LIMIT ?6"#,
                whose,
                state,
                opened_before,
                request_before,
                id_before,
                page.limit(),
            )
        };
    }
    listed_sql!(page_rows).fetch_all(conn).await
}

/// How many of the Requests and Followed Shows the User `whose` may see, or all if `None`, are
/// in the stored state `state`, or any if `None`.
async fn count(
    conn: &mut SqliteConnection,
    whose: Option<i64>,
    state: Option<&str>,
) -> Result<u32, sqlx::Error> {
    macro_rules! count {
        ($listed:literal) => {
            sqlx::query_scalar!(
                r#"SELECT COUNT(*) AS "count!: u32" FROM ("# + $listed + ")",
                whose,
                state
            )
        };
    }
    listed_sql!(count).fetch_one(conn).await
}

/// The id a Request is kept under, from its `title_id`, and every id of its title, in namespace
/// order, from the JSON object of its `request_title_ids`.
fn read_ids(key: &str, ids: &str) -> Result<(ExternalId, Vec<ExternalId>), sqlx::Error> {
    let key = read_title_id(key)?;
    // `json_group_object` keeps the rows' order, which isn't namespace order.
    let ids: BTreeMap<String, String> =
        serde_json::from_str(ids).map_err(|err| sqlx::Error::Decode(err.into()))?;
    Ok((
        key,
        ids.iter()
            .map(|(namespace, value)| title_id(namespace, value))
            .collect(),
    ))
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
        r#"SELECT id AS "id!", title_id, season AS "season: u32", title, release_date,
                  (SELECT json_group_object(namespace, value) FROM request_title_ids
                   WHERE request_id = requests.id) AS "title_ids!: String",
                  state, created_at, decline_reason, failure_reason, failure_needs,
                  followed_show_id,
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
    let mut requests = Vec::with_capacity(rows.len());
    for row in rows {
        let (key, ids) = read_ids(&row.title_id, &row.title_ids)?;
        requests.push(Request {
            id: row.id,
            movie_or_season: match row.season {
                Some(season_number) => RequestedSeason {
                    show_id: key.into(),
                    show_ids: ids,
                    season_number,
                    show_title: row.title,
                    air_date: row.release_date,
                    ..Default::default()
                }
                .into(),
                None => RequestedMovie {
                    id: key.into(),
                    ids,
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
            failure: row
                .failure_needs
                .map(|needs| title_id_missing(row.fulfiller_plugin_id.as_deref(), needs))
                .into(),
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
        });
    }
    requests.retain(|request| may_see(requester, &request.requesters));
    Ok(requests)
}

/// Why the Kernel failed a Request without handing it to the Plugin `plugin`'s Fulfiller, which
/// needs an id in one of the comma-separated `namespaces`.
fn title_id_missing(plugin: Option<&str>, namespaces: String) -> Refusal {
    Refusal {
        reason: RefusalReason::REFUSAL_REASON_TITLE_ID_MISSING.into(),
        params: [
            ("pluginId".to_owned(), plugin.unwrap_or_default().to_owned()),
            ("namespaces".to_owned(), namespaces),
        ]
        .into_iter()
        .collect(),
        ..Default::default()
    }
}

#[cfg(test)]
#[path = "listing_tests.rs"]
mod tests;
