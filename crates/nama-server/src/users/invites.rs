//! Invites: single-use links with which one person creates their own User, with Permissions their
//! creator picked.

use nama_proto::nama::v1::Permission;
use sqlx::SqliteConnection;

use super::{
    Caller, Error, NewDevice, Permissions, SignedIn, User, Users, checked_name,
    codes::random_secret, hash_token, insert_device, insert_user, names::normalised, new_token,
    passwords::set_password, rate_limits::Attempt,
};
use crate::db::now;

/// How long, in seconds, an Invite works after it's made: 7 days.
const INVITE_LIFETIME: i64 = 7 * 24 * 60 * 60;

/// A pending Invite, as Users who manage Users see it.
#[derive(Debug)]
pub(crate) struct Invite {
    pub(crate) id: i64,
    /// Who it's for, such as "Mum", or empty.
    pub(crate) note: String,
    pub(crate) creator_name: String,
    /// What accepting it grants now: see [`granted`].
    pub(crate) permissions: Permissions,
    /// In seconds since the Unix epoch.
    pub(crate) created_at: i64,
    /// In seconds since the Unix epoch.
    pub(crate) expires_at: i64,
}

/// An Invite just made, and the token its link carries. Not `Debug`, so the token can't end up in
/// a log.
pub(crate) struct CreatedInvite {
    pub(crate) invite: Invite,
    pub(crate) token: String,
}

impl Users {
    /// Makes an Invite granting `permissions`, noted `note`, as `by` asks. Never grants
    /// `PERMISSION_ADMIN`, nor a Permission `by` lacks.
    pub(crate) async fn create_invite(
        &self,
        by: &User,
        note: &str,
        permissions: Permissions,
    ) -> Result<CreatedInvite, Error> {
        if let Some(refused) = permissions
            .iter()
            .find(|&p| p == Permission::Admin || !by.permissions.allow(p))
        {
            return Err(Error::NotGrantable(refused));
        }
        let note = match normalised(note) {
            empty if empty.is_empty() => empty,
            note => checked_name("note", "an Invite's note", &note)?,
        };
        let token = random_secret()?;
        let token_hash = hash_token(&token);
        let by = by.clone();
        let invite = self
            .db
            .write(move |tx| {
                Box::pin(async move { insert_invite(tx, by, note, permissions, &token_hash).await })
            })
            .await?;
        Ok(CreatedInvite { invite, token })
    }

    /// Every Invite that still works, oldest first.
    pub(crate) async fn invites(&self) -> Result<Vec<Invite>, sqlx::Error> {
        let now = now();
        let rows = sqlx::query!(
            r#"SELECT i.id AS "id!", i.note, i.permissions AS "permissions: Permissions",
                      u.display_username AS creator_name,
                      u.permissions AS "creator_permissions: Permissions",
                      i.created_at, i.expires_at
               FROM invites i JOIN users u ON u.id = i.created_by
               WHERE i.expires_at > ? ORDER BY i.id"#,
            now
        )
        .fetch_all(self.db.reader())
        .await?;
        Ok(rows
            .into_iter()
            .filter_map(|row| {
                Some(Invite {
                    id: row.id,
                    note: row.note,
                    creator_name: row.creator_name,
                    permissions: granted(row.permissions, row.creator_permissions)?,
                    created_at: row.created_at,
                    expires_at: row.expires_at,
                })
            })
            .collect())
    }

    /// Deletes the Invite `id`, so its link stops working.
    pub(crate) async fn revoke_invite(&self, id: i64) -> Result<(), Error> {
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query_scalar!(
                        r#"DELETE FROM invites WHERE id = ? RETURNING id AS "id!""#,
                        id
                    )
                    .fetch_optional(&mut **tx)
                    .await?
                    .map(drop)
                    .ok_or(Error::InviteNotFound { field: "invite_id" })
                })
            })
            .await
    }

    /// Fails with [`Error::InviteNotFound`] unless `token` is a working Invite's. Checked before
    /// the slow hashing in [`Users::accept_invite`], so a guess costs a read.
    ///
    /// `_admitted` is a guess `SignInLimits::admit` let through, as for [`Users::verify`].
    pub(super) async fn ensure_invite_works(
        &self,
        _admitted: &Attempt,
        token: &str,
    ) -> Result<(), Error> {
        let mut conn = self.db.reader().acquire().await?;
        working_invite(&mut conn, &hash_token(token)).await?;
        Ok(())
    }

    /// Creates a User named `name` with `password` from the Invite whose link carries `token`, and
    /// signs `device`, whose name has been checked, in as them. The Invite is used up only if all
    /// of that succeeds, so a taken name leaves it working.
    pub(super) async fn accept_invite(
        &self,
        token: &str,
        name: &str,
        password: &str,
        device: NewDevice,
    ) -> Result<SignedIn, Error> {
        let name = checked_name("name", "a User's name", name)?;
        let credential = self.hash_password("password", password).await?;
        let invite_hash = hash_token(token);
        let session = new_token()?;
        let session_hash = hash_token(&session);
        let caller = self
            .db
            .write(move |tx| {
                Box::pin(async move {
                    let (invite, permissions) = working_invite(tx, &invite_hash).await?;
                    let user = insert_user(tx, &name, permissions).await?;
                    let method = set_password(tx, user.id, &credential).await?;
                    sqlx::query!("DELETE FROM invites WHERE id = ?", invite)
                        .execute(&mut **tx)
                        .await?;
                    let device =
                        insert_device(tx, user.id, Some(method), device, &session_hash).await?;
                    Ok::<_, Error>(Caller { user, device })
                })
            })
            .await?;
        Ok(SignedIn {
            caller,
            token: session,
        })
    }
}

/// Deletes the Invites the User `creator` made, once they can't manage Users, so getting that
/// back doesn't bring them back to life.
pub(super) async fn delete_invites_by(
    conn: &mut SqliteConnection,
    creator: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM invites WHERE created_by = ?", creator)
        .execute(conn)
        .await?;
    Ok(())
}

/// Stores an Invite made by `by`, with the hash of its token, and forgets expired ones, which
/// nothing else deletes.
async fn insert_invite(
    conn: &mut SqliteConnection,
    by: User,
    note: String,
    permissions: Permissions,
    token_hash: &[u8; 32],
) -> Result<Invite, sqlx::Error> {
    let created_at = now();
    let expires_at = created_at + INVITE_LIFETIME;
    sqlx::query!("DELETE FROM invites WHERE expires_at <= ?", created_at)
        .execute(&mut *conn)
        .await?;
    let token_hash = token_hash.as_slice();
    let id = sqlx::query_scalar!(
        r#"INSERT INTO invites (token_hash, note, permissions, created_by, created_at, expires_at)
           VALUES (?, ?, ?, ?, ?, ?)
           RETURNING id AS "id!""#,
        token_hash,
        note,
        permissions.0,
        by.id,
        created_at,
        expires_at,
    )
    .fetch_one(conn)
    .await?;
    Ok(Invite {
        id,
        note,
        creator_name: by.name,
        permissions,
        created_at,
        expires_at,
    })
}

/// The id of the working Invite whose token hashes to `token_hash`, and what it grants now.
async fn working_invite(
    conn: &mut SqliteConnection,
    token_hash: &[u8; 32],
) -> Result<(i64, Permissions), Error> {
    let now = now();
    let token_hash = token_hash.as_slice();
    let row = sqlx::query!(
        r#"SELECT i.id AS "id!", i.permissions AS "permissions: Permissions",
                  u.permissions AS "creator_permissions: Permissions"
           FROM invites i JOIN users u ON u.id = i.created_by
           WHERE i.token_hash = ? AND i.expires_at > ?"#,
        token_hash,
        now
    )
    .fetch_optional(conn)
    .await?;
    row.and_then(|row| Some((row.id, granted(row.permissions, row.creator_permissions)?)))
        .ok_or(Error::InviteNotFound { field: "token" })
}

/// What an Invite picked to grant `picked` grants while its creator holds `creator`: those the
/// creator still has, or `None` once they can't manage Users, which ends it.
fn granted(picked: Permissions, creator: Permissions) -> Option<Permissions> {
    creator
        .allow(Permission::ManageUsers)
        .then(|| picked.iter().filter(|&p| creator.allow(p)).collect())
}
