//! Each User's Maximum Age, which an Admin sets (`users.max_age` and `users.allow_unrated`).

use std::{collections::HashMap, sync::Arc};

use nama_proto::nama::v1::Permission;
use sqlx::SqliteConnection;

use super::{Error, User, Users, ensure_may_change, until_heard};
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

    /// The Maximum Age of the User `user` now, and a future that resolves once it changes, so
    /// whatever they opened under it can end, to be opened again under the new one. It resolves at
    /// once if theirs no longer reads the same, or can't be read.
    pub(crate) async fn maximum_age_watched(
        &self,
        user: i64,
    ) -> Result<(MaximumAge, impl Future<Output = ()> + Send + use<>), sqlx::Error> {
        // Listening before reading means no change falls between the two.
        let changes = self.maximum_age_changes.subscribe();
        let seen = maximum_age(&self.db, user).await?;
        let db = Arc::clone(&self.db);
        let set_again = until_heard(changes, user, async move || {
            let read = maximum_age(&db, user).await;
            read.inspect_err(|err| tracing::error!("could not look up a User's Maximum Age: {err}"))
                .is_ok_and(|read| read == seen)
        });
        Ok((seen, set_again))
    }

    /// Gives the User `user` the Maximum Age `max`, as `by` asks. Without `PERMISSION_ADMIN`, `by`
    /// may change neither an Admin's nor their own, so a User can't lift theirs. It applies from
    /// their next request, which reads it afresh. Once the change commits,
    /// [`Self::maximum_age_watched`]'s future resolves for them.
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
        let changes = self.maximum_age_changes.clone();
        let tell_waiters = move || {
            // Fails only when nobody is waiting, which is fine.
            let _: Result<usize, _> = changes.send(user);
        };
        self.db
            .write_then(
                move |tx| {
                    Box::pin(async move {
                        ensure_may_change(tx, &by, user).await?;
                        update(tx, user, max).await
                    })
                },
                tell_waiters,
            )
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
