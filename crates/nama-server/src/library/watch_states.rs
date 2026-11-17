//! Watch State: how far each User has got through a Movie or Episode, and whether they've played
//! it.
//!
//! [`WatchStates`] owns `watch_states` (see `src/library/AGENTS.md`). It keeps Watch State against
//! a Movie's or Episode's external id, never against an Item or file (ADR 0013), so it survives
//! rescans, renames and new Versions. Which Movie or Episodes a Version or a marked title stands
//! for is for [`crate::browse::Browse`] to work out.

use std::sync::Arc;

use nama_plugin_api::metadata::{ExternalId, ExternalIds};
use sqlx::SqliteConnection;

use crate::db::{Database, now};

// The `kind` of a Watch State, part of its key since Movie and Episode TMDB ids are separate id
// spaces.
const MOVIE: &str = "movie";
const EPISODE: &str = "episode";

#[derive(Debug, Clone)]
pub(crate) struct WatchStates {
    db: Arc<Database>,
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
    pub(crate) const fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Records, as of now, that the User `user` has got as far as `progress` through a file
    /// holding `titles`, in order, `None` for an Item that isn't Matched. The file plays as one, so
    /// its progress is split across them (ADR 0013), and one it hasn't reached is left alone, so
    /// rewinding keeps progress stored on a later one. One once played, under any of its ids, stays
    /// played.
    pub(crate) async fn report(
        &self,
        user: i64,
        titles: Vec<Option<impl Into<Watched>>>,
        progress: Progress,
    ) -> Result<(), sqlx::Error> {
        let shares = progress.split(titles.len());
        let reached = titles
            .into_iter()
            .zip(shares)
            .filter_map(|(title, share)| Some((title?.into(), share?)))
            .collect();
        self.db
            .write(move |tx| Box::pin(upsert(tx, user, reached)))
            .await
    }

    /// Marks, as of now, each of `titles` played or unplayed for the User `user`, in one write.
    /// Played keeps the position; unplayed forgets the Watch State under every id, so it starts
    /// from the beginning.
    pub(crate) async fn mark(
        &self,
        user: i64,
        titles: Vec<impl Into<Watched>>,
        played: bool,
    ) -> Result<(), sqlx::Error> {
        let titles = titles.into_iter().map(Into::into).collect();
        self.db
            .write(move |tx| Box::pin(mark_each(tx, user, titles, played)))
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

    /// Every Watch State of the User `user`, unordered, with the Movie or Episode it's kept under
    /// and its row id, which breaks ties within a second so Continue Watching pages by
    /// `(updated_at, row id)`.
    pub(crate) async fn of_user(
        &self,
        user: i64,
    ) -> Result<Vec<(MovieOrEpisode, WatchState, i64)>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT rowid AS "row!: i64", kind, title_id, position_ms, duration_ms,
                      played AS "played: bool", played_at, updated_at
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
                (watched, state, row.row)
            })
            .collect())
    }
}

/// One once played under any of its ids stays played, as of when it first was.
async fn upsert(
    conn: &mut SqliteConnection,
    user: i64,
    titles: Vec<(Watched, Progress)>,
) -> Result<(), sqlx::Error> {
    let now = now();
    for (title, progress) in titles {
        let (kind, title, ids) = title.key();
        let played = progress.is_played();
        sqlx::query!(
            "INSERT INTO watch_states
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
                 played_at = coalesce(watch_states.played_at, excluded.played_at)",
            user,
            kind,
            title,
            progress.position_ms,
            progress.duration_ms,
            played,
            now,
            ids,
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

async fn mark_each(
    conn: &mut SqliteConnection,
    user: i64,
    titles: Vec<Watched>,
    played: bool,
) -> Result<(), sqlx::Error> {
    let now = now();
    for title in titles {
        let (kind, title, ids) = title.key();
        if played {
            mark_played(&mut *conn, user, kind, title, now).await?;
        } else {
            forget(&mut *conn, user, kind, ids).await?;
        }
    }
    Ok(())
}

/// Keeps any position, and when it became played if it already was. One the User hasn't started
/// is at 0 of an unknown duration, 0.
async fn mark_played(
    conn: &mut SqliteConnection,
    user: i64,
    kind: &'static str,
    title: &str,
    now: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO watch_states
             (user_id, kind, title_id, position_ms, duration_ms, played, updated_at, played_at)
         VALUES (?1, ?2, ?3, 0, 0, 1, ?4, ?4)
         ON CONFLICT (user_id, kind, title_id) DO UPDATE
         SET played = 1, updated_at = excluded.updated_at,
             played_at = coalesce(watch_states.played_at, excluded.played_at)",
        user,
        kind,
        title,
        now,
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Deleting the rows is how a title becomes unplayed with no position. Its rows under every id
/// in the JSON array `ids` go, so an older one under another id doesn't come back.
async fn forget(
    conn: &mut SqliteConnection,
    user: i64,
    kind: &'static str,
    ids: String,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "DELETE FROM watch_states
         WHERE user_id = ? AND kind = ? AND title_id IN (SELECT value FROM json_each(?))",
        user,
        kind,
        ids,
    )
    .execute(conn)
    .await?;
    Ok(())
}

#[cfg(test)]
#[path = "watch_states_tests.rs"]
mod tests;
