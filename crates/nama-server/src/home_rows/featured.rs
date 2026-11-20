//! The titles Plugins suggest for one User's Featured through `user_featured` (ADR 0072): stored
//! per Plugin and User, and refreshed in the background as that User's own Home rows are, so
//! `ListFeatured` never waits on a Plugin.

use std::{collections::HashMap, sync::Arc, time::Duration};

use nama_plugin_api::{
    PluginError, PluginId,
    contract::{FeaturedSuggestions, UserFeaturedAsked},
    home_row::{HomeRowMethod, HomeRowSource},
    metadata::ExternalId,
};
use tracing::warn;

use super::{
    Error, HomeRows, Kind, MAX_TTL, MIN_TTL, Title, personal,
    store::{Refreshing, from_json, stale_after, to_json},
};
use crate::{
    db,
    plugins::{Answer, Failure, without_secrets},
    users::personal_settings_of,
};

/// The most suggestions of one answer the Kernel keeps.
const MAX_SUGGESTIONS: usize = 20;

/// The Plugins' suggestions for one User's Featured: each title's heaviest weight, from 0.0 to
/// 1.0, by whether it is a Show and the id it was suggested by.
pub(crate) type Suggested = HashMap<(bool, ExternalId), f64>;

/// A title one Plugin suggested, with its weight.
type Suggestion = (Title, f32);

impl HomeRows {
    /// What each enabled Plugin giving Featured suggestions last suggested for the User `user`.
    /// Starts a background refresh of each such Plugin's suggestions for `user` that are stale or
    /// were never refreshed, and deletes every User's suggestions from a Plugin no longer giving
    /// them.
    pub(crate) async fn featured(&self, user: i64) -> Result<Suggested, Error> {
        let stored = self.stored_suggestions(user).await?;
        let now = db::now();
        for plugin in self.points.providers::<dyn HomeRowSource>() {
            if stored
                .get(&plugin)
                .is_none_or(|(stale_at, _)| *stale_at <= now)
            {
                self.refresh_featured(&plugin, user).await;
            }
        }
        // ponytail: deleted on the next listing, as Home rows are; see `HomeRows::list`.
        self.forget_featured_of_plugins_no_longer_giving_it()
            .await?;
        let mut suggested = Suggested::new();
        for (plugin, (_, suggestions)) in stored {
            if self.gives(&plugin, HomeRowMethod::Featured).await {
                add_heaviest(&mut suggested, suggestions);
            }
        }
        Ok(suggested)
    }

    /// Refreshes the Plugin `plugin`'s suggestions for the User `user` in the background, in the
    /// Plugin's scope, as [`HomeRows::refresh`] does rows. Does nothing if it isn't an enabled
    /// Home row source giving them, or they're already being refreshed.
    async fn refresh_featured(&self, plugin: &PluginId, user: i64) {
        if !self.gives(plugin, HomeRowMethod::Featured).await {
            return;
        }
        let scope = (plugin.clone(), Some(user));
        let Some(refreshing) = Refreshing::start(&self.refreshing_featured, scope) else {
            return;
        };
        let this = self.clone();
        let id = plugin.clone();
        self.points.spawn_in(plugin, async move {
            this.refresh_featured_now(&id, user).await;
            drop(refreshing);
        });
    }

    async fn refresh_featured_now(&self, plugin: &PluginId, user: i64) {
        // Put off before the Plugin is asked, as `refresh_now` puts off rows, and for the same
        // reasons: the triggers on `personal_settings` delete it.
        if let Err(err) = self.put_off_featured(plugin, user).await {
            warn!(plugin = %plugin, user, "could not put off refreshing the Plugin's Featured suggestions: {err}");
            return;
        }
        let settings = match personal_settings_of(&self.db, user, plugin).await {
            Ok(settings) => settings,
            Err(err) => {
                warn!(plugin = %plugin, user, "could not read Personal settings to refresh Featured suggestions: {err}");
                return;
            }
        };
        let (settings, secrets) = personal(settings);
        let asked = UserFeaturedAsked {
            personal_settings: settings.into_iter().collect(),
            user_id: user,
            ..UserFeaturedAsked::default()
        };
        let answer: Option<Answer<FeaturedSuggestions>> = (self.points)
            .ask(plugin, |source: Arc<dyn HomeRowSource>| {
                let (asked, secrets) = (&asked, &secrets);
                async move {
                    let answer = source.user_featured(asked).await;
                    answer
                        .map_err(|err| PluginError::from(without_secrets(err.to_string(), secrets)))
                }
            })
            .await;
        let Some(answer) = answer else {
            return;
        };
        let given = match answer.result {
            Ok(given) => given,
            Err(Failure { message, .. }) => {
                warn!(plugin = %plugin, user, error = message, "refreshing the Plugin's Featured suggestions failed");
                return;
            }
        };
        let (suggestions, fresh_for) = kept_suggestions(&given);
        if let Err(err) = self
            .replace_suggestions(plugin, user, suggestions, fresh_for)
            .await
        {
            warn!(plugin = %plugin, user, "could not store the Plugin's Featured suggestions: {err}");
        }
    }

    /// Each Plugin's suggestions for the User `user`, with when they go stale.
    async fn stored_suggestions(
        &self,
        user: i64,
    ) -> Result<HashMap<PluginId, (i64, Vec<Suggestion>)>, sqlx::Error> {
        let rows = sqlx::query!(
            "SELECT plugin_id, suggestions, stale_at FROM featured_suggestions WHERE user_id = ?",
            user
        )
        .fetch_all(self.db.reader())
        .await?;
        Ok((rows.into_iter())
            .map(|row| {
                let suggestions = from_json(&row.suggestions);
                (PluginId::from(row.plugin_id), (row.stale_at, suggestions))
            })
            .collect())
    }

    /// Keeps the Plugin `plugin`'s suggestions for the User `user` as they are, none before its
    /// first refresh, and refreshes them no sooner than [`MIN_TTL`] from now.
    async fn put_off_featured(&self, plugin: &PluginId, user: i64) -> Result<(), sqlx::Error> {
        let plugin = plugin.as_str().to_owned();
        let stale_at = stale_after(MIN_TTL);
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!(
                        "INSERT INTO featured_suggestions (plugin_id, user_id, suggestions, stale_at)
                         VALUES (?, ?, '[]', ?)
                         ON CONFLICT (plugin_id, user_id) DO UPDATE SET stale_at = excluded.stale_at",
                        plugin,
                        user,
                        stale_at
                    )
                    .execute(&mut **tx)
                    .await?;
                    Ok(())
                })
            })
            .await
    }

    /// Replaces the Plugin `plugin`'s suggestions for the User `user` with `suggestions`, fresh
    /// for `fresh_for`. Stores nothing once the put-off its refresh started with is gone, as the
    /// User changed their Personal settings meanwhile.
    async fn replace_suggestions(
        &self,
        plugin: &PluginId,
        user: i64,
        suggestions: Vec<Suggestion>,
        fresh_for: Duration,
    ) -> Result<(), sqlx::Error> {
        let plugin = plugin.as_str().to_owned();
        let suggestions = to_json(&suggestions)?;
        let stale_at = stale_after(fresh_for);
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!(
                        "UPDATE featured_suggestions SET suggestions = ?, stale_at = ?
                         WHERE plugin_id = ? AND user_id = ?",
                        suggestions,
                        stale_at,
                        plugin,
                        user
                    )
                    .execute(&mut **tx)
                    .await?;
                    Ok(())
                })
            })
            .await
    }

    /// Deletes every User's suggestions from each Plugin no longer giving them, as it is disabled
    /// or its version stopped serving `user_featured`.
    async fn forget_featured_of_plugins_no_longer_giving_it(&self) -> Result<(), sqlx::Error> {
        let plugins = sqlx::query_scalar!("SELECT DISTINCT plugin_id FROM featured_suggestions")
            .fetch_all(self.db.reader())
            .await?;
        let mut gone = Vec::new();
        for plugin in plugins.into_iter().map(PluginId::from) {
            if !self.gives(&plugin, HomeRowMethod::Featured).await {
                gone.push(plugin.as_str().to_owned());
            }
        }
        if gone.is_empty() {
            return Ok(());
        }
        let gone = to_json(&gone)?;
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!(
                        "DELETE FROM featured_suggestions
                         WHERE plugin_id IN (SELECT value FROM json_each(?))",
                        gone
                    )
                    .execute(&mut **tx)
                    .await?;
                    Ok(())
                })
            })
            .await
    }
}

/// Adds each of `suggestions` to `suggested`, a title suggested twice at its heavier weight.
fn add_heaviest(suggested: &mut Suggested, suggestions: Vec<Suggestion>) {
    for (title, weight) in suggestions {
        let Some(id) = title.id() else { continue };
        let weight = f64::from(weight);
        let heaviest = (suggested.entry((title.0 == Kind::Show, id))).or_insert(weight);
        *heaviest = heaviest.max(weight);
    }
}

/// What of `given` the Kernel keeps (ADR 0072): of its first [`MAX_SUGGESTIONS`] suggestions,
/// those it can read, each weight clamped to between 0.0 and 1.0; and how long they stay fresh,
/// its time to live clamped to between [`MIN_TTL`] and [`MAX_TTL`], or [`MIN_TTL`] for none.
fn kept_suggestions(given: &FeaturedSuggestions) -> (Vec<Suggestion>, Duration) {
    let suggestions: Vec<Suggestion> = (given.suggestions.iter().take(MAX_SUGGESTIONS))
        .filter(|suggestion| !suggestion.weight.is_nan())
        .filter_map(|suggestion| {
            let id = suggestion.id.as_option()?;
            let title = Title::given(suggestion.kind.as_known()?, &id.namespace, &id.value)?;
            // `abs`, as -0.0 clamps to itself and would sort below 0.0 by `total_cmp`.
            Some((title, suggestion.weight.clamp(0.0, 1.0).abs()))
        })
        .collect();
    let fresh_for = if suggestions.is_empty() {
        MIN_TTL
    } else {
        Duration::from_secs(u64::from(given.ttl_seconds)).clamp(MIN_TTL, MAX_TTL)
    };
    (suggestions, fresh_for)
}

#[cfg(test)]
#[path = "featured_tests.rs"]
mod tests;
