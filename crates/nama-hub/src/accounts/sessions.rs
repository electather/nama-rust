//! Web sessions: a random opaque token the browser keeps in a cookie, which the Hub keeps only as
//! its SHA-256 hash and ends by deleting its row (ADR 0098).

use sqlx::{PgConnection, PgExecutor, PgPool};

use super::{Account, AccountId, AccountSession, Error, new_token, token_hash};

/// How many days a session lasts after it starts.
pub(crate) const SESSION_DAYS: i32 = 30;

/// How many minutes after signing in a session has Recent Authentication (GLOSSARY.md).
pub(crate) const RECENT_AUTHENTICATION_MINS: i32 = 10;

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
    let found = sqlx::query!(
        r#"SELECT account_sessions.id, accounts.id AS account_id, accounts.email,
             accounts.email_proven,
             account_sessions.created_at > now() - make_interval(mins => $3)
                 AS "authenticated_recently!"
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

#[cfg(test)]
#[path = "sessions_tests.rs"]
mod tests;
