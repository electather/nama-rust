//! Viewing Events (GLOSSARY.md): each play by a Linked User, which their Linked Server records under
//! the Account they are Linked to, and which that Account lists and deletes (ADR 0093).

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
                unknown.insert(self.links.user_account_id(server, event.user_id).await?);
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
                insert(&mut tx, account, &key, &event).await?;
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
        // shortcut: every event in one answer, a few thousand after years of watching; page it
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

/// Records `event` under `account` as `key`, unless it was already.
async fn insert(
    tx: &mut Transaction<'_, Postgres>,
    account: AccountId,
    key: &str,
    event: &ViewingEvent,
) -> Result<(), Error> {
    // `is_valid` kept it within a day's minutes.
    let time_of_day =
        i16::try_from(event.time_of_day_minutes).map_err(|_out_of_range| Error::Invalid)?;
    sqlx::query!(
        "INSERT INTO viewing_events (account_id, server_key, kind, external_id, started_at,
         time_of_day_minutes, stop_position_ms, duration_ms, finished, device_kind)
         VALUES ($1, $2, $3, $4, to_timestamp($5::bigint), $6, $7, $8, $9, $10)
         ON CONFLICT (account_id, server_key) DO NOTHING",
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
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[cfg(test)]
#[path = "viewing/viewing_tests.rs"]
mod tests;
