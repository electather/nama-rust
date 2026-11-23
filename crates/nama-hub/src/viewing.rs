//! Viewing Events (GLOSSARY.md): each play by a Linked User, which their Linked Server records under
//! the Account they are Linked to, and which that Account lists and deletes, and the counts of
//! Accounts that finished each Movie and pair of Movies they pool into (ADR 0093).

use std::{
    collections::{HashMap, hash_map::Entry},
    fmt,
    str::FromStr,
};

use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    accounts::AccountId,
    links::{LinkedServer, Links},
};

/// The most Viewing Events one call records.
pub(crate) const MAX_RECORDED: usize = 1000;

/// The most characters an External ID has.
const EXTERNAL_ID_LENGTH: usize = 100;

/// The most characters the key a Server names a Viewing Event by has.
const KEY_LENGTH: usize = 64;

/// The fewest Accounts a pooled count is read with, so none tells about one Account (ADR 0093).
const MIN_POOLED_ACCOUNTS: i64 = 20;

/// The minutes in a day, which a time of day is less than.
const MINUTES_PER_DAY: i32 = 24 * 60;

/// A Viewing Event's id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ViewingEventId(Uuid);

impl fmt::Display for ViewingEventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for ViewingEventId {
    type Err = uuid::Error;

    fn from_str(id: &str) -> Result<Self, Self::Err> {
        id.parse().map(Self)
    }
}

/// Whether a Viewing Event played a Movie or an Episode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Movie,
    Episode,
}

impl Kind {
    /// Its value in `viewing_events.kind`.
    const fn as_str(self) -> &'static str {
        match self {
            Self::Movie => "movie",
            Self::Episode => "episode",
        }
    }

    /// The kind `viewing_events.kind` holds as `stored`.
    fn stored(stored: &str) -> Self {
        if stored == Self::Movie.as_str() {
            Self::Movie
        } else {
            Self::Episode
        }
    }
}

/// What kind of Device played.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeviceKind {
    Unknown,
    Computer,
    Phone,
    Tablet,
    Tv,
}

impl DeviceKind {
    const ALL: [Self; 5] = [
        Self::Unknown,
        Self::Computer,
        Self::Phone,
        Self::Tablet,
        Self::Tv,
    ];

    /// Its value in `viewing_events.device_kind`.
    const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Computer => "computer",
            Self::Phone => "phone",
            Self::Tablet => "tablet",
            Self::Tv => "tv",
        }
    }

    /// The kind `viewing_events.device_kind` holds as `stored`.
    fn stored(stored: &str) -> Self {
        (Self::ALL.into_iter())
            .find(|kind| kind.as_str() == stored)
            .unwrap_or(Self::Unknown)
    }
}

/// One play of a Movie or Episode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ViewingEvent {
    pub(crate) kind: Kind,
    /// Such as `tmdb-603`.
    pub(crate) external_id: String,
    /// In seconds since the Unix epoch.
    pub(crate) started_at: i64,
    /// In the Server's time zone.
    pub(crate) time_of_day_minutes: i32,
    pub(crate) stop_position_ms: i64,
    pub(crate) duration_ms: i64,
    pub(crate) finished: bool,
    pub(crate) device_kind: DeviceKind,
}

impl ViewingEvent {
    /// Whether it keeps the rules `nama.hub.v1.ViewingEvent` gives.
    fn is_valid(&self) -> bool {
        let id_length = self.external_id.chars().count();
        (1..=EXTERNAL_ID_LENGTH).contains(&id_length)
            && (0..MINUTES_PER_DAY).contains(&self.time_of_day_minutes)
            && self.duration_ms > 0
            && (0..=self.duration_ms).contains(&self.stop_position_ms)
    }

    /// Whether pooled counts take it in, when its Account pools.
    fn is_poolable(&self) -> bool {
        // ponytail: Movies only. An Episode isn't a title the Hub knows, and the Hub can't tell
        // its Show; pool Shows once it can.
        self.finished && self.kind == Kind::Movie
    }
}

/// A Viewing Event a Linked Server records for its User `user_id`, named by its `key`.
#[derive(Debug)]
pub(crate) struct UserViewingEvent {
    pub(crate) user_id: i64,
    pub(crate) key: String,
    pub(crate) event: ViewingEvent,
}

impl UserViewingEvent {
    fn is_valid(&self) -> bool {
        (1..=KEY_LENGTH).contains(&self.key.chars().count()) && self.event.is_valid()
    }
}

/// One of an Account's Viewing Events, as its list shows it.
#[derive(Debug)]
pub(crate) struct ListedViewingEvent {
    pub(crate) id: ViewingEventId,
    pub(crate) event: ViewingEvent,
}

/// Why Viewing Events couldn't be recorded.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("send at most {MAX_RECORDED} Viewing Events at once")]
    TooMany,
    #[error("a Viewing Event breaks the rules ViewingEvent gives")]
    Invalid,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Every Account's Viewing Events.
#[derive(Clone)]
pub(crate) struct Viewing {
    db: PgPool,
    links: Links,
}

impl Viewing {
    pub(crate) const fn new(db: PgPool, links: Links) -> Self {
        Self { db, links }
    }

    /// Records `events` under the Accounts `server`'s Users are Linked to, leaving out those of
    /// Users who aren't, and each one already recorded by its key. Records none if any is invalid.
    pub(crate) async fn record(
        &self,
        server: &LinkedServer,
        events: Vec<UserViewingEvent>,
    ) -> Result<(), Error> {
        if events.len() > MAX_RECORDED {
            return Err(Error::TooMany);
        }
        if !events.iter().all(UserViewingEvent::is_valid) {
            return Err(Error::Invalid);
        }
        let mut accounts: HashMap<i64, Option<AccountId>> = HashMap::new();
        for event in &events {
            if let Entry::Vacant(unknown) = accounts.entry(event.user_id) {
                unknown.insert(self.links.linked_account(server, event.user_id).await?);
            }
        }
        let mut tx = self.db.begin().await?;
        for UserViewingEvent {
            user_id,
            key,
            event,
        } in events
        {
            if let Some(&Some(account)) = accounts.get(&user_id) {
                record_one(&mut tx, account, &key, &event).await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }

    /// `account`'s Viewing Events, the latest started first.
    pub(crate) async fn list(
        &self,
        account: AccountId,
    ) -> Result<Vec<ListedViewingEvent>, sqlx::Error> {
        // ponytail: every event in one answer, a few thousand after years of watching; page it
        // once an Account's history outgrows one response.
        let rows = sqlx::query!(
            r#"SELECT id, kind, external_id, extract(epoch FROM started_at)::bigint AS "started_at!",
             time_of_day_minutes, stop_position_ms, duration_ms, finished, device_kind
             FROM viewing_events WHERE account_id = $1 ORDER BY started_at DESC, id DESC"#,
            account.0,
        )
        .fetch_all(&self.db)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| ListedViewingEvent {
                id: ViewingEventId(row.id),
                event: ViewingEvent {
                    kind: Kind::stored(&row.kind),
                    external_id: row.external_id,
                    started_at: row.started_at,
                    time_of_day_minutes: row.time_of_day_minutes.into(),
                    stop_position_ms: row.stop_position_ms,
                    duration_ms: row.duration_ms,
                    finished: row.finished,
                    device_kind: DeviceKind::stored(&row.device_kind),
                },
            })
            .collect())
    }

    /// Whether `account` pools its finished Movies into the counts every Account shares.
    pub(crate) async fn pooling(&self, account: AccountId) -> Result<bool, sqlx::Error> {
        let chosen = sqlx::query_scalar!(
            "SELECT pooling FROM pooling_settings WHERE account_id = $1",
            account.0,
        )
        .fetch_optional(&self.db)
        .await?;
        Ok(chosen.unwrap_or(true))
    }

    /// Turns pooling on or off for `account`. Turning it off stops new counts; what was counted
    /// stays, as nothing in the counts points back to the Account.
    pub(crate) async fn set_pooling(
        &self,
        account: AccountId,
        pooling: bool,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "INSERT INTO pooling_settings (account_id, pooling) VALUES ($1, $2)
             ON CONFLICT (account_id) DO UPDATE SET pooling = excluded.pooling",
            account.0,
            pooling,
        )
        .execute(&self.db)
        .await?;
        Ok(())
    }

    /// How many Accounts finished the Movie `a` and the Movie `b`, or `a` alone when they're the
    /// same, or `None` while fewer than [`MIN_POOLED_ACCOUNTS`] did.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "recommendations read pooled counts (#1819, #1825); until then only tests do"
        )
    )]
    pub(crate) async fn pooled_count(&self, a: &str, b: &str) -> Result<Option<i64>, sqlx::Error> {
        sqlx::query_scalar!(
            "SELECT accounts FROM pooled_counts
             WHERE title_a = least($1::text, $2::text) AND title_b = greatest($1::text, $2::text)
             AND accounts >= $3",
            a,
            b,
            MIN_POOLED_ACCOUNTS,
        )
        .fetch_optional(&self.db)
        .await
    }

    /// Deletes `account`'s Viewing Event `id`. Another Account's stays.
    pub(crate) async fn delete(
        &self,
        account: AccountId,
        id: ViewingEventId,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "DELETE FROM viewing_events WHERE id = $1 AND account_id = $2",
            id.0,
            account.0,
        )
        .execute(&self.db)
        .await?;
        Ok(())
    }
}

/// Records `event` under `account` as `key`, unless it was already, and pools it when it is the
/// Account's first counted play of its Movie.
async fn record_one(
    tx: &mut Transaction<'_, Postgres>,
    account: AccountId,
    key: &str,
    event: &ViewingEvent,
) -> Result<(), Error> {
    // ponytail: deleting the counted play forgets the Movie was counted, so finishing it again
    // counts the Account twice; keep a mark outside the history if inflated counts ever matter.
    if insert(tx, account, key, event).await? {
        add_to_pool(tx, account, &event.external_id).await?;
    }
    Ok(())
}

/// Inserts `event` under `account` as `key`, unless it was already, and answers whether the new row
/// is to be pooled.
async fn insert(
    tx: &mut Transaction<'_, Postgres>,
    account: AccountId,
    key: &str,
    event: &ViewingEvent,
) -> Result<bool, Error> {
    // `is_valid` kept it within a day's minutes.
    let time_of_day =
        i16::try_from(event.time_of_day_minutes).map_err(|_out_of_range| Error::Invalid)?;
    let pooled = sqlx::query_scalar!(
        "INSERT INTO viewing_events (account_id, server_key, kind, external_id, started_at,
         time_of_day_minutes, stop_position_ms, duration_ms, finished, device_kind, pooled)
         VALUES ($1, $2, $3, $4, to_timestamp($5::bigint), $6, $7, $8, $9, $10,
         $11 AND coalesce((SELECT pooling FROM pooling_settings WHERE account_id = $1), true)
         AND NOT EXISTS (SELECT 1 FROM viewing_events
          WHERE account_id = $1 AND external_id = $4 AND pooled))
         ON CONFLICT (account_id, server_key) DO NOTHING
         RETURNING pooled",
        account.0,
        key,
        event.kind.as_str(),
        event.external_id,
        event.started_at,
        time_of_day,
        event.stop_position_ms,
        event.duration_ms,
        event.finished,
        event.device_kind.as_str(),
        event.is_poolable(),
    )
    .fetch_optional(&mut **tx)
    .await?;
    Ok(pooled == Some(true))
}

/// Counts `account` once more for `movie`, and for each pair it makes with a Movie the Account
/// was counted for before, which `viewing_events.pooled` marks, `movie`'s just-inserted play too.
async fn add_to_pool(
    tx: &mut Transaction<'_, Postgres>,
    account: AccountId,
    movie: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO pooled_counts (title_a, title_b)
         SELECT least($2::text, external_id), greatest($2::text, external_id)
         FROM viewing_events WHERE account_id = $1 AND pooled
         ON CONFLICT (title_a, title_b) DO UPDATE SET accounts = pooled_counts.accounts + 1",
        account.0,
        movie,
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[cfg(test)]
#[path = "viewing/viewing_tests.rs"]
mod tests;
