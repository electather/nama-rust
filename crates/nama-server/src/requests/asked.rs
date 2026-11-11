//! What a Request and a Followed Show share, as things Users ask for together and a Request
//! manager decides: written once for both, by "Requester rules once" in `nama-server/AGENTS.md`.

use std::collections::HashMap;

use nama_proto::nama::v1::Requester;
use sqlx::SqliteConnection;

/// Which kind of thing Users asked for.
#[derive(Debug, Clone, Copy)]
pub(super) enum Asked {
    Request,
    FollowedShow,
}

// sqlx checks a query at compile time from its literal text, so a table or column name can't be
// a bound parameter: `$query` is a local macro that takes them as literals, expanded once per kind.
macro_rules! per_kind {
    ($asked:expr, $query:ident) => {
        match $asked {
            Asked::Request => $query!("requesters", "request_id"),
            Asked::FollowedShow => $query!("followed_show_requesters", "followed_show_id"),
        }
    };
}

/// A Requester of something asked for, as both kinds' Requester tables store one.
struct AskedRequester {
    asked_id: i64,
    user_id: i64,
    name: String,
    requested_at: i64,
}

/// The Requesters of the `asked` with the ID `id`, or of every one of that kind if `None`, by its
/// ID, first asker first.
pub(super) async fn requesters(
    conn: &mut SqliteConnection,
    asked: Asked,
    id: Option<i64>,
) -> Result<HashMap<i64, Vec<Requester>>, sqlx::Error> {
    macro_rules! requesters {
        ($requesters:literal, $asked_id:literal) => {
            sqlx::query_as!(
                AskedRequester,
                "SELECT r."
                    + $asked_id
                    + r#" AS "asked_id!", r.user_id,
                        users.display_username AS name, r.requested_at
                 FROM "#
                    + $requesters
                    + " AS r
                 JOIN users ON users.id = r.user_id
                 WHERE ?1 IS NULL OR r."
                    + $asked_id
                    + " = ?1
                 ORDER BY r.requested_at, r.rowid",
                id
            )
            .fetch_all(&mut *conn)
            .await?
        };
    }
    // ponytail: every Requester of this kind, even when listing one User's; filter by them in SQL
    // if the list gets big.
    let rows = per_kind!(asked, requesters);
    let mut requesters: HashMap<i64, Vec<Requester>> = HashMap::new();
    for row in rows {
        requesters.entry(row.asked_id).or_default().push(Requester {
            user_id: row.user_id,
            name: row.name,
            requested_at: row.requested_at,
            ..Default::default()
        });
    }
    Ok(requesters)
}

/// Whether `viewer`, as [`super::visible_to`] gives one, may see something `requesters` asked
/// for: everyone's if `None`, else only what that User is among the Requesters of.
pub(super) fn may_see(viewer: Option<i64>, requesters: &[Requester]) -> bool {
    viewer.is_none_or(|user| requesters.iter().any(|requester| requester.user_id == user))
}

/// Adds `user` as a Requester of the `asked` with the ID `id`, as of `requested_at`, which orders
/// them among its Requesters. Returns whether they joined: `false` if they already were one.
pub(super) async fn join(
    conn: &mut SqliteConnection,
    asked: Asked,
    id: i64,
    user: i64,
    requested_at: i64,
) -> Result<bool, sqlx::Error> {
    macro_rules! join {
        ($requesters:literal, $asked_id:literal) => {
            sqlx::query!(
                "INSERT INTO "
                    + $requesters
                    + " ("
                    + $asked_id
                    + ", user_id, requested_at) VALUES (?, ?, ?) ON CONFLICT DO NOTHING",
                id,
                user,
                requested_at
            )
            .execute(&mut *conn)
            .await?
        };
    }
    Ok(per_kind!(asked, join).rows_affected() > 0)
}
