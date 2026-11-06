//! Users, Devices and their sessions: Kernel state (ADR 0014).
//!
//! [`Users`] is the only way in. It checks names before anything is written, has the enabled
//! Authenticator turn passwords into the credential string it stores, and keeps only the SHA-256
//! hash of each Device's session token, so the database never holds a password or a token.

use std::{fmt, ops::RangeInclusive, sync::Arc};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use nama_plugin_api::{
    PluginError, PluginId,
    auth::{Authenticator, PasswordError},
};
use sha2::{Digest, Sha256};
use sqlx::SqliteConnection;
use tokio::{sync::broadcast, task::JoinHandle};

use crate::{
    db::{Database, now},
    plugins::Registry,
};

/// How many characters a User's or Device's name may have, once trimmed.
const NAME_LENGTH: RangeInclusive<usize> = 1..=64;

/// What every session token starts with, so a leaked one is easy to recognise.
const TOKEN_PREFIX: &str = "nama_";

/// How many random bytes a session token carries.
const TOKEN_BYTES: usize = 32;

/// Crockford's base32 alphabet, which leaves out I, L, O and U so a code read off a log can't be
/// mistyped as another.
const SETUP_CODE_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// How many characters a Setup Code has: 60 random bits.
const SETUP_CODE_LENGTH: usize = 12;

/// How long, in seconds, a Device may go unused before it expires and its session stops working:
/// 90 days.
pub(crate) const DEVICE_LIFETIME: i64 = 90 * 24 * 60 * 60;

/// How often, in seconds, a Device in use has its last-seen time written: an hour. Writing on every
/// request would make every request wait for the one database writer.
const LAST_SEEN_PRECISION: i64 = 60 * 60;

/// How many sign-outs may happen before a waiter in [`Users::signed_out`] has seen them. One that
/// falls further behind checks its Device in the database instead.
const SIGN_OUTS_BUFFERED: usize = 64;

/// The Server's Users, Devices and sessions, kept in its database.
///
/// Cloning a `Users` gives another handle to the same ones.
#[derive(Debug, Clone)]
pub struct Users {
    db: Arc<Database>,
    plugins: Registry,
    /// The ids of Devices as they are signed out.
    sign_outs: broadcast::Sender<i64>,
}

/// The one-time code a fresh Server prints to its log, which whoever creates the first Admin must
/// enter. It is shown as `XXXX-XXXX-XXXX` and matched ignoring case, dashes and spaces.
#[derive(Debug, Clone)]
pub struct SetupCode(String);

/// What a User is allowed to do on the Server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
pub(crate) enum Role {
    Admin,
    Member,
}

/// A User as clients see it.
#[derive(Debug, Clone)]
pub(crate) struct User {
    pub(crate) id: i64,
    pub(crate) name: String,
    pub(crate) role: Role,
}

/// A Device as clients see it.
#[derive(Debug, Clone)]
pub(crate) struct Device {
    pub(crate) id: i64,
    pub(crate) name: String,
}

/// Who sent a request: the User and Device its session belongs to.
#[derive(Debug, Clone)]
pub(crate) struct Caller {
    pub(crate) user: User,
    pub(crate) device: Device,
}

/// The session a request carried.
#[derive(Debug)]
pub(crate) struct Session {
    /// Who it belongs to.
    pub(crate) caller: Caller,
    /// Whether this use pushed the Device's expiry back, so a cookie's should be pushed back too.
    pub(crate) refreshed: bool,
}

/// A Device just signed in, and the session token it was given. Not `Debug`, so the token can't
/// end up in a log.
pub(crate) struct SignedIn {
    pub(crate) caller: Caller,
    pub(crate) token: String,
}

/// A credential string the Kernel keeps for a User, and the Plugin whose Authenticator made it,
/// which is the only one that can check it.
struct Credential {
    plugin: PluginId,
    value: String,
}

/// Why a change to the Users was refused or failed. No variant holds a password or a token.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    /// The request itself is wrong, such as a name that is too long or a password the
    /// Authenticator rejects, for this reason.
    #[error("{0}")]
    Invalid(String),
    /// Another User already has this name, ignoring case.
    #[error("another User already has that name")]
    NameTaken,
    /// No User has this name and password. Which of the two is wrong isn't said, so sign-in
    /// doesn't reveal which names exist.
    #[error("the name or password is wrong")]
    WrongNameOrPassword,
    /// The first Admin can only be created while the Server has no Users.
    #[error("the Server already has Users")]
    AlreadySetUp,
    /// No enabled Plugin provides an Authenticator.
    #[error("no Authenticator is enabled")]
    NoAuthenticator,
    /// The Authenticator failed, or panicked, making a credential.
    #[error("the Authenticator failed: {0}")]
    Authenticator(String),
    /// The operating system's random number generator failed.
    #[error("could not generate a session token: {0}")]
    Random(#[from] getrandom::Error),
    /// The database failed.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl Users {
    /// The Users kept in `db`, whose passwords the Authenticator enabled in `plugins` checks.
    #[must_use]
    pub fn new(db: Arc<Database>, plugins: Registry) -> Self {
        Self {
            db,
            plugins,
            sign_outs: broadcast::channel(SIGN_OUTS_BUFFERED).0,
        }
    }

    /// Whether the Server has any User yet. Until it has, the first Admin can be created with a
    /// [`SetupCode`].
    ///
    /// # Errors
    ///
    /// If the database fails.
    pub async fn any(&self) -> Result<bool, sqlx::Error> {
        any_user(&mut *self.db.reader().acquire().await?).await
    }

    /// Creates the first Admin, named `name` with `password`, and signs the Device `device_name`
    /// in as them. Checking the Setup Code is the caller's job.
    pub(crate) async fn create_first_admin(
        &self,
        name: &str,
        password: &str,
        device_name: &str,
    ) -> Result<SignedIn, Error> {
        let name = checked_name("a User's name", name)?;
        let device_name = checked_name("a Device's name", device_name)?;
        // Before hashing, which is slow, though only the check in the write below is race-free.
        if self.any().await? {
            return Err(Error::AlreadySetUp);
        }
        let credential = self.hash_password(password).await?;
        let token = new_token()?;
        let token_hash = hash_token(&token);
        let caller = self
            .db
            .write(move |tx| {
                Box::pin(async move {
                    insert_first_admin(tx, &name, &credential, device_name, &token_hash).await
                })
            })
            .await?;
        Ok(SignedIn { caller, token })
    }

    /// Signs the Device `device_name` in as the User named `name`, ignoring case, if `password` is
    /// theirs.
    pub(crate) async fn sign_in(
        &self,
        name: &str,
        password: &str,
        device_name: &str,
    ) -> Result<SignedIn, Error> {
        let device_name = checked_name("a Device's name", device_name)?;
        let password: Arc<str> = Arc::from(password);
        let found = self.user_named(name).await?;
        let verified = if let Some((_, credentials)) = &found {
            self.verify_password(&password, credentials).await?
        } else {
            self.verify_unknown_user(&password).await?;
            false
        };
        let user = found
            .filter(|_| verified)
            .map(|(user, _)| user)
            .ok_or(Error::WrongNameOrPassword)?;
        let token = new_token()?;
        let token_hash = hash_token(&token);
        let device = self
            .db
            .write(move |tx| {
                Box::pin(async move { insert_device(tx, user.id, device_name, &token_hash).await })
            })
            .await?;
        Ok(SignedIn {
            caller: Caller { user, device },
            token,
        })
    }

    /// Signs the Device `device` out by deleting it, so its session stops working and
    /// [`Users::signed_out`] resolves for it.
    pub(crate) async fn sign_out(&self, device: i64) -> Result<(), sqlx::Error> {
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!("DELETE FROM devices WHERE id = ?", device)
                        .execute(&mut **tx)
                        .await
                })
            })
            .await?;
        // Fails only when nobody is waiting, which is fine.
        let _: Result<usize, _> = self.sign_outs.send(device);
        Ok(())
    }

    /// Resolves once the Device `device` is signed out, so whatever it opened can end with it. It
    /// resolves at once if `device` was already signed out before this call, such as by a sign-out
    /// that came after its session was checked, or if whether it was can't be found out.
    pub(crate) fn signed_out(&self, device: i64) -> impl Future<Output = ()> + Send + 'static {
        // Listening before looking `device` up means no sign-out falls between the two.
        let sign_outs = self.sign_outs.subscribe();
        self.clone().wait_for_sign_out(sign_outs, device)
    }

    /// Resolves once `sign_outs` hears that `device` was signed out, or the database says it has
    /// been. The database is asked first, and again whenever `sign_outs` falls so far behind that
    /// `device` may have been among the sign-outs it skipped.
    async fn wait_for_sign_out(self, mut sign_outs: broadcast::Receiver<i64>, device: i64) {
        let mut look = true;
        loop {
            if look && !self.still_signed_in(device).await {
                return;
            }
            look = match sign_outs.recv().await {
                Ok(signed_out) if signed_out == device => return,
                Ok(_) => false,
                Err(broadcast::error::RecvError::Lagged(_)) => true,
                Err(broadcast::error::RecvError::Closed) => return,
            };
        }
    }

    /// Whether the Device `device` still exists. If that can't be found out, it is taken not to,
    /// so what it opened ends rather than outlive it.
    async fn still_signed_in(&self, device: i64) -> bool {
        let found = sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM devices WHERE id = ?) AS "exists!: bool""#,
            device
        )
        .fetch_one(self.db.reader())
        .await;
        found.unwrap_or_else(|err| {
            tracing::error!("could not look up a Device: {err}");
            false
        })
    }

    /// The session `token` is, if it is one whose Device hasn't expired, as of now.
    pub(crate) async fn session(&self, token: &str) -> Result<Option<Session>, sqlx::Error> {
        self.session_at(token, now()).await
    }

    /// The session `token` is, if it is one whose Device hasn't expired by `now`, in seconds since
    /// the Unix epoch. Counts as a use of the Device, which pushes its expiry back, though at most
    /// once every [`LAST_SEEN_PRECISION`] seconds.
    async fn session_at(&self, token: &str, now: i64) -> Result<Option<Session>, sqlx::Error> {
        let token_hash = hash_token(token);
        let token_hash = token_hash.as_slice();
        let expired_before = now - DEVICE_LIFETIME;
        let row = sqlx::query!(
            r#"SELECT d.id AS "device_id!", d.name AS device_name, d.last_seen_at,
                      u.id AS "user_id!", u.name AS user_name, u.role AS "role: Role"
               FROM devices d JOIN users u ON u.id = d.user_id
               WHERE d.token_hash = ? AND d.last_seen_at > ?"#,
            token_hash,
            expired_before,
        )
        .fetch_optional(self.db.reader())
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        // A session that checks out still works if its last-seen time can't be written. It is
        // only written again on a later use.
        let refreshed = now - row.last_seen_at >= LAST_SEEN_PRECISION
            && self
                .saw_device(row.device_id, now)
                .await
                .inspect_err(|err| tracing::error!("could not record a Device's use: {err}"))
                .is_ok();
        let caller = Caller {
            user: User {
                id: row.user_id,
                name: row.user_name,
                role: row.role,
            },
            device: Device {
                id: row.device_id,
                name: row.device_name,
            },
        };
        Ok(Some(Session { caller, refreshed }))
    }

    /// Records that the Device `device` was last seen at `now`, unless another request already
    /// did within [`LAST_SEEN_PRECISION`] seconds of it.
    async fn saw_device(&self, device: i64, now: i64) -> Result<(), sqlx::Error> {
        let stale_since = now - LAST_SEEN_PRECISION;
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!(
                        "UPDATE devices SET last_seen_at = ? WHERE id = ? AND last_seen_at <= ?",
                        now,
                        device,
                        stale_since,
                    )
                    .execute(&mut **tx)
                    .await
                })
            })
            .await?;
        Ok(())
    }

    /// Has the first enabled Authenticator turn `password` into a credential string.
    async fn hash_password(&self, password: &str) -> Result<Credential, Error> {
        let password: Arc<str> = Arc::from(password);
        let (plugin, hashed) = self
            .plugins
            .call_provider_with_id(|authenticator: Arc<dyn Authenticator>| {
                let password = Arc::clone(&password);
                // Hashing is meant to be slow, so it runs where blocking is fine.
                tokio::task::spawn_blocking(move || authenticator.hash_password(&password))
            })
            .await
            .ok_or(Error::NoAuthenticator)?;
        match hashed {
            Ok(Ok(value)) => Ok(Credential { plugin, value }),
            Ok(Err(PasswordError::Rejected(reason))) => Err(Error::Invalid(reason)),
            Ok(Err(PasswordError::Failed(err))) => Err(Error::Authenticator(err.to_string())),
            Err(panicked) => Err(Error::Authenticator(panicked.to_string())),
        }
    }

    /// The User whose name is `name` ignoring case, with those of their credentials an enabled
    /// Plugin made.
    async fn user_named(&self, name: &str) -> Result<Option<(User, Vec<Credential>)>, Error> {
        let name_key = name_key(name.trim());
        let rows = sqlx::query!(
            r#"SELECT u.id AS "id!", u.name, u.role AS "role: Role", c.plugin, c.credential
               FROM users u JOIN user_credentials c ON c.user_id = u.id
               WHERE u.name_key = ?"#,
            name_key
        )
        .fetch_all(self.db.reader())
        .await?;
        let Some(first) = rows.first() else {
            return Ok(None);
        };
        let user = User {
            id: first.id,
            name: first.name.clone(),
            role: first.role,
        };
        let enabled = self.plugins.enabled();
        let credentials = rows
            .into_iter()
            .filter_map(|row| {
                let plugin = enabled.iter().find(|id| id.as_str() == row.plugin)?;
                Some(Credential {
                    plugin: plugin.clone(),
                    value: row.credential,
                })
            })
            .collect();
        Ok(Some((user, credentials)))
    }

    /// Whether `password` is the one one of `credentials` was made from, as checked by the
    /// Authenticator that made it. If none of their Plugins provides one any more, the answer is
    /// no, after as long as checking would have taken.
    async fn verify_password(
        &self,
        password: &Arc<str>,
        credentials: &[Credential],
    ) -> Result<bool, Error> {
        for credential in credentials {
            let value: Arc<str> = Arc::from(credential.value.as_str());
            let verified = self
                .plugins
                .call_provider_of(&credential.plugin, |authenticator| {
                    verify_where_blocking_is_fine(authenticator, password, &value)
                })
                .await;
            match verified {
                None => {}
                Some(Ok(Ok(verified))) => return Ok(verified),
                Some(Ok(Err(err))) => return Err(Error::Authenticator(err.to_string())),
                Some(Err(panicked)) => return Err(Error::Authenticator(panicked.to_string())),
            }
        }
        self.verify_unknown_user(password).await?;
        Ok(false)
    }

    /// Has the first enabled Authenticator spend as long on `password` as checking a real User's
    /// would, for a name no User has.
    async fn verify_unknown_user(&self, password: &Arc<str>) -> Result<(), Error> {
        self.plugins
            .call_provider(|authenticator: Arc<dyn Authenticator>| {
                let password = Arc::clone(password);
                tokio::task::spawn_blocking(move || authenticator.verify_unknown_user(&password))
            })
            .await
            .ok_or(Error::NoAuthenticator)?
            .map_err(|panicked| Error::Authenticator(panicked.to_string()))
    }
}

impl SetupCode {
    /// A new random Setup Code.
    ///
    /// # Errors
    ///
    /// If the operating system's random number generator fails.
    pub fn generate() -> Result<Self, getrandom::Error> {
        let mut bytes = [0; SETUP_CODE_LENGTH];
        getrandom::fill(&mut bytes)?;
        // 256 is a multiple of 32, so every character is equally likely.
        Ok(Self(
            bytes
                .iter()
                .filter_map(|byte| SETUP_CODE_ALPHABET.get(usize::from(byte % 32)))
                .map(|&c| char::from(c))
                .collect(),
        ))
    }

    /// Whether `entered` is this code, ignoring case, dashes and spaces.
    pub(crate) fn matches(&self, entered: &str) -> bool {
        let entered: String = entered
            .chars()
            .filter(|c| *c != '-' && !c.is_whitespace())
            .map(|c| c.to_ascii_uppercase())
            .collect();
        // Comparing hashes, so how long a wrong guess takes says nothing about the code.
        Sha256::digest(entered) == Sha256::digest(&self.0)
    }
}

impl fmt::Display for SetupCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, c) in self.0.chars().enumerate() {
            if i > 0 && i % 4 == 0 {
                f.write_str("-")?;
            }
            write!(f, "{c}")?;
        }
        Ok(())
    }
}

/// Has `authenticator` check `password` against `credential`, which is meant to be slow, where
/// blocking is fine.
fn verify_where_blocking_is_fine(
    authenticator: Arc<dyn Authenticator>,
    password: &Arc<str>,
    credential: &Arc<str>,
) -> JoinHandle<Result<bool, PluginError>> {
    let password = Arc::clone(password);
    let credential = Arc::clone(credential);
    tokio::task::spawn_blocking(move || authenticator.verify_password(&password, &credential))
}

/// `name` trimmed, if it then has an allowed length. `what` names it in the error.
fn checked_name(what: &str, name: &str) -> Result<String, Error> {
    let name = name.trim();
    if NAME_LENGTH.contains(&name.chars().count()) {
        Ok(name.to_owned())
    } else {
        Err(Error::Invalid(format!(
            "{what} needs {} to {} characters",
            NAME_LENGTH.start(),
            NAME_LENGTH.end()
        )))
    }
}

/// A new session token: [`TOKEN_BYTES`] random bytes, base64url-encoded after [`TOKEN_PREFIX`].
fn new_token() -> Result<String, getrandom::Error> {
    let mut bytes = [0; TOKEN_BYTES];
    getrandom::fill(&mut bytes)?;
    Ok(format!("{TOKEN_PREFIX}{}", URL_SAFE_NO_PAD.encode(bytes)))
}

/// What the database keeps of a session token.
fn hash_token(token: &str) -> [u8; 32] {
    Sha256::digest(token).into()
}

/// Whether the database holds any User.
async fn any_user(conn: &mut SqliteConnection) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar!(r#"SELECT EXISTS (SELECT 1 FROM users) AS "any!: bool""#)
        .fetch_one(conn)
        .await
}

/// Stores the first Admin, named `name`, with `credential`, and their Device named `device_name`
/// with the hash of its session token, unless the Server already has a User. The names have been
/// checked.
async fn insert_first_admin(
    conn: &mut SqliteConnection,
    name: &str,
    credential: &Credential,
    device_name: String,
    token_hash: &[u8; 32],
) -> Result<Caller, Error> {
    if any_user(conn).await? {
        return Err(Error::AlreadySetUp);
    }
    let user = insert_user(conn, name, Role::Admin).await?;
    sqlx::query!(
        "INSERT INTO user_credentials (user_id, plugin, credential) VALUES (?, ?, ?)",
        user.id,
        credential.plugin.as_str(),
        credential.value,
    )
    .execute(&mut *conn)
    .await?;
    let device = insert_device(conn, user.id, device_name, token_hash).await?;
    Ok(Caller { user, device })
}

/// What a User's trimmed `name` is stored and looked up under, so names are unique and found
/// ignoring case.
fn name_key(name: &str) -> String {
    name.to_lowercase()
}

/// Stores a User named `name`, which has been checked, unless another has it ignoring case.
async fn insert_user(conn: &mut SqliteConnection, name: &str, role: Role) -> Result<User, Error> {
    let name_key = name_key(name);
    let created_at = now();
    let id = sqlx::query_scalar!(
        r#"INSERT INTO users (name, name_key, role, created_at) VALUES (?, ?, ?, ?)
           RETURNING id AS "id!""#,
        name,
        name_key,
        role,
        created_at,
    )
    .fetch_one(conn)
    .await
    .map_err(|err| match err.as_database_error() {
        Some(db_err) if db_err.is_unique_violation() => Error::NameTaken,
        _ => Error::Database(err),
    })?;
    Ok(User {
        id,
        name: name.to_owned(),
        role,
    })
}

/// Stores a Device named `name`, which has been checked, for the User `user`, with the hash of its
/// session token.
async fn insert_device(
    conn: &mut SqliteConnection,
    user: i64,
    name: String,
    token_hash: &[u8; 32],
) -> Result<Device, sqlx::Error> {
    let now = now();
    let token_hash = token_hash.as_slice();
    let id = sqlx::query_scalar!(
        r#"INSERT INTO devices (user_id, name, token_hash, created_at, last_seen_at)
           VALUES (?, ?, ?, ?, ?)
           RETURNING id AS "id!""#,
        user,
        name,
        token_hash,
        now,
        now,
    )
    .fetch_one(conn)
    .await?;
    Ok(Device { id, name })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Stores a User named `name` in `db`, as creating any User does.
    async fn create_user(db: &Database, name: &str) -> Result<User, Error> {
        let name = name.to_owned();
        db.write(move |tx| Box::pin(async move { insert_user(tx, &name, Role::Member).await }))
            .await
    }

    /// The session token of the Device [`signed_in_device`] makes.
    const TOKEN: &str = "nama_test-token";

    /// 90 days, in seconds, counted independently of the code under test.
    const NINETY_DAYS: i64 = 90 * 24 * 60 * 60;

    /// [`Users`] on a fresh database in `data_dir` with one Device, whose session is [`TOKEN`],
    /// and when it was signed in. It was last seen then, or a second after.
    async fn signed_in_device(data_dir: &std::path::Path) -> (Users, i64) {
        let db = Arc::new(Database::open(data_dir).await.unwrap());
        let user = create_user(&db, "Ada").await.unwrap();
        let signed_in_at = now();
        db.write(move |tx| {
            Box::pin(async move {
                insert_device(tx, user.id, "Laptop".to_owned(), &hash_token(TOKEN)).await
            })
        })
        .await
        .unwrap();
        (Users::new(db, Registry::new()), signed_in_at)
    }

    #[tokio::test]
    async fn a_device_unused_for_90_days_no_longer_authenticates() {
        let data_dir = tempfile::tempdir().unwrap();
        let (users, signed_in_at) = signed_in_device(data_dir.path()).await;

        let session = users
            .session_at(TOKEN, signed_in_at + 1 + NINETY_DAYS)
            .await;

        assert!(session.unwrap().is_none());
    }

    #[tokio::test]
    async fn a_device_used_within_90_days_still_authenticates() {
        let data_dir = tempfile::tempdir().unwrap();
        let (users, signed_in_at) = signed_in_device(data_dir.path()).await;

        let session = users
            .session_at(TOKEN, signed_in_at + NINETY_DAYS - 1)
            .await;

        assert!(session.unwrap().is_some());
    }

    #[tokio::test]
    async fn using_a_device_within_the_hour_it_was_last_seen_leaves_last_seen_alone() {
        let data_dir = tempfile::tempdir().unwrap();
        let (users, signed_in_at) = signed_in_device(data_dir.path()).await;

        let used = users.session_at(TOKEN, signed_in_at + 1800).await.unwrap();
        // Still counted from the sign-in, so not written by the use above.
        let later = users
            .session_at(TOKEN, signed_in_at + 1 + NINETY_DAYS)
            .await;

        assert!(!used.unwrap().refreshed);
        assert!(later.unwrap().is_none(), "last seen was written");
    }

    #[tokio::test]
    async fn using_a_device_an_hour_after_it_was_last_seen_writes_last_seen() {
        let data_dir = tempfile::tempdir().unwrap();
        let (users, signed_in_at) = signed_in_device(data_dir.path()).await;
        let used_at = signed_in_at + 1 + 3600;

        let used = users.session_at(TOKEN, used_at).await.unwrap();
        // Counted from the use above, so it was written.
        let later = users.session_at(TOKEN, used_at + NINETY_DAYS - 1).await;

        assert!(used.unwrap().refreshed);
        assert!(later.unwrap().is_some(), "last seen wasn't written");
    }

    /// How long a test waits for a sign-out to be noticed before failing.
    const WAIT: std::time::Duration = std::time::Duration::from_secs(10);

    /// The id of the Device [`signed_in_device`] made, signed in at `signed_in_at`.
    async fn device_id(users: &Users, signed_in_at: i64) -> i64 {
        let session = users.session_at(TOKEN, signed_in_at).await.unwrap();
        session.unwrap().caller.device.id
    }

    #[tokio::test]
    async fn waiting_for_a_device_already_signed_out_ends_at_once() {
        let data_dir = tempfile::tempdir().unwrap();
        let (users, signed_in_at) = signed_in_device(data_dir.path()).await;
        let device = device_id(&users, signed_in_at).await;

        // As when the sign-out lands after the session was checked but before the wait began.
        users.sign_out(device).await.unwrap();
        let waited = tokio::time::timeout(WAIT, users.signed_out(device)).await;

        assert!(waited.is_ok(), "the wait didn't end");
    }

    #[tokio::test]
    async fn a_burst_of_other_sign_outs_doesnt_end_the_wait_for_a_device() {
        let data_dir = tempfile::tempdir().unwrap();
        let (users, signed_in_at) = signed_in_device(data_dir.path()).await;
        let device = device_id(&users, signed_in_at).await;
        let mut waiting = std::pin::pin!(users.signed_out(device));

        // More than the waiter can buffer, so it falls behind.
        for other in (device + 1..).take(SIGN_OUTS_BUFFERED * 2) {
            users.sign_out(other).await.unwrap();
        }
        let during_burst =
            tokio::time::timeout(std::time::Duration::from_millis(200), &mut waiting).await;
        // Checked before waiting again, since a wait that ended can't be waited on.
        assert!(during_burst.is_err(), "other sign-outs ended the wait");
        users.sign_out(device).await.unwrap();
        let after_sign_out = tokio::time::timeout(WAIT, waiting).await;

        assert!(
            after_sign_out.is_ok(),
            "signing the Device out didn't end it"
        );
    }

    #[tokio::test]
    async fn a_name_another_user_has_in_a_different_case_is_taken() {
        let data_dir = tempfile::tempdir().unwrap();
        let db = Database::open(data_dir.path()).await.unwrap();
        create_user(&db, "Émile").await.unwrap();

        let duplicate = create_user(&db, "éMILE").await;

        assert!(matches!(duplicate, Err(Error::NameTaken)), "{duplicate:?}");
    }

    #[test]
    fn a_setup_code_matches_ignoring_case_dashes_and_spaces() {
        let code = SetupCode("ABCD0123WXYZ".to_owned());

        assert!(code.matches(" abcd-0123 wxyz\n"));
        assert!(!code.matches("ABCD-0123-WXY"));
    }

    #[test]
    fn a_setup_code_is_shown_in_groups_of_four() {
        assert_eq!(
            SetupCode("ABCD0123WXYZ".to_owned()).to_string(),
            "ABCD-0123-WXYZ"
        );
    }
}
