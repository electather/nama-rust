//! Accounts: creating one with an email and a password, and signing it in and out, each sign-in a
//! web session (ADR 0098).

mod passwords;
mod rate_limits;
mod sessions;

use std::{fmt, net::IpAddr, sync::Arc, time::Instant};

use sqlx::PgPool;
use uuid::Uuid;

pub(crate) use passwords::PASSWORD_LENGTH;
pub(crate) use rate_limits::Blocked;
use rate_limits::SignInLimits;
pub(crate) use sessions::SESSION_DAYS;

/// The most characters an email may have: the longest a mailbox can be (RFC 5321).
pub(crate) const EMAIL_LENGTH: usize = 254;

/// An Account's id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AccountId(Uuid);

impl fmt::Display for AccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// A person's identity on the Hub.
#[derive(Clone, Debug)]
pub(crate) struct Account {
    pub(crate) id: AccountId,
    /// As it was typed.
    pub(crate) email: String,
}

/// A browser signed in as an Account.
#[derive(Clone, Debug)]
pub(crate) struct AccountSession {
    pub(crate) account: Account,
    id: Uuid,
}

/// An Account just signed in, and the session token its browser presents from now on.
#[derive(Debug)]
pub(crate) struct SignedIn {
    pub(crate) account: Account,
    pub(crate) token: String,
}

/// Why an Account couldn't be created or signed in.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("another Account has this email")]
    EmailTaken,
    #[error("the email isn't an address, or is over {EMAIL_LENGTH} characters")]
    EmailInvalid,
    #[error(
        "a password needs {} to {} characters",
        PASSWORD_LENGTH.start(),
        PASSWORD_LENGTH.end()
    )]
    PasswordLength,
    #[error("no Account has this email and password")]
    WrongEmailOrPassword,
    #[error(transparent)]
    Blocked(#[from] Blocked),
    #[error("could not hash or check a password: {0}")]
    Password(#[from] passwords::PasswordError),
    #[error("could not generate a session token: {0}")]
    Random(#[from] getrandom::Error),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// The Accounts kept in the database, and the recent failed sign-ins.
#[derive(Clone)]
pub(crate) struct Accounts {
    db: PgPool,
    limits: Arc<SignInLimits>,
}

impl Accounts {
    pub(crate) fn new(db: PgPool) -> Self {
        Self {
            db,
            limits: Arc::default(),
        }
    }

    /// Creates an Account with `email`, trimmed, and `password`, and signs it in.
    pub(crate) async fn create(&self, email: &str, password: &str) -> Result<SignedIn, Error> {
        let email = email.trim();
        if !is_email(email) {
            return Err(Error::EmailInvalid);
        }
        if !PASSWORD_LENGTH.contains(&password.chars().count()) {
            return Err(Error::PasswordLength);
        }
        let password_hash = passwords::hash(password.to_owned()).await?;
        let mut tx = self.db.begin().await?;
        let id = sqlx::query_scalar!(
            "INSERT INTO accounts (email, password_hash) VALUES ($1, $2)
             ON CONFLICT ((lower(email))) DO NOTHING RETURNING id",
            email,
            password_hash,
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(Error::EmailTaken)?;
        let account = Account {
            id: AccountId(id),
            email: email.to_owned(),
        };
        let token = sessions::start(&mut tx, account.id).await?;
        tx.commit().await?;
        Ok(SignedIn { account, token })
    }

    /// Signs in the Account whose email is `email`, compared without regard to case, if
    /// `password` is its password, from the client at `ip` at `now`.
    pub(crate) async fn sign_in(
        &self,
        email: &str,
        password: &str,
        ip: IpAddr,
        now: Instant,
    ) -> Result<SignedIn, Error> {
        let email = email.trim();
        let attempt = self.limits.admit(ip, email, now)?;
        let found = sqlx::query!(
            "SELECT id, email, password_hash FROM accounts WHERE lower(email) = lower($1)",
            email,
        )
        .fetch_optional(&self.db)
        .await?;
        let Some(found) = found else {
            passwords::verify_unknown(password.to_owned()).await;
            return Err(Error::WrongEmailOrPassword);
        };
        if !passwords::verify(password.to_owned(), found.password_hash).await? {
            return Err(Error::WrongEmailOrPassword);
        }
        self.limits.succeeded(&attempt);
        let account = Account {
            id: AccountId(found.id),
            email: found.email,
        };
        let mut conn = self.db.acquire().await?;
        let token = sessions::start(&mut conn, account.id).await?;
        Ok(SignedIn { account, token })
    }

    /// The session `token` names, unless it has ended or expired.
    pub(crate) async fn session(&self, token: &str) -> Result<Option<AccountSession>, sqlx::Error> {
        sessions::find(&self.db, token).await
    }

    /// Ends `session`: its token stops working at once.
    pub(crate) async fn sign_out(&self, session: &AccountSession) -> Result<(), sqlx::Error> {
        sessions::end(&self.db, session).await
    }
}

/// Whether `email` looks like an address: something, an `@`, then something, with no whitespace,
/// and not too long. Only a Sign-in Link proves the address is real (ADR 0100).
fn is_email(email: &str) -> bool {
    email.chars().count() <= EMAIL_LENGTH
        && !email.contains(char::is_whitespace)
        && email
            .rsplit_once('@')
            .is_some_and(|(local, domain)| !local.is_empty() && !domain.is_empty())
}

#[cfg(test)]
#[path = "accounts/accounts_tests.rs"]
mod tests;
