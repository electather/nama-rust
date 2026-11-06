//! Users, Devices and their sessions: Kernel state (ADR 0014).
//!
//! [`Users`] is the only way in. It checks names before anything is written, has the enabled
//! Authenticator turn passwords into the credential string it stores, and keeps only the SHA-256
//! hash of each Device's session token, so the database never holds a password or a token.

use std::{fmt, ops::RangeInclusive, sync::Arc};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use nama_plugin_api::auth::{Authenticator, PasswordError};
use sha2::{Digest, Sha256};
use sqlx::SqliteConnection;

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

/// The Server's Users, Devices and sessions, kept in its database.
///
/// Cloning a `Users` gives another handle to the same ones.
#[derive(Debug, Clone)]
pub struct Users {
    db: Arc<Database>,
    plugins: Registry,
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

/// A Device just signed in, and the session token it was given. Not `Debug`, so the token can't
/// end up in a log.
pub(crate) struct SignedIn {
    pub(crate) caller: Caller,
    pub(crate) token: String,
}

/// A credential string the Kernel keeps for a User, and the Plugin whose Authenticator made it,
/// which is the only one that can check it.
struct Credential {
    plugin: String,
    string: String,
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
    pub const fn new(db: Arc<Database>, plugins: Registry) -> Self {
        Self { db, plugins }
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

    /// The User and Device whose session `token` is, if it is one.
    pub(crate) async fn caller(&self, token: &str) -> Result<Option<Caller>, sqlx::Error> {
        let token_hash = hash_token(token);
        let token_hash = token_hash.as_slice();
        let row = sqlx::query!(
            r#"SELECT d.id AS "device_id!", d.name AS device_name,
                      u.id AS "user_id!", u.name AS user_name, u.role AS "role: Role"
               FROM devices d JOIN users u ON u.id = d.user_id
               WHERE d.token_hash = ?"#,
            token_hash
        )
        .fetch_optional(self.db.reader())
        .await?;
        Ok(row.map(|row| Caller {
            user: User {
                id: row.user_id,
                name: row.user_name,
                role: row.role,
            },
            device: Device {
                id: row.device_id,
                name: row.device_name,
            },
        }))
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
            Ok(Ok(string)) => Ok(Credential {
                plugin: plugin.to_string(),
                string,
            }),
            Ok(Err(PasswordError::Rejected(reason))) => Err(Error::Invalid(reason)),
            Ok(Err(PasswordError::Failed(err))) => Err(Error::Authenticator(err.to_string())),
            Err(panicked) => Err(Error::Authenticator(panicked.to_string())),
        }
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
        credential.plugin,
        credential.string,
    )
    .execute(&mut *conn)
    .await?;
    let device = insert_device(conn, user.id, device_name, token_hash).await?;
    Ok(Caller { user, device })
}

/// Stores a User named `name`, which has been checked, unless another has it ignoring case.
async fn insert_user(conn: &mut SqliteConnection, name: &str, role: Role) -> Result<User, Error> {
    let name_key = name.to_lowercase();
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
