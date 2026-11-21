//! Changing other Users: their Permissions, the default ones, and deleting them.

use nama_proto::nama::v1::Permission;
use sqlx::SqliteConnection;

use super::{Error, Permissions, User, Users, invites::delete_invites_by, until_heard};
use crate::db::now;

impl Users {
    /// The Permissions [`Users::create`] gives a new User.
    pub(crate) async fn default_permissions(&self) -> Result<Permissions, sqlx::Error> {
        default_permissions(&mut *self.db.reader().acquire().await?).await
    }

    /// Makes `permissions` the ones new Users get, as `by` asks. Without `PERMISSION_ADMIN`, `by`
    /// may not put it in them. Existing Users keep theirs.
    pub(crate) async fn set_default_permissions(
        &self,
        by: &User,
        permissions: Permissions,
    ) -> Result<(), Error> {
        ensure_may_grant(by.permissions, permissions)?;
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!(
                        "UPDATE default_permissions SET permissions = ?",
                        permissions.0
                    )
                    .execute(&mut **tx)
                    .await?;
                    Ok(())
                })
            })
            .await
    }

    /// Gives the User `user` exactly `permissions`, as `by` asks, unless that leaves no User with
    /// `PERMISSION_ADMIN` or [`ensure_may_change`] refuses. Without `PERMISSION_ADMIN`, `by` may
    /// neither grant it nor change their own Permissions. Their Devices stay signed in, and the
    /// Permissions apply from their next request, which reads them afresh. Once the change commits,
    /// [`Users::permissions_changed`] resolves for them.
    pub(crate) async fn set_permissions(
        &self,
        by: &User,
        user: i64,
        permissions: Permissions,
    ) -> Result<(), Error> {
        ensure_may_grant(by.permissions, permissions)?;
        if !by.permissions.allow(Permission::Admin) && by.id == user {
            return Err(Error::AdminRequired);
        }
        let by = by.clone();
        let changes = self.permission_changes.clone();
        let tell_waiters = move || {
            // Fails only when nobody is waiting, which is fine.
            let _: Result<usize, _> = changes.send(user);
        };
        self.db
            .write_then(
                move |tx| {
                    Box::pin(async move {
                        ensure_may_change(tx, &by, user).await?;
                        update_permissions(tx, user, permissions).await
                    })
                },
                tell_waiters,
            )
            .await
    }

    /// Resolves once the User `user`'s Permissions are set, so whatever they opened while holding
    /// `permissions` can end, to be opened again under the new ones. It resolves at once if they
    /// no longer hold exactly `permissions`, are gone, or that can't be found out.
    pub(crate) fn permissions_changed(
        &self,
        user: i64,
        permissions: Permissions,
    ) -> impl Future<Output = ()> + Send + 'static {
        // Listening before looking `user` up means no change falls between the two.
        let changes = self.permission_changes.subscribe();
        let users = self.clone();
        until_heard(changes, user, async move || {
            users.still_hold(user, permissions).await
        })
    }

    /// Whether the User `user` still holds exactly `permissions`. If that can't be found out, they
    /// are taken not to, so what was opened under them ends rather than outlive a change.
    async fn still_hold(&self, user: i64, permissions: Permissions) -> bool {
        let held = sqlx::query_scalar!(
            r#"SELECT permissions AS "permissions: Permissions" FROM users WHERE id = ?"#,
            user
        )
        .fetch_optional(self.db.reader())
        .await;
        let held = held.unwrap_or_else(|err| {
            tracing::error!("could not look up a User's Permissions: {err}");
            None
        });
        held == Some(permissions)
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
        .map(drop)
    }
}

pub(super) async fn default_permissions(
    conn: &mut SqliteConnection,
) -> Result<Permissions, sqlx::Error> {
    sqlx::query_scalar!(
        r#"SELECT permissions AS "permissions: Permissions" FROM default_permissions"#
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
    if !permissions.allow(Permission::ManageUsers) {
        delete_invites_by(conn, user).await?;
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
/// `user`, who has it. Only an Admin changes an Admin, or signs their Devices out, as in Seerr, so
/// `PERMISSION_MANAGE_USERS` can't become `PERMISSION_ADMIN` by taking over an Admin. Read in the
/// write's transaction, so `user` can't be made an Admin between the check and the change.
pub(super) async fn ensure_may_change(
    conn: &mut SqliteConnection,
    by: &User,
    user: i64,
) -> Result<(), Error> {
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

/// Fails with [`Error::AdminRequired`] if a User holding `by`, without `PERMISSION_ADMIN`, would
/// give it out in `permissions`.
pub(super) fn ensure_may_grant(by: Permissions, permissions: Permissions) -> Result<(), Error> {
    if !by.allow(Permission::Admin) && permissions.allow(Permission::Admin) {
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
