//! Watch State: how far each User has got through a Movie or Episode, and whether they've played
//! it.
//!
//! [`WatchStates`] owns `watch_states` (see `src/library/AGENTS.md`). It keeps Watch State against
//! a Movie's or Episode's external id, never against an Item or file (ADR 0013), so it survives
//! rescans, renames and new Versions. Which Movie or Episodes a Version or a marked title stands
//! for is for [`crate::browse::Browse`] to work out.

use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
    iter,
    sync::Arc,
};

use nama_plugin_api::{
    metadata::{ExternalId, ExternalIds},
    points::{WatchStateChange, WatchStateChanges, WatchStatesChanged},
};
use sqlx::{Sqlite, SqliteConnection, Transaction};

use super::watch_sync::scrobble;
use crate::{
    db::{BoxFuture, Database, detached, now_ms},
    link,
    plugins::ExtensionPoints,
};

// The `kind` of a Watch State, part of its key since Movie and Episode TMDB ids are separate id
// spaces.
const MOVIE: &str = "movie";
const EPISODE: &str = "episode";

#[derive(Debug, Clone)]
pub(crate) struct WatchStates {
    db: Arc<Database>,
    /// Told of each change a write makes.
    points: ExtensionPoints,
}

/// Whether a Watch State is a Movie's or an Episode's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Kind {
    Movie,
    Episode,
}

impl Kind {
    /// Its `kind` in `watch_states`.
    const fn stored(self) -> &'static str {
        match self {
            Self::Movie => MOVIE,
            Self::Episode => EPISODE,
        }
    }
}

/// A Movie or Episode, by the external id its Watch State is kept against.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum MovieOrEpisode {
    Movie(String),
    Episode(String),
}

impl MovieOrEpisode {
    pub(crate) fn movie(id: &ExternalId) -> Self {
        Self::Movie(id.to_string())
    }

    pub(crate) fn episode(id: &ExternalId) -> Self {
        Self::Episode(id.to_string())
    }

    /// The one stored in `watch_states` with `kind` and `title_id`.
    fn stored(kind: &str, title_id: String) -> Self {
        if kind == MOVIE {
            Self::Movie(title_id)
        } else {
            Self::Episode(title_id)
        }
    }

    /// Its key in `watch_states`, after the User: its kind and external id.
    fn key(&self) -> (&'static str, &str) {
        match self {
            Self::Movie(id) => (MOVIE, id),
            Self::Episode(id) => (EPISODE, id),
        }
    }

    /// It, known by every id of `ids` too.
    pub(crate) fn known_as(self, ids: &ExternalIds) -> Watched {
        Watched {
            ids: ids.iter().map(|id| id.to_string()).collect(),
            title: self,
        }
    }
}

/// A Movie or Episode whose Watch State is written under `title`, its preferred id, and found
/// under any of its ids (ADR 0042).
#[derive(Debug, Clone)]
pub(crate) struct Watched {
    title: MovieOrEpisode,
    ids: Vec<String>,
}

impl Watched {
    /// Its kind, the id written under and every id it is found by, as a JSON array.
    fn key(&self) -> (&'static str, &str, String) {
        let (kind, title) = self.title.key();
        let mut ids: Vec<&str> = self.ids.iter().map(String::as_str).collect();
        ids.push(title);
        (kind, title, serde_json::Value::from(ids).to_string())
    }

    /// The change that left it `played` at `progress` at `changed_at_ms`, as Plugins are told of
    /// it.
    fn changed(&self, played: bool, progress: Progress, changed_at_ms: i64) -> WatchStateChange {
        let (kind, title) = self.title.key();
        let others = self.ids.iter().filter(|&id| id != title).cloned();
        WatchStateChange {
            episode: kind == EPISODE,
            ids: iter::once(title.to_owned()).chain(others).collect(),
            played,
            position_ms: u64::try_from(progress.position_ms).unwrap_or_default(),
            duration_ms: u64::try_from(progress.duration_ms).unwrap_or_default(),
            changed_at_ms,
        }
    }

    /// The change that forgot it at `changed_at_ms`: unplayed at 0 of 0.
    fn forgotten(&self, changed_at_ms: i64) -> WatchStateChange {
        let start = Progress {
            position_ms: 0,
            duration_ms: 0,
        };
        self.changed(false, start, changed_at_ms)
    }
}

impl From<MovieOrEpisode> for Watched {
    fn from(title: MovieOrEpisode) -> Self {
        Self {
            title,
            ids: Vec::new(),
        }
    }
}

/// How far a player says it has got through a Version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Progress {
    position_ms: i64,
    duration_ms: i64,
}

/// A User's progress through a Movie or Episode, as stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WatchState {
    /// How far in, in milliseconds.
    pub(crate) position_ms: i64,
    /// How long the Movie or Episode is, in milliseconds, as the player last reported it. Zero if
    /// no player has, as when it was marked played without being played.
    pub(crate) duration_ms: i64,
    /// Whether the position has ever passed 90% of the duration, or it was marked played.
    pub(crate) played: bool,
    /// When it became played, in seconds since the Unix epoch, if it has: rewatching keeps it.
    pub(crate) played_at: Option<i64>,
    /// When it last changed, in seconds since the Unix epoch.
    pub(crate) updated_at: i64,
}

impl Progress {
    /// `position_ms` into something `duration_ms` long, with a position past the end taken as the
    /// end. `None` if the duration is zero or too long to store.
    pub(crate) fn new(position_ms: u64, duration_ms: u64) -> Option<Self> {
        let duration_ms = i64::try_from(duration_ms).ok().filter(|&ms| ms > 0)?;
        let position_ms = i64::try_from(position_ms)
            .unwrap_or(i64::MAX)
            .min(duration_ms);
        Some(Self {
            position_ms,
            duration_ms,
        })
    }

    /// Whether the position has passed 90% of the duration, which marks the Movie or Episode played.
    fn is_played(self) -> bool {
        i128::from(self.position_ms) * 10 > i128::from(self.duration_ms) * 9
    }

    /// This progress through a file holding `episodes` Episodes, split across them in order: each
    /// gets an even share of the duration, and the position fills the shares in turn. `None` for
    /// an Episode whose share the position hasn't reached, or a share too short to hold any.
    fn split(self, episodes: usize) -> Vec<Option<Self>> {
        let count = i128::try_from(episodes).unwrap_or(i128::MAX);
        // Where the `nth` share starts: `nth / count` of the way through.
        let start = |nth: i128| {
            let ms = i128::from(self.duration_ms) * nth / count;
            i64::try_from(ms).unwrap_or(self.duration_ms)
        };
        (0..count)
            .map(|nth| {
                let (start, end) = (start(nth), start(nth + 1));
                let reached = nth == 0 || self.position_ms > start;
                let share = Self {
                    position_ms: (self.position_ms - start).clamp(0, end - start),
                    duration_ms: end - start,
                };
                (reached && share.duration_ms > 0).then_some(share)
            })
            .collect()
    }
}

impl WatchStates {
    pub(crate) const fn new(db: Arc<Database>, points: ExtensionPoints) -> Self {
        Self { db, points }
    }

    /// Records, as of now, that the User `user` has got as far as `progress` through a file
    /// holding `titles`, in order, `None` for an Item that isn't Matched. The file plays as one, so
    /// its progress is split across them (ADR 0013), and one it hasn't reached is left alone, so
    /// rewinding keeps progress stored on a later one. One once played, under any of its ids, stays
    /// played. Returns each change it made, which Observers are told of.
    pub(crate) async fn report(
        &self,
        user: i64,
        titles: Vec<Option<impl Into<Watched>>>,
        progress: Progress,
    ) -> Result<Vec<WatchStateChange>, sqlx::Error> {
        let shares = progress.split(titles.len());
        let reached = titles
            .into_iter()
            .zip(shares)
            .filter_map(|(title, share)| Some((title?.into(), share?)))
            .collect();
        self.write_and_tell(user, move |tx| Box::pin(upsert(tx, user, reached)))
            .await
    }

    /// Marks, as of now, each of `titles` played or unplayed for the User `user`, in one write.
    /// Played keeps the position; unplayed forgets the Watch State under every id, so it starts
    /// from the beginning. Returns each change it made, which Observers are told of.
    pub(crate) async fn mark(
        &self,
        user: i64,
        titles: Vec<impl Into<Watched>>,
        played: bool,
    ) -> Result<Vec<WatchStateChange>, sqlx::Error> {
        let titles = titles.into_iter().map(Into::into).collect();
        self.write_and_tell(user, move |tx| {
            Box::pin(mark_each(tx, user, titles, played))
        })
        .await
    }

    /// Marks, as of now, each of `titles` unplayed for the User `user`, as [`WatchStates::mark`]
    /// does, and where that forgot any Watch State shows each of `entry` again in their
    /// Continue Watching, in one write. Forgetting writes no Watch State that would show it again,
    /// as playing it does. Returns each change it made, which Observers are told of.
    pub(crate) async fn mark_unplayed(
        &self,
        user: i64,
        titles: Vec<impl Into<Watched>>,
        entry: Vec<impl Into<Watched>>,
    ) -> Result<Vec<WatchStateChange>, sqlx::Error> {
        let titles = titles.into_iter().map(Into::into).collect();
        let entry = entry.into_iter().map(Into::into).collect();
        self.write_and_tell(user, move |tx| {
            Box::pin(forget_each(tx, user, titles, entry))
        })
        .await
    }

    /// Runs `write` of the User `user`'s Watch State, queuing the changes it returns for the Hub in
    /// the same write, then tells `WatchStatesChanged` Observers and Watch sync Plugins of them, in a
    /// task of its own, so they're queued and told even if the caller stops waiting once the write
    /// is sent (ADR 0029).
    async fn write_and_tell<F>(
        &self,
        user: i64,
        write: F,
    ) -> Result<Vec<WatchStateChange>, sqlx::Error>
    where
        F: for<'t> FnOnce(
                &'t mut Transaction<'_, Sqlite>,
            ) -> BoxFuture<'t, Result<Vec<WatchStateChange>, sqlx::Error>>
            + Send
            + 'static,
    {
        let (db, points) = (Arc::clone(&self.db), self.points.clone());
        detached(async move {
            let changes = (db.write(move |tx| Box::pin(queued_for_hub(tx, user, write)))).await?;
            if !changes.is_empty() {
                let told = WatchStateChanges {
                    user_id: user,
                    changes: changes.clone(),
                };
                scrobble(&db, &points, &told);
                points.notify::<WatchStatesChanged>(told);
            }
            Ok(changes)
        })
        .await
    }

    /// Takes the newest of `pulled`, other Servers' changes to the User `user`'s Watch State, of
    /// each Movie or Episode here, made after any this Server has of it under any of its ids, in
    /// one write: as of when it was made, it forgets one marked unplayed and stores any other,
    /// under the preferred id. One once played here stays played. Queues nothing for the Hub and tells
    /// neither Observers nor Watch sync Plugins: the change is the service's own news, so it never
    /// echoes back.
    pub(crate) async fn take_pulled(
        &self,
        user: i64,
        mut pulled: Vec<(Watched, WatchStateChange)>,
    ) -> Result<(), sqlx::Error> {
        // The service may hand a title's ids apart; only the newest of them is the title's.
        pulled.sort_by_key(|(_, change)| Reverse(change.changed_at_ms));
        let mut seen = HashSet::new();
        pulled.retain(|(watched, _)| seen.insert(watched.title.clone()));
        self.db
            .write(move |tx| Box::pin(take_each(tx, user, pulled)))
            .await?;
        Ok(())
    }

    /// Takes each of `offline`, changes the User `user`'s Device made to their Watch State while
    /// offline, in the order they were made, as [`WatchStates::take_pulled`] takes a pulled one:
    /// only when made after any this Server has of it, and as of when it was made, though one dated
    /// in the future is taken as made now. Each should be at a duration of more than zero, as a 0 of 0 unplayed one forgets it.
    /// Unlike a pulled change, each it stores shows the title again in Continue Watching, as
    /// [`WatchStates::report`] does, and is queued and told of. Returns each change it stored.
    pub(crate) async fn take_offline(
        &self,
        user: i64,
        mut offline: Vec<(Watched, WatchStateChange)>,
    ) -> Result<Vec<WatchStateChange>, sqlx::Error> {
        let now = now_ms();
        for (_, change) in &mut offline {
            change.changed_at_ms = change.changed_at_ms.min(now);
        }
        // Replayed as made, so a later change in the batch wins and an earlier played one stays.
        offline.sort_by_key(|(_, change)| change.changed_at_ms);
        self.write_and_tell(user, move |tx| Box::pin(take_each(tx, user, offline)))
            .await
    }

    /// Hides each of `titles` from the User `user`'s Continue Watching, or shows it again, under
    /// every id, in one write. Leaves their Watch State alone; the next change to it shows the
    /// title again.
    pub(crate) async fn hide(
        &self,
        user: i64,
        titles: Vec<impl Into<Watched>>,
        hidden: bool,
    ) -> Result<(), sqlx::Error> {
        let titles: Vec<Watched> = titles.into_iter().map(Into::into).collect();
        self.db
            .write(move |tx| Box::pin(hide_each(tx, user, titles, hidden)))
            .await
    }

    /// Every User's Watch State of each of `titles`, or only the User `user`'s if given, in the
    /// same order, each a Movie or Episode kept under any of its ids, by User id. Where a User has
    /// one under more than one of a title's ids, as when a Library's title source order changed,
    /// the most recently updated.
    pub(crate) async fn of_titles(
        &self,
        user: Option<i64>,
        titles: &[(Kind, ExternalIds)],
    ) -> Result<Vec<Vec<(i64, WatchState)>>, sqlx::Error> {
        let asked: Vec<serde_json::Value> = (titles.iter())
            .map(|(kind, ids)| {
                let ids: Vec<String> = ids.iter().map(|id| id.to_string()).collect();
                serde_json::json!([kind.stored(), ids])
            })
            .collect();
        let asked = serde_json::Value::from(asked).to_string();
        // Grouped by title, then User, the most recent first, so the first row of each title's
        // User is the one kept.
        let rows = sqlx::query!(
            r#"SELECT CAST(title.key AS INTEGER) AS "position!: i64", user_id AS "user_id!",
                      position_ms, duration_ms, played AS "played: bool", played_at,
                      updated_at
               FROM json_each(?) AS title
               JOIN json_each(title.value, '$[1]') AS id
               JOIN watch_states ON watch_states.kind = json_extract(title.value, '$[0]')
                                AND watch_states.title_id = id.value
               WHERE ? IS NULL OR user_id = ?
               ORDER BY title.key, user_id, updated_at DESC, watch_states.title_id"#,
            asked,
            user,
            user
        )
        .fetch_all(self.db.reader())
        .await?;
        let mut states: Vec<Vec<(i64, WatchState)>> = vec![Vec::new(); titles.len()];
        for row in rows {
            let Some(of_title) =
                (usize::try_from(row.position).ok()).and_then(|position| states.get_mut(position))
            else {
                continue;
            };
            if of_title
                .last()
                .is_some_and(|&(kept, _)| kept == row.user_id)
            {
                continue;
            }
            let state = WatchState {
                position_ms: row.position_ms,
                duration_ms: row.duration_ms,
                played: row.played,
                played_at: row.played_at,
                updated_at: row.updated_at,
            };
            of_title.push((row.user_id, state));
        }
        Ok(states)
    }

    /// Every Movie, or Episode, the User `user` has started or played, by the id each Watch State
    /// is kept under, in its text form, and whether it is played.
    pub(crate) async fn started(
        &self,
        user: i64,
        kind: Kind,
    ) -> Result<Vec<(String, bool)>, sqlx::Error> {
        let kind = kind.stored();
        let rows = sqlx::query!(
            r#"SELECT title_id, played AS "played: bool" FROM watch_states
               WHERE user_id = ? AND kind = ? AND (played OR position_ms > 0)"#,
            user,
            kind
        )
        .fetch_all(self.db.reader())
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| (row.title_id, row.played))
            .collect())
    }

    /// Every Watch State of the User `user`, unordered, with the Movie or Episode it's kept under,
    /// its row id, which breaks ties within a second so Continue Watching pages by
    /// `(updated_at, row id)`, and whether it's hidden from Continue Watching.
    pub(crate) async fn of_user(
        &self,
        user: i64,
    ) -> Result<Vec<(MovieOrEpisode, WatchState, i64, bool)>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT rowid AS "row!: i64", kind, title_id, position_ms, duration_ms,
                      played AS "played: bool", played_at, updated_at, hidden AS "hidden: bool"
             FROM watch_states
             WHERE user_id = ?"#,
            user
        )
        .fetch_all(self.db.reader())
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let watched = MovieOrEpisode::stored(&row.kind, row.title_id);
                let state = WatchState {
                    position_ms: row.position_ms,
                    duration_ms: row.duration_ms,
                    played: row.played,
                    played_at: row.played_at,
                    updated_at: row.updated_at,
                };
                (watched, state, row.row, row.hidden)
            })
            .collect())
    }

    /// Every Watch State of the User `user`, each as the change that left it, as Plugins are told
    /// of it: known by every id `known`, keyed by whether it's an Episode and one of its ids, has
    /// for it, or by its stored id alone. Each is dated when it last changed, never now, so a later
    /// change elsewhere wins.
    pub(crate) async fn changes_of_user(
        &self,
        user: i64,
        known: &HashMap<(bool, String), Watched>,
    ) -> Result<Vec<WatchStateChange>, sqlx::Error> {
        let stored = self.of_user(user).await?;
        Ok((stored.into_iter())
            .map(|(title, state, _, _)| {
                let (kind, id) = title.key();
                let watched = (known.get(&(kind == EPISODE, id.to_owned())).cloned())
                    .unwrap_or_else(|| Watched::from(title));
                let progress = Progress {
                    position_ms: state.position_ms,
                    duration_ms: state.duration_ms,
                };
                let changed_at_ms = state.updated_at.saturating_mul(1000);
                watched.changed(state.played, progress, changed_at_ms)
            })
            .collect())
    }
}

/// One once played under any of its ids stays played, as of when it first was.
async fn upsert(
    conn: &mut SqliteConnection,
    user: i64,
    titles: Vec<(Watched, Progress)>,
) -> Result<Vec<WatchStateChange>, sqlx::Error> {
    // In milliseconds for the Plugins told, as two changes in one second must still order.
    let changed_at_ms = now_ms();
    let now = changed_at_ms.div_euclid(1000);
    let mut changes = Vec::with_capacity(titles.len());
    for (watched, progress) in titles {
        let played = store(
            &mut *conn,
            user,
            &watched,
            progress,
            progress.is_played(),
            now,
        )
        .await?;
        changes.push(watched.changed(played, progress, changed_at_ms));
    }
    Ok(changes)
}

/// Stores `watched` at `progress`, `played` or played before under any of its ids, as of `at`, in
/// seconds since the Unix epoch. Returns whether it is played.
#[expect(
    clippy::too_many_arguments,
    reason = "each is a column of the row it writes, which a struct would only rename"
)]
async fn store(
    conn: &mut SqliteConnection,
    user: i64,
    watched: &Watched,
    progress: Progress,
    played: bool,
    at: i64,
) -> Result<bool, sqlx::Error> {
    let (kind, title, ids) = watched.key();
    // An upsert, never a replace: the triggers on `watch_states` make Home rows stale on an
    // insert, a delete or a change to `played`, which a position report must not fire.
    let stored = sqlx::query!(
        r#"INSERT INTO watch_states
             (user_id, kind, title_id, position_ms, duration_ms, played, updated_at,
              played_at)
         VALUES (?1, ?2, ?3, ?4, ?5,
                 ?6 OR EXISTS (SELECT 1 FROM watch_states
                               WHERE user_id = ?1 AND kind = ?2 AND played
                                 AND title_id IN (SELECT value FROM json_each(?8))),
                 ?7,
                 coalesce((SELECT max(played_at) FROM watch_states
                           WHERE user_id = ?1 AND kind = ?2
                             AND title_id IN (SELECT value FROM json_each(?8))),
                          CASE WHEN ?6 THEN ?7 END))
         ON CONFLICT (user_id, kind, title_id) DO UPDATE
         SET position_ms = excluded.position_ms,
             duration_ms = excluded.duration_ms,
             played = watch_states.played OR excluded.played,
             updated_at = excluded.updated_at,
             played_at = coalesce(watch_states.played_at, excluded.played_at),
             hidden = 0
         RETURNING played AS "played: bool""#,
        user,
        kind,
        title,
        progress.position_ms,
        progress.duration_ms,
        played,
        at,
        ids,
    )
    .fetch_one(conn)
    .await?;
    Ok(stored.played)
}

/// Takes each change made elsewhere, as [`WatchStates::take_pulled`] says. Returns each it took.
async fn take_each(
    conn: &mut SqliteConnection,
    user: i64,
    made_elsewhere: Vec<(Watched, WatchStateChange)>,
) -> Result<Vec<WatchStateChange>, sqlx::Error> {
    let mut taken = Vec::new();
    for (watched, change) in made_elsewhere {
        // The Server keeps seconds, so one made in the same second as its own, as its own pushed
        // and pulled back is, isn't newer.
        let at = change.changed_at_ms.div_euclid(1000);
        let (kind, title, ids) = watched.key();
        let newest = sqlx::query_scalar!(
            r#"SELECT MAX(updated_at) AS "newest?: i64" FROM watch_states
               WHERE user_id = ? AND kind = ? AND title_id IN (SELECT value FROM json_each(?))"#,
            user,
            kind,
            ids,
        )
        .fetch_one(&mut *conn)
        .await?;
        if newest.is_some_and(|newest| newest >= at) {
            continue;
        }
        let progress = Progress::new(change.position_ms, change.duration_ms);
        let ms = change.changed_at_ms;
        taken.push(match progress {
            Some(progress) => {
                let played = store(&mut *conn, user, &watched, progress, change.played, at).await?;
                watched.changed(played, progress, ms)
            }
            None if change.played => {
                let kept = mark_played(&mut *conn, user, kind, title, at).await?;
                watched.changed(true, kept, ms)
            }
            // Unplayed at 0 of 0: marked unplayed.
            None => {
                forget(&mut *conn, user, kind, ids).await?;
                watched.forgotten(ms)
            }
        });
    }
    Ok(taken)
}

async fn mark_each(
    conn: &mut SqliteConnection,
    user: i64,
    titles: Vec<Watched>,
    played: bool,
) -> Result<Vec<WatchStateChange>, sqlx::Error> {
    // In milliseconds for the Plugins told, as two changes in one second must still order.
    let changed_at_ms = now_ms();
    let now = changed_at_ms.div_euclid(1000);
    let mut changes = Vec::with_capacity(titles.len());
    for watched in titles {
        let (kind, title, ids) = watched.key();
        changes.push(if played {
            let kept = mark_played(&mut *conn, user, kind, title, now).await?;
            watched.changed(true, kept, changed_at_ms)
        } else {
            forget(&mut *conn, user, kind, ids).await?;
            watched.forgotten(changed_at_ms)
        });
    }
    Ok(changes)
}

/// Every one of `titles` is a change, forgotten or not: another Server may have played it.
async fn forget_each(
    conn: &mut SqliteConnection,
    user: i64,
    titles: Vec<Watched>,
    entry: Vec<Watched>,
) -> Result<Vec<WatchStateChange>, sqlx::Error> {
    let changed_at_ms = now_ms();
    let mut forgot = false;
    for title in &titles {
        let (kind, _, ids) = title.key();
        forgot |= forget(&mut *conn, user, kind, ids).await?;
    }
    if forgot {
        hide_each(conn, user, entry, false).await?;
    }
    Ok(titles
        .iter()
        .map(|title| title.forgotten(changed_at_ms))
        .collect())
}

async fn hide_each(
    conn: &mut SqliteConnection,
    user: i64,
    titles: Vec<Watched>,
    hidden: bool,
) -> Result<(), sqlx::Error> {
    for title in titles {
        let (kind, _, ids) = title.key();
        sqlx::query!(
            "UPDATE watch_states SET hidden = ?
             WHERE user_id = ? AND kind = ? AND title_id IN (SELECT value FROM json_each(?))",
            hidden,
            user,
            kind,
            ids,
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// Keeps any position, and when it became played if it already was. One the User hasn't started
/// is at 0 of an unknown duration, 0. Returns the position kept.
async fn mark_played(
    conn: &mut SqliteConnection,
    user: i64,
    kind: &'static str,
    title: &str,
    now: i64,
) -> Result<Progress, sqlx::Error> {
    sqlx::query_as!(
        Progress,
        "INSERT INTO watch_states
             (user_id, kind, title_id, position_ms, duration_ms, played, updated_at, played_at)
         VALUES (?1, ?2, ?3, 0, 0, 1, ?4, ?4)
         ON CONFLICT (user_id, kind, title_id) DO UPDATE
         SET played = 1, updated_at = excluded.updated_at,
             played_at = coalesce(watch_states.played_at, excluded.played_at), hidden = 0
         RETURNING position_ms, duration_ms",
        user,
        kind,
        title,
        now,
    )
    .fetch_one(conn)
    .await
}

/// Deleting the rows is how a title becomes unplayed with no position. Its rows under every id
/// in the JSON array `ids` go, so an older one under another id doesn't come back. Says whether
/// there were any.
async fn forget(
    conn: &mut SqliteConnection,
    user: i64,
    kind: &'static str,
    ids: String,
) -> Result<bool, sqlx::Error> {
    let forgot = sqlx::query!(
        "DELETE FROM watch_states
         WHERE user_id = ? AND kind = ? AND title_id IN (SELECT value FROM json_each(?))",
        user,
        kind,
        ids,
    )
    .execute(conn)
    .await?;
    Ok(forgot.rows_affected() > 0)
}

/// Runs `write` of the User `user`'s Watch State through `tx`, and queues the changes it returns
/// for the Hub in the same write.
async fn queued_for_hub<F>(
    tx: &mut Transaction<'_, Sqlite>,
    user: i64,
    write: F,
) -> Result<Vec<WatchStateChange>, sqlx::Error>
where
    F: for<'t> FnOnce(
        &'t mut Transaction<'_, Sqlite>,
    ) -> BoxFuture<'t, Result<Vec<WatchStateChange>, sqlx::Error>>,
{
    let changes = write(tx).await?;
    link::queue_watch_states(tx, user, &changes).await?;
    Ok(changes)
}

#[cfg(test)]
#[path = "watch_states_tests.rs"]
mod tests;
