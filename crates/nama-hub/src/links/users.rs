//! Linking a Linked Server's Users to Accounts (ADR 0008): the code a User's Server shows, entering
//! it on the Hub, and Unlinking from either side.

use std::{collections::HashMap, fmt, str::FromStr};

use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use super::{
    CODE_LIFETIME_MINS, Error, LinkedServer, LinkedServerId, Links, kept_name, may_link,
    random_code, shown_code, typed_code,
};
use crate::accounts::{AccountId, AccountSession};

/// The id of a User's Link to an Account.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LinkId(Uuid);

impl fmt::Display for LinkId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for LinkId {
    type Err = uuid::Error;

    fn from_str(id: &str) -> Result<Self, Self::Err> {
        id.parse().map(Self)
    }
}

/// One of an Account's Linked Users, as its list shows them.
#[derive(Debug)]
pub(crate) struct ListedUser {
    pub(crate) id: LinkId,
    pub(crate) name: String,
    pub(crate) server_name: String,
    /// In seconds since the Unix epoch.
    pub(crate) linked_at: i64,
}

impl Links {
    /// Starts Linking `server`'s User `user_id`, named `name`, and answers the code they show,
    /// such as `BDFH-JKLM`. It replaces the code they waited on, and forgets every expired one.
    pub(crate) async fn start_user(
        &self,
        server: &LinkedServer,
        user_id: i64,
        name: &str,
    ) -> Result<String, Error> {
        let name = kept_name(name);
        let code = random_code()?;
        let mut tx = self.db.begin().await?;
        sqlx::query!("DELETE FROM user_link_codes WHERE expires_at < now()")
            .execute(&mut *tx)
            .await?;
        let linked = sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT FROM user_links
             WHERE linked_server_id = $1 AND server_user_id = $2) AS "linked!""#,
            server.id.0,
            user_id,
        )
        .fetch_one(&mut *tx)
        .await?;
        if linked {
            return Err(Error::UserLinked);
        }
        sqlx::query!(
            "INSERT INTO user_link_codes
             (linked_server_id, server_user_id, user_name, user_code, expires_at)
             VALUES ($1, $2, $3, $4, now() + make_interval(mins => $5))
             ON CONFLICT (linked_server_id, server_user_id) DO UPDATE
             SET user_name = excluded.user_name, user_code = excluded.user_code,
             expires_at = excluded.expires_at",
            server.id.0,
            user_id,
            name,
            code,
            CODE_LIFETIME_MINS,
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(shown_code(&code))
    }

    /// Links the User waiting on `code` to `session`'s Account, if it has Recent Authentication
    /// and a proven email.
    pub(crate) async fn approve_user(
        &self,
        session: &AccountSession,
        code: &str,
    ) -> Result<ListedUser, Error> {
        may_link(session)?;
        let mut tx = self.db.begin().await?;
        let waiting = sqlx::query!(
            "DELETE FROM user_link_codes WHERE user_code = $1 AND expires_at > now()
             RETURNING linked_server_id, server_user_id, user_name",
            typed_code(code),
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(Error::CodeNotFound)?;
        // A refusal rolls back the code's delete above, so it works again once the clashing User
        // is Unlinked.
        let user = insert_link(
            &mut tx,
            session.account.id,
            LinkedServerId(waiting.linked_server_id),
            waiting.server_user_id,
            waiting.user_name,
        )
        .await?;
        tx.commit().await?;
        Ok(user)
    }

    /// The email of the Account `server`'s User `user_id` is Linked to, if they are.
    pub(crate) async fn user_account(
        &self,
        server: &LinkedServer,
        user_id: i64,
    ) -> Result<Option<String>, sqlx::Error> {
        match self.linked_account(server, user_id).await? {
            Some(account) => self.accounts.email(account).await,
            None => Ok(None),
        }
    }

    /// The Account `server`'s User `user_id` is Linked to, if they are.
    pub(crate) async fn linked_account(
        &self,
        server: &LinkedServer,
        user_id: i64,
    ) -> Result<Option<AccountId>, sqlx::Error> {
        let account = sqlx::query_scalar!(
            "SELECT account_id FROM user_links WHERE linked_server_id = $1 AND server_user_id = $2",
            server.id.0,
            user_id,
        )
        .fetch_optional(&self.db)
        .await?;
        Ok(account.map(AccountId))
    }

    /// Each item of `batch` whose User, as `user_id` names them on `server`, is Linked, paired
    /// with their Account, in `batch`'s order, in one query (`links/AGENTS.md`). Call it before
    /// opening a transaction: inside one, it would hold a connection while asking the pool for
    /// another.
    pub(crate) async fn linked_accounts<T>(
        &self,
        server: &LinkedServer,
        batch: Vec<T>,
        user_id: impl Fn(&T) -> i64,
    ) -> Result<Vec<(AccountId, T)>, sqlx::Error> {
        let user_ids: Vec<i64> = batch.iter().map(&user_id).collect();
        let linked: HashMap<i64, AccountId> = sqlx::query!(
            "SELECT server_user_id, account_id FROM user_links
             WHERE linked_server_id = $1 AND server_user_id = ANY($2)",
            server.id.0,
            &user_ids,
        )
        .fetch_all(&self.db)
        .await?
        .into_iter()
        .map(|link| (link.server_user_id, AccountId(link.account_id)))
        .collect();
        Ok((batch.into_iter())
            .filter_map(|item| Some((*linked.get(&user_id(&item))?, item)))
            .collect())
    }

    /// Unlinks `server`'s User `user_id`, and forgets the code they wait on and any Account asked
    /// to accept them.
    pub(crate) async fn unlink_user(
        &self,
        server: &LinkedServer,
        user_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "WITH codes AS (DELETE FROM user_link_codes
             WHERE linked_server_id = $1 AND server_user_id = $2),
             invites AS (DELETE FROM user_invites
             WHERE linked_server_id = $1 AND server_user_id = $2)
             DELETE FROM user_links WHERE linked_server_id = $1 AND server_user_id = $2",
            server.id.0,
            user_id,
        )
        .execute(&self.db)
        .await?;
        Ok(())
    }

    /// `account`'s Linked Users, oldest first.
    pub(crate) async fn list_users(
        &self,
        account: AccountId,
    ) -> Result<Vec<ListedUser>, sqlx::Error> {
        let users = sqlx::query!(
            r#"SELECT user_links.id, user_links.user_name, linked_servers.name AS server_name,
             extract(epoch FROM user_links.created_at)::bigint AS "linked_at!"
             FROM user_links JOIN linked_servers ON linked_servers.id = user_links.linked_server_id
             WHERE user_links.account_id = $1 ORDER BY user_links.created_at, user_links.id"#,
            account.0,
        )
        .fetch_all(&self.db)
        .await?;
        Ok(users
            .into_iter()
            .map(|user| ListedUser {
                id: LinkId(user.id),
                name: user.user_name,
                server_name: user.server_name,
                linked_at: user.linked_at,
            })
            .collect())
    }

    /// Unlinks `account`'s User whose Link is `link`. Another Account's Link stays.
    pub(crate) async fn unlink_linked_user(
        &self,
        account: AccountId,
        link: LinkId,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "DELETE FROM user_links WHERE id = $1 AND account_id = $2",
            link.0,
            account.0,
        )
        .execute(&self.db)
        .await?;
        Ok(())
    }
}

/// Links `server`'s User `user_id`, named `name`, to `account`, failing with [`Error::UserLinked`]
/// if they are Linked already, and with [`Error::AccountHasUserOnServer`] if `account` has another
/// User there.
pub(super) async fn insert_link(
    tx: &mut Transaction<'_, Postgres>,
    account: AccountId,
    server: LinkedServerId,
    user_id: i64,
    name: String,
) -> Result<ListedUser, Error> {
    let linked = sqlx::query!(
        r#"INSERT INTO user_links (account_id, linked_server_id, server_user_id, user_name)
             VALUES ($1, $2, $3, $4) ON CONFLICT (linked_server_id, server_user_id) DO NOTHING
             RETURNING id, extract(epoch FROM created_at)::bigint AS "linked_at!",
             (SELECT name FROM linked_servers WHERE id = $2) AS "server_name!""#,
        account.0,
        server.0,
        user_id,
        name,
    )
    .fetch_optional(&mut **tx)
    .await
    .map_err(|err| {
        let constraint = err.as_database_error().and_then(|err| err.constraint());
        // Named by the migration `one_user_per_account_and_server`.
        if constraint == Some("user_links_one_per_account_and_server") {
            Error::AccountHasUserOnServer
        } else {
            err.into()
        }
    })?
    .ok_or(Error::UserLinked)?;
    Ok(ListedUser {
        id: LinkId(linked.id),
        name,
        server_name: linked.server_name,
        linked_at: linked.linked_at,
    })
}
