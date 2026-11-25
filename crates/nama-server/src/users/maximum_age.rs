//! Each User's Maximum Age, which an Admin sets (`users.max_age` and `users.allow_unrated`).

use std::collections::HashMap;

use nama_proto::nama::v1::Permission;
use sqlx::SqliteConnection;

use super::{Error, User, Users, ensure_may_change};
use crate::{
    content_ratings::MaximumAge,
    db::{Database, now},
};

/// The Maximum Age of the User `user`: none for one who isn't there, who sees nothing anyway.
pub(crate) async fn maximum_age(db: &Database, user: i64) -> Result<MaximumAge, sqlx::Error> {
    let stored = sqlx::query!(
        r#"SELECT max_age, allow_unrated AS "allow_unrated: bool" FROM users WHERE id = ?"#,
        user
    )
    .fetch_optional(db.reader())
    .await?;
    Ok(stored.map_or_else(MaximumAge::default, |row| MaximumAge {
        age: row.max_age.and_then(|age| u32::try_from(age).ok()),
        allow_unrated: row.allow_unrated,
    }))
}

impl Users {
    /// Each User's Maximum Age, by User id, for those with one.
    pub(crate) async fn maximum_ages(&self) -> Result<HashMap<i64, MaximumAge>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT id AS "id!", max_age AS "max_age!", allow_unrated AS "allow_unrated: bool"
               FROM users WHERE max_age IS NOT NULL"#
        )
        .fetch_all(self.db.reader())
        .await?;
        Ok((rows.into_iter())
            .map(|row| {
                let age = u32::try_from(row.max_age).ok();
                let allow_unrated = row.allow_unrated;
                (row.id, MaximumAge { age, allow_unrated })
            })
            .collect())
    }

    /// Gives the User `user` the Maximum Age `max`, as `by` asks. Without `PERMISSION_ADMIN`, `by`
    /// may change neither an Admin's nor their own, so a User can't lift theirs. It applies from
    /// their next request, which reads it afresh.
    pub(crate) async fn set_maximum_age(
        &self,
        by: &User,
        user: i64,
        max: MaximumAge,
    ) -> Result<(), Error> {
        if !by.permissions.allow(Permission::Admin) && by.id == user {
            return Err(Error::AdminRequired);
        }
        let by = by.clone();
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    ensure_may_change(tx, &by, user).await?;
                    update(tx, user, max).await
                })
            })
            .await
    }
}

async fn update(conn: &mut SqliteConnection, user: i64, max: MaximumAge) -> Result<(), Error> {
    let now = now();
    let updated = sqlx::query!(
        "UPDATE users SET max_age = ?, allow_unrated = ?, updated_at = ? WHERE id = ?",
        max.age,
        max.allow_unrated,
        now,
        user
    )
    .execute(conn)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(Error::NoSuchUser);
    }
    Ok(())
}
