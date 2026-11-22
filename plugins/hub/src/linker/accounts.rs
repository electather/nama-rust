//! Signing in to the Server with an Account, and Users an Admin creates from one (ADR 0092). The
//! Hub runs the sign-in as a device flow: the person approves it on the Hub's page, and the
//! Server collects the token naming their User with its credential, so the token never passes
//! through the browser.

use nama_plugin_api::{PluginError, server_link::AccountSignIn};

use super::{Kept, Linker};

impl Linker {
    /// Asks the Account with `email`, if any, to accept `user_id`, named `name`.
    pub(super) async fn invite(
        &self,
        user_id: i64,
        name: &str,
        email: &str,
    ) -> Result<(), PluginError> {
        let kept = self.linked().await?;
        self.hub
            .invite_user(&kept.credential, user_id, name, email)
            .await
    }

    /// Starts a sign-in with an Account, which the Hub's page sends back to `return_url`.
    pub(super) async fn start_sign_in(
        &self,
        return_url: &str,
    ) -> Result<AccountSignIn, PluginError> {
        let kept = self.linked().await?;
        let started = self.hub.start_sign_in(&kept.credential, return_url).await?;
        Ok(AccountSignIn {
            url: format!("{}/server-sign-in/{}", self.hub.url(), started.id),
            handle: started.device_code,
        })
    }

    /// The User the sign-in `handle` signs in as, once approved and finished with the one-time
    /// `code` the Hub sent the person back with, as the Hub's signed token names them.
    // ponytail: the token's `aud` isn't checked, as the Hub hands it only to this Server's
    // credential; check it once tokens reach the Server through a client.
    pub(super) async fn finish_sign_in(
        &self,
        handle: &str,
        code: &str,
    ) -> Result<Option<i64>, PluginError> {
        let kept = self.linked().await?;
        let Some(token) = self
            .hub
            .poll_sign_in(&kept.credential, handle, code)
            .await?
        else {
            return Ok(None);
        };
        let user = self.verify(&token).await?.sub;
        Ok(Some(user.parse().map_err(|_not_a_number| {
            format!("the Hub's token names no User: {user:?}")
        })?))
    }

    /// The Server's Link, or why there is none.
    async fn linked(&self) -> Result<Kept, PluginError> {
        Ok(self
            .kept()
            .await?
            .ok_or("the Server isn't Linked to the Hub")?)
    }
}
