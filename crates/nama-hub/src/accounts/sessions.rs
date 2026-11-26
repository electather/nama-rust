//! Web sessions: a random opaque token the browser keeps in a cookie, which the Hub keeps only as
//! its SHA-256 hash and ends by deleting its row (ADR 0098).

use sqlx::{PgConnection, PgExecutor, PgPool};
use uuid::Uuid;

use super::{Account, AccountId, AccountSession, Accounts, Error, new_token, token_hash};

/// How many days a session lasts after it starts.
pub(crate) const SESSION_DAYS: i32 = 30;

/// How many minutes after signing in a session has Recent Authentication (GLOSSARY.md).
pub(crate) const RECENT_AUTHENTICATION_MINS: i32 = 10;

/// How many minutes a session's last use may lag, so not every request writes it.
const LAST_USED_MINS: i32 = 5;

/// The most characters of a `User-Agent` kept, since any client can send a long one.
const USER_AGENT_LENGTH: usize = 512;

/// One of an Account's live sessions, as its Account page lists them.
#[derive(Debug)]
pub(crate) struct ListedSession {
    pub(crate) id: Uuid,
    /// The browser and OS it was last used from, such as "Firefox on Windows".
    pub(crate) name: String,
    /// In seconds since the Unix epoch, to within [`LAST_USED_MINS`].
    pub(crate) last_used_at: i64,
    /// Whether it's the session asking.
    pub(crate) is_current: bool,
}

/// A request refused because its session lacks Recent Authentication, for any area to pass on.
#[derive(Debug, thiserror::Error)]
#[error("this needs a sign-in in the last {RECENT_AUTHENTICATION_MINS} minutes: sign in again")]
pub(crate) struct RecentAuthenticationRequired;

/// Starts a session for `account` and returns its token.
pub(super) async fn start(conn: &mut PgConnection, account: AccountId) -> Result<String, Error> {
    let token = new_token()?;
    sqlx::query!(
        "INSERT INTO account_sessions (account_id, token_hash) VALUES ($1, $2)",
        account.0,
        &token_hash(&token)[..],
    )
    .execute(conn)
    .await?;
    Ok(token)
}

/// The session `token` names, unless it has ended or is older than [`SESSION_DAYS`].
// ponytail: an expired session's row stays until its Account is deleted; sweep them once a
// periodic job runs in the Hub.
pub(super) async fn find(db: &PgPool, token: &str) -> Result<Option<AccountSession>, sqlx::Error> {
    // `floor`, as a bare cast to bigint rounds: a session made late in a second would say it
    // signed in during the next one, in the future, in a Server token's `auth_time` (#2405).
    let found = sqlx::query!(
        r#"SELECT account_sessions.id, accounts.id AS account_id, accounts.email,
             accounts.email_proven,
             account_sessions.created_at > now() - make_interval(mins => $3)
                 AS "authenticated_recently!",
             floor(extract(epoch FROM account_sessions.created_at))::bigint AS "signed_in_at!"
         FROM account_sessions JOIN accounts ON accounts.id = account_sessions.account_id
         WHERE token_hash = $1 AND account_sessions.created_at > now() - make_interval(days => $2)"#,
        &token_hash(token)[..],
        SESSION_DAYS,
        RECENT_AUTHENTICATION_MINS,
    )
    .fetch_optional(db)
    .await?;
    Ok(found.map(|found| AccountSession {
        account: Account {
            id: AccountId(found.account_id),
            email: found.email,
        },
        email_proven: found.email_proven,
        authenticated_recently: found.authenticated_recently,
        signed_in_at: found.signed_in_at,
        id: found.id,
    }))
}

/// Ends `session`.
pub(super) async fn end(
    db: impl PgExecutor<'_>,
    session: &AccountSession,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "DELETE FROM account_sessions WHERE id = $1 AND account_id = $2",
        session.id,
        session.account.id.0,
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Locks `session`'s Account row until `conn`'s transaction ends, and says whether `session` still
/// has Recent Authentication.
// A Sign-in Link's takeover (ADR 0100) locks the same row before it ends the Account's sessions,
// so a password set while it runs either commits first, and the takeover then removes it, or sees
// `session` ended. The check is a second statement so it reads what committed while it waited.
pub(super) async fn lock_while_recent(
    conn: &mut PgConnection,
    session: &AccountSession,
) -> Result<bool, sqlx::Error> {
    sqlx::query!(
        "SELECT id FROM accounts WHERE id = $1 FOR UPDATE",
        session.account.id.0,
    )
    .fetch_optional(&mut *conn)
    .await?;
    sqlx::query_scalar!(
        r#"SELECT EXISTS (
             SELECT 1 FROM account_sessions
             WHERE id = $1 AND created_at > now() - make_interval(mins => $2)
           ) AS "recent!""#,
        session.id,
        RECENT_AUTHENTICATION_MINS,
    )
    .fetch_one(conn)
    .await
}

/// Ends every session of `session`'s Account but `session`.
pub(super) async fn end_others(
    conn: &mut PgConnection,
    session: &AccountSession,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "DELETE FROM account_sessions WHERE account_id = $1 AND id <> $2",
        session.account.id.0,
        session.id,
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Ends every session of `account`.
pub(super) async fn end_all(
    conn: &mut PgConnection,
    account: AccountId,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "DELETE FROM account_sessions WHERE account_id = $1",
        account.0
    )
    .execute(conn)
    .await?;
    Ok(())
}

impl Accounts {
    /// Records that a browser sending `user_agent` used `session` now.
    pub(crate) async fn record_use(
        &self,
        session: &AccountSession,
        user_agent: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let user_agent: Option<String> =
            user_agent.map(|sent| sent.chars().take(USER_AGENT_LENGTH).collect());
        sqlx::query!(
            "UPDATE account_sessions SET last_used_at = now(), user_agent = $3
             WHERE id = $1 AND account_id = $2
               AND (last_used_at < now() - make_interval(mins => $4)
                 OR user_agent IS DISTINCT FROM $3)",
            session.id,
            session.account.id.0,
            user_agent,
            LAST_USED_MINS,
        )
        .execute(&self.db)
        .await?;
        Ok(())
    }

    /// The live sessions of `session`'s Account: `session` first, then the most recently used.
    pub(crate) async fn sessions(
        &self,
        session: &AccountSession,
    ) -> Result<Vec<ListedSession>, sqlx::Error> {
        let listed = sqlx::query!(
            r#"SELECT id, user_agent, extract(epoch FROM last_used_at)::bigint AS "last_used_at!"
             FROM account_sessions
             WHERE account_id = $1 AND created_at > now() - make_interval(days => $2)
             ORDER BY id = $3 DESC, last_used_at DESC, id"#,
            session.account.id.0,
            SESSION_DAYS,
            session.id,
        )
        .fetch_all(&self.db)
        .await?;
        Ok(listed
            .into_iter()
            .map(|listed| ListedSession {
                id: listed.id,
                name: browser_name(listed.user_agent.as_deref()),
                last_used_at: listed.last_used_at,
                is_current: listed.id == session.id,
            })
            .collect())
    }

    /// Ends the session `id` if it's another of `session`'s Account's. Needs Recent
    /// Authentication.
    pub(crate) async fn sign_out_session(
        &self,
        session: &AccountSession,
        id: Uuid,
    ) -> Result<(), Error> {
        let mut tx = self.db.begin().await?;
        // Under the row lock, as every check of Recent Authentication that changes something is.
        if !lock_while_recent(&mut tx, session).await? {
            return Err(RecentAuthenticationRequired.into());
        }
        if id != session.id {
            sqlx::query!(
                "DELETE FROM account_sessions WHERE id = $1 AND account_id = $2",
                id,
                session.account.id.0,
            )
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Ends every session of `session`'s Account but `session`. Needs Recent Authentication.
    pub(crate) async fn sign_out_other_sessions(
        &self,
        session: &AccountSession,
    ) -> Result<(), Error> {
        let mut tx = self.db.begin().await?;
        if !lock_while_recent(&mut tx, session).await? {
            return Err(RecentAuthenticationRequired.into());
        }
        end_others(&mut tx, session).await?;
        tx.commit().await?;
        Ok(())
    }
}

/// Browsers, matched in order: Edge and Opera also claim to be Chrome, and Chrome claims Safari.
const BROWSERS: &[(&str, &str)] = &[
    ("Edg", "Edge"),
    ("OPR/", "Opera"),
    ("Firefox/", "Firefox"),
    ("FxiOS", "Firefox"),
    ("CriOS", "Chrome"),
    ("Chrome/", "Chrome"),
    ("Safari/", "Safari"),
];

/// Operating systems, matched in order: iOS says "like Mac OS X", and Android and `ChromeOS` say
/// Linux.
const SYSTEMS: &[(&str, &str)] = &[
    ("iPhone", "iPhone"),
    ("iPad", "iPad"),
    ("Android", "Android"),
    ("CrOS", "ChromeOS"),
    ("Windows", "Windows"),
    ("Macintosh", "macOS"),
    ("Linux", "Linux"),
];

/// A session's name after the browser and OS in its `user_agent`, such as "Chrome on macOS", as
/// the Server names a Device (`nama-server`'s `device_names.rs`).
// ponytail: substring matching covers the common browsers; reach for a UA parser crate if
// sessions from other clients need better names.
fn browser_name(user_agent: Option<&str>) -> String {
    let user_agent = user_agent.unwrap_or_default();
    let find = |table: &[(&str, &'static str)]| {
        table
            .iter()
            .find(|(needle, _)| user_agent.contains(needle))
            .map(|&(_, name)| name)
    };
    match (find(BROWSERS), find(SYSTEMS)) {
        (Some(browser), Some(system)) => format!("{browser} on {system}"),
        (Some(name), None) | (None, Some(name)) => name.to_owned(),
        (None, None) => "Unknown browser".to_owned(),
    }
}

#[cfg(test)]
#[path = "sessions_tests.rs"]
mod tests;
