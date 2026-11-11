//! Watch State: how far each User has got through a Movie or Episode, and whether they've played
//! it.
//!
//! [`WatchStates`] is the only code that reads or writes `watch_states`. It keeps Watch State
//! against a Movie's or Episode's external id, never against an Item or file (ADR 0013), so it
//! survives rescans, renames and new Versions. Which Movie or Episodes a Version or a marked title
//! stands for is for [`crate::browse::Browse`] to work out.

use std::{collections::HashMap, sync::Arc};

use nama_plugin_api::metadata::ExternalId;
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
    /// rewinding keeps progress stored on a later one. One once played stays played.
    pub(crate) async fn report(
        &self,
        user: i64,
        titles: Vec<Option<MovieOrEpisode>>,
        progress: Progress,
    ) -> Result<(), sqlx::Error> {
        let shares = progress.split(titles.len());
        let reached = titles
            .into_iter()
            .zip(shares)
            .filter_map(|(title, share)| Some((title?, share?)))
            .collect();
        self.db
            .write(move |tx| Box::pin(upsert(tx, user, reached)))
            .await
    }

    /// Marks, as of now, each of `titles` played or unplayed for the User `user`, in one write.
    /// Played keeps the position; unplayed forgets the Watch State, so it starts from the
    /// beginning.
    pub(crate) async fn mark(
        &self,
        user: i64,
        titles: Vec<MovieOrEpisode>,
        played: bool,
    ) -> Result<(), sqlx::Error> {
        self.db
            .write(move |tx| Box::pin(mark_each(tx, user, titles, played)))
            .await
    }

    /// The User `user`'s Watch State of each of `titles`, in the same order: `None` for one they
    /// haven't started.
    // ponytail: reads all of a User's Watch States even for one; query by id if that gets big.
    pub(crate) async fn of(
        &self,
        user: i64,
        titles: &[MovieOrEpisode],
    ) -> Result<Vec<Option<WatchState>>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT kind, title_id, position_ms, duration_ms, played AS "played: bool", updated_at
               FROM watch_states
               WHERE user_id = ?"#,
            user
        )
        .fetch_all(self.db.reader())
        .await?;
        let mut states: HashMap<MovieOrEpisode, WatchState> = rows
            .into_iter()
            .map(|row| {
                let state = WatchState {
                    position_ms: row.position_ms,
                    duration_ms: row.duration_ms,
                    played: row.played,
                    updated_at: row.updated_at,
                };
                (MovieOrEpisode::stored(&row.kind, row.title_id), state)
            })
            .collect();
        Ok(titles.iter().map(|title| states.remove(title)).collect())
    }

    /// The User `user`'s Continue Watching: the Movies and Episodes they've started and not
    /// played, with their Watch State, most recent first. Ones changed in the same second come in
    /// a fixed order, by kind and id, so the list doesn't shuffle between calls.
    pub(crate) async fn continue_watching(
        &self,
        user: i64,
    ) -> Result<Vec<(MovieOrEpisode, WatchState)>, sqlx::Error> {
        let rows = sqlx::query!(
            "SELECT kind, title_id, position_ms, duration_ms, updated_at
             FROM watch_states
             WHERE user_id = ? AND position_ms > 0 AND NOT played
             ORDER BY updated_at DESC, kind, title_id",
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
                    played: false,
                    updated_at: row.updated_at,
                };
                (watched, state)
            })
            .collect())
    }
}

/// One once played stays played.
async fn upsert(
    conn: &mut SqliteConnection,
    user: i64,
    titles: Vec<(MovieOrEpisode, Progress)>,
) -> Result<(), sqlx::Error> {
    let now = now();
    for (title, progress) in titles {
        let (kind, title) = title.key();
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

async fn mark_each(
    conn: &mut SqliteConnection,
    user: i64,
    titles: Vec<MovieOrEpisode>,
    played: bool,
) -> Result<(), sqlx::Error> {
    let now = now();
    for title in titles {
        let (kind, title) = title.key();
        if played {
            mark_played(&mut *conn, user, kind, title, now).await?;
        } else {
            forget(&mut *conn, user, kind, title).await?;
        }
    }
    Ok(())
}

/// Keeps any position. One the User hasn't started is at 0 of an unknown duration, 0.
async fn mark_played(
    conn: &mut SqliteConnection,
    user: i64,
    kind: &'static str,
    title: &str,
    now: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO watch_states
             (user_id, kind, title_id, position_ms, duration_ms, played, updated_at)
         VALUES (?, ?, ?, 0, 0, 1, ?)
         ON CONFLICT (user_id, kind, title_id) DO UPDATE
         SET played = 1, updated_at = excluded.updated_at",
        user,
        kind,
        title,
        now,
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Deleting the row is how a title becomes unplayed with no position.
async fn forget(
    conn: &mut SqliteConnection,
    user: i64,
    kind: &'static str,
    title: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "DELETE FROM watch_states WHERE user_id = ? AND kind = ? AND title_id = ?",
        user,
        kind,
        title,
    )
    .execute(conn)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use nama_plugin_api::metadata::ExternalId;

    use super::{MovieOrEpisode, Progress, WatchStates};
    use crate::{
        db::Database,
        plugins::Registry,
        users::{Permissions, User, Users},
    };

    const ALIEN_TMDB_ID: i64 = 348;

    /// Watch States on a database in a temp folder, and a User to keep them for.
    async fn watch_states(data: &tempfile::TempDir) -> (WatchStates, i64) {
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let root = User {
            id: 0,
            name: "Root".to_owned(),
            permissions: Permissions::ADMIN,
            created_at: 0,
        };
        let plugins = Registry::new();
        plugins
            .enable(Arc::new(nama_plugin_local_auth::LocalAuth))
            .unwrap();
        let user = Users::new(Arc::clone(&db), plugins)
            .create(&root, "Ada", "correct horse battery staple")
            .await
            .unwrap();
        (WatchStates::new(db), user)
    }

    fn alien() -> MovieOrEpisode {
        MovieOrEpisode::movie(&ExternalId::Tmdb(ALIEN_TMDB_ID))
    }

    #[tokio::test]
    async fn a_movie_once_played_stays_played_when_played_again_from_the_start() {
        let data = tempfile::tempdir().unwrap();
        let (watch_states, user) = watch_states(&data).await;
        let near_the_end = Progress::new(1_950, 2_000).unwrap();
        let near_the_start = Progress::new(100, 2_000).unwrap();

        for progress in [near_the_end, near_the_start] {
            watch_states
                .report(user, vec![Some(alien())], progress)
                .await
                .unwrap();
        }

        let state = watch_states.of(user, &[alien()]).await.unwrap()[0].unwrap();
        assert_eq!((state.position_ms, state.played), (100, true));
    }

    #[tokio::test]
    async fn marking_a_movie_unplayed_forgets_its_position() {
        let data = tempfile::tempdir().unwrap();
        let (watch_states, user) = watch_states(&data).await;
        let halfway = Progress::new(1_000, 2_000).unwrap();
        watch_states
            .report(user, vec![Some(alien())], halfway)
            .await
            .unwrap();
        watch_states.mark(user, vec![alien()], true).await.unwrap();

        watch_states.mark(user, vec![alien()], false).await.unwrap();

        assert_eq!(watch_states.of(user, &[alien()]).await.unwrap(), [None]);
    }

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
