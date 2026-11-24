//! Setting and checking Users' passwords through the enabled Authenticator.

use std::sync::{Arc, LazyLock};

use nama_plugin_api::{
    PluginError, PluginId,
    auth::{Authenticator, PasswordError},
};
use sqlx::SqliteConnection;
use tokio::sync::Semaphore;
use tracing::Span;

use super::{
    Caller, Error, Permissions, User, Users, managing::ensure_may_change, name_key,
    rate_limits::Attempt, user_exists,
};
use crate::{
    db::now,
    plugins::{Failure, FailureKind, ProviderError},
};

/// What a User is shown for their password among their Sign-in Methods.
const PASSWORD_METHOD_NAME: &str = "Password";

/// How many Authenticator calls on a password may run at once. An argon2id hash holds 19 MiB, and
/// sign-ins with unknown names cost one too, so without a cap a burst of them grows memory without
/// bound on the 1 GiB machines #725 targets. 4 is about 78 MiB at peak. Calls past it wait their
/// turn.
const CONCURRENT_HASHES: usize = 4;

static HASHING: LazyLock<Semaphore> = LazyLock::new(|| Semaphore::new(CONCURRENT_HASHES));

/// A credential string the Kernel keeps for a User, and the Plugin whose Authenticator made it,
/// which is the only one that can check it.
pub(super) struct Credential {
    plugin: PluginId,
    value: String,
}

/// A User's password Sign-in Method, made by a Plugin that is enabled.
pub(super) struct StoredPassword {
    id: i64,
    credential: Credential,
}

impl Users {
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
        .map(drop)
    }

    /// Checks that `password` is `user`'s, refusing it with [`Error::WrongPassword`] if not.
    ///
    /// `_admitted` is a guess `SignInLimits::admit` let through: see "Guessable requests go through
    /// `SignIn`" in `crates/nama-server/AGENTS.md`. ponytail: a fresh `SignInLimits` can still mint
    /// one, and it isn't checked against `user`.
    pub(crate) async fn confirm_password(
        &self,
        _admitted: &Attempt,
        user: &User,
        password: &str,
    ) -> Result<(), Error> {
        // Users are never renamed, so their name still finds them.
        let found = self.user_named(&user.name).await?;
        let stored = found.as_ref().and_then(|(_, stored)| stored.as_ref());
        match self.verify_password(&Arc::from(password), stored).await? {
            Some(_) => Ok(()),
            None => Err(Error::WrongPassword),
        }
    }

    /// Gives the Device `device` Recent Authentication, failing with [`Error::SignedOut`] if it
    /// was signed out meanwhile.
    pub(crate) async fn record_authentication(&self, device: i64) -> Result<(), Error> {
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
        .map(drop)
    }

    /// The User named `name`, ignoring case, and the id of their password Sign-in Method, if
    /// `password` is theirs. An unknown name takes as long as a wrong password, and is refused the
    /// same way.
    ///
    /// `_admitted` is a guess `SignInLimits::admit` let through: see "Guessable requests go through
    /// `SignIn`" in `crates/nama-server/AGENTS.md`. ponytail: a fresh `SignInLimits` can still mint
    /// one, and it isn't checked against `name`.
    pub(crate) async fn verify(
        &self,
        _admitted: &Attempt,
        name: &str,
        password: &str,
    ) -> Result<(User, i64), Error> {
        let found = self.user_named(name).await?;
        let stored = found.as_ref().and_then(|(_, stored)| stored.as_ref());
        let method = self.verify_password(&Arc::from(password), stored).await?;
        match (found, method) {
            (Some((user, _)), Some(method)) => Ok((user, method)),
            _ => Err(Error::WrongNameOrPassword),
        }
    }

    /// Has the first enabled Authenticator turn `password`, given as the request field `field`,
    /// into a credential string.
    pub(super) async fn hash_password(
        &self,
        field: &'static str,
        password: &str,
    ) -> Result<Credential, Error> {
        let password: Arc<str> = Arc::from(password);
        let hashed = self
            .plugins
            .ask_first(|authenticator: Arc<dyn Authenticator>| {
                let password = Arc::clone(&password);
                async move {
                    blocking(move || authenticator.hash_password(&password))
                        .await
                        .unwrap_or_else(|panicked| Err(PasswordError::Failed(panicked)))
                }
            })
            .await
            .ok_or(Error::NoAuthenticator)?;
        match hashed.result {
            Ok(value) => Ok(Credential {
                plugin: hashed.plugin,
                value,
            }),
            Err(Failure {
                kind: Hashing::Rejected,
                message,
            }) => Err(Error::PasswordRejected { field, message }),
            Err(Failure { message, .. }) => Err(Error::Authenticator(message)),
        }
    }

    /// The User whose name is `name` ignoring case, whether or not they have a password, with their
    /// password if an enabled Authenticator made it.
    pub(super) async fn user_named(
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
        let authenticators = self.plugins.providers::<dyn Authenticator>();
        let plugin = authenticators
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
            let credential: Arc<str> = Arc::from(stored.credential.value.as_str());
            let verified = self
                .plugins
                .ask(&stored.credential.plugin, |authenticator| {
                    verify_where_blocking_is_fine(authenticator, password, &credential)
                })
                .await;
            match verified.map(|answer| answer.result) {
                None => {}
                Some(Ok(verified)) => return Ok(verified.then_some(stored.id)),
                Some(Err(failure)) => return Err(Error::Authenticator(failure.message)),
            }
        }
        self.verify_unknown_user(password).await?;
        Ok(None)
    }

    /// Has the first enabled Authenticator spend as long on `password` as checking a real User's
    /// would, for a name no User has.
    async fn verify_unknown_user(&self, password: &Arc<str>) -> Result<(), Error> {
        self.plugins
            .ask_first(|authenticator: Arc<dyn Authenticator>| {
                let password = Arc::clone(password);
                blocking(move || authenticator.verify_unknown_user(&password))
            })
            .await
            .ok_or(Error::NoAuthenticator)?
            .result
            .map_err(|failure| Error::Authenticator(failure.message))
    }
}

/// Has `authenticator` check `password` against `credential`, which is meant to be slow, where
/// blocking is fine.
async fn verify_where_blocking_is_fine(
    authenticator: Arc<dyn Authenticator>,
    password: &Arc<str>,
    credential: &Arc<str>,
) -> Result<bool, PluginError> {
    let (password, credential) = (Arc::clone(password), Arc::clone(credential));
    blocking(move || authenticator.verify_password(&password, &credential)).await?
}

/// Runs `work`, an Authenticator's on a password, which is meant to be slow, where blocking is
/// fine, once [`HASHING`] has a permit for it.
// A panic in it fails the call and leaves the Plugin enabled: see "Plugin code runs contained" in
// `crates/nama-server/src/plugins/AGENTS.md`.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, PluginError> {
    // `spawn_blocking` doesn't carry the span over, and the one naming a Built-in must reach its
    // lines (ADR 0061).
    let span = Span::current();
    let permit = HASHING.acquire().await?;
    // The thread holds the permit, not this future: a client hanging up drops the future, but the
    // hash runs on.
    Ok(tokio::task::spawn_blocking(move || {
        let _permit = permit;
        span.in_scope(work)
    })
    .await?)
}

/// What an Authenticator's failure to hash a password means for the User.
enum Hashing {
    /// The password breaks its rules, which the User is told.
    Rejected,
    Failed,
}

/// A failure unless the password was [`Hashing::Rejected`], which is the Authenticator's answer.
impl FailureKind for Hashing {
    fn is_failure(&self) -> bool {
        matches!(self, Self::Failed)
    }
}

impl ProviderError<Hashing> for PasswordError {
    fn failure(self) -> Failure<Hashing> {
        match self {
            Self::Rejected(message) => Failure {
                kind: Hashing::Rejected,
                message,
            },
            Self::Failed(error) => Failure {
                kind: Hashing::Failed,
                message: error.to_string(),
            },
        }
    }
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
pub(super) async fn set_password(
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

#[cfg(test)]
#[path = "passwords_tests.rs"]
mod tests;
