//! Watch State: how far each User has got through a Movie, and whether they've played it.
//!
//! [`WatchStates`] is the only code that reads or writes `watch_states`. It keeps Watch State
//! against a Movie's external id, never against an Item or file (ADR 0013), so it survives
//! rescans, renames and new Versions. Which Movie a Version is Matched to is for
//! [`crate::contents::Contents`] to say.

use std::{collections::HashMap, sync::Arc};

use nama_plugin_api::metadata::ExternalId;

use crate::db::{Database, now};

/// Reads and writes each User's Watch State.
#[derive(Debug, Clone)]
pub(crate) struct WatchStates {
    db: Arc<Database>,
}

/// How far a player says it has got through a Version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Progress {
    position_ms: i64,
    duration_ms: i64,
}

/// A User's progress through a Movie, as stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WatchState {
    pub(crate) position_ms: i64,
    pub(crate) duration_ms: i64,
    /// Whether the position has ever passed 90% of the duration.
    pub(crate) played: bool,
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

    /// Whether the position has passed 90% of the duration, which marks the title played.
    fn is_played(self) -> bool {
        i128::from(self.position_ms) * 10 > i128::from(self.duration_ms) * 9
    }
}

impl WatchStates {
    /// The Watch State kept in `db`.
    pub(crate) const fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Records, as of now, that the User `user` has got as far as `progress` through the Movie
    /// with the id `movie`. A Movie once played stays played.
    pub(crate) async fn report_movie(
        &self,
        user: i64,
        movie: &ExternalId,
        progress: Progress,
    ) -> Result<(), sqlx::Error> {
        let movie = movie.to_string();
        let played = progress.is_played();
        let now = now();
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!(
                        "INSERT INTO watch_states
                             (user_id, kind, title_id, position_ms, duration_ms, played,
                              updated_at)
                         VALUES (?, 'movie', ?, ?, ?, ?, ?)
                         ON CONFLICT (user_id, kind, title_id) DO UPDATE
                         SET position_ms = excluded.position_ms,
                             duration_ms = excluded.duration_ms,
                             played = watch_states.played OR excluded.played,
                             updated_at = excluded.updated_at",
                        user,
                        movie,
                        progress.position_ms,
                        progress.duration_ms,
                        played,
                        now,
                    )
                    .execute(&mut **tx)
                    .await?;
                    Ok(())
                })
            })
            .await
    }

    /// The User `user`'s Watch State of each Movie they've started, keyed by the Movie's id as
    /// [`ExternalId`]'s `Display` writes it.
    // ponytail: reads all of a User's Movies even for one; query by id if that gets big.
    pub(crate) async fn movies(
        &self,
        user: i64,
    ) -> Result<HashMap<String, WatchState>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT title_id, position_ms, duration_ms, played AS "played: bool", updated_at
               FROM watch_states
               WHERE user_id = ? AND kind = 'movie'"#,
            user
        )
        .fetch_all(self.db.reader())
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let state = WatchState {
                    position_ms: row.position_ms,
                    duration_ms: row.duration_ms,
                    played: row.played,
                    updated_at: row.updated_at,
                };
                (row.title_id, state)
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::Progress;

    #[test]
    fn a_position_past_the_duration_is_the_end_and_a_zero_duration_is_refused() {
        assert_eq!(Progress::new(5_000, 1_000), Progress::new(1_000, 1_000));
        assert_eq!(Progress::new(0, 0), None);
        assert_eq!(Progress::new(0, u64::MAX), None);
    }
}
