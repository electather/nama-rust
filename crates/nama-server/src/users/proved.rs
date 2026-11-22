//! Who a Device signs in as, and how that was proved, checked again in the write that signs it in.

use sqlx::SqliteConnection;

use super::{ApprovingDevice, Error, Permissions, User};
use crate::db::now;

/// Who a Device signs in as, and how that was proved.
pub(crate) enum Proved {
    /// `user`, with their Sign-in Method `method`.
    Method { user: User, method: i64 },
    /// The User of the Device that approved a Sign-in Code.
    Code(ApprovingDevice),
}

/// The User `proved` signs a Device in as, and the Sign-in Method it proved, if any. Read in the
/// write that inserts the Device, so a delete or sign-out since the proof was checked can't slip
/// through.
pub(super) async fn proved_user(
    conn: &mut SqliteConnection,
    proved: Proved,
) -> Result<(User, Option<i64>), Error> {
    match proved {
        Proved::Method { user, method } => {
            ensure_method_exists(conn, user.id, method).await?;
            Ok((user, Some(method)))
        }
        Proved::Code(approving) => Ok((approving_user(conn, approving).await?, None)),
    }
}

/// Fails with [`Error::WrongNameOrPassword`] if the User `user` no longer has the Sign-in Method
/// `method`, such as because the User was deleted, which deletes it too.
async fn ensure_method_exists(
    conn: &mut SqliteConnection,
    user: i64,
    method: i64,
) -> Result<(), Error> {
    let exists = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM sign_in_methods WHERE id = ? AND user_id = ?)
               AS "exists!: bool""#,
        method,
        user
    )
    .fetch_one(conn)
    .await?;
    if exists {
        Ok(())
    } else {
        Err(Error::WrongNameOrPassword)
    }
}

/// The User of `approving`, failing if they were deleted or it signed out or expired.
async fn approving_user(
    conn: &mut SqliteConnection,
    approving: ApprovingDevice,
) -> Result<User, Error> {
    let now = now();
    let row = sqlx::query!(
        r#"SELECT u.id AS "id!", u.display_username AS name,
                  u.permissions AS "permissions: Permissions", u.created_at,
                  EXISTS (SELECT 1 FROM devices WHERE id = ? AND user_id = u.id AND expires_at > ?)
                      AS "signed_in!: bool"
           FROM users u WHERE u.id = ?"#,
        approving.device_id,
        now,
        approving.user_id
    )
    .fetch_optional(conn)
    .await?
    .ok_or(Error::ApprovingUserDeleted)?;
    if !row.signed_in {
        return Err(Error::ApprovingDeviceSignedOut);
    }
    Ok(User {
        id: row.id,
        name: row.name,
        permissions: row.permissions,
        created_at: row.created_at,
    })
}
