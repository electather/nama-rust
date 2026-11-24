//! Users an Admin created from an Account (ADR 0092): asking the Account to accept them, which
//! Links them to it.

use std::{fmt, str::FromStr};

use uuid::Uuid;

use super::{
    Error, LinkedServer, LinkedServerId, Links, ListedUser, kept_name, users::insert_link,
};
use crate::accounts::{AccountId, AccountSession};

/// The id of a User waiting for an Account to accept them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct InviteId(Uuid);

impl fmt::Display for InviteId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for InviteId {
    type Err = uuid::Error;

    fn from_str(id: &str) -> Result<Self, Self::Err> {
        id.parse().map(Self)
    }
}

/// A User waiting for an Account to accept them, as its list shows them.
#[derive(Debug)]
pub(crate) struct ListedInvite {
    pub(crate) id: InviteId,
    pub(crate) name: String,
    pub(crate) server_name: String,
    /// In seconds since the Unix epoch.
    pub(crate) invited_at: i64,
}

impl Links {
    /// Asks the Account whose email is `email`, if there is one, to accept `server`'s User
    /// `user_id`, named `name`, replacing what it asked for them before. Without one it does
    /// nothing, and says so to no one.
    pub(crate) async fn invite_user(
        &self,
        server: &LinkedServer,
        user_id: i64,
        name: &str,
        email: &str,
    ) -> Result<(), sqlx::Error> {
        let Some(account) = self.accounts.with_email(email).await? else {
            return Ok(());
        };
        sqlx::query!(
            "INSERT INTO user_invites (account_id, linked_server_id, server_user_id, user_name)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (linked_server_id, server_user_id) DO UPDATE
             SET account_id = excluded.account_id, user_name = excluded.user_name,
             created_at = now()",
            account.0,
            server.id.0,
            user_id,
            kept_name(name),
        )
        .execute(&self.db)
        .await?;
        Ok(())
    }

    /// The Users waiting for `account` to accept them, oldest first.
    pub(crate) async fn list_invites(
        &self,
        account: AccountId,
    ) -> Result<Vec<ListedInvite>, sqlx::Error> {
        let invites = sqlx::query!(
            r#"SELECT user_invites.id, user_invites.user_name, linked_servers.name AS server_name,
             extract(epoch FROM user_invites.created_at)::bigint AS "invited_at!"
             FROM user_invites JOIN linked_servers ON linked_servers.id = user_invites.linked_server_id
             WHERE user_invites.account_id = $1 ORDER BY user_invites.created_at, user_invites.id"#,
            account.0,
        )
        .fetch_all(&self.db)
        .await?;
        Ok(invites
            .into_iter()
            .map(|invite| ListedInvite {
                id: InviteId(invite.id),
                name: invite.user_name,
                server_name: invite.server_name,
                invited_at: invite.invited_at,
            })
            .collect())
    }

    /// Accepts the User waiting on `invite` for `session`'s Account, Linking them to it, if it has
    /// Recent Authentication and a proven email (ADR 0100).
    pub(crate) async fn accept_invite(
        &self,
        session: &AccountSession,
        invite: InviteId,
    ) -> Result<ListedUser, Error> {
        self.may_link(session)?;
        let mut tx = self.db.begin().await?;
        let invited = sqlx::query!(
            "DELETE FROM user_invites WHERE id = $1 AND account_id = $2
             RETURNING linked_server_id, server_user_id, user_name",
            invite.0,
            session.account.id.0,
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(Error::InviteNotFound)?;
        let user = insert_link(
            &mut tx,
            session.account.id,
            LinkedServerId(invited.linked_server_id),
            invited.server_user_id,
            invited.user_name,
        )
        .await?;
        tx.commit().await?;
        Ok(user)
    }
}
