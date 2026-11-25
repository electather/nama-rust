//! Each User's inbox: every Request notification the Kernel told them, kept whether or not a
//! Notifier delivered it, their newest [`KEPT`] at most.

use nama_proto::nama::v1::{
    InboxNotification, NotificationKind, PageResponse, Request, request::MovieOrSeason,
};

use crate::{
    db::{Database, now},
    page::Page,
};

/// How many notifications a User keeps; storing one more deletes their oldest.
// ponytail: a fixed ceiling; make it a Server setting if Users ask for a longer history.
const KEPT: i64 = 200;

/// Stores that `kind` happened to `request` in the inbox of each of `users`, who
/// `crate::events` picked, and returns each notification stored with its User. A User deleted
/// since is skipped.
pub(crate) async fn store(
    db: &Database,
    users: Vec<i64>,
    kind: NotificationKind,
    request: &Request,
) -> Result<Vec<(i64, InboxNotification)>, sqlx::Error> {
    let (title, season_number) = match &request.movie_or_season {
        Some(MovieOrSeason::Movie(movie)) => (movie.title.clone(), None),
        Some(MovieOrSeason::Season(season)) => {
            (season.show_title.clone(), Some(season.season_number))
        }
        None => (String::new(), None),
    };
    let (stored_kind, request, created_at) = (kind as i32, request.id, now());
    let notification = InboxNotification {
        kind: kind.into(),
        title: title.clone(),
        season_number,
        request_id: request,
        created_at,
        ..Default::default()
    };
    let stored = db.write(move |tx| {
        Box::pin(async move {
            let mut stored = Vec::new();
            for user in users {
                let id = sqlx::query_scalar!(
                    r#"INSERT INTO inbox (user_id, kind, title, season_number, request_id, created_at)
                     SELECT id, ?, ?, ?, ?, ? FROM users WHERE id = ? RETURNING id AS "id!""#,
                    stored_kind,
                    title,
                    season_number,
                    request,
                    created_at,
                    user
                )
                .fetch_optional(&mut **tx)
                .await?;
                // Deletes nothing until the User has more than `KEPT`, as the subquery is NULL.
                sqlx::query!(
                    "DELETE FROM inbox WHERE user_id = ? AND id <= (
                       SELECT id FROM inbox WHERE user_id = ? ORDER BY id DESC LIMIT 1 OFFSET ?)",
                    user,
                    user,
                    KEPT
                )
                .execute(&mut **tx)
                .await?;
                stored.extend(id.map(|id| (user, id)));
            }
            Ok::<_, sqlx::Error>(stored)
        })
    })
    .await?;
    let stored = stored.into_iter().map(|(user, id)| {
        let notification = InboxNotification {
            id,
            ..notification.clone()
        };
        (user, notification)
    });
    Ok(stored.collect())
}

/// The `page` of `user`'s notifications, newest first. Only the first page counts them all.
pub(crate) async fn list(
    db: &Database,
    user: i64,
    page: &Page<()>,
) -> Result<(Vec<InboxNotification>, PageResponse), sqlx::Error> {
    let before = page.after.as_ref().map_or(i64::MAX, |&((), id)| id);
    let limit = page.limit();
    // One snapshot, so the count and the page agree.
    let mut tx = db.reader().begin().await?;
    let rows = sqlx::query!(
        r#"SELECT id AS "id!", kind, title, season_number, request_id, created_at, read_at
           FROM inbox WHERE user_id = ? AND id < ? ORDER BY id DESC LIMIT ?"#,
        user,
        before,
        limit
    )
    .fetch_all(&mut *tx)
    .await?;
    let mut listed: Vec<_> = (rows.into_iter())
        .map(|row| InboxNotification {
            id: row.id,
            kind: i32::try_from(row.kind).unwrap_or_default().into(),
            title: row.title,
            season_number: row
                .season_number
                .and_then(|number| u32::try_from(number).ok()),
            request_id: row.request_id,
            created_at: row.created_at,
            read_at: row.read_at,
            ..Default::default()
        })
        .collect();
    let mut paging = page.finish(&mut listed, |notification| ((), notification.id));
    if page.is_first() {
        let total = sqlx::query_scalar!("SELECT COUNT(*) FROM inbox WHERE user_id = ?", user)
            .fetch_one(&mut *tx)
            .await?;
        paging.total_size = Some(i32::try_from(total).unwrap_or(i32::MAX));
    }
    Ok((listed, paging))
}

/// How many of `user`'s notifications are unread.
pub(crate) async fn unread(db: &Database, user: i64) -> Result<u32, sqlx::Error> {
    let count = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM inbox WHERE user_id = ? AND read_at IS NULL",
        user
    )
    .fetch_one(db.reader())
    .await?;
    Ok(u32::try_from(count).unwrap_or(u32::MAX))
}

/// Marks `user`'s notification `id` read, keeping when they first did. `false` if they have none
/// with that id, as another User's isn't theirs.
pub(crate) async fn mark_read(db: &Database, user: i64, id: i64) -> Result<bool, sqlx::Error> {
    let read_at = now();
    db.write(move |tx| {
        Box::pin(async move {
            let marked = sqlx::query!(
                "UPDATE inbox SET read_at = coalesce(read_at, ?) WHERE id = ? AND user_id = ?",
                read_at,
                id,
                user
            )
            .execute(&mut **tx)
            .await?;
            Ok(marked.rows_affected() == 1)
        })
    })
    .await
}

/// Marks every one of `user`'s unread notifications read.
pub(crate) async fn mark_all_read(db: &Database, user: i64) -> Result<(), sqlx::Error> {
    let read_at = now();
    db.write(move |tx| {
        Box::pin(async move {
            sqlx::query!(
                "UPDATE inbox SET read_at = ? WHERE user_id = ? AND read_at IS NULL",
                read_at,
                user
            )
            .execute(&mut **tx)
            .await?;
            Ok(())
        })
    })
    .await
}

#[cfg(test)]
#[path = "inbox_tests.rs"]
mod tests;
