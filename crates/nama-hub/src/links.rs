//! Links (GLOSSARY.md): registering a Server through an OAuth device flow (ADR 0011), the Linked
//! Server's opaque credential, the Server Addresses it reports, whether its public port can be
//! reached and the records under its own name (`server_addresses.rs`), whether the Hub names
//! Servers, its own name and every write and removal of its names (`names.rs`), Unlinking, and the
//! tokens signed for a Server once it is Linked and for the Relay it dials out to.
//! Linking its Users to Accounts is in `users.rs`, Users created from an Account in `invites.rs`,
//! and signing in to a Server with an Account in `server_sign_ins.rs`.

mod invites;
mod names;
mod server_addresses;
mod server_sign_ins;
mod users;

use std::{
    fmt::{self, Write as _},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use sha2::{Digest as _, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use self::names::Names;
use crate::{
    accounts::{self, AccountId, AccountSession, Accounts, RecentAuthenticationRequired},
    dns::{DnsError, ServerNames},
    probe::Probe,
    signing::Keys,
};

pub(crate) use self::{
    invites::{InviteId, ListedInvite},
    names::MAX_CHALLENGE_VALUES,
    server_addresses::{ADDRESS_LENGTH, MAX_LAN_ADDRESSES, ReportedAddresses},
    server_sign_ins::SignInId,
    users::{LinkId, ListedUser},
};

/// How long a Server's or a User's codes work after it starts.
pub(crate) const CODE_LIFETIME_MINS: i32 = 10;

/// How long a Server should wait between polls (RFC 8628 suggests 5 seconds).
pub(crate) const POLL_INTERVAL_SECS: i32 = 5;

/// How many characters of a Server's or a User's name are kept.
const NAME_LENGTH: usize = 100;

/// The letters a code is made of: consonants only, so a code never spells a word, and none that
/// read alike (RFC 8628, section 6.1).
const CODE_LETTERS: &[u8] = b"BCDFGHJKLMNPQRSTVWXZ";

/// How many letters a code has: 20^8 codes, against the few waiting at once.
const CODE_LENGTH: usize = 8;

/// The audience of the token a Server shows a Relay, which the Relay checks (ADR 0057).
const RELAY_AUDIENCE: &str = "relay";

/// How long the token a newly Linked Server gets, one for a sign-in, or one for a Relay, stays
/// valid.
const TOKEN_LIFETIME_SECS: u64 = 5 * 60;

/// A Linked Server's id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct LinkedServerId(Uuid);

#[cfg(test)]
impl From<Uuid> for LinkedServerId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl std::str::FromStr for LinkedServerId {
    type Err = uuid::Error;

    fn from_str(id: &str) -> Result<Self, Self::Err> {
        id.parse().map(Self)
    }
}

impl fmt::Display for LinkedServerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// A Server calling with its credential: the caller kind `LinkedServerService` serves.
#[derive(Clone, Debug)]
pub(crate) struct LinkedServer {
    pub(crate) id: LinkedServerId,
}

/// One of an Account's Linked Servers, as its list shows it.
#[derive(Debug)]
pub(crate) struct ListedServer {
    pub(crate) id: LinkedServerId,
    pub(crate) name: String,
    /// In seconds since the Unix epoch.
    pub(crate) linked_at: i64,
    /// The Server URL its Admin set, or empty for none.
    pub(crate) url: String,
    /// The Server Addresses a client tries, best first: the Server URL, then the public address
    /// the Hub reached, then its Relay, then the LAN addresses, plain `http://<lan-ip>` (ADR 0057),
    /// each origin once.
    pub(crate) addresses: Vec<String>,
}

/// A Server's codes, just made.
#[derive(Debug)]
pub(crate) struct Started {
    pub(crate) device_code: String,
    /// As shown, such as `BDFH-JKLM`.
    pub(crate) user_code: String,
}

/// What a poll found.
#[derive(Debug)]
pub(crate) enum Polled {
    /// Nobody entered the code yet.
    Waiting,
    /// Linked: the credential for every later call, and the token naming the Account.
    Linked { credential: String, token: String },
}

/// Why a Link couldn't be made or polled.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error(transparent)]
    RecentAuthenticationRequired(#[from] RecentAuthenticationRequired),
    #[error("prove your email with a Sign-in Link before Linking")]
    EmailNotProven,
    #[error("the Hub has no mail set up, so it can't prove emails, and Linking needs a proven one")]
    SignInLinksOff,
    #[error("nothing is waiting on this code")]
    CodeNotFound,
    #[error("the User is Linked already; Unlink them first")]
    UserLinked,
    #[error("your Account has a User on this Server already; Unlink them first")]
    AccountHasUserOnServer,
    #[error("the code expired; start again")]
    Expired,
    #[error("no User created from your Account is waiting on this")]
    InviteNotFound,
    #[error("this sign-in expired or was used; start signing in again on the Server")]
    SignInNotFound,
    #[error("your Account has no User on this Server; ask its Admin to create one from it")]
    NoUserOnServer,
    #[error(
        "the return address must be an http or https URL, on one of the Server's Server Addresses \
         once it is Linked"
    )]
    UrlInvalid,
    #[error(
        "a Server Address must be an http or https URL of up to {ADDRESS_LENGTH} characters, \
         a Server has up to {MAX_LAN_ADDRESSES} LAN addresses, a port is at most 65535, and a \
         Relay is an <ip>:<port> a client can connect to"
    )]
    AddressInvalid,
    #[error("could not generate a code: {0}")]
    Random(#[from] getrandom::Error),
    #[error(
        "a Server publishes up to {MAX_CHALLENGE_VALUES} certificate challenge values, each a \
         DNS-01 challenge's"
    )]
    ChallengeInvalid,
    #[error("this Hub names no Server")]
    NamesOff,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("could not write the Server's names: {0}")]
    Names(#[from] DnsError),
}

/// The claims of the token a Server shows a Relay: its own name, which the Relay routes to it.
#[derive(Serialize)]
struct RelayClaims {
    sub: String,
    aud: &'static str,
    iat: u64,
    exp: u64,
}

/// The claims of the token a newly Linked Server gets.
#[derive(Serialize)]
struct LinkedClaims<'a> {
    sub: String,
    email: &'a str,
    iat: u64,
    exp: u64,
}

/// The Linked Servers and the Servers waiting to be.
#[derive(Clone)]
pub(crate) struct Links {
    db: PgPool,
    accounts: Accounts,
    keys: Arc<Keys>,
    names: Names,
    /// How the Hub tries a Server's public port.
    probe: Arc<dyn Probe>,
}

impl Links {
    pub(crate) fn new(
        db: PgPool,
        accounts: Accounts,
        keys: Arc<Keys>,
        names: Option<ServerNames>,
        probe: Arc<dyn Probe>,
    ) -> Self {
        Self {
            names: Names::new(db.clone(), names),
            db,
            accounts,
            keys,
            probe,
        }
    }

    /// Starts registering a Server named `name`, forgetting every Server whose codes expired, and
    /// every Linked Server whose codes expired before it collected its credential. Approving its
    /// code sends the person back to `return_url`, when it isn't empty (`links/AGENTS.md`).
    pub(crate) async fn start(&self, name: &str, return_url: &str) -> Result<Started, Error> {
        if !return_url.is_empty() && !server_addresses::is_web_url(return_url) {
            return Err(Error::UrlInvalid);
        }
        let name = kept_name(name);
        let device_code = random_token()?;
        let user_code = random_code()?;
        let mut tx = self.db.begin().await?;
        sqlx::query!("DELETE FROM server_link_codes WHERE expires_at < now()")
            .execute(&mut *tx)
            .await?;
        sqlx::query!(
            "DELETE FROM linked_servers WHERE credential_hash IS NULL AND id NOT IN
             (SELECT linked_server_id FROM server_link_codes WHERE linked_server_id IS NOT NULL)"
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "INSERT INTO server_link_codes
             (device_code_hash, user_code, server_name, return_url, expires_at)
             VALUES ($1, $2, $3, $4, now() + make_interval(mins => $5))",
            &hash(&device_code)[..],
            user_code,
            name,
            return_url,
            CODE_LIFETIME_MINS,
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(Started {
            user_code: shown_code(&user_code),
            device_code,
        })
    }

    /// Makes the Server waiting on `code` a Linked Server of `session`'s Account, if it has Recent
    /// Authentication and a proven email, and answers it with the `return_url` it started with.
    pub(crate) async fn approve(
        &self,
        session: &AccountSession,
        code: &str,
    ) -> Result<(ListedServer, String), Error> {
        self.may_link(session)?;
        let code = typed_code(code);
        let mut tx = self.db.begin().await?;
        let waiting = sqlx::query!(
            "SELECT id, server_name, return_url FROM server_link_codes
             WHERE user_code = $1 AND expires_at > now() AND linked_server_id IS NULL
             FOR UPDATE",
            code,
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(Error::CodeNotFound)?;
        // Floored, not cast: `crates/nama-hub/AGENTS.md`, "Data".
        let linked = sqlx::query!(
            r#"INSERT INTO linked_servers (account_id, name) VALUES ($1, $2)
             RETURNING id, floor(extract(epoch FROM created_at))::bigint AS "linked_at!""#,
            session.account.id.0,
            waiting.server_name,
        )
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query!(
            "UPDATE server_link_codes SET linked_server_id = $1 WHERE id = $2",
            linked.id,
            waiting.id,
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        let server = ListedServer {
            id: LinkedServerId(linked.id),
            name: waiting.server_name,
            linked_at: linked.linked_at,
            url: String::new(),
            addresses: Vec::new(),
        };
        Ok((server, waiting.return_url))
    }

    /// Whether the code of the Server polling with `device_code` was entered. The first poll after
    /// it was hands out the Server's credential, and forgets the codes.
    // ponytail: a Server that polls only after its codes expire never collects its credential,
    // and its row stays on its Account's list until the next Server starts Linking.
    pub(crate) async fn poll(&self, device_code: &str) -> Result<Polled, Error> {
        let mut tx = self.db.begin().await?;
        let device_code_hash = hash(device_code);
        let linked_server_id = sqlx::query_scalar!(
            "SELECT linked_server_id FROM server_link_codes
             WHERE device_code_hash = $1 AND expires_at > now()
             FOR UPDATE",
            &device_code_hash[..],
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(Error::Expired)?;
        let Some(linked_server_id) = linked_server_id else {
            return Ok(Polled::Waiting);
        };
        let credential = random_token()?;
        let account_id = sqlx::query_scalar!(
            "UPDATE linked_servers SET credential_hash = $1 WHERE id = $2 RETURNING account_id",
            &hash(&credential)[..],
            linked_server_id,
        )
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query!(
            "DELETE FROM server_link_codes WHERE device_code_hash = $1",
            &device_code_hash[..],
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        // Its Account can't go meanwhile without taking this row along, by its foreign key.
        let email = self
            .accounts
            .email(AccountId(account_id))
            .await?
            .unwrap_or_default();
        let iat = issued_at();
        let token = self.keys.sign(&LinkedClaims {
            sub: linked_server_id.to_string(),
            email: &email,
            iat,
            exp: iat + TOKEN_LIFETIME_SECS,
        });
        Ok(Polled::Linked { credential, token })
    }

    /// A token proving to a Relay that `server` holds its own name, which the Relay then routes
    /// to it (ADR 0057).
    pub(crate) fn relay_token(&self, server: &LinkedServer) -> Result<String, Error> {
        let iat = issued_at();
        Ok(self.keys.sign(&RelayClaims {
            sub: self.names.own_name(server.id)?,
            aud: RELAY_AUDIENCE,
            iat,
            exp: iat + TOKEN_LIFETIME_SECS,
        }))
    }

    /// Makes `values` the only TXT records of `server`'s DNS-01 challenge for a certificate for
    /// its names (ADR 0057), or removes them when it is empty (`names.rs`).
    pub(crate) async fn set_challenge(
        &self,
        server: &LinkedServer,
        values: Vec<String>,
    ) -> Result<(), Error> {
        self.names.publish_challenge(server.id, values).await
    }

    /// The Linked Server whose credential is `credential`, if it is still Linked.
    pub(crate) async fn server(
        &self,
        credential: &str,
    ) -> Result<Option<LinkedServer>, sqlx::Error> {
        let id = sqlx::query_scalar!(
            "SELECT id FROM linked_servers WHERE credential_hash = $1",
            &hash(credential)[..],
        )
        .fetch_optional(&self.db)
        .await?;
        Ok(id.map(|id| LinkedServer {
            id: LinkedServerId(id),
        }))
    }

    /// Unlinks `server`: its credential stops working at once, and its names are gone. While its
    /// names can't be removed, it stays Linked.
    pub(crate) async fn unlink(&self, server: &LinkedServer) -> Result<(), Error> {
        let removal = async {
            sqlx::query!("DELETE FROM linked_servers WHERE id = $1", server.id.0)
                .execute(&self.db)
                .await?;
            Ok(())
        };
        self.names.unlink(server.id, removal).await
    }

    /// Deletes `session`'s Account, as [`Accounts::delete`] does, then the names of the Linked
    /// Servers it owned, which deleting it Unlinked. Names that can't be removed are only logged:
    /// the Account is gone by then.
    pub(crate) async fn delete_account(
        &self,
        session: &AccountSession,
    ) -> Result<(), accounts::Error> {
        let owned = sqlx::query_scalar!(
            "SELECT id FROM linked_servers WHERE account_id = $1",
            session.account.id.0,
        )
        .fetch_all(&self.db)
        .await?;
        self.accounts.delete(session).await?;
        (self.names)
            .forget_deleted(owned.into_iter().map(LinkedServerId))
            .await;
        Ok(())
    }

    /// Whether `session` may Link: with a proven email (ADR 0100), checked first since signing in
    /// again can't fix it, then Recent Authentication. Without mail nothing can prove an email, so
    /// an unproven one is refused as [`Error::SignInLinksOff`] rather than offered a Sign-in Link.
    fn may_link(&self, session: &AccountSession) -> Result<(), Error> {
        if !session.email_proven {
            return Err(if self.accounts.sends_sign_in_links() {
                Error::EmailNotProven
            } else {
                Error::SignInLinksOff
            });
        }
        Ok(session.require_recent_authentication()?)
    }
}

/// Now, in seconds since the Unix epoch, as a token's `iat`.
fn issued_at() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// `name` as the Hub keeps it: trimmed, and cut to [`NAME_LENGTH`] characters.
fn kept_name(name: &str) -> String {
    name.trim().chars().take(NAME_LENGTH).collect()
}

/// A code as typed, in any case and with or without its hyphen, as it is stored.
fn typed_code(code: &str) -> String {
    code.chars()
        .filter(char::is_ascii_alphabetic)
        .map(|letter| letter.to_ascii_uppercase())
        .collect()
}

/// A code as stored, as it is shown, such as `BDFH-JKLM`.
fn shown_code(code: &str) -> String {
    let (first, second) = code.split_at(CODE_LENGTH / 2);
    format!("{first}-{second}")
}

/// 32 random bytes in hex, for a device code or a credential.
fn random_token() -> Result<String, getrandom::Error> {
    let mut bytes = [0; 32];
    getrandom::fill(&mut bytes)?;
    Ok(bytes.iter().fold(String::new(), |mut hex, byte| {
        let _infallible = write!(hex, "{byte:02x}");
        hex
    }))
}

/// [`CODE_LENGTH`] random letters of [`CODE_LETTERS`], each as likely as another.
fn random_code() -> Result<String, getrandom::Error> {
    // A byte from here up is drawn again, as taking it modulo would favour the first letters.
    const FAIR: usize = 256 / CODE_LETTERS.len() * CODE_LETTERS.len();
    let mut code = String::with_capacity(CODE_LENGTH);
    while code.len() < CODE_LENGTH {
        let mut byte = [0];
        getrandom::fill(&mut byte)?;
        let [byte] = byte.map(usize::from);
        if byte < FAIR {
            code.extend(
                CODE_LETTERS
                    .get(byte % CODE_LETTERS.len())
                    .map(|&letter| char::from(letter)),
            );
        }
    }
    Ok(code)
}

/// What the database keeps of a device code or credential.
fn hash(token: &str) -> [u8; 32] {
    Sha256::digest(token).into()
}

#[cfg(test)]
#[path = "links/links_tests.rs"]
mod tests;
