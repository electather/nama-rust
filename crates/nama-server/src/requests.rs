//! Requests: Users asking for a Movie that isn't on the Server to be added to it (ADR 0012).
//!
//! [`Requests`] is the only code that reads or writes `requests` and `requesters`. It hands
//! Requests out as their proto messages, since those are also what its events carry.

use std::{collections::HashMap, sync::Arc};

use nama_plugin_api::{
    PluginId,
    metadata::{Candidate, ExternalId, SearchQuery, TitleKind},
};
use nama_proto::nama::v1::{
    Event, Permission, Request, RequestCancelled, RequestCreated, RequestState,
    RequestStateChanged, RequestedMovie, Requester, RequesterAdded,
};
use sqlx::SqliteConnection;

use crate::{
    contents::{self, Contents, Metadata},
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

/// Why a Request couldn't be made, found or changed.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    /// There is no such Request, or none the caller may see.
    #[error("there is no Request {0}")]
    NoSuchRequest(i64),
    #[error("the Request {0} isn't Pending")]
    NotPending(i64),
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
        // Refuses before asking TMDB, which may be down or slow; `ask` checks again for a scan
        // that Matched the Movie meanwhile.
        if self.contents.library_with_movie(tmdb_id).await?.is_some() {
            return Err(Error::InLibrary(tmdb_id));
        }
        // ponytail: looked up even to join an open Request, so the write has it if that Request
        // closes meanwhile. Look up only when opening one if TMDB's rate limit starts to bite.
        let movie = self.fetch(tmdb_id).await?;
        let state = if asker.permissions.allow(Permission::AutoApproveMovie) {
            RequestState::Approved
        } else {
            RequestState::Pending
        };
        let user = asker.id;
        let (request, happened) = self
            .db
            .write(move |tx| Box::pin(ask(tx, user, tmdb_id, movie, state)))
            .await?;
        self.events.publish(happened.into_iter().collect());
        Ok(request)
    }

    /// Approves the Pending Request `id`, and publishes that its state changed.
    pub(crate) async fn approve(&self, id: i64) -> Result<Request, Error> {
        self.decide(id, RequestState::Approved, None).await
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
        let query = SearchQuery {
            kind: TitleKind::Movie,
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
        let mut conn = self.db.reader().acquire().await?;
        let mut found = Vec::with_capacity(candidates.len());
        // ponytail: two queries per Movie found, which a search caps at a page of 20.
        for (plugin, candidate) in candidates {
            // Requests name Movies by TMDB id, so a Movie without one can't be asked for.
            let ExternalId::Tmdb(tmdb_id) = candidate.id else {
                continue;
            };
            found.push(look_around(&mut conn, viewer, tmdb_id, plugin, candidate).await?);
        }
        Ok(found)
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
            .map_err(|err| match err {
                // `movie` names no item, so its only refusal is the Movie's.
                ManualError::NoSuchTitle(_) | ManualError::NoSuchItem(_) => {
                    Error::NoSuchMovie(tmdb_id)
                }
                ManualError::Unavailable => Error::Unavailable,
                ManualError::Database(err) => Error::Database(err),
            })
    }

    /// The Request `id`, if `viewer` may see it.
    pub(crate) async fn get(&self, viewer: &User, id: i64) -> Result<Request, Error> {
        let mut conn = self.db.reader().acquire().await?;
        let found = load(&mut conn, Some(id), visible_to(viewer)).await?;
        found.into_iter().next().ok_or(Error::NoSuchRequest(id))
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
    let requester = visible_to(viewer);
    // Open as `one_open_request_per_movie` has it.
    let open = sqlx::query_scalar!(
        r#"SELECT id AS "id!" FROM requests
           WHERE tmdb_id = ?1 AND state NOT IN ('declined', 'available')
             AND (?2 IS NULL OR EXISTS (SELECT 1 FROM requesters
                                        WHERE request_id = requests.id AND user_id = ?2))"#,
        tmdb_id,
        requester
    )
    .fetch_optional(&mut *conn)
    .await?;
    let request = match open {
        Some(id) => load_one(conn, id).await?,
        None => None,
    };
    Ok(Found {
        tmdb_id,
        plugin,
        movie,
        library,
        request,
    })
}

/// Adds `user` as a Requester of the open Request for the Movie `tmdb_id`, opening one in
/// `state` for `movie` if there is none, unless a Library has the Movie. Returns the Request, and
/// the event for what changed, if anything did.
async fn ask(
    conn: &mut SqliteConnection,
    user: i64,
    tmdb_id: i64,
    movie: Metadata,
    state: RequestState,
) -> Result<(Request, Option<Event>), Error> {
    if contents::library_with_movie(conn, tmdb_id).await?.is_some() {
        return Err(Error::InLibrary(tmdb_id));
    }
    let now = now();
    let state = stored(state);
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

/// Removes the Request `id`, and its Requesters with it, and returns it as it was.
async fn cancel(conn: &mut SqliteConnection, id: i64) -> Result<Request, Error> {
    let request = load_one(conn, id).await?.ok_or(Error::NoSuchRequest(id))?;
    sqlx::query!("DELETE FROM requests WHERE id = ?", id)
        .execute(&mut *conn)
        .await?;
    Ok(request)
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
        r#"SELECT id AS "id!", tmdb_id, title, release_date, state, created_at, decline_reason
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
            decline_reason: row.decline_reason,
            ..Default::default()
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
