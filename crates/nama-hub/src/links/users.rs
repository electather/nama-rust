//! Linking a Linked Server's Users to Accounts (ADR 0008): the code a User's Server shows, entering
//! it on the Hub, and Unlinking from either side.

use std::{fmt, str::FromStr};

use uuid::Uuid;

use super::{
    CODE_LIFETIME_MINS, Error, LinkedServer, Links, kept_name, may_link, random_code, shown_code,
    typed_code,
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
        // Nothing conflicts but a Link made between starting this code and entering it.
        let linked = sqlx::query!(
            r#"INSERT INTO user_links (account_id, linked_server_id, server_user_id, user_name)
             VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING
             RETURNING id, extract(epoch FROM created_at)::bigint AS "linked_at!",
             (SELECT name FROM linked_servers WHERE id = $2) AS "server_name!""#,
            session.account.id.0,
            waiting.linked_server_id,
            waiting.server_user_id,
            waiting.user_name,
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(Error::UserLinked)?;
        tx.commit().await?;
        Ok(ListedUser {
            id: LinkId(linked.id),
            name: waiting.user_name,
            server_name: linked.server_name,
            linked_at: linked.linked_at,
        })
    }

    /// The email of the Account `server`'s User `user_id` is Linked to, if they are.
    pub(crate) async fn user_account(
        &self,
        server: &LinkedServer,
        user_id: i64,
    ) -> Result<Option<String>, sqlx::Error> {
        let account = sqlx::query_scalar!(
            "SELECT account_id FROM user_links WHERE linked_server_id = $1 AND server_user_id = $2",
            server.id.0,
            user_id,
        )
        .fetch_optional(&self.db)
        .await?;
        match account {
            Some(account) => self.accounts.email(AccountId(account)).await,
            None => Ok(None),
        }
    }

    /// Unlinks `server`'s User `user_id`, and forgets the code they wait on.
    pub(crate) async fn unlink_user(
        &self,
        server: &LinkedServer,
        user_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "WITH codes AS (DELETE FROM user_link_codes
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
