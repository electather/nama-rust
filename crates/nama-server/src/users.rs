//! Users, Devices and their sessions: Kernel state (ADR 0014).
//!
//! [`Users`] is the only way in. It checks names before anything is written and has the enabled
//! Authenticator turn passwords into credentials. Nothing secret is stored: see `AGENTS.md`.

use std::{fmt, ops::RangeInclusive, sync::Arc};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use nama_plugin_api::{
    PluginError, PluginId,
    auth::{Authenticator, PasswordError},
};
use sha2::{Digest, Sha256};
use sqlx::{Sqlite, SqliteConnection, Transaction};
use tokio::{sync::broadcast, task::JoinHandle};

use crate::{
    db::{BoxFuture, Database, now},
    plugins::Registry,
};

/// How many characters a User's or Device's name may have, once trimmed.
pub(crate) const NAME_LENGTH: RangeInclusive<usize> = 1..=64;

/// What every session token starts with, so a leaked one is easy to recognise.
const TOKEN_PREFIX: &str = "nama_";

/// How many random bytes a session token carries.
const TOKEN_BYTES: usize = 32;

/// Crockford's base32 alphabet, which leaves out I, L, O and U so a code read off a log or a screen
/// can't be mistyped as another.
const CODE_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

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
    /// When they were created, in seconds since the Unix epoch.
    pub(crate) created_at: i64,
}

/// A Device as clients see it.
#[derive(Debug, Clone)]
pub(crate) struct Device {
    pub(crate) id: i64,
    pub(crate) name: String,
    /// When it signed in, in seconds since the Unix epoch.
    pub(crate) created_at: i64,
    /// When it was last used, in seconds since the Unix epoch, to within
    /// [`LAST_SEEN_PRECISION`].
    pub(crate) last_seen_at: i64,
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
#[expect(
    missing_debug_implementations,
    reason = "it holds a session token, which must never be logged"
)]
pub struct SignedIn {
    pub(crate) caller: Caller,
    pub(crate) token: String,
}

impl SignedIn {
    /// The session token the Device presents from now on, as a cookie or a bearer token.
    #[must_use]
    pub fn token(&self) -> &str {
        &self.token
    }
}

/// A credential string the Kernel keeps for a User, and the Plugin whose Authenticator made it,
/// which is the only one that can check it.
struct Credential {
    plugin: PluginId,
    value: String,
}

/// Why a change to the Users was refused or failed. No variant holds a password or a token.
#[derive(Debug, thiserror::Error)]
pub enum Error {
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
    /// The password given as a User's current one isn't.
    #[error("the current password is wrong")]
    WrongPassword,
    /// The Device asking for a change was signed out before it was made.
    #[error("this Device was signed out")]
    SignedOut,
    /// No User has this ID.
    #[error("no User has that ID")]
    NoSuchUser,
    /// The change would leave the Server with no Admin.
    #[error("the Server must keep at least one Admin")]
    LastAdmin,
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

    /// Creates a User named `name` with `password` and `role`, who can then sign in, and returns
    /// their ID.
    pub(crate) async fn create(
        &self,
        name: &str,
        password: &str,
        role: Role,
    ) -> Result<i64, Error> {
        let name = checked_name("a User's name", name)?;
        let credential = self.hash_password(password).await?;
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    let user = insert_user(tx, &name, role).await?;
                    insert_credential(tx, user.id, &credential).await?;
                    Ok(user.id)
                })
            })
            .await
    }

    /// Every User, oldest first.
    pub(crate) async fn list(&self) -> Result<Vec<User>, sqlx::Error> {
        sqlx::query_as!(
            User,
            r#"SELECT id AS "id!", name, role AS "role: Role", created_at FROM users ORDER BY id"#
        )
        .fetch_all(self.db.reader())
        .await
    }

    /// Gives the User `user` the Role `role`, unless that leaves the Server with no Admin. Their
    /// Devices stay signed in, and the Role applies from their next request, which reads it afresh.
    pub(crate) async fn set_role(&self, user: i64, role: Role) -> Result<(), Error> {
        self.db
            .write(move |tx| Box::pin(async move { update_role(tx, user, role).await }))
            .await
    }

    /// Deletes the User `user` with their credentials and Devices, unless that leaves the Server
    /// with no Admin. Their Devices are signed out as [`Users::sign_out`] does.
    pub(crate) async fn delete(&self, user: i64) -> Result<(), Error> {
        self.write_signing_out(move |tx| Box::pin(async move { delete_user(tx, user).await }))
            .await
    }

    /// The Devices of the User `user` that haven't expired, oldest first, or `None` if there is no
    /// such User.
    pub(crate) async fn devices(&self, user: i64) -> Result<Option<Vec<Device>>, sqlx::Error> {
        let mut conn = self.db.reader().acquire().await?;
        if !user_exists(&mut conn, user).await? {
            return Ok(None);
        }
        let expired_before = now() - DEVICE_LIFETIME;
        let devices = sqlx::query_as!(
            Device,
            r#"SELECT id AS "id!", name, created_at, last_seen_at FROM devices
               WHERE user_id = ? AND last_seen_at > ? ORDER BY id"#,
            user,
            expired_before,
        )
        .fetch_all(&mut *conn)
        .await?;
        Ok(Some(devices))
    }

    /// Signs the Device `device` out, as [`Users::sign_out`] does, if it is the User `user`'s.
    /// Returns whether it was.
    pub(crate) async fn revoke(&self, user: i64, device: i64) -> Result<bool, sqlx::Error> {
        // Checked before the write, not in it, which is still race-free: a Device never changes
        // User, its ID is never reused, and signing out one already gone does nothing.
        let theirs = sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM devices WHERE id = ? AND user_id = ?) AS "theirs!: bool""#,
            device,
            user
        )
        .fetch_one(self.db.reader())
        .await?;
        if theirs {
            self.sign_out(device).await?;
        }
        Ok(theirs)
    }

    /// Changes the password of `caller`'s User from `current` to `new`, and signs every other
    /// Device of theirs out. `caller`'s own Device stays signed in.
    pub(crate) async fn change_password(
        &self,
        caller: &Caller,
        current: &str,
        new: &str,
    ) -> Result<(), Error> {
        // Users are never renamed, so the caller's name still finds them.
        let found = self.user_named(&caller.user.name).await?;
        let verified = match &found {
            Some((_, credentials)) => {
                self.verify_password(&Arc::from(current), credentials)
                    .await?
            }
            None => false,
        };
        if !verified {
            return Err(Error::WrongPassword);
        }
        let credential = self.hash_password(new).await?;
        let (user, kept) = (caller.user.id, caller.device.id);
        self.write_signing_out(move |tx| {
            Box::pin(async move {
                replace_credential_signing_others_out(tx, user, &credential, kept).await
            })
        })
        .await
    }

    /// Sets the password of the User `user` to `new`, without their current one, and signs every
    /// Device of theirs out.
    pub(crate) async fn reset_password(&self, user: i64, new: &str) -> Result<(), Error> {
        let credential = self.hash_password(new).await?;
        self.write_signing_out(move |tx| {
            Box::pin(async move { replace_credential_signing_all_out(tx, user, &credential).await })
        })
        .await
    }

    /// Signs the Device `device_name` in as the User named `name`, ignoring case, if `password` is
    /// theirs.
    ///
    /// # Errors
    ///
    /// If the Device name is invalid, the name or password is wrong, the Authenticator fails, or
    /// the database fails.
    pub async fn sign_in(
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
        self.sign_in_as(user, device_name).await
    }

    /// Signs the Device `device_name`, which has been checked, in as `user`, whose credentials have
    /// been, or who approved a Sign-in Code.
    pub(crate) async fn sign_in_as(
        &self,
        user: User,
        device_name: String,
    ) -> Result<SignedIn, Error> {
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
        self.write_signing_out(move |tx| {
            Box::pin(async move {
                sqlx::query!("DELETE FROM devices WHERE id = ?", device)
                    .execute(&mut **tx)
                    .await?;
                Ok(vec![device])
            })
        })
        .await
    }

    /// Runs `write`, which deletes Devices and returns their ids, then tells the waiters in
    /// [`Users::signed_out`] about them. Both run in a task of their own: a queued write goes
    /// through even if the caller stops waiting, and its Devices' waiters must still hear of it.
    async fn write_signing_out<E, F>(&self, write: F) -> Result<(), E>
    where
        E: From<sqlx::Error> + Send + 'static,
        F: for<'t> FnOnce(&'t mut Transaction<'_, Sqlite>) -> BoxFuture<'t, Result<Vec<i64>, E>>
            + Send
            + 'static,
    {
        let users = self.clone();
        let written = tokio::spawn(async move {
            for device in users.db.write(write).await? {
                // Fails only when nobody is waiting, which is fine.
                let _: Result<usize, _> = users.sign_outs.send(device);
            }
            Ok(())
        });
        // The task is never aborted, so it only fails if it panicked.
        written
            .await
            .unwrap_or_else(|_| Err(sqlx::Error::WorkerCrashed.into()))
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
            r#"SELECT d.id AS "device_id!", d.name AS device_name, d.created_at AS device_created_at,
                      d.last_seen_at,
                      u.id AS "user_id!", u.name AS user_name, u.role AS "role: Role",
                      u.created_at
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
                created_at: row.created_at,
            },
            device: Device {
                id: row.device_id,
                name: row.device_name,
                created_at: row.device_created_at,
                last_seen_at: row.last_seen_at,
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
        let name_key = name_key(name);
        let rows = sqlx::query!(
            r#"SELECT u.id AS "id!", u.name, u.role AS "role: Role", u.created_at, c.plugin,
                      c.credential
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
            created_at: first.created_at,
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
        random_code(SETUP_CODE_LENGTH).map(Self)
    }

    /// Whether `entered` is this code, ignoring case, dashes and spaces.
    pub(crate) fn matches(&self, entered: &str) -> bool {
        // Comparing hashes, so how long a wrong guess takes says nothing about the code.
        Sha256::digest(normalized_code(entered)) == Sha256::digest(&self.0)
    }
}

impl fmt::Display for SetupCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&code_in_groups(&self.0))
    }
}

/// `length` random characters of [`CODE_ALPHABET`], for a code a person reads and types.
pub(crate) fn random_code(length: usize) -> Result<String, getrandom::Error> {
    let mut bytes = vec![0; length];
    getrandom::fill(&mut bytes)?;
    // 256 is a multiple of 32, so every character is equally likely.
    Ok(bytes
        .iter()
        .filter_map(|byte| CODE_ALPHABET.get(usize::from(byte % 32)))
        .map(|&c| char::from(c))
        .collect())
}

/// A code as a person typed it, in the form [`random_code`] makes: without dashes or spaces, and
/// in upper case.
pub(crate) fn normalized_code(entered: &str) -> String {
    entered
        .chars()
        .filter(|c| *c != '-' && !c.is_whitespace())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// A code [`random_code`] made, as it is shown: in groups of four joined by dashes.
pub(crate) fn code_in_groups(code: &str) -> String {
    let mut shown = String::with_capacity(code.len() + code.len() / 4);
    for (i, c) in code.chars().enumerate() {
        if i > 0 && i % 4 == 0 {
            shown.push('-');
        }
        shown.push(c);
    }
    shown
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
pub(crate) fn checked_name(what: &str, name: &str) -> Result<String, Error> {
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

fn new_token() -> Result<String, getrandom::Error> {
    Ok(format!("{TOKEN_PREFIX}{}", random_secret()?))
}

/// [`TOKEN_BYTES`] random bytes, base64url-encoded: too many to guess.
pub(crate) fn random_secret() -> Result<String, getrandom::Error> {
    let mut bytes = [0; TOKEN_BYTES];
    getrandom::fill(&mut bytes)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

/// What the database keeps of a session token.
fn hash_token(token: &str) -> [u8; 32] {
    Sha256::digest(token).into()
}

async fn any_user(conn: &mut SqliteConnection) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar!(r#"SELECT EXISTS (SELECT 1 FROM users) AS "any!: bool""#)
        .fetch_one(conn)
        .await
}

async fn user_exists(conn: &mut SqliteConnection, user: i64) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM users WHERE id = ?) AS "exists!: bool""#,
        user
    )
    .fetch_one(conn)
    .await
}

/// Gives the User `user` the Role `role`, failing if that leaves no Admin. Checked after the write,
/// in the same transaction, so a refusal rolls the write back. Writes run one at a time, so two
/// demotions at once can't both pass the check.
async fn update_role(conn: &mut SqliteConnection, user: i64, role: Role) -> Result<(), Error> {
    let updated = sqlx::query!("UPDATE users SET role = ? WHERE id = ?", role, user)
        .execute(&mut *conn)
        .await?;
    if updated.rows_affected() == 0 {
        return Err(Error::NoSuchUser);
    }
    ensure_an_admin_left(conn).await
}

/// Deletes the User `user`, whose credentials and Devices go with them, failing if that leaves no
/// Admin, as [`update_role`] does. Returns the IDs of the Devices it signed out.
async fn delete_user(conn: &mut SqliteConnection, user: i64) -> Result<Vec<i64>, Error> {
    let devices = sqlx::query_scalar!(r#"SELECT id AS "id!" FROM devices WHERE user_id = ?"#, user)
        .fetch_all(&mut *conn)
        .await?;
    // Their `user_credentials` and `devices` rows are deleted by `ON DELETE CASCADE`.
    let deleted = sqlx::query!("DELETE FROM users WHERE id = ?", user)
        .execute(&mut *conn)
        .await?;
    if deleted.rows_affected() == 0 {
        return Err(Error::NoSuchUser);
    }
    ensure_an_admin_left(conn).await?;
    Ok(devices)
}

/// Fails with [`Error::LastAdmin`] if the Server has no Admin, so the write before it, in the same
/// transaction, is rolled back.
async fn ensure_an_admin_left(conn: &mut SqliteConnection) -> Result<(), Error> {
    let any_admin = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM users WHERE role = 'admin') AS "any!: bool""#
    )
    .fetch_one(conn)
    .await?;
    if any_admin {
        Ok(())
    } else {
        Err(Error::LastAdmin)
    }
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
    insert_credential(conn, user.id, credential).await?;
    let device = insert_device(conn, user.id, device_name, token_hash).await?;
    Ok(Caller { user, device })
}

/// Makes `credential` the only one the User `user` has, as [`replace_credential`] does, and signs
/// out every Device of theirs but `kept`, returning the ids of those signed out.
///
/// Fails if `kept` was signed out meanwhile, such as by another password change, so a change never
/// succeeds for a Device that is no longer signed in, nor overwrites the one that signed it out.
async fn replace_credential_signing_others_out(
    conn: &mut SqliteConnection,
    user: i64,
    credential: &Credential,
    kept: i64,
) -> Result<Vec<i64>, Error> {
    let still_signed_in = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM devices WHERE id = ?) AS "exists!: bool""#,
        kept
    )
    .fetch_one(&mut *conn)
    .await?;
    if !still_signed_in {
        return Err(Error::SignedOut);
    }
    replace_credential(conn, user, credential).await?;
    let signed_out = sqlx::query_scalar!(
        r#"DELETE FROM devices WHERE user_id = ? AND id != ? RETURNING id AS "id!""#,
        user,
        kept
    )
    .fetch_all(conn)
    .await?;
    Ok(signed_out)
}

/// Makes `credential` the only one the User `user` has, as [`replace_credential`] does, and signs
/// out every Device of theirs, returning their ids. Fails if there is no such User.
async fn replace_credential_signing_all_out(
    conn: &mut SqliteConnection,
    user: i64,
    credential: &Credential,
) -> Result<Vec<i64>, Error> {
    if !user_exists(conn, user).await? {
        return Err(Error::NoSuchUser);
    }
    replace_credential(conn, user, credential).await?;
    let signed_out = sqlx::query_scalar!(
        r#"DELETE FROM devices WHERE user_id = ? RETURNING id AS "id!""#,
        user
    )
    .fetch_all(conn)
    .await?;
    Ok(signed_out)
}

/// Makes `credential` the only one the User `user` has. Credentials other Plugins made go too, so
/// the old password can't come back with a Plugin that is enabled again.
async fn replace_credential(
    conn: &mut SqliteConnection,
    user: i64,
    credential: &Credential,
) -> Result<(), Error> {
    sqlx::query!("DELETE FROM user_credentials WHERE user_id = ?", user)
        .execute(&mut *conn)
        .await?;
    insert_credential(conn, user, credential).await
}

async fn insert_credential(
    conn: &mut SqliteConnection,
    user: i64,
    credential: &Credential,
) -> Result<(), Error> {
    sqlx::query!(
        "INSERT INTO user_credentials (user_id, plugin, credential) VALUES (?, ?, ?)",
        user,
        credential.plugin.as_str(),
        credential.value,
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// What a User's `name` is stored and looked up under, trimmed, so names are unique and found
/// ignoring case.
pub(crate) fn name_key(name: &str) -> String {
    name.trim().to_lowercase()
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
        created_at,
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
    Ok(Device {
        id,
        name,
        created_at: now,
        last_seen_at: now,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn create_user(db: &Database, name: &str) -> Result<User, Error> {
        let name = name.to_owned();
        db.write(move |tx| Box::pin(async move { insert_user(tx, &name, Role::Member).await }))
            .await
    }

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
    async fn a_sign_out_whose_caller_stops_waiting_still_ends_the_wait() {
        let data_dir = tempfile::tempdir().unwrap();
        let (users, signed_in_at) = signed_in_device(data_dir.path()).await;
        let device = device_id(&users, signed_in_at).await;
        let mut waiting = std::pin::pin!(users.signed_out(device));
        let before =
            tokio::time::timeout(std::time::Duration::from_millis(200), &mut waiting).await;
        assert!(before.is_err(), "the wait ended before the sign-out");

        // Polled once, which queues the write, then dropped, as when a client disconnects.
        let mut abandoned = Box::pin(users.sign_out(device));
        let polled = abandoned
            .as_mut()
            .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()));
        assert!(polled.is_pending(), "the sign-out finished at once");
        drop(abandoned);
        let after = tokio::time::timeout(WAIT, waiting).await;

        assert!(after.is_ok(), "the wait didn't end");
    }

    #[tokio::test]
    async fn a_name_another_user_has_in_a_different_case_is_taken() {
        let data_dir = tempfile::tempdir().unwrap();
        let db = Database::open(data_dir.path()).await.unwrap();
        create_user(&db, "Émile").await.unwrap();

        let duplicate = create_user(&db, "éMILE").await;

        assert!(matches!(duplicate, Err(Error::NameTaken)), "{duplicate:?}");
    }

    #[tokio::test]
    async fn demoting_two_admins_at_once_leaves_exactly_one() {
        let data_dir = tempfile::tempdir().unwrap();
        let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
        let users = Users::new(Arc::clone(&db), Registry::new());
        let ada = create_user(&db, "Ada").await.unwrap().id;
        let grace = create_user(&db, "Grace").await.unwrap().id;
        users.set_role(ada, Role::Admin).await.unwrap();
        users.set_role(grace, Role::Admin).await.unwrap();

        // Unlike over the API, no access check stands between the two and the database.
        let (ada_demoted, grace_demoted) = tokio::join!(
            users.set_role(ada, Role::Member),
            users.set_role(grace, Role::Member),
        );

        let refused = [&ada_demoted, &grace_demoted]
            .into_iter()
            .filter(|demoted| matches!(demoted, Err(Error::LastAdmin)))
            .count();
        assert_eq!(refused, 1, "{ada_demoted:?} / {grace_demoted:?}");
        let admins = users.list().await.unwrap();
        let admins = admins.iter().filter(|user| user.role == Role::Admin);
        assert_eq!(admins.count(), 1);
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
