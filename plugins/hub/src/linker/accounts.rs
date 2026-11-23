//! Signing in to the Server with an Account, and Users an Admin creates from one (ADR 0092). The
//! Hub runs the sign-in as a device flow: the person approves it on the Hub's page, and the
//! Server collects the token naming their User with its credential, so the token never passes
//! through the browser.

use nama_plugin_api::{PluginError, server_link::AccountSignIn};

use super::Linker;

/// Why a sign-in or an invite can't start.
const NOT_LINKED: &str = "the Server isn't Linked to the Hub";

impl Linker {
    /// Asks the Account with `email`, if any, to accept `user_id`, named `name`.
    pub(super) async fn invite(
        &self,
        user_id: i64,
        name: &str,
        email: &str,
    ) -> Result<(), PluginError> {
        let invited = self.call(|hub| hub.invite_user(user_id, name, email));
        Ok(invited.await?.ok_or(NOT_LINKED)?)
    }

    /// Starts a sign-in with an Account, which the Hub's page sends back to `return_url`.
    pub(super) async fn start_sign_in(
        &self,
        return_url: &str,
    ) -> Result<AccountSignIn, PluginError> {
        let started = self.call(|hub| hub.start_sign_in(return_url)).await?;
        let started = started.ok_or(NOT_LINKED)?;
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
        let polled = self.call(|hub| hub.poll_sign_in(handle, code)).await?;
        let Some(token) = polled.ok_or(NOT_LINKED)? else {
            return Ok(None);
        };
        let user = self.verify(&token).await?.sub;
        Ok(Some(user.parse().map_err(|_not_a_number| {
            format!("the Hub's token names no User: {user:?}")
        })?))
    }
}
