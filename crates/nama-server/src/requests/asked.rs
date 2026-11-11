//! What a Request and a Followed Show share, as things Users ask for together and a Request
//! manager decides: written once for both, by "Requester rules once" in `nama-server/AGENTS.md`.

use std::{collections::HashMap, fmt};

use nama_proto::nama::v1::{RequestState, Requester};
use sqlx::SqliteConnection;

/// Which kind of thing Users asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Asked {
    Request,
    FollowedShow,
}

impl fmt::Display for Asked {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Request => "Request",
            Self::FollowedShow => "Followed Show",
        })
    }
}

// sqlx checks a query at compile time from its literal text, so a table or column name can't be
// a bound parameter: `$query` is a local macro that takes them as literals, expanded once per kind.
macro_rules! per_kind {
    ($asked:expr, $query:ident) => {
        match $asked {
            Asked::Request => $query!("requests", "requesters", "request_id"),
            Asked::FollowedShow => {
                $query!(
                    "followed_shows",
                    "followed_show_requesters",
                    "followed_show_id"
                )
            }
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
        ($table:literal, $requesters:literal, $asked_id:literal) => {
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
    viewer.is_none_or(|user| is_requester(user, requesters))
}

/// Whether the User `user` is among `requesters`.
pub(super) fn is_requester(user: i64, requesters: &[Requester]) -> bool {
    requesters.iter().any(|requester| requester.user_id == user)
}

/// Takes `user` off the Requesters of the `asked` with the ID `id`. Returns whether they were its
/// last Requester, in which case it is removed too.
pub(super) async fn leave(
    conn: &mut SqliteConnection,
    asked: Asked,
    id: i64,
    user: i64,
) -> Result<bool, sqlx::Error> {
    macro_rules! leave {
        ($table:literal, $requesters:literal, $asked_id:literal) => {{
            sqlx::query!(
                "DELETE FROM " + $requesters + " WHERE " + $asked_id + " = ? AND user_id = ?",
                id,
                user
            )
            .execute(&mut *conn)
            .await?;
            sqlx::query!(
                "DELETE FROM "
                    + $table
                    + " WHERE id = ?1 AND NOT EXISTS (SELECT 1 FROM "
                    + $requesters
                    + " WHERE "
                    + $asked_id
                    + " = ?1)",
                id
            )
            .execute(&mut *conn)
            .await?
        }};
    }
    Ok(per_kind!(asked, leave).rows_affected() > 0)
}

/// Moves the `asked` with the ID `id` to `state` with `reason` if it is Pending. Returns whether
/// it was.
pub(super) async fn decide(
    conn: &mut SqliteConnection,
    asked: Asked,
    id: i64,
    state: RequestState,
    reason: Option<&str>,
) -> Result<bool, sqlx::Error> {
    let state = super::stored(state);
    macro_rules! decide {
        ($table:literal, $requesters:literal, $asked_id:literal) => {
            sqlx::query!(
                "UPDATE "
                    + $table
                    + " SET state = ?, decline_reason = ? WHERE id = ? AND state = 'pending'",
                state,
                reason,
                id
            )
            .execute(&mut *conn)
            .await?
        };
    }
    Ok(per_kind!(asked, decide).rows_affected() > 0)
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
        ($table:literal, $requesters:literal, $asked_id:literal) => {
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
