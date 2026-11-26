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
use super::state::{open_request, open_requests, open_season_requests, read_state, stored};
use super::{Error, Requests};
use crate::content_ratings::MaximumAge;
use crate::page::Page;
use crate::users::{User, is_requester, maximum_age, may_see};

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

/// Hands `$query!` the SQL for the Requests and Followed Shows a viewer may see, as [`Seen`]
/// binds them to `?1` and `?3` to `?5`, in the state `?2`, or any if it is NULL: one row each of
/// `is_request`, `id` and `created_at`. It asks `may_see`'s question in SQL, so a page reads only
/// the keys it sorts.
macro_rules! listed_sql {
    ($query:ident) => {
        $query!(
            "SELECT 1 AS is_request, id, created_at FROM requests
             WHERE (?2 IS NULL OR state = ?2)
               AND (?1 IS NULL OR id IN (SELECT request_id FROM requesters WHERE user_id = ?1))
               AND (?3 IS NULL OR content_rating IN (SELECT value FROM json_each(?3))
                    OR (content_rating IS NULL AND ?4)
                    OR id IN (SELECT request_id FROM requesters WHERE user_id = ?5))
             UNION ALL
             SELECT 0, id, created_at FROM followed_shows
             WHERE (?2 IS NULL OR state = ?2)
               AND (?1 IS NULL OR id IN (SELECT followed_show_id FROM followed_show_requesters
                                         WHERE user_id = ?1))
               AND (?3 IS NULL OR content_rating IN (SELECT value FROM json_each(?3))
                    OR (content_rating IS NULL AND ?4)
                    OR id IN (SELECT followed_show_id FROM followed_show_requesters
                              WHERE user_id = ?5))"
        )
    };
}

/// Whose Requests and Followed Shows a viewer's list holds, as [`listed_sql!`] binds it: those
/// [`User::whose_requests`] lets them see, less others' for a title over their Maximum Age (#1414).
/// Their own stay, as they asked while it let them.
struct Seen {
    whose: Option<i64>,
    /// The stored Content Ratings their Maximum Age lets through, as a JSON array, or `None` to
    /// let every one through. Judged in Rust, as `minimum_age` is, for SQL to match.
    ratings: Option<String>,
    allow_unrated: bool,
    viewer: i64,
}

impl Seen {
    /// What `viewer`, whose Maximum Age is `max_age`, sees, read in the snapshot `conn` is in.
    async fn read(
        conn: &mut SqliteConnection,
        viewer: &User,
        max_age: MaximumAge,
    ) -> Result<Self, sqlx::Error> {
        let whose = viewer.whose_requests();
        let ratings = if whose.is_some() || max_age.lets_every_title() {
            None
        } else {
            let stored = sqlx::query_scalar!(
                r#"SELECT content_rating AS "content_rating!" FROM requests
                   WHERE content_rating IS NOT NULL
                   UNION
                   SELECT content_rating FROM followed_shows WHERE content_rating IS NOT NULL"#
            )
            .fetch_all(conn)
            .await?;
            let allowed = stored.iter().filter(|rating| max_age.allows(Some(rating)));
            Some(serde_json::Value::from_iter(allowed.map(String::as_str)).to_string())
        };
        Ok(Self {
            whose,
            ratings,
            allow_unrated: max_age.allow_unrated,
            viewer: viewer.id,
        })
    }
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
        let max_age = maximum_age(&self.db, viewer.id).await?;
        // One snapshot, so the count, the page and its rows agree.
        let mut tx = self.db.reader().begin().await?;
        let seen = Seen::read(&mut tx, viewer, max_age).await?;
        let mut rows = page_rows(&mut tx, &seen, state, &page).await?;
        let mut paging = page.finish(&mut rows, |row| ((row.created_at, row.is_request), row.id));
        if page.is_first() {
            let total = count(&mut tx, &seen, state).await?;
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
                let request = (self.visible_under(&mut tx, Some(row.id), viewer, max_age)).await?;
                listed.requests.extend(request);
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
        let max_age = maximum_age(&self.db, viewer.id).await?;
        let mut tx = self.db.reader().begin().await?;
        let seen = Seen::read(&mut tx, viewer, max_age).await?;
        count(&mut tx, &seen, Some(stored(RequestState::Pending))).await
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

    /// The open Request for the earliest Season of the Show with any of `ids` that has one `viewer`
    /// may see.
    pub(super) async fn visible_open_season_request(
        &self,
        conn: &mut SqliteConnection,
        viewer: &User,
        ids: &metadata::ExternalIds,
    ) -> Result<Option<Request>, sqlx::Error> {
        let open = open_season_requests(conn, ids).await?;
        self.first_visible(conn, viewer, open).await
    }

    /// The open Request for the Movie with any of `ids` that `viewer` may see.
    pub(super) async fn visible_open_movie_request(
        &self,
        conn: &mut SqliteConnection,
        viewer: &User,
        ids: &metadata::ExternalIds,
    ) -> Result<Option<Request>, sqlx::Error> {
        let open = open_requests(conn, ids, None).await?;
        self.first_visible(conn, viewer, open).await
    }

    /// The first of the Requests `ids` that `viewer` may see.
    async fn first_visible(
        &self,
        conn: &mut SqliteConnection,
        viewer: &User,
        ids: Vec<i64>,
    ) -> Result<Option<Request>, sqlx::Error> {
        for id in ids {
            if let Some(request) = self.visible(conn, Some(id), viewer).await?.pop() {
                return Ok(Some(request));
            }
        }
        Ok(None)
    }

    /// The Requests `viewer` may see, newest first, only the one with the ID `id` if given, each
    /// with what its Fulfiller is downloading for it.
    ///
    /// The one read of a Request for a viewer, so none misses its downloads or the viewer's
    /// Maximum Age.
    async fn visible(
        &self,
        conn: &mut SqliteConnection,
        id: Option<i64>,
        viewer: &User,
    ) -> Result<Vec<Request>, sqlx::Error> {
        let max_age = maximum_age(&self.db, viewer.id).await?;
        self.visible_under(conn, id, viewer, max_age).await
    }

    /// [`Self::visible`], for a viewer whose Maximum Age is `max_age`.
    async fn visible_under(
        &self,
        conn: &mut SqliteConnection,
        id: Option<i64>,
        viewer: &User,
        max_age: MaximumAge,
    ) -> Result<Vec<Request>, sqlx::Error> {
        let mut visible = Vec::new();
        for request in load(conn, id, viewer.whose_requests()).await? {
            if title_seen(conn, viewer, max_age, &request).await? {
                visible.push(self.progress.attach(request));
            }
        }
        Ok(visible)
    }
}

/// Whether `viewer`, whose Maximum Age is `max_age`, sees the title of `request`: theirs whatever
/// its stored Content Rating, as they asked while it let them, another's only if that rating
/// passes (#1414), as [`listed_sql!`] asks it.
async fn title_seen(
    conn: &mut SqliteConnection,
    viewer: &User,
    max_age: MaximumAge,
    request: &Request,
) -> Result<bool, sqlx::Error> {
    if max_age.lets_every_title() || is_requester(viewer.id, &request.requesters) {
        return Ok(true);
    }
    let rating = sqlx::query_scalar!(
        "SELECT content_rating FROM requests WHERE id = ?",
        request.id
    )
    .fetch_optional(&mut *conn)
    .await?;
    // Gone since it was loaded, so not there to see.
    Ok(rating.is_some_and(|rating| max_age.allows(rating.as_deref())))
}

/// A row of [`Requests::list`], before it is read in full.
struct Row {
    is_request: bool,
    id: i64,
    created_at: i64,
}

/// The rows of `page` of the Requests and Followed Shows `seen` holds, in the stored state
/// `state`, or any if `None`, newest first.
async fn page_rows(
    conn: &mut SqliteConnection,
    seen: &Seen,
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
                   WHERE ?6 IS NULL OR (created_at, is_request, id) < (?6, ?7, ?8)
                   ORDER BY created_at DESC, is_request DESC, id DESC
                   LIMIT ?9"#,
                seen.whose,
                state,
                seen.ratings,
                seen.allow_unrated,
                seen.viewer,
                opened_before,
                request_before,
                id_before,
                page.limit(),
            )
        };
    }
    listed_sql!(page_rows).fetch_all(conn).await
}

/// How many of the Requests and Followed Shows `seen` holds are in the stored state `state`, or
/// any if `None`.
async fn count(
    conn: &mut SqliteConnection,
    seen: &Seen,
    state: Option<&str>,
) -> Result<u32, sqlx::Error> {
    macro_rules! count {
        ($listed:literal) => {
            sqlx::query_scalar!(
                r#"SELECT COUNT(*) AS "count!: u32" FROM ("# + $listed + ")",
                seen.whose,
                state,
                seen.ratings,
                seen.allow_unrated,
                seen.viewer
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
