//! The Users a Device is signed in as, and which of them it acts as now (ADR 0106): signing a
//! Device in, adding a User to one already signed in, listing its Users and switching between them.
//!
//! The `device_users_leave` trigger signs a Device out when its last User leaves, and makes it act
//! as none when the one it acts as leaves, so every way a User leaves agrees.

use sqlx::SqliteConnection;

use super::{
    Caller, DEVICE_LIFETIME, Device, Error, NewDevice, Permissions, Proved, SignedIn, User, Users,
    codes::new_token, hash_token, proved::proved_user,
};
use crate::db::now;

impl Users {
    /// Signs `device`, whose name has been checked, in as whoever `proved` says. A Device already
    /// signed in, holding the session token `session`, has the User added to it instead and keeps
    /// its name and token (ADR 0106). A User deleted since their password was checked is refused as
    /// a wrong name or password; a Sign-in Code is refused if its approving User was deleted or
    /// signed out of its approving Device. The only place sessions are issued.
    pub(crate) async fn sign_in_as(
        &self,
        proved: Proved,
        device: NewDevice,
        session: Option<&str>,
    ) -> Result<SignedIn, Error> {
        let held = session.map(str::to_owned);
        let new = new_token()?;
        self.write_telling(move |tx| {
            Box::pin(async move { sign_in_device(tx, proved, device, held, new).await })
        })
        .await
    }

    /// The Users signed in to the Device `device`, oldest User first.
    pub(crate) async fn users_on(&self, device: i64) -> Result<Vec<User>, sqlx::Error> {
        sqlx::query_as!(
            User,
            r#"SELECT u.id AS "id!", u.display_username AS name,
                      u.permissions AS "permissions: Permissions", u.created_at
               FROM device_users du JOIN users u ON u.id = du.user_id
               WHERE du.device_id = ? ORDER BY u.id"#,
            device
        )
        .fetch_all(self.db.reader())
        .await
    }

    /// The stored hash of the Switch PIN of the User `user`, if they set one, failing with
    /// [`Error::NoSuchUser`] unless they are signed in to the Device `device`.
    pub(super) async fn switch_pin_on(
        &self,
        device: i64,
        user: i64,
    ) -> Result<Option<String>, Error> {
        sqlx::query_scalar!(
            r#"SELECT p.credential AS "credential?"
               FROM device_users du LEFT JOIN switch_pins p ON p.user_id = du.user_id
               WHERE du.device_id = ? AND du.user_id = ?"#,
            device,
            user
        )
        .fetch_optional(self.db.reader())
        .await?
        .ok_or(Error::NoSuchUser)
    }

    /// Signs the User `user` out of the Device `device`, as too many wrong Switch PINs do, leaving
    /// its other Users signed in.
    pub(super) async fn sign_out_of(&self, device: i64, user: i64) -> Result<(), Error> {
        self.write_signing_out(move |tx| {
            Box::pin(async move {
                sqlx::query!(
                    "DELETE FROM device_users WHERE device_id = ? AND user_id = ?",
                    device,
                    user
                )
                .execute(&mut **tx)
                .await?;
                Ok(vec![device])
            })
        })
        .await
        .map(drop)
    }

    /// Makes the Device `device` act as the User `user`, one of those signed in to it, failing with
    /// [`Error::NoSuchUser`] for any other. Clears `user`'s Recent Authentication on it, and ends
    /// what it opened as the User it acted as, as [`Users::signed_out`] says. Only
    /// [`SignIn::switch`](super::SignIn::switch) calls it, once the Switch PIN checks out.
    pub(super) async fn switch(&self, device: i64, user: i64) -> Result<(), Error> {
        self.write_signing_out(move |tx| Box::pin(async move { switch(tx, device, user).await }))
            .await
            .map(drop)
    }
}

/// Makes the Device `device` act as the User `user`, as [`Users::switch`] does, returning its id
/// for the waiters on what it opened as the User it acted as.
async fn switch(conn: &mut SqliteConnection, device: i64, user: i64) -> Result<Vec<i64>, Error> {
    // Switching never brings Recent Authentication with it (ADR 0106).
    let signed_in = sqlx::query!(
        "UPDATE device_users SET authenticated_at = 0 WHERE device_id = ? AND user_id = ?",
        device,
        user
    )
    .execute(&mut *conn)
    .await?;
    if signed_in.rows_affected() == 0 {
        return Err(Error::NoSuchUser);
    }
    let now = now();
    sqlx::query!(
        "UPDATE devices SET user_id = ?, updated_at = ? WHERE id = ?",
        user,
        now,
        device
    )
    .execute(conn)
    .await?;
    Ok(vec![device])
}

/// Signs `device` in as whoever `proved` says, as [`Users::sign_in_as`] does: adds them to the
/// Device holding the session token `held`, if one does, or else stores `device` with the session
/// token `new`. Answers the id of the Device it switched, if any, and the session.
async fn sign_in_device(
    conn: &mut SqliteConnection,
    proved: Proved,
    device: NewDevice,
    held: Option<String>,
    new: String,
) -> Result<(Vec<i64>, SignedIn), Error> {
    let (user, method, authenticated_at) = proved_user(conn, proved).await?;
    if let Some(token) = held
        && let Some(joined) =
            add_to_device(conn, &hash_token(&token), user.id, method, authenticated_at).await?
    {
        // It no longer acts as whoever it did, so what it opened as them ends.
        let switched = vec![joined.id];
        let caller = Caller {
            user,
            device: joined,
        };
        return Ok((switched, SignedIn { caller, token }));
    }
    let inserted = insert_device(
        conn,
        user.id,
        method,
        authenticated_at,
        device,
        &hash_token(&new),
    )
    .await?;
    let caller = Caller {
        user,
        device: inserted,
    };
    Ok((Vec::new(), SignedIn { caller, token: new }))
}

/// Stores `device`, whose name has been checked, signed in as and acting as the User `user`, who
/// proved it with the Sign-in Method `method`, or by a Sign-in Code or an Account, last proving who
/// holds it at `authenticated_at` (0 for never), and the hash of its session token.
#[expect(
    clippy::too_many_arguments,
    reason = "each is its own fact about the sign-in, and a struct would only rename them"
)]
pub(super) async fn insert_device(
    conn: &mut SqliteConnection,
    user: i64,
    method: Option<i64>,
    authenticated_at: i64,
    device: NewDevice,
    token_hash: &[u8; 32],
) -> Result<Device, sqlx::Error> {
    let now = now();
    let expires_at = now + DEVICE_LIFETIME;
    let token_hash = token_hash.as_slice();
    let ip_address = device.ip_address.to_string();
    let id = sqlx::query_scalar!(
        r#"INSERT INTO devices
               (user_id, name, token_hash, ip_address, user_agent, created_at, updated_at,
                last_seen_at, expires_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
           RETURNING id AS "id!""#,
        user,
        device.name,
        token_hash,
        ip_address,
        device.user_agent,
        now,
        now,
        now,
        expires_at,
    )
    .fetch_one(&mut *conn)
    .await?;
    sign_in_to(conn, id, user, method, authenticated_at).await?;
    Ok(Device {
        id,
        name: device.name,
        created_at: now,
        last_seen_at: now,
        authenticated_at,
    })
}

/// Adds the User `user`, who proved it with `method` at `authenticated_at` as for
/// [`insert_device`], to the Device whose session token hashes to `token_hash`, and makes it act as
/// them, or `None` if no unexpired Device has that token. A User already signed in to it signs in
/// again.
pub(super) async fn add_to_device(
    conn: &mut SqliteConnection,
    token_hash: &[u8; 32],
    user: i64,
    method: Option<i64>,
    authenticated_at: i64,
) -> Result<Option<Device>, sqlx::Error> {
    let now = now();
    let token_hash = token_hash.as_slice();
    let found = sqlx::query!(
        r#"UPDATE devices SET user_id = ?, updated_at = ?
           WHERE token_hash = ? AND expires_at > ?
           RETURNING id AS "id!", name, created_at, last_seen_at"#,
        user,
        now,
        token_hash,
        now,
    )
    .fetch_optional(&mut *conn)
    .await?;
    let Some(found) = found else {
        return Ok(None);
    };
    sign_in_to(conn, found.id, user, method, authenticated_at).await?;
    Ok(Some(Device {
        id: found.id,
        name: found.name,
        created_at: found.created_at,
        last_seen_at: found.last_seen_at,
        authenticated_at,
    }))
}

/// Records that the User `user` signed in to the Device `device` with `method`, last proving who
/// holds it at `authenticated_at`, as for [`insert_device`].
async fn sign_in_to(
    conn: &mut SqliteConnection,
    device: i64,
    user: i64,
    method: Option<i64>,
    authenticated_at: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO device_users (device_id, user_id, sign_in_method_id, authenticated_at)
         VALUES (?, ?, ?, ?)
         ON CONFLICT (device_id, user_id) DO UPDATE
             SET sign_in_method_id = excluded.sign_in_method_id,
                 authenticated_at = excluded.authenticated_at",
        device,
        user,
        method,
        authenticated_at,
    )
    .execute(conn)
    .await?;
    Ok(())
}
