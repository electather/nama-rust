//! Signing in to a Linked Server with an Account (ADR 0092), as a device flow (ADR 0011): the
//! Server starts it and polls with a device code, and the Account approves it on the Hub's page,
//! which sends the person back to the Server. The token the Server collects names the User their
//! Link is to, so the Server finds them through it (ADR 0014).

use std::{fmt, str::FromStr};

use serde::Serialize;
use uuid::Uuid;

use super::{
    CODE_LIFETIME_MINS, Error, LinkedServer, Links, TOKEN_LIFETIME_SECS, hash, issued_at,
    random_token,
};
use crate::accounts::AccountId;

/// The longest return address kept, as browsers handle about this much.
const URL_LENGTH: usize = 2048;

/// The id of a sign-in with an Account, which its page's address carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SignInId(Uuid);

impl fmt::Display for SignInId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for SignInId {
    type Err = uuid::Error;

    fn from_str(id: &str) -> Result<Self, Self::Err> {
        id.parse().map(Self)
    }
}

/// A sign-in just started.
#[derive(Debug)]
pub(crate) struct StartedSignIn {
    pub(crate) id: SignInId,
    pub(crate) device_code: String,
}

/// A sign-in waiting for an Account's approval, as its page shows it to that Account.
#[derive(Debug)]
pub(crate) struct WaitingSignIn {
    pub(crate) server_name: String,
    /// The Account's User on that Server, if it has one.
    pub(crate) user_name: Option<String>,
}

/// The claims of the token a Server collects for an approved sign-in.
#[derive(Serialize)]
struct SignInClaims {
    /// The User's id on the Server.
    sub: String,
    /// The Linked Server's id, so no other Server takes it.
    aud: String,
    iat: u64,
    exp: u64,
}

impl Links {
    /// Starts a sign-in to `server`, which the Hub's page sends back to `return_url`, forgetting
    /// every expired one.
    pub(crate) async fn start_sign_in(
        &self,
        server: &LinkedServer,
        return_url: &str,
    ) -> Result<StartedSignIn, Error> {
        if !is_web_address(return_url) {
            return Err(Error::UrlInvalid);
        }
        let device_code = random_token()?;
        let mut tx = self.db.begin().await?;
        sqlx::query!("DELETE FROM server_sign_ins WHERE expires_at < now()")
            .execute(&mut *tx)
            .await?;
        let id = sqlx::query_scalar!(
            "INSERT INTO server_sign_ins (linked_server_id, device_code_hash, return_url, expires_at)
             VALUES ($1, $2, $3, now() + make_interval(mins => $4)) RETURNING id",
            server.id.0,
            &hash(&device_code)[..],
            return_url,
            CODE_LIFETIME_MINS,
        )
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(StartedSignIn {
            id: SignInId(id),
            device_code,
        })
    }

    /// The sign-in `id`, as `account` sees it, while it waits for approval.
    pub(crate) async fn sign_in(
        &self,
        account: AccountId,
        id: SignInId,
    ) -> Result<WaitingSignIn, Error> {
        // ponytail: an Account Linked to more than one User on the Server signs in as the one it
        // Linked first, here and in `approve_sign_in`; let it choose if anyone needs both.
        let waiting = sqlx::query!(
            "SELECT linked_servers.name AS server_name,
             (SELECT user_name FROM user_links WHERE account_id = $2
             AND linked_server_id = server_sign_ins.linked_server_id
             ORDER BY created_at, id LIMIT 1) AS user_name
             FROM server_sign_ins JOIN linked_servers
             ON linked_servers.id = server_sign_ins.linked_server_id
             WHERE server_sign_ins.id = $1 AND server_sign_ins.expires_at > now()
             AND server_sign_ins.user_link_id IS NULL",
            id.0,
            account.0,
        )
        .fetch_optional(&self.db)
        .await?
        .ok_or(Error::SignInNotFound)?;
        Ok(WaitingSignIn {
            server_name: waiting.server_name,
            user_name: waiting.user_name,
        })
    }

    /// Approves the sign-in `id` as `account`'s User on its Server, and answers where to send the
    /// person back to.
    pub(crate) async fn approve_sign_in(
        &self,
        account: AccountId,
        id: SignInId,
    ) -> Result<String, Error> {
        if self.sign_in(account, id).await?.user_name.is_none() {
            return Err(Error::NoUserOnServer);
        }
        // Nothing matches but a sign-in approved, or a User Unlinked, since the look above.
        sqlx::query_scalar!(
            "UPDATE server_sign_ins SET user_link_id = (SELECT id FROM user_links
             WHERE account_id = $2 AND linked_server_id = server_sign_ins.linked_server_id
             ORDER BY created_at, id LIMIT 1)
             WHERE id = $1 AND expires_at > now() AND user_link_id IS NULL
             AND EXISTS (SELECT 1 FROM user_links WHERE account_id = $2
             AND linked_server_id = server_sign_ins.linked_server_id)
             RETURNING return_url",
            id.0,
            account.0,
        )
        .fetch_optional(&self.db)
        .await?
        .ok_or(Error::SignInNotFound)
    }

    /// The token naming the User `server`'s sign-in polled with `device_code` signs in as, once it
    /// was approved, handed out once; `None` until then.
    pub(crate) async fn poll_sign_in(
        &self,
        server: &LinkedServer,
        device_code: &str,
    ) -> Result<Option<String>, Error> {
        let found = sqlx::query!(
            "SELECT id, (SELECT server_user_id FROM user_links
             WHERE user_links.id = server_sign_ins.user_link_id) AS server_user_id
             FROM server_sign_ins
             WHERE device_code_hash = $1 AND linked_server_id = $2 AND expires_at > now()",
            &hash(device_code)[..],
            server.id.0,
        )
        .fetch_optional(&self.db)
        .await?
        .ok_or(Error::Expired)?;
        let Some(user_id) = found.server_user_id else {
            return Ok(None);
        };
        let collected = sqlx::query!("DELETE FROM server_sign_ins WHERE id = $1", found.id)
            .execute(&self.db)
            .await?;
        // Another poll collected it first.
        if collected.rows_affected() == 0 {
            return Err(Error::Expired);
        }
        let iat = issued_at();
        Ok(Some(self.keys.sign(&SignInClaims {
            sub: user_id.to_string(),
            aud: server.id.to_string(),
            iat,
            exp: iat + TOKEN_LIFETIME_SECS,
        })))
    }
}

/// Whether `url` is an `http` or `https` address short enough to keep. The Hub's page navigates to
/// it, so any other scheme, such as `javascript:`, would run on the Hub's origin.
fn is_web_address(url: &str) -> bool {
    let scheme = url
        .split_once("://")
        .map(|(scheme, _)| scheme.to_ascii_lowercase());
    url.len() <= URL_LENGTH && matches!(scheme.as_deref(), Some("http" | "https"))
}

#[cfg(test)]
#[path = "server_sign_ins_tests.rs"]
mod tests;
