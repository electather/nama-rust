//! Requests: Users asking for a Movie that isn't on the Server to be added to it (ADR 0012).
//!
//! [`Requests`] is the only code that reads or writes `requests` and `requesters`. It hands
//! Requests out as their proto messages, since those are also what its events carry.

use std::{collections::HashMap, sync::Arc};

use nama_plugin_api::metadata::ExternalId;
use nama_proto::nama::v1::{
    Event, Permission, Request, RequestCreated, RequestState, RequestedMovie, Requester,
    RequesterAdded,
};
use sqlx::SqliteConnection;

use crate::{
    contents::{Contents, Metadata},
    db::{Database, now},
    events::{EventBus, event},
    matching::{ManualError, Matcher},
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

/// Why a Request couldn't be made or found.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    /// There is no such Request, or none the caller may see.
    #[error("there is no Request {0}")]
    NoSuchRequest(i64),
    #[error("a Library already has the Movie tmdb-{0}")]
    InLibrary(i64),
    #[error("the metadata source has no Movie tmdb-{0}")]
    NoSuchMovie(i64),
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
    /// Looks up a newly Requested Movie's title.
    matcher: Matcher,
    events: EventBus,
}

/// Whose Requests `viewer` may see: everyone's (`None`), or only those they are a Requester of.
pub(crate) fn visible_to(viewer: &User) -> Option<i64> {
    (!viewer.permissions.allow(Permission::ViewRequests)).then_some(viewer.id)
}

impl Requests {
    /// The Requests in `db`, looking Movies up through `matcher` and publishing on `events`.
    pub(crate) fn new(db: Arc<Database>, matcher: Matcher, events: EventBus) -> Self {
        Self {
            contents: Contents::new(Arc::clone(&db)),
            db,
            matcher,
            events,
        }
    }

    /// Asks for the Movie with the TMDB id `tmdb_id` for `asker`, who joins its open Request, or
    /// opens one: Approved if `asker` may have Movies approved as they ask, else Pending. Publishes
    /// what changed, if anything did.
    pub(crate) async fn create(&self, asker: &User, tmdb_id: i64) -> Result<Request, Error> {
        if self.contents.has_movie(tmdb_id).await? {
            return Err(Error::InLibrary(tmdb_id));
        }
        // ponytail: looked up even to join an open Request, so the write has it if that Request
        // closes meanwhile. Look up only when opening one if TMDB's rate limit starts to bite.
        let movie = self
            .matcher
            .movie(&ExternalId::Tmdb(tmdb_id))
            .await
            .map_err(|err| match err {
                // `movie` names no item, so its only refusal is the Movie's.
                ManualError::NoSuchTitle(_) | ManualError::NoSuchItem(_) => {
                    Error::NoSuchMovie(tmdb_id)
                }
                ManualError::Unavailable => Error::Unavailable,
                ManualError::Database(err) => Error::Database(err),
            })?;
        let state = if asker.permissions.allow(Permission::AutoApproveMovie) {
            "approved"
        } else {
            "pending"
        };
        let user = asker.id;
        let (request, happened) = self
            .db
            .write(move |tx| Box::pin(ask(tx, user, tmdb_id, movie, state)))
            .await?;
        self.events.publish(happened.into_iter().collect());
        Ok(request)
    }

    /// The Requests `viewer` may see, newest first.
    pub(crate) async fn list(&self, viewer: &User) -> Result<Vec<Request>, sqlx::Error> {
        let mut conn = self.db.reader().acquire().await?;
        load(&mut conn, None, visible_to(viewer)).await
    }

    /// The Request `id`, if `viewer` may see it.
    pub(crate) async fn get(&self, viewer: &User, id: i64) -> Result<Request, Error> {
        let mut conn = self.db.reader().acquire().await?;
        let found = load(&mut conn, Some(id), visible_to(viewer)).await?;
        found.into_iter().next().ok_or(Error::NoSuchRequest(id))
    }
}

/// Adds `user` as a Requester of the open Request for the Movie `tmdb_id`, opening one in the
/// stored `state` for `movie` if there is none. Returns the Request, and the event for what
/// changed, if anything did.
async fn ask(
    conn: &mut SqliteConnection,
    user: i64,
    tmdb_id: i64,
    movie: Metadata,
    state: &'static str,
) -> Result<(Request, Option<Event>), sqlx::Error> {
    let now = now();
    // Open as `one_open_request_per_movie` has it.
    let open = sqlx::query_scalar!(
        "SELECT id FROM requests
         WHERE tmdb_id = ? AND state NOT IN ('declined', 'available')",
        tmdb_id
    )
    .fetch_optional(&mut *conn)
    .await?;
    let opened = open.is_none();
    let id = match open {
        Some(id) => id,
        None => {
            sqlx::query_scalar!(
                "INSERT INTO requests (tmdb_id, title, release_date, state, created_at)
                 VALUES (?, ?, ?, ?, ?)
                 RETURNING id",
                tmdb_id,
                movie.title,
                movie.release_date,
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
    let request = load(conn, Some(id), None)
        .await?
        .into_iter()
        .next()
        .ok_or(sqlx::Error::RowNotFound)?;
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

/// The Requests, newest first: only the one with the ID `id` if given, and only those the User
/// `requester` is a Requester of if given.
async fn load(
    conn: &mut SqliteConnection,
    id: Option<i64>,
    requester: Option<i64>,
) -> Result<Vec<Request>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT id AS "id!", tmdb_id, title, release_date, state, created_at
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
        .map(|row| Request {
            id: row.id,
            movie: RequestedMovie {
                tmdb_id: row.tmdb_id,
                title: row.title,
                release_date: row.release_date,
                ..Default::default()
            }
            .into(),
            state: read_state(&row.state).into(),
            requesters: requesters.remove(&row.id).unwrap_or_default(),
            created_at: row.created_at,
            ..Default::default()
        })
        .collect())
}

fn read_state(stored: &str) -> RequestState {
    STATES
        .iter()
        .find(|&&(_, name)| name == stored)
        .map_or(RequestState::REQUEST_STATE_UNSPECIFIED, |&(state, _)| state)
}
