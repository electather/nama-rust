//! Web sessions: a random opaque token the browser keeps in a cookie, which the Hub keeps only as
//! its SHA-256 hash and ends by deleting its row (ADR 0098).

use std::fmt::Write as _;

use sha2::{Digest as _, Sha256};
use sqlx::{PgConnection, PgPool};

use super::{Account, AccountId, AccountSession, Error};

/// How many days a session lasts after it starts.
pub(crate) const SESSION_DAYS: i32 = 30;

/// How many minutes after signing in a session has Recent Authentication (GLOSSARY.md).
pub(crate) const RECENT_AUTHENTICATION_MINS: i32 = 10;

/// Starts a session for `account` and returns its token.
pub(super) async fn start(conn: &mut PgConnection, account: AccountId) -> Result<String, Error> {
    let mut bytes = [0; 32];
    getrandom::fill(&mut bytes)?;
    let token = bytes.iter().fold(String::new(), |mut hex, byte| {
        let _infallible = write!(hex, "{byte:02x}");
        hex
    });
    sqlx::query!(
        "INSERT INTO account_sessions (account_id, token_hash) VALUES ($1, $2)",
        account.0,
        &hash(&token)[..],
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
             accounts.email_proven_at IS NOT NULL AS "email_proven!",
             account_sessions.created_at > now() - make_interval(mins => $3)
                 AS "authenticated_recently!"
         FROM account_sessions JOIN accounts ON accounts.id = account_sessions.account_id
         WHERE token_hash = $1 AND account_sessions.created_at > now() - make_interval(days => $2)"#,
        &hash(token)[..],
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
pub(super) async fn end(db: &PgPool, session: &AccountSession) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "DELETE FROM account_sessions WHERE id = $1 AND account_id = $2",
        session.id,
        session.account.id.0,
    )
    .execute(db)
    .await?;
    Ok(())
}

/// What the database keeps of a session token.
fn hash(token: &str) -> [u8; 32] {
    Sha256::digest(token).into()
}
