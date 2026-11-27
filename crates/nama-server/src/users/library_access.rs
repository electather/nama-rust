//! Which Libraries each User sees: every one, or a chosen list an Admin keeps (`user_libraries`).

use std::collections::{BTreeSet, HashMap};

use nama_proto::nama::v1::Permission;
use sqlx::SqliteConnection;

use super::{Error, Permissions, User, Users, ensure_may_change};
use crate::db::{Database, now};

/// Which Libraries a User sees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LibraryAccess {
    /// Every Library, those added later included.
    All,
    /// Only these, by id: a list that never grows by itself.
    Only(BTreeSet<i64>),
}

impl LibraryAccess {
    /// Whether it lets its User see the Library `library`.
    pub(crate) fn sees(&self, library: i64) -> bool {
        match self {
            Self::All => true,
            Self::Only(libraries) => libraries.contains(&library),
        }
    }
}

/// The Libraries the User `user` sees. One who may Manage Libraries sees every one, whatever an
/// Admin chose, since they can read and change any Library anyway; one who isn't there sees none.
pub(super) async fn library_access(db: &Database, user: i64) -> Result<LibraryAccess, sqlx::Error> {
    let mut conn = db.reader().acquire().await?;
    match access_row(&mut conn, user).await? {
        Some(row) if row.sees_every_library() => Ok(LibraryAccess::All),
        Some(_) => Ok(LibraryAccess::Only(chosen(&mut conn, user).await?)),
        None => Ok(LibraryAccess::Only(BTreeSet::new())),
    }
}

/// Fails with [`Error::ManageUsersNeedsEveryLibrary`] if the User `user` holds
/// `PERMISSION_MANAGE_USERS` but sees only a chosen list of Libraries ("Managing Users needs every
/// Library", `AGENTS.md`). Run after the write, in its transaction, so a refusal rolls it back.
pub(super) async fn ensure_a_user_manager_sees_every_library(
    conn: &mut SqliteConnection,
    user: i64,
) -> Result<(), Error> {
    match access_row(conn, user).await? {
        Some(row)
            if row.permissions.allow(Permission::ManageUsers) && !row.sees_every_library() =>
        {
            Err(Error::ManageUsersNeedsEveryLibrary)
        }
        _ => Ok(()),
    }
}

/// What `users` holds that decides whether a User sees every Library.
struct AccessRow {
    all_libraries: bool,
    permissions: Permissions,
}

impl AccessRow {
    fn sees_every_library(&self) -> bool {
        self.all_libraries || self.permissions.allow(Permission::ManageLibraries)
    }
}

/// What `users` holds of the User `user`, if they are there.
async fn access_row(
    conn: &mut SqliteConnection,
    user: i64,
) -> Result<Option<AccessRow>, sqlx::Error> {
    sqlx::query_as!(
        AccessRow,
        r#"SELECT all_libraries AS "all_libraries: bool",
                  permissions AS "permissions: Permissions"
           FROM users WHERE id = ?"#,
        user
    )
    .fetch_optional(conn)
    .await
}

impl Users {
    /// Each User's chosen list of Libraries, by User id, as an Admin set it: one who sees every
    /// Library has none.
    pub(crate) async fn chosen_libraries(
        &self,
    ) -> Result<HashMap<i64, BTreeSet<i64>>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT users.id AS "user!", user_libraries.library_id AS "library?"
               FROM users LEFT JOIN user_libraries ON user_libraries.user_id = users.id
               WHERE NOT users.all_libraries"#
        )
        .fetch_all(self.db.reader())
        .await?;
        let mut chosen: HashMap<i64, BTreeSet<i64>> = HashMap::new();
        for row in rows {
            let libraries = chosen.entry(row.user).or_default();
            libraries.extend(row.library);
        }
        Ok(chosen)
    }

    /// Gives the User `user` `access`, as `by` asks, leaving out an id no Library has, unless it's
    /// a chosen list for one holding `PERMISSION_MANAGE_USERS`
    /// ([`ensure_a_user_manager_sees_every_library`]). Without `PERMISSION_ADMIN`, `by` may change
    /// neither an Admin's nor their own, so a capped User can't lift theirs. It applies from their
    /// next request, which reads it afresh. Once the change commits,
    /// [`Self::reach_watched`]'s future resolves for them.
    pub(crate) async fn set_library_access(
        &self,
        by: &User,
        user: i64,
        access: LibraryAccess,
    ) -> Result<(), Error> {
        if !by.permissions.allow(Permission::Admin) && by.id == user {
            return Err(Error::AdminRequired);
        }
        let by = by.clone();
        let tell_waiters = self.tell_reach_changed(user);
        self.db
            .write_then(
                move |tx| {
                    Box::pin(async move {
                        ensure_may_change(tx, &by, user).await?;
                        replace(tx, user, &access).await
                    })
                },
                tell_waiters,
            )
            .await
    }
}

/// The Libraries `user_libraries` lists for the User `user`.
async fn chosen(conn: &mut SqliteConnection, user: i64) -> Result<BTreeSet<i64>, sqlx::Error> {
    let chosen = sqlx::query_scalar!(
        "SELECT library_id FROM user_libraries WHERE user_id = ?",
        user
    )
    .fetch_all(conn)
    .await?;
    Ok(chosen.into_iter().collect())
}

async fn replace(
    conn: &mut SqliteConnection,
    user: i64,
    access: &LibraryAccess,
) -> Result<(), Error> {
    let (all, chosen) = match access {
        LibraryAccess::All => (true, Vec::new()),
        LibraryAccess::Only(libraries) => (false, Vec::from_iter(libraries.iter().copied())),
    };
    let now = now();
    let updated = sqlx::query!(
        "UPDATE users SET all_libraries = ?, updated_at = ? WHERE id = ?",
        all,
        now,
        user
    )
    .execute(&mut *conn)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(Error::NoSuchUser);
    }
    sqlx::query!("DELETE FROM user_libraries WHERE user_id = ?", user)
        .execute(&mut *conn)
        .await?;
    let chosen = serde_json::Value::from(chosen).to_string();
    sqlx::query!(
        "INSERT INTO user_libraries (user_id, library_id)
         SELECT ?, id FROM libraries WHERE id IN (SELECT value FROM json_each(?))",
        user,
        chosen
    )
    .execute(&mut *conn)
    .await?;
    ensure_a_user_manager_sees_every_library(conn, user).await
}

#[cfg(test)]
#[path = "library_access_tests.rs"]
mod tests;
