//! Links (GLOSSARY.md): registering a Server through an OAuth device flow (ADR 0011), the Linked
//! Server's opaque credential, the Server Addresses it reports and whether its public port can be
//! reached, Unlinking, and the token signed for a Server once it is Linked. Linking its Users to
//! Accounts is in `users.rs`.

mod users;

use std::{
    fmt::{self, Write as _},
    net::SocketAddr,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::http::Uri;
use serde::Serialize;
use sha2::{Digest as _, Sha256};
use sqlx::PgPool;
use tokio::net::TcpStream;
use uuid::Uuid;

use crate::{
    accounts::{AccountId, AccountSession, Accounts},
    signing::Keys,
};

pub(crate) use self::users::{LinkId, ListedUser};

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

/// The most characters a Server Address may have.
pub(crate) const ADDRESS_LENGTH: usize = 2000;

/// The most LAN addresses a Server may report.
pub(crate) const MAX_LAN_ADDRESSES: usize = 16;

/// How long the Hub waits for a Server's public port to take a connection.
const REACH_WITHIN: Duration = Duration::from_secs(5);

/// How long the token a newly Linked Server gets stays valid.
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
    /// the Hub reached, then the LAN addresses, plain `http://<lan-ip>` (ADR 0057).
    pub(crate) addresses: Vec<String>,
}

/// Where the Hub tried to reach a Server from outside, and whether it could.
pub(crate) struct PublicAddress {
    /// Such as `http://203.0.113.7:7980`.
    pub(crate) address: String,
    pub(crate) reachable: bool,
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
    #[error("sign in again to Link")]
    RecentAuthenticationRequired,
    #[error("prove your email with a Sign-in Link before Linking")]
    EmailNotProven,
    #[error("nothing is waiting on this code")]
    CodeNotFound,
    #[error("the User is Linked already; Unlink them first")]
    UserLinked,
    #[error("the code expired; start Linking again")]
    Expired,
    #[error(
        "a Server Address must be an http or https URL of up to {ADDRESS_LENGTH} characters, \
         a Server has up to {MAX_LAN_ADDRESSES} LAN addresses, and a port is at most 65535"
    )]
    AddressInvalid,
    #[error("could not generate a code: {0}")]
    Random(#[from] getrandom::Error),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
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
}

impl Links {
    pub(crate) fn new(db: PgPool, accounts: Accounts, keys: Arc<Keys>) -> Self {
        Self { db, accounts, keys }
    }

    /// Starts registering a Server named `name`, forgetting every Server whose codes expired, and
    /// every Linked Server whose codes expired before it collected its credential.
    pub(crate) async fn start(&self, name: &str) -> Result<Started, Error> {
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
            "INSERT INTO server_link_codes (device_code_hash, user_code, server_name, expires_at)
             VALUES ($1, $2, $3, now() + make_interval(mins => $4))",
            &hash(&device_code)[..],
            user_code,
            name,
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
    /// Authentication and a proven email.
    pub(crate) async fn approve(
        &self,
        session: &AccountSession,
        code: &str,
    ) -> Result<ListedServer, Error> {
        may_link(session)?;
        let code = typed_code(code);
        let mut tx = self.db.begin().await?;
        let waiting = sqlx::query!(
            "SELECT id, server_name FROM server_link_codes
             WHERE user_code = $1 AND expires_at > now() AND linked_server_id IS NULL
             FOR UPDATE",
            code,
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(Error::CodeNotFound)?;
        let linked = sqlx::query!(
            r#"INSERT INTO linked_servers (account_id, name) VALUES ($1, $2)
             RETURNING id, extract(epoch FROM created_at)::bigint AS "linked_at!""#,
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
        Ok(ListedServer {
            id: LinkedServerId(linked.id),
            name: waiting.server_name,
            linked_at: linked.linked_at,
            url: String::new(),
            addresses: Vec::new(),
        })
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
        let iat = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let token = self.keys.sign(&LinkedClaims {
            sub: linked_server_id.to_string(),
            email: &email,
            iat,
            exp: iat + TOKEN_LIFETIME_SECS,
        });
        Ok(Polled::Linked { credential, token })
    }

    /// The Linked Servers `account` owns or has a User on, oldest first.
    pub(crate) async fn list(&self, account: AccountId) -> Result<Vec<ListedServer>, sqlx::Error> {
        let servers = sqlx::query!(
            r#"SELECT id, name, server_url, public_address, lan_addresses,
             extract(epoch FROM created_at)::bigint AS "linked_at!"
             FROM linked_servers WHERE account_id = $1
             OR id IN (SELECT linked_server_id FROM user_links WHERE account_id = $1)
             ORDER BY created_at, id"#,
            account.0,
        )
        .fetch_all(&self.db)
        .await?;
        Ok(servers
            .into_iter()
            .map(|server| ListedServer {
                id: LinkedServerId(server.id),
                name: server.name,
                linked_at: server.linked_at,
                addresses: [server.server_url.clone(), server.public_address]
                    .into_iter()
                    .filter(|address| !address.is_empty())
                    .chain(server.lan_addresses)
                    .collect(),
                url: server.server_url,
            })
            .collect())
    }

    /// Replaces the Server Addresses `server` reported before with its Server URL `url`, empty
    /// for none, its `lan_addresses`, and `public`, its router's public IP address with the port
    /// that reaches it, if it has one, once the Hub has tried to connect there.
    pub(crate) async fn set_addresses(
        &self,
        server: &LinkedServer,
        url: &str,
        lan_addresses: &[String],
        public: Option<SocketAddr>,
    ) -> Result<Option<PublicAddress>, Error> {
        let addresses_valid = lan_addresses.len() <= MAX_LAN_ADDRESSES
            && (url.is_empty() || is_address(url))
            && lan_addresses.iter().all(|address| is_address(address));
        if !addresses_valid {
            return Err(Error::AddressInvalid);
        }
        let public = match public {
            Some(public) => Some(PublicAddress {
                address: format!("http://{public}"),
                reachable: reaches(public).await,
            }),
            None => None,
        };
        let reached = public
            .as_ref()
            .filter(|public| public.reachable)
            .map_or("", |public| public.address.as_str());
        sqlx::query!(
            "UPDATE linked_servers SET server_url = $1, lan_addresses = $2, public_address = $3
             WHERE id = $4",
            url,
            lan_addresses,
            reached,
            server.id.0,
        )
        .execute(&self.db)
        .await?;
        Ok(public)
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

    /// Unlinks `server`: its credential stops working at once.
    pub(crate) async fn unlink(&self, server: &LinkedServer) -> Result<(), sqlx::Error> {
        sqlx::query!("DELETE FROM linked_servers WHERE id = $1", server.id.0)
            .execute(&self.db)
            .await?;
        Ok(())
    }
}

/// Whether `session` may Link: with a proven email (ADR 0100), checked first since signing in again
/// can't fix it, then Recent Authentication.
fn may_link(session: &AccountSession) -> Result<(), Error> {
    if !session.email_proven {
        return Err(Error::EmailNotProven);
    }
    if !session.authenticated_recently {
        return Err(Error::RecentAuthenticationRequired);
    }
    Ok(())
}

/// Whether `address` may be a Server Address: an absolute `http` or `https` URL, so the Hub's pages
/// never link to a `javascript:` one a Server sent, of at most [`ADDRESS_LENGTH`] characters.
fn is_address(address: &str) -> bool {
    address.len() <= ADDRESS_LENGTH
        && address.parse::<Uri>().is_ok_and(|uri| {
            matches!(uri.scheme_str(), Some("http" | "https")) && uri.authority().is_some()
        })
}

/// Whether `public` takes a TCP connection within [`REACH_WITHIN`].
// shortcut: a connection shows only that something listens there; check that it is this Server,
// over TLS to its name, once Servers have names (#1830).
async fn reaches(public: SocketAddr) -> bool {
    let connected = tokio::time::timeout(REACH_WITHIN, TcpStream::connect(public)).await;
    connected.is_ok_and(|connected| connected.is_ok())
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
