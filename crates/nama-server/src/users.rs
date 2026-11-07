//! Users, Devices and their sessions: Kernel state (ADR 0014).
//!
//! [`Users`] is the only way in. It checks names before anything is written and has the enabled
//! Authenticator turn passwords into credentials. Nothing secret is stored: see `AGENTS.md`.

use std::{fmt, net::IpAddr, ops::RangeInclusive, sync::Arc};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use buffa::Enumeration;
use nama_plugin_api::{
    PluginError, PluginId,
    auth::{Authenticator, PasswordError},
};
use nama_proto::nama::v1::Permission;
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

/// How often, in seconds, a Device in use has its last-seen time written and its expiry pushed
/// back: an hour. Writing on every request would make every request wait for the one database
/// writer.
const LAST_SEEN_PRECISION: i64 = 60 * 60;

/// How long, in seconds, a Device has Recent Authentication after proving who it is: 10 minutes
/// (ADR 0014).
const RECENT_AUTHENTICATION_WINDOW: i64 = 10 * 60;

/// What a User is shown for their password among their Sign-in Methods.
const PASSWORD_METHOD_NAME: &str = "Password";

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

/// The set of Permissions a User holds (ADR 0020).
// Stored in `users.permissions` as a bitset: bit n is the `Permission` numbered n. Those numbers
// are API contract, so they never change under stored rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(transparent)]
pub(crate) struct Permissions(i64);

impl Permissions {
    /// What the first Admin gets.
    pub(crate) const ADMIN: Self = Self(bit(Permission::Admin));

    /// What every other new User gets.
    // ponytail: hardcoded; the Server-wide default set an Admin edits replaces it (#158).
    pub(crate) const NEW_USER: Self =
        Self(bit(Permission::Request) | bit(Permission::ViewRequests));

    /// Whether these pass a check for `needed`: they hold it, `PERMISSION_ADMIN`, or the
    /// Permission that covers it, as `PERMISSION_REQUEST` covers `PERMISSION_REQUEST_MOVIE` and
    /// `PERMISSION_REQUEST_TV`, and `PERMISSION_AUTO_APPROVE` its variants, as in Seerr.
    pub(crate) const fn allow(self, needed: Permission) -> bool {
        let covering = match needed {
            Permission::RequestMovie | Permission::RequestTv => Permission::Request,
            Permission::AutoApproveMovie | Permission::AutoApproveTv => Permission::AutoApprove,
            // Listed, not a wildcard, so a new Permission gets a decision about what covers it.
            Permission::Unspecified
            | Permission::Admin
            | Permission::ManageUsers
            | Permission::ManageLibraries
            | Permission::ManageSettings
            | Permission::ManageRequests
            | Permission::Request
            | Permission::AutoApprove
            | Permission::ViewRequests => needed,
        };
        self.0 & (bit(Permission::Admin) | bit(needed) | bit(covering)) != 0
    }

    /// Each Permission held, in the order of their values.
    pub(crate) fn iter(self) -> impl Iterator<Item = Permission> {
        Permission::values()
            .iter()
            .copied()
            .filter(move |&permission| self.0 & bit(permission) != 0)
    }
}

impl FromIterator<Permission> for Permissions {
    fn from_iter<I: IntoIterator<Item = Permission>>(iter: I) -> Self {
        Self(iter.into_iter().fold(0, |bits, p| bits | bit(p)))
    }
}

/// The bit `permission` is stored as in [`Permissions`].
// ponytail: an `i64` holds Permissions numbered up to 62; past that, `users.permissions` needs
// another encoding.
const fn bit(permission: Permission) -> i64 {
    1 << permission as i32
}

/// A User as clients see it.
#[derive(Debug, Clone)]
pub(crate) struct User {
    pub(crate) id: i64,
    pub(crate) name: String,
    pub(crate) permissions: Permissions,
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
    /// When it last proved who it is with one of its User's Sign-in Methods, in seconds since the
    /// Unix epoch, or 0 if it never has.
    pub(crate) authenticated_at: i64,
}

impl Device {
    /// Whether it has Recent Authentication at `now`, in seconds since the Unix epoch.
    pub(crate) const fn authenticated_recently(&self, now: i64) -> bool {
        now - self.authenticated_at < RECENT_AUTHENTICATION_WINDOW
    }
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

/// A Device about to sign in, and where from.
#[derive(Debug)]
pub struct NewDevice {
    /// What it is called. Signing in checks it, except where the Device was named earlier.
    pub name: String,
    /// The client IP address it signs in from.
    pub ip_address: IpAddr,
    /// The `User-Agent` it sent, if any.
    pub user_agent: Option<String>,
}

/// A credential string the Kernel keeps for a User, and the Plugin whose Authenticator made it,
/// which is the only one that can check it.
struct Credential {
    plugin: PluginId,
    value: String,
}

/// A User's password Sign-in Method, made by a Plugin that is enabled.
struct StoredPassword {
    id: i64,
    credential: Credential,
}

/// Why a change to the Users was refused or failed. No variant holds a password or a token.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A name, trimmed, is too short or too long.
    #[error(
        "{what} needs {min} to {max} characters",
        min = NAME_LENGTH.start(),
        max = NAME_LENGTH.end()
    )]
    NameLength {
        /// The request field it is about, as clients name it.
        field: &'static str,
        /// What the name belongs to, as the message says it: "a User's name".
        what: &'static str,
    },
    /// The Authenticator rejects the password, for the reason `message` gives.
    #[error("{message}")]
    PasswordRejected {
        /// The request field it is about, as clients name it.
        field: &'static str,
        /// Why, in the Authenticator's words.
        message: String,
    },
    /// Another User already has this name, ignoring case.
    #[error("another User already has that name")]
    NameTaken,
    /// No User has this name and password. Which of the two is wrong isn't said, so sign-in
    /// doesn't reveal which names exist.
    #[error("the name or password is wrong")]
    WrongNameOrPassword,
    /// The password given isn't the User's.
    #[error("the password is wrong")]
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
    /// Only a User with `PERMISSION_ADMIN` may make the change.
    #[error("only an Admin may change an Admin, make an Admin or change their own Permissions")]
    AdminRequired,
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

    /// Creates the first Admin, named `name` with `password`, and signs `device` in as them.
    /// Checking the Setup Code is the caller's job.
    pub(crate) async fn create_first_admin(
        &self,
        name: &str,
        password: &str,
        device: NewDevice,
    ) -> Result<SignedIn, Error> {
        let name = checked_name("name", "a User's name", name)?;
        let device = device.checked()?;
        // Before hashing, which is slow, though only the check in the write below is race-free.
        if self.any().await? {
            return Err(Error::AlreadySetUp);
        }
        let credential = self.hash_password("password", password).await?;
        let token = new_token()?;
        let token_hash = hash_token(&token);
        let caller = self
            .db
            .write(move |tx| {
                Box::pin(async move {
                    insert_first_admin(tx, &name, &credential, device, &token_hash).await
                })
            })
            .await?;
        Ok(SignedIn { caller, token })
    }

    /// Creates a User named `name` with `password` and [`Permissions::NEW_USER`], who can then sign
    /// in, and returns their ID.
    pub(crate) async fn create(&self, name: &str, password: &str) -> Result<i64, Error> {
        let name = checked_name("name", "a User's name", name)?;
        let credential = self.hash_password("password", password).await?;
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    let user = insert_user(tx, &name, Permissions::NEW_USER).await?;
                    set_password(tx, user.id, &credential).await?;
                    Ok(user.id)
                })
            })
            .await
    }

    /// Every User, oldest first.
    pub(crate) async fn list(&self) -> Result<Vec<User>, sqlx::Error> {
        sqlx::query_as!(
            User,
            r#"SELECT id AS "id!", display_username AS name,
                      permissions AS "permissions: Permissions", created_at
               FROM users ORDER BY id"#
        )
        .fetch_all(self.db.reader())
        .await
    }

    /// Gives the User `user` exactly `permissions`, as `by` asks, unless that leaves no User with
    /// `PERMISSION_ADMIN` or [`ensure_may_change`] refuses. Without `PERMISSION_ADMIN`, `by` may
    /// neither grant it nor change their own Permissions. Their Devices stay signed in, and the
    /// Permissions apply from their next request, which reads them afresh.
    pub(crate) async fn set_permissions(
        &self,
        by: &User,
        user: i64,
        permissions: Permissions,
    ) -> Result<(), Error> {
        let by_admin = by.permissions.allow(Permission::Admin);
        if !by_admin && (permissions.allow(Permission::Admin) || by.id == user) {
            return Err(Error::AdminRequired);
        }
        let by = by.clone();
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    ensure_may_change(tx, &by, user).await?;
                    update_permissions(tx, user, permissions).await
                })
            })
            .await
    }

    /// Deletes the User `user` with their Sign-in Methods and Devices, as `by` asks, unless that
    /// leaves no User with `PERMISSION_ADMIN` or [`ensure_may_change`] refuses. Their Devices are
    /// signed out as [`Users::sign_out`] does.
    pub(crate) async fn delete(&self, by: &User, user: i64) -> Result<(), Error> {
        let by = by.clone();
        self.write_signing_out(move |tx| {
            Box::pin(async move {
                ensure_may_change(tx, &by, user).await?;
                delete_user(tx, user).await
            })
        })
        .await
    }

    /// The Devices of the User `user` that haven't expired, oldest first, or `None` if there is no
    /// such User.
    pub(crate) async fn devices(&self, user: i64) -> Result<Option<Vec<Device>>, sqlx::Error> {
        let mut conn = self.db.reader().acquire().await?;
        if !user_exists(&mut conn, user).await? {
            return Ok(None);
        }
        let now = now();
        let devices = sqlx::query_as!(
            Device,
            r#"SELECT id AS "id!", name, created_at, last_seen_at, authenticated_at FROM devices
               WHERE user_id = ? AND expires_at > ? ORDER BY id"#,
            user,
            now,
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

    /// Changes the password of `caller`'s User to `new`, and signs every other Device of theirs
    /// out. `caller`'s own Device stays signed in. Recent Authentication stands in for the current
    /// password: the access layer checked it.
    pub(crate) async fn change_password(&self, caller: &Caller, new: &str) -> Result<(), Error> {
        let credential = self.hash_password("new_password", new).await?;
        let (user, kept) = (caller.user.id, caller.device.id);
        self.write_signing_out(move |tx| {
            Box::pin(
                async move { set_password_signing_others_out(tx, user, &credential, kept).await },
            )
        })
        .await
    }

    /// Gives `caller`'s Device Recent Authentication if `password` is their User's.
    pub(crate) async fn reauthenticate(
        &self,
        caller: &Caller,
        password: &str,
    ) -> Result<(), Error> {
        // Users are never renamed, so the caller's name still finds them.
        let found = self.user_named(&caller.user.name).await?;
        let stored = found.as_ref().and_then(|(_, stored)| stored.as_ref());
        if self
            .verify_password(&Arc::from(password), stored)
            .await?
            .is_none()
        {
            return Err(Error::WrongPassword);
        }
        let device = caller.device.id;
        self.db
            .write(move |tx| Box::pin(async move { record_authentication(tx, device).await }))
            .await
    }

    /// Sets the password of the User `user` to `new`, as `by` asks, without their current one,
    /// adding it if they have none, and signs every Device of theirs out, unless
    /// [`ensure_may_change`] refuses.
    pub(crate) async fn reset_password(
        &self,
        by: &User,
        user: i64,
        new: &str,
    ) -> Result<(), Error> {
        let credential = self.hash_password("new_password", new).await?;
        let by = by.clone();
        self.write_signing_out(move |tx| {
            Box::pin(async move {
                ensure_may_change(tx, &by, user).await?;
                set_password_signing_all_out(tx, user, &credential).await
            })
        })
        .await
    }

    /// Signs `device` in as the User named `name`, ignoring case, if `password` is theirs.
    ///
    /// # Errors
    ///
    /// If the Device name is invalid, the name or password is wrong, the Authenticator fails, or
    /// the database fails.
    pub async fn sign_in(
        &self,
        name: &str,
        password: &str,
        device: NewDevice,
    ) -> Result<SignedIn, Error> {
        let device = device.checked()?;
        let found = self.user_named(name).await?;
        let stored = found.as_ref().and_then(|(_, stored)| stored.as_ref());
        let method = self.verify_password(&Arc::from(password), stored).await?;
        match (found, method) {
            (Some((user, _)), Some(method)) => self.sign_in_as(user, Some(method), device).await,
            _ => Err(Error::WrongNameOrPassword),
        }
    }

    /// Signs `device`, whose name has been checked, in as `user`, who proved who they are with the
    /// Sign-in Method `method`, or with none, by approving a Sign-in Code. The only place sessions
    /// are issued.
    pub(crate) async fn sign_in_as(
        &self,
        user: User,
        method: Option<i64>,
        device: NewDevice,
    ) -> Result<SignedIn, Error> {
        let token = new_token()?;
        let token_hash = hash_token(&token);
        let device = self
            .db
            .write(move |tx| {
                Box::pin(
                    async move { insert_device(tx, user.id, method, device, &token_hash).await },
                )
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
        let row = sqlx::query!(
            r#"SELECT d.id AS "device_id!", d.name AS device_name, d.created_at AS device_created_at,
                      d.last_seen_at, d.authenticated_at,
                      u.id AS "user_id!", u.display_username AS user_name,
                      u.permissions AS "permissions: Permissions", u.created_at
               FROM devices d JOIN users u ON u.id = d.user_id
               WHERE d.token_hash = ? AND d.expires_at > ?"#,
            token_hash,
            now,
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
                permissions: row.permissions,
                created_at: row.created_at,
            },
            device: Device {
                id: row.device_id,
                name: row.device_name,
                created_at: row.device_created_at,
                last_seen_at: row.last_seen_at,
                authenticated_at: row.authenticated_at,
            },
        };
        Ok(Some(Session { caller, refreshed }))
    }

    /// Records that the Device `device` was last seen at `now`, which pushes its expiry back,
    /// unless another request already did within [`LAST_SEEN_PRECISION`] seconds of it.
    async fn saw_device(&self, device: i64, now: i64) -> Result<(), sqlx::Error> {
        let stale_since = now - LAST_SEEN_PRECISION;
        let expires_at = now + DEVICE_LIFETIME;
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!(
                        "UPDATE devices SET last_seen_at = ?, updated_at = ?, expires_at = ?
                         WHERE id = ? AND last_seen_at <= ?",
                        now,
                        now,
                        expires_at,
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

    /// Has the first enabled Authenticator turn `password`, given as the request field `field`,
    /// into a credential string.
    async fn hash_password(
        &self,
        field: &'static str,
        password: &str,
    ) -> Result<Credential, Error> {
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
            Ok(Err(PasswordError::Rejected(message))) => {
                Err(Error::PasswordRejected { field, message })
            }
            Ok(Err(PasswordError::Failed(err))) => Err(Error::Authenticator(err.to_string())),
            Err(panicked) => Err(Error::Authenticator(panicked.to_string())),
        }
    }

    /// The User whose name is `name` ignoring case, whether or not they have a password, with their
    /// password if an enabled Plugin made it.
    async fn user_named(
        &self,
        name: &str,
    ) -> Result<Option<(User, Option<StoredPassword>)>, Error> {
        let username = name_key(name);
        let row = sqlx::query!(
            r#"SELECT u.id AS "id!", u.display_username AS name,
                      u.permissions AS "permissions: Permissions", u.created_at,
                      m.id AS "method_id?", m.provider_id AS "provider_id?",
                      m.credential AS "credential?"
               FROM users u LEFT JOIN sign_in_methods m ON m.user_id = u.id AND m.subject IS NULL
               WHERE u.username = ?"#,
            username
        )
        .fetch_optional(self.db.reader())
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let user = User {
            id: row.id,
            name: row.name,
            permissions: row.permissions,
            created_at: row.created_at,
        };
        let enabled = self.plugins.enabled();
        let plugin = enabled
            .into_iter()
            .find(|id| row.provider_id.as_deref() == Some(id.as_str()));
        let stored = match (row.method_id, plugin, row.credential) {
            (Some(id), Some(plugin), Some(value)) => Some(StoredPassword {
                id,
                credential: Credential { plugin, value },
            }),
            _ => None,
        };
        Ok(Some((user, stored)))
    }

    /// The id of `stored`'s Sign-in Method if `password` is the one it was made from, as checked by
    /// the Authenticator that made it. With no `stored` password, or none whose Plugin still
    /// provides one, the answer is no, after as long as checking would have taken.
    async fn verify_password(
        &self,
        password: &Arc<str>,
        stored: Option<&StoredPassword>,
    ) -> Result<Option<i64>, Error> {
        if let Some(stored) = stored {
            let value: Arc<str> = Arc::from(stored.credential.value.as_str());
            let verified = self
                .plugins
                .call_provider_of(&stored.credential.plugin, |authenticator| {
                    verify_where_blocking_is_fine(authenticator, password, &value)
                })
                .await;
            match verified {
                None => {}
                Some(Ok(Ok(verified))) => return Ok(verified.then_some(stored.id)),
                Some(Ok(Err(err))) => return Err(Error::Authenticator(err.to_string())),
                Some(Err(panicked)) => return Err(Error::Authenticator(panicked.to_string())),
            }
        }
        self.verify_unknown_user(password).await?;
        Ok(None)
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

impl NewDevice {
    /// This Device with its name checked.
    fn checked(self) -> Result<Self, Error> {
        let name = checked_name("device_name", "a Device's name", &self.name)?;
        Ok(Self { name, ..self })
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

/// `name`, given as the request field `field`, trimmed, if it then has an allowed length. `what`
/// names it in the error.
pub(crate) fn checked_name(
    field: &'static str,
    what: &'static str,
    name: &str,
) -> Result<String, Error> {
    let name = name.trim();
    if NAME_LENGTH.contains(&name.chars().count()) {
        Ok(name.to_owned())
    } else {
        Err(Error::NameLength { field, what })
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

/// Gives the User `user` exactly `permissions`, failing if that leaves no User with
/// `PERMISSION_ADMIN`. Checked after the write, in the same transaction, so a refusal rolls the
/// write back. Writes run one at a time, so two demotions at once can't both pass the check.
async fn update_permissions(
    conn: &mut SqliteConnection,
    user: i64,
    permissions: Permissions,
) -> Result<(), Error> {
    let now = now();
    let updated = sqlx::query!(
        "UPDATE users SET permissions = ?, updated_at = ? WHERE id = ?",
        permissions.0,
        now,
        user
    )
    .execute(&mut *conn)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(Error::NoSuchUser);
    }
    ensure_an_admin_left(conn).await
}

/// Deletes the User `user`, whose Sign-in Methods and Devices go with them, failing if that leaves
/// no User with `PERMISSION_ADMIN`, as [`update_permissions`] does. Returns the IDs of the Devices
/// it signed out.
async fn delete_user(conn: &mut SqliteConnection, user: i64) -> Result<Vec<i64>, Error> {
    let devices = sqlx::query_scalar!(r#"SELECT id AS "id!" FROM devices WHERE user_id = ?"#, user)
        .fetch_all(&mut *conn)
        .await?;
    // Their `sign_in_methods` and `devices` rows are deleted by `ON DELETE CASCADE`.
    let deleted = sqlx::query!("DELETE FROM users WHERE id = ?", user)
        .execute(&mut *conn)
        .await?;
    if deleted.rows_affected() == 0 {
        return Err(Error::NoSuchUser);
    }
    ensure_an_admin_left(conn).await?;
    Ok(devices)
}

/// Fails with [`Error::AdminRequired`] if `by`, without `PERMISSION_ADMIN`, would change the User
/// `user`, who has it. Only an Admin changes an Admin, as in Seerr, so `PERMISSION_MANAGE_USERS`
/// can't become `PERMISSION_ADMIN` by taking over an Admin. Read in the write's transaction, so
/// `user` can't be made an Admin between the check and the change.
async fn ensure_may_change(conn: &mut SqliteConnection, by: &User, user: i64) -> Result<(), Error> {
    if by.permissions.allow(Permission::Admin) {
        return Ok(());
    }
    let admin = Permissions::ADMIN.0;
    let changing_an_admin = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM users WHERE id = ? AND permissions & ? != 0)
               AS "admin!: bool""#,
        user,
        admin
    )
    .fetch_one(conn)
    .await?;
    if changing_an_admin {
        Err(Error::AdminRequired)
    } else {
        Ok(())
    }
}

/// Fails with [`Error::LastAdmin`] if no User has `PERMISSION_ADMIN`, so the write before it, in
/// the same transaction, is rolled back.
async fn ensure_an_admin_left(conn: &mut SqliteConnection) -> Result<(), Error> {
    let admin = Permissions::ADMIN.0;
    let any_admin = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM users WHERE permissions & ? != 0) AS "any!: bool""#,
        admin
    )
    .fetch_one(conn)
    .await?;
    if any_admin {
        Ok(())
    } else {
        Err(Error::LastAdmin)
    }
}

/// Stores the first Admin, named `name`, with `credential` as their password, and `device`
/// signed in with it, with the hash of its session token, unless the Server already has a User.
/// The names have been checked.
async fn insert_first_admin(
    conn: &mut SqliteConnection,
    name: &str,
    credential: &Credential,
    device: NewDevice,
    token_hash: &[u8; 32],
) -> Result<Caller, Error> {
    if any_user(conn).await? {
        return Err(Error::AlreadySetUp);
    }
    let user = insert_user(conn, name, Permissions::ADMIN).await?;
    let method = set_password(conn, user.id, credential).await?;
    let device = insert_device(conn, user.id, Some(method), device, token_hash).await?;
    Ok(Caller { user, device })
}

/// Records that the Device `device` proved who it is just now, failing if it was signed out
/// meanwhile.
async fn record_authentication(conn: &mut SqliteConnection, device: i64) -> Result<(), Error> {
    let now = now();
    let updated = sqlx::query!(
        "UPDATE devices SET authenticated_at = ?, updated_at = ? WHERE id = ?",
        now,
        now,
        device
    )
    .execute(conn)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(Error::SignedOut);
    }
    Ok(())
}

/// Makes `credential` the User `user`'s password, as [`set_password`] does, and signs out every
/// Device of theirs but `kept`, returning the ids of those signed out.
///
/// Fails if `kept` was signed out meanwhile, such as by another password change, so a change never
/// succeeds for a Device that is no longer signed in, nor overwrites the one that signed it out.
async fn set_password_signing_others_out(
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
    set_password(conn, user, credential).await?;
    let signed_out = sqlx::query_scalar!(
        r#"DELETE FROM devices WHERE user_id = ? AND id != ? RETURNING id AS "id!""#,
        user,
        kept
    )
    .fetch_all(conn)
    .await?;
    Ok(signed_out)
}

/// Makes `credential` the User `user`'s password, as [`set_password`] does, and signs out every
/// Device of theirs, returning their ids. Fails if there is no such User.
async fn set_password_signing_all_out(
    conn: &mut SqliteConnection,
    user: i64,
    credential: &Credential,
) -> Result<Vec<i64>, Error> {
    if !user_exists(conn, user).await? {
        return Err(Error::NoSuchUser);
    }
    set_password(conn, user, credential).await?;
    let signed_out = sqlx::query_scalar!(
        r#"DELETE FROM devices WHERE user_id = ? RETURNING id AS "id!""#,
        user
    )
    .fetch_all(conn)
    .await?;
    Ok(signed_out)
}

/// Makes `credential` the User `user`'s password and returns its Sign-in Method's id. A password
/// they had is overwritten, whichever Plugin made it, so it can't come back with a Plugin that is
/// enabled again. It keeps its id, so Devices signed in with it aren't signed out by
/// `ON DELETE CASCADE`; the callers sign out the ones they mean to.
async fn set_password(
    conn: &mut SqliteConnection,
    user: i64,
    credential: &Credential,
) -> Result<i64, sqlx::Error> {
    let now = now();
    sqlx::query_scalar!(
        r#"INSERT INTO sign_in_methods
               (user_id, provider_id, credential, name, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, ?)
           ON CONFLICT (user_id) WHERE subject IS NULL DO UPDATE
               SET provider_id = excluded.provider_id, credential = excluded.credential,
                   updated_at = excluded.updated_at
           RETURNING id AS "id!""#,
        user,
        credential.plugin.as_str(),
        credential.value,
        PASSWORD_METHOD_NAME,
        now,
        now,
    )
    .fetch_one(conn)
    .await
}

/// The `username` a User's `name` is stored and looked up under, so names are unique and found
/// ignoring case.
pub(crate) fn name_key(name: &str) -> String {
    name.trim().to_lowercase()
}

/// Stores a User named `name`, which has been checked, with `permissions`, unless another has the
/// name ignoring case.
async fn insert_user(
    conn: &mut SqliteConnection,
    name: &str,
    permissions: Permissions,
) -> Result<User, Error> {
    let username = name_key(name);
    let created_at = now();
    let id = sqlx::query_scalar!(
        r#"INSERT INTO users (username, display_username, permissions, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?)
           RETURNING id AS "id!""#,
        username,
        name,
        permissions.0,
        created_at,
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
        permissions,
        created_at,
    })
}

/// Stores `device`, whose name has been checked, signed in as the User `user` with the Sign-in
/// Method `method`, or by a Sign-in Code, and the hash of its session token.
async fn insert_device(
    conn: &mut SqliteConnection,
    user: i64,
    method: Option<i64>,
    device: NewDevice,
    token_hash: &[u8; 32],
) -> Result<Device, sqlx::Error> {
    let now = now();
    let expires_at = now + DEVICE_LIFETIME;
    // A Device signed in by a Sign-in Code never proved who it is itself (ADR 0014).
    let authenticated_at = if method.is_some() { now } else { 0 };
    let token_hash = token_hash.as_slice();
    let ip_address = device.ip_address.to_string();
    let id = sqlx::query_scalar!(
        r#"INSERT INTO devices
               (user_id, sign_in_method_id, name, token_hash, ip_address, user_agent, created_at,
                updated_at, last_seen_at, expires_at, authenticated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
           RETURNING id AS "id!""#,
        user,
        method,
        device.name,
        token_hash,
        ip_address,
        device.user_agent,
        now,
        now,
        now,
        expires_at,
        authenticated_at,
    )
    .fetch_one(conn)
    .await?;
    Ok(Device {
        id,
        name: device.name,
        created_at: now,
        last_seen_at: now,
        authenticated_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn create_user(db: &Database, name: &str) -> Result<User, Error> {
        let name = name.to_owned();
        db.write(move |tx| {
            Box::pin(async move { insert_user(tx, &name, Permissions::NEW_USER).await })
        })
        .await
    }

    /// An Admin who isn't stored, to make changes as.
    fn admin() -> User {
        User {
            id: 0,
            name: "Root".to_owned(),
            permissions: Permissions::ADMIN,
            created_at: 0,
        }
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
            Box::pin(
                async move { insert_device(tx, user.id, None, laptop(), &hash_token(TOKEN)).await },
            )
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
        users
            .set_permissions(&admin(), ada, Permissions::ADMIN)
            .await
            .unwrap();
        users
            .set_permissions(&admin(), grace, Permissions::ADMIN)
            .await
            .unwrap();

        // Unlike over the API, no access check stands between the two and the database.
        let root = admin();
        let (ada_demoted, grace_demoted) = tokio::join!(
            users.set_permissions(&root, ada, Permissions::NEW_USER),
            users.set_permissions(&root, grace, Permissions::NEW_USER),
        );

        let refused = [&ada_demoted, &grace_demoted]
            .into_iter()
            .filter(|demoted| matches!(demoted, Err(Error::LastAdmin)))
            .count();
        assert_eq!(refused, 1, "{ada_demoted:?} / {grace_demoted:?}");
        let admins = users.list().await.unwrap();
        let admins = admins
            .iter()
            .filter(|user| user.permissions == Permissions::ADMIN);
        assert_eq!(admins.count(), 1);
    }

    #[test]
    fn request_covers_requesting_movies_and_shows_but_not_auto_approval() {
        let request = Permissions::from_iter([Permission::Request]);

        assert!(request.allow(Permission::RequestMovie));
        assert!(request.allow(Permission::RequestTv));
        assert!(!request.allow(Permission::AutoApproveMovie));
        assert!(!request.allow(Permission::ViewRequests));
    }

    #[test]
    fn auto_approve_covers_its_movie_and_show_variants() {
        let auto_approve = Permissions::from_iter([Permission::AutoApprove]);

        assert!(auto_approve.allow(Permission::AutoApproveMovie));
        assert!(auto_approve.allow(Permission::AutoApproveTv));
        assert!(!auto_approve.allow(Permission::RequestMovie));
    }

    #[test]
    fn a_variant_does_not_cover_its_parent_or_its_sibling() {
        let movies = Permissions::from_iter([Permission::RequestMovie]);

        assert!(movies.allow(Permission::RequestMovie));
        assert!(!movies.allow(Permission::Request));
        assert!(!movies.allow(Permission::RequestTv));
    }

    #[test]
    fn admin_passes_every_check() {
        for &needed in Permission::values() {
            assert!(Permissions::ADMIN.allow(needed), "{needed:?}");
        }
    }

    #[test]
    fn manage_libraries_does_not_cover_managing_users() {
        let libraries = Permissions::from_iter([Permission::ManageLibraries]);

        assert!(libraries.allow(Permission::ManageLibraries));
        assert!(!libraries.allow(Permission::ManageUsers));
    }

    #[test]
    fn permissions_list_each_held_one_in_value_order() {
        let held = Permissions::from_iter([
            Permission::ViewRequests,
            Permission::Request,
            Permission::ViewRequests,
        ]);

        assert_eq!(
            held.iter().collect::<Vec<_>>(),
            [Permission::Request, Permission::ViewRequests]
        );
        assert_eq!(held, Permissions::NEW_USER);
    }

    #[tokio::test]
    async fn upgrading_gives_admins_admin_and_members_request_and_view_requests() {
        let data_dir = tempfile::tempdir().unwrap();
        let file = data_dir.path().join(crate::db::FILE_NAME);
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&file)
            .create_if_missing(true);
        // A raw connection, not `Database::write`: `Database::open` would run every migration.
        let mut before = sqlx::ConnectOptions::connect(&options).await.unwrap();
        // The last migration with Roles.
        sqlx::migrate!().run_to(9, &mut before).await.unwrap();
        sqlx::query(
            "INSERT INTO users (username, display_username, role, created_at, updated_at)
             VALUES ('ada', 'Ada', 'admin', 0, 0), ('grace', 'Grace', 'member', 0, 0)",
        )
        .execute(&mut before)
        .await
        .unwrap();
        sqlx::Connection::close(before).await.unwrap();

        let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
        let users = Users::new(db, Registry::new()).list().await.unwrap();

        let permissions: Vec<_> = users
            .iter()
            .map(|user| (user.name.as_str(), user.permissions))
            .collect();
        assert_eq!(
            permissions,
            [
                ("Ada", Permissions::ADMIN),
                ("Grace", Permissions::NEW_USER)
            ]
        );
    }

    /// [`Users`] on a fresh database in `data_dir`, whose Authenticator is `local-auth`.
    async fn users_with_local_auth(data_dir: &std::path::Path) -> Users {
        let db = Arc::new(Database::open(data_dir).await.unwrap());
        let plugins = Registry::new();
        plugins
            .enable(Arc::new(nama_plugin_local_auth::LocalAuth))
            .unwrap();
        Users::new(db, plugins)
    }

    const PASSWORD: &str = "correct horse battery staple";

    fn laptop() -> NewDevice {
        NewDevice {
            name: "Laptop".to_owned(),
            ip_address: "192.0.2.7".parse().unwrap(),
            user_agent: Some("TestAgent/1.0".to_owned()),
        }
    }

    /// How the Device `device` signed in: the name of its Sign-in Method, its IP address and user
    /// agent, and when it last proved who it is.
    async fn sign_in_details(
        users: &Users,
        device: i64,
    ) -> (Option<String>, String, Option<String>, i64) {
        let row = sqlx::query!(
            r#"SELECT m.name AS "method?", d.ip_address, d.user_agent, d.authenticated_at
               FROM devices d LEFT JOIN sign_in_methods m ON m.id = d.sign_in_method_id
               WHERE d.id = ?"#,
            device
        )
        .fetch_one(users.db.reader())
        .await
        .unwrap();
        (
            row.method,
            row.ip_address,
            row.user_agent,
            row.authenticated_at,
        )
    }

    /// Gives the User `user` a Sign-in Method that isn't a password, and returns its id.
    async fn add_other_sign_in_method(users: &Users, user: i64) -> i64 {
        users
            .db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query_scalar!(
                        r#"INSERT INTO sign_in_methods
                               (user_id, provider_id, subject, credential, name, created_at,
                                updated_at)
                           VALUES (?, 'test', 'their-id', '', 'Test', 0, 0)
                           RETURNING id AS "id!""#,
                        user
                    )
                    .fetch_one(&mut **tx)
                    .await
                })
            })
            .await
            .unwrap()
    }

    async fn delete_sign_in_method(users: &Users, method: i64) -> Result<(), sqlx::Error> {
        users
            .db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!("DELETE FROM sign_in_methods WHERE id = ?", method)
                        .execute(&mut **tx)
                        .await
                })
            })
            .await?;
        Ok(())
    }

    async fn expire_devices(users: &Users, at: i64) {
        users
            .db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!("UPDATE devices SET expires_at = ?", at)
                        .execute(&mut **tx)
                        .await
                })
            })
            .await
            .unwrap();
    }

    async fn password_method(users: &Users, user: i64) -> i64 {
        sqlx::query_scalar!(
            r#"SELECT id AS "id!" FROM sign_in_methods WHERE user_id = ? AND subject IS NULL"#,
            user
        )
        .fetch_one(users.db.reader())
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn a_device_signed_in_with_a_password_records_its_method_ip_user_agent_and_when() {
        let data_dir = tempfile::tempdir().unwrap();
        let users = users_with_local_auth(data_dir.path()).await;
        users.create("Ada", PASSWORD).await.unwrap();
        let before = now();

        let signed_in = users.sign_in("ada", PASSWORD, laptop()).await.unwrap();

        let (method, ip, user_agent, authenticated_at) =
            sign_in_details(&users, signed_in.caller.device.id).await;
        assert_eq!(method.as_deref(), Some("Password"));
        assert_eq!(ip, "192.0.2.7");
        assert_eq!(user_agent.as_deref(), Some("TestAgent/1.0"));
        assert!(
            (before..=now()).contains(&authenticated_at),
            "{authenticated_at}"
        );
    }

    #[tokio::test]
    async fn a_device_signed_in_by_a_sign_in_code_records_no_method_and_never_authenticated() {
        let data_dir = tempfile::tempdir().unwrap();
        let users = users_with_local_auth(data_dir.path()).await;
        let ada = create_user(&users.db, "Ada").await.unwrap();

        let signed_in = users.sign_in_as(ada, None, laptop()).await.unwrap();

        let (method, ip, _, authenticated_at) =
            sign_in_details(&users, signed_in.caller.device.id).await;
        assert_eq!(
            (method, ip.as_str(), authenticated_at),
            (None, "192.0.2.7", 0)
        );
    }

    #[tokio::test]
    async fn a_device_past_its_expiry_is_no_session_however_recently_it_was_seen() {
        let data_dir = tempfile::tempdir().unwrap();
        let (users, _) = signed_in_device(data_dir.path()).await;
        expire_devices(&users, now() - 1).await;

        let session = users.session(TOKEN).await;

        assert!(session.unwrap().is_none());
    }

    #[tokio::test]
    async fn deleting_a_sign_in_method_signs_out_only_the_devices_that_used_it() {
        let data_dir = tempfile::tempdir().unwrap();
        let users = users_with_local_auth(data_dir.path()).await;
        let ada = users.create("Ada", PASSWORD).await.unwrap();
        add_other_sign_in_method(&users, ada).await;
        let by_password = users.sign_in("Ada", PASSWORD, laptop()).await.unwrap();
        let by_code = users
            .sign_in_as(by_password.caller.user.clone(), None, laptop())
            .await
            .unwrap();

        let password = password_method(&users, ada).await;
        delete_sign_in_method(&users, password).await.unwrap();

        assert!(!users.still_signed_in(by_password.caller.device.id).await);
        assert!(users.still_signed_in(by_code.caller.device.id).await);
    }

    #[tokio::test]
    async fn a_users_last_sign_in_method_cant_be_deleted() {
        let data_dir = tempfile::tempdir().unwrap();
        let users = users_with_local_auth(data_dir.path()).await;
        let ada = users.create("Ada", PASSWORD).await.unwrap();

        let password = password_method(&users, ada).await;
        let deleted = delete_sign_in_method(&users, password).await;

        assert!(deleted.is_err(), "the last Sign-in Method was deleted");
    }

    #[tokio::test]
    async fn resetting_the_password_of_a_user_without_one_adds_it_and_keeps_their_other_method() {
        let data_dir = tempfile::tempdir().unwrap();
        let users = users_with_local_auth(data_dir.path()).await;
        let ada = create_user(&users.db, "Ada").await.unwrap().id;
        add_other_sign_in_method(&users, ada).await;
        assert!(
            users.user_named("Ada").await.unwrap().is_some(),
            "not found by name"
        );

        users.reset_password(&admin(), ada, PASSWORD).await.unwrap();

        users.sign_in("Ada", PASSWORD, laptop()).await.unwrap();
        let password = password_method(&users, ada).await;
        // Refused if it were Ada's last Sign-in Method.
        delete_sign_in_method(&users, password).await.unwrap();
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
