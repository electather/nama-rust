//! Accounts: creating one with an email and a password, signing it in, with its password, a
//! passkey or a Sign-in Link, and out, each sign-in a web session (ADR 0098) it lists and can end,
//! setting its password, its passkeys, and deleting it.

mod passkeys;
mod passwords;
mod rate_limits;
mod sessions;
mod sign_in_links;
mod webauthn;

use std::{
    fmt::{self, Write as _},
    sync::Arc,
};

use sha2::{Digest as _, Sha256};
use sqlx::PgPool;
use url::Url;
use uuid::Uuid;

use crate::mail::Mail;
pub(crate) use passkeys::{Assertion, NewPasskey, PASSKEY_NAME_LENGTH, Passkey};
pub(crate) use passwords::PASSWORD_LENGTH;
pub(crate) use rate_limits::{Blocked, Caller};
use rate_limits::{CreationLimits, SignInLimits};
pub(crate) use sessions::{
    ListedSession, RECENT_AUTHENTICATION_MINS, RecentAuthenticationRequired, SESSION_DAYS,
};
use sign_in_links::Outbox;
pub(crate) use sign_in_links::{SignInLink, SignedInByLink};
use webauthn::RelyingParty;

/// The most characters an email may have: the longest a mailbox can be (RFC 5321).
pub(crate) const EMAIL_LENGTH: usize = 254;

/// An Account's id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AccountId(pub(crate) Uuid);

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
    /// Whether a Sign-in Link proved the Account owns its email, which Linking needs (ADR 0100).
    pub(crate) email_proven: bool,
    /// Whether the session signed in within [`RECENT_AUTHENTICATION_MINS`]: read only through
    /// [`AccountSession::require_recent_authentication`], so the rule lives here.
    authenticated_recently: bool,
    id: Uuid,
}

impl AccountSession {
    /// Refuses unless the session has Recent Authentication, as of when it was looked up.
    pub(crate) fn require_recent_authentication(&self) -> Result<(), RecentAuthenticationRequired> {
        if self.authenticated_recently {
            Ok(())
        } else {
            Err(RecentAuthenticationRequired)
        }
    }
}

/// An Account just signed in, and the session token its browser presents from now on.
#[derive(Debug)]
pub(crate) struct SignedIn {
    pub(crate) account: Account,
    pub(crate) token: String,
}

/// Why an Account couldn't be created, signed in or changed.
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
    #[error(transparent)]
    RecentAuthenticationRequired(#[from] RecentAuthenticationRequired),
    #[error("no Account has this email and password")]
    WrongEmailOrPassword,
    #[error("the Hub has no mail set up, so it can't email Sign-in Links")]
    SignInLinksOff,
    #[error("the Sign-in Link expired, was used already, or never existed")]
    SignInLinkExpired,
    #[error("the Sign-in Links waiting to be sent are too many, or no longer being sent")]
    MailQueueFull,
    #[error(transparent)]
    Blocked(#[from] Blocked),
    #[error("the Hub has no public address set, so it can't use passkeys")]
    PasskeysOff,
    #[error("no Account has this passkey, or its answer doesn't check out")]
    PasskeyNotAccepted,
    #[error(
        "a passkey's name needs {} to {} characters",
        PASSKEY_NAME_LENGTH.start(),
        PASSKEY_NAME_LENGTH.end()
    )]
    PasskeyNameLength,
    #[error("the Account has no passkey with this id")]
    PasskeyNotFound,
    #[error("the Account would have no way left to sign in")]
    LastSignInMethod,
    #[error("too many passkey sign-ins are under way; try again shortly")]
    ChallengesBusy,
    #[error("could not hash or check a password: {0}")]
    Password(#[from] passwords::PasswordError),
    #[error("could not generate a session token: {0}")]
    Random(#[from] getrandom::Error),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// The Accounts kept in the database, the recent failed sign-ins and creations, the Sign-in Links
/// being sent, and the passkey challenges handed out and waiting for an answer.
#[derive(Clone)]
pub(crate) struct Accounts {
    db: PgPool,
    sign_ins: Arc<SignInLimits>,
    creations: Arc<CreationLimits>,
    /// Only with mail set up.
    outbox: Option<Outbox>,
    /// The Hub at its public address, without which it can't use passkeys.
    relying_party: Option<Arc<RelyingParty>>,
}

impl Accounts {
    /// Accounts in `db`, sending Sign-in Links through `mail` if it is set up, whose passkeys are
    /// for the Hub at `public_url`, if it has one. Must run inside a Tokio runtime.
    pub(crate) fn new(db: PgPool, mail: Option<Mail>, public_url: Option<&Url>) -> Self {
        Self {
            outbox: mail.map(|mail| Outbox::start(db.clone(), mail)),
            db,
            sign_ins: Arc::default(),
            creations: Arc::default(),
            relying_party: public_url.and_then(RelyingParty::new).map(Arc::new),
        }
    }

    /// Creates an Account with `email`, trimmed, and `password`, for `caller`, and signs it in.
    pub(crate) async fn create(
        &self,
        email: &str,
        password: &str,
        caller: Caller,
    ) -> Result<SignedIn, Error> {
        let email = email.trim();
        if !is_email(email) {
            return Err(Error::EmailInvalid);
        }
        check_password_length(password)?;
        self.creations.admit(caller)?;
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
    /// `password` is its password, for `caller`, in place of `replacing`, the browser's session,
    /// if any.
    pub(crate) async fn sign_in(
        &self,
        email: &str,
        password: &str,
        replacing: Option<&AccountSession>,
        caller: Caller,
    ) -> Result<SignedIn, Error> {
        let email = email.trim();
        let attempt = self.sign_ins.admit(caller, email)?;
        let found = sqlx::query!(
            "SELECT id, email, password_hash FROM accounts WHERE lower(email) = lower($1)",
            email,
        )
        .fetch_optional(&self.db)
        .await?;
        // An Account a Sign-in Link took over has no password (ADR 0100).
        let Some((account, password_hash)) = found.and_then(|found| {
            let account = Account {
                id: AccountId(found.id),
                email: found.email,
            };
            Some((account, found.password_hash?))
        }) else {
            passwords::verify_unknown(password.to_owned()).await;
            return Err(Error::WrongEmailOrPassword);
        };
        if !passwords::verify(password.to_owned(), password_hash).await? {
            return Err(Error::WrongEmailOrPassword);
        }
        self.sign_ins.succeeded(&attempt);
        let mut tx = self.db.begin().await?;
        if let Some(session) = replacing {
            sessions::end(&mut *tx, session).await?;
        }
        let token = sessions::start(&mut tx, account.id).await?;
        tx.commit().await?;
        Ok(SignedIn { account, token })
    }

    /// Sets the password of `session`'s Account to `password`, replacing any it has, and ends the
    /// Account's other sessions. Needs Recent Authentication, checked before the password.
    pub(crate) async fn set_password(
        &self,
        session: &AccountSession,
        password: &str,
    ) -> Result<(), Error> {
        session.require_recent_authentication()?;
        check_password_length(password)?;
        let password_hash = passwords::hash(password.to_owned()).await?;
        let mut tx = self.db.begin().await?;
        // Checked again: the session may have ended, or aged, while the password hashed.
        if !sessions::lock_while_recent(&mut tx, session).await? {
            return Err(RecentAuthenticationRequired.into());
        }
        sqlx::query!(
            "UPDATE accounts SET password_hash = $1 WHERE id = $2",
            password_hash,
            session.account.id.0,
        )
        .execute(&mut *tx)
        .await?;
        sessions::end_others(&mut tx, session).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Deletes `session`'s Account. Every table holding its data references `accounts(id) ON
    /// DELETE CASCADE`, so this one `DELETE` removes it all and Unlinks its Users, while pooled
    /// counts, which name no Account, stay (ADR 0093, ADR 0099). Needs Recent Authentication.
    pub(crate) async fn delete(&self, session: &AccountSession) -> Result<(), Error> {
        let mut tx = self.db.begin().await?;
        // Under the row lock, so a Sign-in Link's takeover can't end the session meanwhile.
        if !sessions::lock_while_recent(&mut tx, session).await? {
            return Err(RecentAuthenticationRequired.into());
        }
        sqlx::query!("DELETE FROM accounts WHERE id = $1", session.account.id.0)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Whether `account` has a password: one a Sign-in Link took over has none (ADR 0100).
    pub(crate) async fn has_password(&self, account: AccountId) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar!(
            r#"SELECT password_hash IS NOT NULL AS "has_password!" FROM accounts WHERE id = $1"#,
            account.0,
        )
        .fetch_one(&self.db)
        .await
    }

    /// Whether the Hub emails Sign-in Links, which it does with mail set up.
    pub(crate) fn sends_sign_in_links(&self) -> bool {
        self.outbox.is_some()
    }

    /// Emails a Sign-in Link, off the request path, to the Account whose email is `email`, trimmed
    /// and compared without regard to case, if there is one, as asked by `caller`. The link
    /// continues at `continue_to` if it is a Hub path, or else at `/`.
    pub(crate) fn send_sign_in_link(
        &self,
        email: &str,
        continue_to: &str,
        caller: Caller,
    ) -> Result<(), Error> {
        let outbox = self.outbox.as_ref().ok_or(Error::SignInLinksOff)?;
        let email = email.trim();
        if !is_email(email) {
            return Err(Error::EmailInvalid);
        }
        outbox.send(email, continue_to, caller)
    }

    /// The waiting Sign-in Link `token` names, without using it up.
    pub(crate) async fn sign_in_link(&self, token: &str) -> Result<SignInLink, Error> {
        sign_in_links::find(&self.db, token).await
    }

    /// Signs in with the Sign-in Link `token` names, using it up, in place of `replacing`, the
    /// browser's session, if any.
    pub(crate) async fn sign_in_with_link(
        &self,
        token: &str,
        replacing: Option<&AccountSession>,
    ) -> Result<SignedInByLink, Error> {
        sign_in_links::use_up(&self.db, token, replacing).await
    }

    /// The session `token` names, unless it has ended or expired.
    pub(crate) async fn session(&self, token: &str) -> Result<Option<AccountSession>, sqlx::Error> {
        sessions::find(&self.db, token).await
    }

    /// The email of the Account `id`, unless it was deleted.
    pub(crate) async fn email(&self, id: AccountId) -> Result<Option<String>, sqlx::Error> {
        sqlx::query_scalar!("SELECT email FROM accounts WHERE id = $1", id.0)
            .fetch_optional(&self.db)
            .await
    }

    /// The Account whose email is `email`, compared without regard to case, if there is one.
    pub(crate) async fn with_email(&self, email: &str) -> Result<Option<AccountId>, sqlx::Error> {
        let found = sqlx::query!(
            "SELECT id, email FROM accounts WHERE lower(email) = lower($1)",
            email,
        )
        .fetch_optional(&self.db)
        .await?;
        Ok(found.map(|found| AccountId(found.id)))
    }

    /// Ends `session`: its token stops working at once.
    pub(crate) async fn sign_out(&self, session: &AccountSession) -> Result<(), sqlx::Error> {
        sessions::end(&self.db, session).await
    }
}

fn check_password_length(password: &str) -> Result<(), Error> {
    if PASSWORD_LENGTH.contains(&password.chars().count()) {
        Ok(())
    } else {
        Err(Error::PasswordLength)
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

/// A new random opaque token, 32 bytes as hex, for a session or a Sign-in Link.
fn new_token() -> Result<String, getrandom::Error> {
    let mut bytes = [0; 32];
    getrandom::fill(&mut bytes)?;
    Ok(bytes.iter().fold(String::new(), |mut hex, byte| {
        let _infallible = write!(hex, "{byte:02x}");
        hex
    }))
}

/// What the database keeps of a token (ADR 0098).
fn token_hash(token: &str) -> [u8; 32] {
    Sha256::digest(token).into()
}

#[cfg(test)]
#[path = "accounts/accounts_tests.rs"]
mod tests;
