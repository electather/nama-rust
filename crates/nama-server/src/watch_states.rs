//! Watch State: how far each User has got through a Movie or Episode, and whether they've played
//! it.
//!
//! [`WatchStates`] is the only code that reads or writes `watch_states`. It keeps Watch State
//! against a Movie's or Episode's external id, never against an Item or file (ADR 0013), so it
//! survives rescans, renames and new Versions. Which Movie or Episodes a Version is Matched to is
//! for [`crate::contents::Contents`] to say.

use std::{collections::HashMap, sync::Arc};

use nama_plugin_api::metadata::ExternalId;
use sqlx::SqliteConnection;

use crate::db::{Database, now};

/// The `kind` of a Movie's Watch State, and of an Episode's. Their TMDB ids are separate id
/// spaces, so the kind is part of the key.
const MOVIE: &str = "movie";
const EPISODE: &str = "episode";

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

/// A User's progress through a Movie or Episode, as stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WatchState {
    /// How far in, in milliseconds.
    pub(crate) position_ms: i64,
    /// How long the Movie or Episode is, in milliseconds, as the player last reported it.
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
        self.store(user, MOVIE, vec![(movie.to_string(), progress)])
            .await
    }

    /// Records, as of now, that the User `user` has got as far as `progress` through a file
    /// holding `episodes`, in episode order, `None` for one that isn't Matched. The file plays as
    /// one, so its progress is split across them (ADR 0013), and an Episode it hasn't reached is
    /// left alone, so rewinding keeps progress stored on a later one. An Episode once played
    /// stays played.
    pub(crate) async fn report_episodes(
        &self,
        user: i64,
        episodes: &[Option<ExternalId>],
        progress: Progress,
    ) -> Result<(), sqlx::Error> {
        let reached = episodes
            .iter()
            .zip(progress.split(episodes.len()))
            .filter_map(|(episode, share)| Some((episode.as_ref()?.to_string(), share?)))
            .collect();
        self.store(user, EPISODE, reached).await
    }

    /// Stores, as of now, the User `user`'s progress through each title of `kind`, by id, in one
    /// write.
    async fn store(
        &self,
        user: i64,
        kind: &'static str,
        titles: Vec<(String, Progress)>,
    ) -> Result<(), sqlx::Error> {
        self.db
            .write(move |tx| Box::pin(upsert(tx, user, kind, titles)))
            .await
    }

    /// The User `user`'s Watch State of each of the Movies with the ids `movies`, in the same
    /// order: `None` for a Movie they haven't started.
    pub(crate) async fn of_movies(
        &self,
        user: i64,
        movies: &[&ExternalId],
    ) -> Result<Vec<Option<WatchState>>, sqlx::Error> {
        self.of(user, MOVIE, movies).await
    }

    /// The User `user`'s Watch State of each of the Episodes with the ids `episodes`, in the same
    /// order: `None` for an Episode they haven't started.
    pub(crate) async fn of_episodes(
        &self,
        user: i64,
        episodes: &[&ExternalId],
    ) -> Result<Vec<Option<WatchState>>, sqlx::Error> {
        self.of(user, EPISODE, episodes).await
    }

    /// The User `user`'s Watch State of each of the titles of `kind` with the ids `titles`, in the
    /// same order.
    // ponytail: reads all of a User's titles of the kind even for one; query by id if that gets
    // big.
    async fn of(
        &self,
        user: i64,
        kind: &str,
        titles: &[&ExternalId],
    ) -> Result<Vec<Option<WatchState>>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT title_id, position_ms, duration_ms, played AS "played: bool", updated_at
               FROM watch_states
               WHERE user_id = ? AND kind = ?"#,
            user,
            kind
        )
        .fetch_all(self.db.reader())
        .await?;
        let mut states: HashMap<String, WatchState> = rows
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
            .collect();
        Ok(titles
            .iter()
            .map(|title| states.remove(&title.to_string()))
            .collect())
    }
}

/// Stores, as of now, the User `user`'s progress through each title of `kind`, by id. A title
/// once played stays played.
async fn upsert(
    conn: &mut SqliteConnection,
    user: i64,
    kind: &str,
    titles: Vec<(String, Progress)>,
) -> Result<(), sqlx::Error> {
    let now = now();
    for (title, progress) in titles {
        let played = progress.is_played();
        sqlx::query!(
            "INSERT INTO watch_states
                 (user_id, kind, title_id, position_ms, duration_ms, played, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (user_id, kind, title_id) DO UPDATE
             SET position_ms = excluded.position_ms,
                 duration_ms = excluded.duration_ms,
                 played = watch_states.played OR excluded.played,
                 updated_at = excluded.updated_at",
            user,
            kind,
            title,
            progress.position_ms,
            progress.duration_ms,
            played,
            now,
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Progress;

    /// The position and duration of each share, or `None` for one the position hasn't reached.
    fn shares(position_ms: u64, duration_ms: u64, episodes: usize) -> Vec<Option<(i64, i64)>> {
        let progress = Progress::new(position_ms, duration_ms).unwrap();
        progress
            .split(episodes)
            .into_iter()
            .map(|share| share.map(|share| (share.position_ms, share.duration_ms)))
            .collect()
    }

    #[test]
    fn a_two_episode_file_fills_the_first_episodes_half_then_the_seconds() {
        assert_eq!(
            shares(40, 60, 2),
            [Some((30, 30)), Some((10, 30))],
            "40 minutes into an hour"
        );
        assert_eq!(
            shares(30, 60, 2),
            [Some((30, 30)), None],
            "the second not reached"
        );
        assert_eq!(
            shares(0, 60, 2),
            [Some((0, 30)), None],
            "the first is reached at 0"
        );
    }

    #[test]
    fn shares_that_dont_divide_evenly_still_add_up_to_the_duration() {
        assert_eq!(
            shares(1_000, 1_000, 3),
            [Some((333, 333)), Some((333, 333)), Some((334, 334))]
        );
        assert_eq!(
            shares(1, 1, 2),
            [None, Some((1, 1))],
            "a share too short to hold any"
        );
    }

    #[test]
    fn a_position_past_the_duration_is_the_end_and_a_zero_duration_is_refused() {
        assert_eq!(Progress::new(5_000, 1_000), Progress::new(1_000, 1_000));
        assert_eq!(Progress::new(0, 0), None);
        assert_eq!(Progress::new(0, u64::MAX), None);
    }
}
