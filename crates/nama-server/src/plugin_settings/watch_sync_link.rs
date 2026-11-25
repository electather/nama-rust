//! Linking a User's account at a Watch sync Plugin's tracker by device code: the Plugin gives the
//! code, and the tokens once the User entered it, which are kept as their secret Personal settings
//! (ADR 0046). The User's client holds the device code between polls, so nothing waits here.
//!
//! ponytail: a device code isn't tied to the User who started it, so a User who learned another's
//! could link that account to themself; keep each User's started code here if codes ever leave
//! the client that asked for them.

use std::sync::Arc;

use nama_plugin_api::{
    PluginId,
    contract::{LinkStarted, PollLinkAsked, StartLinkAsked},
    watch_sync::WatchSync,
};

use super::PluginSettings;

/// Why linking an account at a Watch sync Plugin's tracker failed.
#[derive(Debug, thiserror::Error)]
pub(crate) enum LinkError {
    #[error("no enabled Plugin with that id is a Watch sync")]
    NoWatchSync,
    /// The Plugin's reason.
    #[error("the Plugin couldn't ask its tracker: {0}")]
    Failed(String),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// Where linking an account stands after a poll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LinkState {
    Pending,
    Linked,
    Expired,
}

impl PluginSettings {
    /// Starts linking an account at the Watch sync Plugin `id`'s tracker.
    ///
    /// # Errors
    ///
    /// [`LinkError::NoWatchSync`] unless the Plugin is enabled and a Watch sync, or
    /// [`LinkError::Failed`], also for a verification URL that isn't `http` or `https`.
    pub(crate) async fn start_watch_sync_link(
        &self,
        id: &PluginId,
    ) -> Result<LinkStarted, LinkError> {
        let points = self.registry.extension_points();
        let answer = (points.ask(id, |sync: Arc<dyn WatchSync>| async move {
            sync.start_link(&StartLinkAsked::default()).await
        }))
        .await
        .ok_or(LinkError::NoWatchSync)?;
        let started = (answer.result).map_err(|failure| LinkError::Failed(failure.message))?;
        // A client opens it as a link, where a `javascript:` URL would run in its page.
        let url = &started.verification_url;
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return Err(LinkError::Failed(format!("{url} isn't an http(s) URL")));
        }
        Ok(started)
    }

    /// Asks the Watch sync Plugin `id` whether `user` entered the code `device_code` came with,
    /// and keeps the tokens it answers once they did as their secret Personal settings, beside
    /// the others they set. A token whose key the Manifest doesn't declare SECRET is dropped.
    ///
    /// # Errors
    ///
    /// As [`PluginSettings::start_watch_sync_link`], also when every token was dropped, or
    /// [`LinkError::Database`].
    pub(crate) async fn poll_watch_sync_link(
        &self,
        user: i64,
        id: &PluginId,
        device_code: String,
    ) -> Result<LinkState, LinkError> {
        let asked = PollLinkAsked {
            device_code,
            ..PollLinkAsked::default()
        };
        let points = self.registry.extension_points();
        let polled = (points.ask(id, |sync: Arc<dyn WatchSync>| {
            let asked = &asked;
            async move { sync.poll_link(asked).await }
        }))
        .await
        .ok_or(LinkError::NoWatchSync)?
        .result
        .map_err(|failure| LinkError::Failed(failure.message))?;
        if polled.expired {
            return Ok(LinkState::Expired);
        }
        if polled.personal_settings.is_empty() {
            return Ok(LinkState::Pending);
        }
        if !(self.keep_tokens(user, id, polled.personal_settings).await?) {
            // Not pending: the tracker hands its tokens out once, so polling again gets none.
            let reason = "the Plugin's tokens name none of its SECRET Personal settings";
            return Err(LinkError::Failed(reason.to_owned()));
        }
        Ok(LinkState::Linked)
    }
}

#[cfg(test)]
#[path = "watch_sync_link_tests.rs"]
mod tests;
