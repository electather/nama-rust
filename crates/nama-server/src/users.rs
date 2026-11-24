//! Users, Devices and their sessions: Kernel state (ADR 0014).
//!
//! [`Users`] is the only way in. It checks names before anything is written and has the enabled
//! Authenticator turn passwords into credentials. Nothing secret is stored: see `AGENTS.md`.

use std::{net::IpAddr, ops::RangeInclusive, sync::Arc};

use buffa::Enumeration as _;
use nama_proto::nama::v1::Permission;
use sha2::{Digest, Sha256};
use sqlx::{Sqlite, SqliteConnection, Transaction};
use tokio::sync::broadcast;
use tracing::Instrument as _;

use crate::{
    db::{BoxFuture, Database, now},
    link::{Linking, ServerLink},
    plugins::ExtensionPoints,
};

mod accounts;
mod codes;
mod device_names;
mod expired;
mod home_layout;
mod inbox;
mod invites;
mod managing;
mod names;
mod notification_preferences;
mod passwords;
mod permissions;
mod personal_settings;
mod proved;
mod rate_limits;
mod seeing;
mod sessions;
mod sign_in;
mod sign_in_codes;

pub use codes::SetupCode;
use codes::new_token;
pub(crate) use codes::random_secret;
pub(crate) use home_layout::{AskedHomeLayout, UserHomeLayout};
pub(crate) use inbox::{
    list as list_inbox, mark_all_read as mark_all_inbox_read, mark_read as mark_inbox_read,
    store as store_in_inbox, unread as count_unread_in_inbox,
};
pub(crate) use invites::Invite;
pub(crate) use managing::remote_cap;
use managing::{default_permissions, ensure_may_change, ensure_may_grant};
use names::checked_name;
pub(crate) use names::name_key;
pub(crate) use notification_preferences::{
    every_turned_off as every_notification_turned_off, of as notification_preferences_of,
    set as set_notification_preference,
};
use passwords::{Credential, set_password};
pub(crate) use permissions::Permissions;
pub(crate) use personal_settings::{
    delete as delete_personal_settings, delete_plugin as delete_plugins_personal_settings,
    get as personal_settings_of, recipients as personal_settings_recipients,
    replace as replace_personal_settings, split as split_personal_settings, text as personal_text,
    value as personal_value,
};
pub(crate) use proved::Proved;
use proved::proved_user;
pub(crate) use rate_limits::Blocked;
pub(crate) use seeing::{is_requester, may_see};
pub(crate) use sessions::{
    cleared_session_cookie, identify_caller, in_session_cookie, session_cookie,
};
pub(crate) use sign_in::{Error as SignInError, Polled, Proof, SignIn};
pub(crate) use sign_in_codes::{Delivery, Error as SignInCodeError};

/// How many characters a User's or Device's name may have, once trimmed.
pub(crate) const NAME_LENGTH: RangeInclusive<usize> = 1..=64;

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

/// How many sign-outs may happen before a waiter in [`Users::signed_out`] has seen them. One that
/// falls further behind checks its Device in the database instead.
const SIGN_OUTS_BUFFERED: usize = 64;

/// How many Permission changes may happen before a waiter in [`Users::permissions_changed`] has
/// seen them. One that falls further behind reads its User's Permissions instead.
const PERMISSION_CHANGES_BUFFERED: usize = 64;

/// The Server's Users, Devices and sessions, kept in its database.
///
/// Cloning a `Users` gives another handle to the same ones.
#[derive(Debug, Clone)]
pub struct Users {
    db: Arc<Database>,
    plugins: ExtensionPoints,
    /// Whether a User's Account is a way in, and creating Users from Accounts.
    linking: Linking,
    sign_outs: broadcast::Sender<i64>,
    permission_changes: broadcast::Sender<i64>,
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

/// The Device a Sign-in Code was approved from, and its User, by id: both are looked up again when
/// the code signs a Device in.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ApprovingDevice {
    pub(crate) user_id: i64,
    pub(crate) device_id: i64,
}

impl From<&Caller> for ApprovingDevice {
    fn from(caller: &Caller) -> Self {
        Self {
            user_id: caller.user.id,
            device_id: caller.device.id,
        }
    }
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
    /// What it is called, checked before it signs in: by `SignIn` when it gives a password or
    /// sets up, and when its Sign-in Code was started.
    pub name: String,
    /// The client IP address it signs in from.
    pub ip_address: IpAddr,
    /// The `User-Agent` it sent, if any.
    pub user_agent: Option<String>,
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
    /// A name holds a character that can't be seen, so it could pass for another.
    #[error(
        "{what} can't hold invisible characters such as tabs, line breaks or zero-width spaces"
    )]
    NameCharacter {
        /// The request field it is about, as clients name it.
        field: &'static str,
        /// What the name belongs to, as the message says it: "a User's name".
        what: &'static str,
        /// The first such character.
        character: char,
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
    /// The User has no Device with this ID.
    #[error("that User has no such Device")]
    NoSuchDevice,
    /// No working Invite has the token or ID given in the request field `field`.
    #[error("that Invite was used, revoked or has expired")]
    InviteNotFound {
        /// The request field it is about, as clients name it.
        field: &'static str,
    },
    /// An Invite can't grant this Permission: it's `PERMISSION_ADMIN`, or its creator lacks it.
    #[error("an Invite can't grant {}", .0.proto_name())]
    NotGrantable(Permission),
    /// The User who approved a Sign-in Code was deleted before it signed its Device in.
    #[error("the User who approved this code no longer exists")]
    ApprovingUserDeleted,
    /// The Device a Sign-in Code was approved from signed out, or expired, before the code signed
    /// its Device in.
    #[error("the Device that approved this code has since signed out")]
    ApprovingDeviceSignedOut,
    /// The change would leave the Server with no Admin who can sign in with a password, without
    /// the Hub (ADR 0092).
    #[error("the Server must keep at least one Admin who can sign in with a password")]
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
    /// The User's Account is their only way in, so they may not Unlink it (ADR 0014).
    #[error(
        "you sign in only with your Account, so Unlinking it would leave you no way in; ask an \
         Admin to give you a password first"
    )]
    LastSignInMethod,
    /// No enabled Plugin links the Server.
    #[error("no enabled Plugin links the Server")]
    NoLinkingPlugin,
    /// The Plugin that links the Server couldn't tell, as with its service unreachable.
    #[error("the Plugin that links the Server failed; its page says why")]
    LinkingFailed,
    /// Creating a User from an Account panicked.
    #[error("creating a User from an Account panicked: {0}")]
    Panicked(tokio::task::JoinError),
    /// The database failed.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl Users {
    /// The Users kept in `db`, whose passwords the Authenticator enabled in `plugins` checks.
    ///
    /// A Server builds one and clones it: a second has its own channels, so it neither hears nor
    /// tells the first about sign-outs and Permission changes (`users/AGENTS.md`).
    #[must_use]
    pub fn new(db: Arc<Database>, plugins: ExtensionPoints) -> Self {
        Self {
            db,
            linking: Linking::new(plugins.clone(), None),
            plugins,
            sign_outs: broadcast::channel(SIGN_OUTS_BUFFERED).0,
            permission_changes: broadcast::channel(PERMISSION_CHANGES_BUFFERED).0,
        }
    }

    /// These Users, whose Server Links with the Hub through `server`, its own Link (ADR 0105).
    #[must_use]
    pub(crate) fn linking_through(self, server: Option<Arc<ServerLink>>) -> Self {
        Self {
            linking: Linking::new(self.plugins.clone(), server),
            ..self
        }
    }

    /// The Server's one [`Linking`], which startup hands out.
    pub(crate) const fn linking(&self) -> &Linking {
        &self.linking
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

    /// Creates the first Admin, named `name` with `password`, and signs `device`, whose name has
    /// been checked, in as them, unless the Server already has a User. Checking the Setup Code is
    /// the caller's job, as is checking for Users before the slow hashing.
    pub(crate) async fn create_first_admin(
        &self,
        name: &str,
        password: &str,
        device: NewDevice,
    ) -> Result<SignedIn, Error> {
        let name = checked_name("name", "a User's name", name)?;
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

    /// Creates a User named `name` with `password` and the default Permissions, as `by` asks, who
    /// can then sign in, and returns their ID. Without `PERMISSION_ADMIN`, `by` may not create one
    /// while the default set holds it, as they may not grant it.
    pub(crate) async fn create(&self, by: &User, name: &str, password: &str) -> Result<i64, Error> {
        let name = checked_name("name", "a User's name", name)?;
        let credential = self.hash_password("password", password).await?;
        self.insert_created(by, name, Some(credential)).await
    }

    /// Stores a User named `name`, checked, with the default Permissions and `credential` as their
    /// password if any, as `by` asks.
    async fn insert_created(
        &self,
        by: &User,
        name: String,
        credential: Option<Credential>,
    ) -> Result<i64, Error> {
        let by = by.permissions;
        self.db
            .write(move |tx| {
                Box::pin(
                    async move { insert_with_defaults(tx, by, &name, credential.as_ref()).await },
                )
            })
            .await
    }

    /// Every User, oldest first.
    pub(crate) async fn list(&self) -> Result<Vec<User>, sqlx::Error> {
        every_user(&self.db).await
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

    /// Signs the Device `device` of the User `user` out, as [`Users::sign_out`] does, as `by` asks.
    /// Fails with [`Error::NoSuchDevice`] if it isn't theirs, or as [`ensure_may_change`] does.
    pub(crate) async fn revoke(&self, by: &User, user: i64, device: i64) -> Result<(), Error> {
        let by = by.clone();
        self.write_signing_out(move |tx| {
            Box::pin(async move {
                ensure_may_change(tx, &by, user).await?;
                sqlx::query_scalar!(
                    r#"DELETE FROM devices WHERE id = ? AND user_id = ? RETURNING id AS "id!""#,
                    device,
                    user
                )
                .fetch_optional(&mut **tx)
                .await?
                .map(|id| vec![id])
                .ok_or(Error::NoSuchDevice)
            })
        })
        .await
        .map(drop)
    }

    /// Signs `device`, whose name has been checked, in as whoever `proved` says. A User deleted
    /// since their password was checked is refused as a wrong name or password; a Sign-in Code is
    /// refused if its approving User was deleted or its approving Device signed out. The only place
    /// sessions are issued.
    pub(crate) async fn sign_in_as(
        &self,
        proved: Proved,
        device: NewDevice,
    ) -> Result<SignedIn, Error> {
        let token = new_token()?;
        let token_hash = hash_token(&token);
        let caller = self
            .db
            .write(move |tx| {
                Box::pin(async move {
                    let (user, method) = proved_user(tx, proved).await?;
                    let device = insert_device(tx, user.id, method, device, &token_hash).await?;
                    Ok::<_, Error>(Caller { user, device })
                })
            })
            .await?;
        Ok(SignedIn { caller, token })
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
        .map(drop)
    }

    /// Runs `write`, which deletes Devices and returns their ids, tells the waiters in
    /// [`Users::signed_out`] about them and answers how many. Both run in a task of their own: a
    /// queued write goes through even if the caller stops waiting, and its waiters must hear of it.
    async fn write_signing_out<E, F>(&self, write: F) -> Result<usize, E>
    where
        E: From<sqlx::Error> + Send + 'static,
        F: for<'t> FnOnce(&'t mut Transaction<'_, Sqlite>) -> BoxFuture<'t, Result<Vec<i64>, E>>
            + Send
            + 'static,
    {
        let users = self.clone();
        let written = tokio::spawn(
            async move {
                let devices = users.db.write(write).await?;
                for &device in &devices {
                    // Fails only when nobody is waiting, which is fine.
                    let _: Result<usize, _> = users.sign_outs.send(device);
                }
                Ok(devices.len())
            }
            .in_current_span(),
        );
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
        let users = self.clone();
        until_heard(sign_outs, device, async move || {
            users.still_signed_in(device).await
        })
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
}

/// What the database keeps of a session token.
fn hash_token(token: &str) -> [u8; 32] {
    Sha256::digest(token).into()
}

/// Every User, oldest first, for a task that holds no [`Users`], such as `crate::events`.
pub(crate) async fn every_user(db: &Database) -> Result<Vec<User>, sqlx::Error> {
    sqlx::query_as!(
        User,
        r#"SELECT id AS "id!", display_username AS name,
                  permissions AS "permissions: Permissions", created_at
           FROM users ORDER BY id"#
    )
    .fetch_all(db.reader())
    .await
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

/// Stores a User named `name`, which has been checked, with the default Permissions, which a User
/// holding `by` must be allowed to grant, and `credential` as their password if any.
async fn insert_with_defaults(
    conn: &mut SqliteConnection,
    by: Permissions,
    name: &str,
    credential: Option<&Credential>,
) -> Result<i64, Error> {
    // Read in the write's transaction, so the default can't gain ADMIN after the check.
    let permissions = default_permissions(conn).await?;
    ensure_may_grant(by, permissions)?;
    let user = insert_user(conn, name, permissions).await?;
    if let Some(credential) = credential {
        set_password(conn, user.id, credential).await?;
    }
    Ok(user.id)
}

/// Stores a User named `name`, which has been checked, with `permissions` and the default cap for
/// remote playback, unless another has the name ignoring case.
async fn insert_user(
    conn: &mut SqliteConnection,
    name: &str,
    permissions: Permissions,
) -> Result<User, Error> {
    let username = name_key(name);
    let created_at = now();
    let id = sqlx::query_scalar!(
        r#"INSERT INTO users (username, display_username, permissions, created_at, updated_at,
                               remote_playback_limit)
           VALUES (?, ?, ?, ?, ?, (SELECT remote_playback_limit FROM default_permissions))
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
/// Method `method`, or by a Sign-in Code or an Account, and the hash of its session token.
async fn insert_device(
    conn: &mut SqliteConnection,
    user: i64,
    method: Option<i64>,
    device: NewDevice,
    token_hash: &[u8; 32],
) -> Result<Device, sqlx::Error> {
    let now = now();
    let expires_at = now + DEVICE_LIFETIME;
    // A Device signed in by a Sign-in Code or an Account never proved who it is itself (ADR 0014):
    // whoever approved on the other Device, or on the Hub, may not be the one holding it.
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

/// Resolves once `heard` names `id`, or `unchanged` says whatever `id` names has changed. It asks
/// `unchanged` first, and again whenever `heard` falls so far behind that it may have skipped `id`.
async fn until_heard(
    mut heard: broadcast::Receiver<i64>,
    id: i64,
    unchanged: impl AsyncFn() -> bool,
) {
    let mut look = true;
    loop {
        if look && !unchanged().await {
            return;
        }
        look = match heard.recv().await {
            Ok(named) if named == id => return,
            Ok(_) => false,
            Err(broadcast::error::RecvError::Lagged(_)) => true,
            Err(broadcast::error::RecvError::Closed) => return,
        };
    }
}

#[cfg(test)]
#[path = "users/users_tests.rs"]
mod tests;
