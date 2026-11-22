//! Sign-in Links: a random token emailed to an Account's address, kept only as its SHA-256 hash
//! (ADR 0098), that signs the Account in once within [`LINK_MINUTES`]. Using one proves the
//! Account owns its email, and the first used on an unproven Account takes it over (ADR 0100).

use std::{net::IpAddr, sync::Arc, time::Instant};

use sqlx::PgPool;
use tokio::sync::mpsc;

use super::{
    Account, AccountId, AccountSession, Error, SignedIn, new_token, rate_limits::LinkLimits,
    sessions, token_hash,
};
use crate::mail::{Email, Mail, MailError};

/// How many minutes a Sign-in Link works for after it is sent.
const LINK_MINUTES: i32 = 15;

/// How many requests may wait to be sent. Past it, a request fails rather than queue without end.
const QUEUE_CAPACITY: usize = 256;

/// The longest path a sign-in may continue at.
const CONTINUE_TO_LENGTH: usize = 2048;

/// Sends Sign-in Links one at a time, off the request path, so asking for one answers as fast
/// whether or not the email has an Account.
// ponytail: one sender in this process (ADR 0096); a queue table once the Hub runs as replicas.
#[derive(Clone, Debug)]
pub(super) struct Outbox {
    requests: mpsc::Sender<LinkRequest>,
    limits: Arc<LinkLimits>,
}

/// A Sign-in Link asked for, waiting to be sent.
#[derive(Debug)]
struct LinkRequest {
    email: String,
    continue_to: String,
}

/// A waiting Sign-in Link, which names its Account without being used up.
#[derive(Debug)]
pub(crate) struct SignInLink {
    pub(crate) account: Account,
    /// If not, using the link takes the Account over (ADR 0100).
    pub(crate) email_proven: bool,
}

/// An Account just signed in with a Sign-in Link, and the Hub path to continue at.
#[derive(Debug)]
pub(crate) struct SignedInByLink {
    pub(crate) signed_in: SignedIn,
    pub(crate) continue_to: String,
}

impl Outbox {
    /// Starts sending through `mail` what [`Outbox::send`] queues, until the last clone of the
    /// returned one is dropped. Must run inside a Tokio runtime.
    pub(super) fn start(db: PgPool, mail: Mail) -> Self {
        let (requests, queued) = mpsc::channel(QUEUE_CAPACITY);
        tokio::spawn(deliver_all(db, mail, queued));
        Self {
            requests,
            limits: Arc::default(),
        }
    }

    /// Queues a Sign-in Link to `email`, already checked to be an address, continuing at
    /// `continue_to`, as asked by `ip` at `now`.
    pub(super) fn send(
        &self,
        email: &str,
        continue_to: &str,
        ip: IpAddr,
        now: Instant,
    ) -> Result<(), Error> {
        self.limits.admit(ip, email, now)?;
        let request = LinkRequest {
            email: email.to_owned(),
            continue_to: hub_path(continue_to).to_owned(),
        };
        self.requests
            .try_send(request)
            .map_err(|_dropped| Error::MailQueueFull)
    }
}

/// `path` if it is a path on the Hub, or else `/`, so a link can't send anyone to another site.
fn hub_path(path: &str) -> &str {
    // A browser reads `//host` and `/\host` as another site, and drops tabs and newlines first.
    let on_hub = path.starts_with('/')
        && !path.starts_with("//")
        && !path.starts_with("/\\")
        && !path.contains(char::is_control)
        && path.len() <= CONTINUE_TO_LENGTH;
    if on_hub { path } else { "/" }
}

/// Sends each request `queued` holds, logging any that fails.
async fn deliver_all(db: PgPool, mail: Mail, mut queued: mpsc::Receiver<LinkRequest>) {
    while let Some(request) = queued.recv().await {
        if let Err(err) = deliver(&db, &mail, &request).await {
            tracing::error!("could not send a Sign-in Link: {err}");
        }
    }
}

/// Why a Sign-in Link wasn't sent.
#[derive(Debug, thiserror::Error)]
enum DeliveryError {
    #[error("could not generate its token: {0}")]
    Random(#[from] getrandom::Error),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("to Account {account}: {err}")]
    Mail { account: AccountId, err: MailError },
}

/// Emails a new Sign-in Link to the Account `request` names, if there is one.
async fn deliver(db: &PgPool, mail: &Mail, request: &LinkRequest) -> Result<(), DeliveryError> {
    let Some((account, token)) = issue(db, &request.email, &request.continue_to).await? else {
        return Ok(());
    };
    let url = format!(
        "{}/sign-in-link#{token}",
        mail.public_url.trim_end_matches('/')
    );
    let email = Email {
        text: format!(
            "Sign in to Nama Hub as {} with this link:\n\n{url}\n\nIt works once, within \
             {LINK_MINUTES} minutes. If you didn't ask to sign in, you can ignore this email.\n",
            account.email
        ),
        to: account.email,
        subject: "Your Nama Hub Sign-in Link".to_owned(),
    };
    mail.mailer
        .send(email)
        .await
        .map_err(|err| DeliveryError::Mail {
            account: account.id,
            err,
        })
}

/// Makes a new Sign-in Link, continuing at `continue_to`, for the Account whose email is `email`,
/// compared without regard to case, replacing any it had. `None` if no Account has the email.
async fn issue(
    db: &PgPool,
    email: &str,
    continue_to: &str,
) -> Result<Option<(Account, String)>, DeliveryError> {
    let Some(found) = sqlx::query!(
        "SELECT id, email FROM accounts WHERE lower(email) = lower($1)",
        email,
    )
    .fetch_optional(db)
    .await?
    else {
        return Ok(None);
    };
    let token = new_token()?;
    sqlx::query!(
        "INSERT INTO sign_in_links (account_id, token_hash, continue_to) VALUES ($1, $2, $3)
         ON CONFLICT (account_id) DO UPDATE
         SET token_hash = excluded.token_hash, continue_to = excluded.continue_to, created_at = now()",
        found.id,
        &token_hash(&token)[..],
        continue_to,
    )
    .execute(db)
    .await?;
    let account = Account {
        id: AccountId(found.id),
        email: found.email,
    };
    Ok(Some((account, token)))
}

/// The waiting Sign-in Link `token` names, unless it expired or was used.
pub(super) async fn find(db: &PgPool, token: &str) -> Result<SignInLink, Error> {
    let found = sqlx::query!(
        "SELECT accounts.id, accounts.email, accounts.email_proven
         FROM sign_in_links JOIN accounts ON accounts.id = sign_in_links.account_id
         WHERE token_hash = $1 AND sign_in_links.created_at > now() - make_interval(mins => $2)",
        &token_hash(token)[..],
        LINK_MINUTES,
    )
    .fetch_optional(db)
    .await?
    .ok_or(Error::SignInLinkExpired)?;
    Ok(SignInLink {
        account: Account {
            id: AccountId(found.id),
            email: found.email,
        },
        email_proven: found.email_proven,
    })
}

/// Uses up the Sign-in Link `token` names: proves its Account's email, taking the Account over if
/// it wasn't proven, ends `replacing`, the browser's session, if any, and signs the Account in.
pub(super) async fn use_up(
    db: &PgPool,
    token: &str,
    replacing: Option<&AccountSession>,
) -> Result<SignedInByLink, Error> {
    let mut tx = db.begin().await?;
    let found = sqlx::query!(
        "DELETE FROM sign_in_links USING accounts
         WHERE sign_in_links.token_hash = $1
           AND sign_in_links.created_at > now() - make_interval(mins => $2)
           AND accounts.id = sign_in_links.account_id
         RETURNING sign_in_links.account_id, accounts.email, sign_in_links.continue_to",
        &token_hash(token)[..],
        LINK_MINUTES,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(Error::SignInLinkExpired)?;
    let account = Account {
        id: AccountId(found.account_id),
        email: found.email,
    };
    // ADR 0100: whoever created the Account may not own the inbox, so the first proof shuts them
    // out. Passkeys go too, once Accounts have them.
    let taken_over = sqlx::query!(
        "UPDATE accounts SET email_proven = true, password_hash = NULL
         WHERE id = $1 AND NOT email_proven",
        found.account_id,
    )
    .execute(&mut *tx)
    .await?
    .rows_affected()
        == 1;
    if taken_over {
        sessions::end_all(&mut tx, account.id).await?;
    }
    if let Some(session) = replacing {
        sessions::end(&mut *tx, session).await?;
    }
    let session_token = sessions::start(&mut tx, account.id).await?;
    tx.commit().await?;
    Ok(SignedInByLink {
        signed_in: SignedIn {
            account,
            token: session_token,
        },
        continue_to: found.continue_to,
    })
}

#[cfg(test)]
#[path = "sign_in_links_tests.rs"]
mod tests;
