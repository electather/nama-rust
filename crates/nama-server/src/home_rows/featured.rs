//! The titles Plugins suggest for one User's Featured through `user_featured` (ADR 0072): stored
//! per Plugin and User, and kept fresh by `Refreshes` as Home rows are, so `ListFeatured` never
//! waits on a Plugin.

use std::{collections::HashMap, sync::Arc, time::Duration};

use nama_plugin_api::{
    PluginId,
    contract::{FeaturedSuggestions, UserFeaturedAsked},
    home_row::{HomeRowMethod, HomeRowSource},
    metadata::ExternalId,
};
use sqlx::SqliteConnection;

use super::{
    Error, HomeRows, Kind, Title, answered, from_json,
    refreshes::{Adapter, MIN_TTL, Scope},
    to_json,
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
        let adapter = Suggestions(self.clone());
        let stored = adapter.stored(user).await?;
        self.refreshes.refresh_stale(&adapter, user).await?;
        // ponytail: deleted on the next listing, as Home rows are; see `HomeRows::list`.
        self.refreshes.forget(&adapter).await?;
        let mut suggested = Suggested::new();
        for (plugin, suggestions) in stored {
            if self.refreshes.gives(&plugin, HomeRowMethod::Featured).await {
                add_heaviest(&mut suggested, suggestions);
            }
        }
        Ok(suggested)
    }
}

/// The Featured suggestions Plugins give each User.
#[derive(Debug, Clone)]
pub(super) struct Suggestions(pub(super) HomeRows);

impl Adapter for Suggestions {
    type Kept = Vec<Suggestion>;
    const NOUN: &'static str = "Featured suggestions";

    fn methods(&self) -> &'static [HomeRowMethod] {
        &[HomeRowMethod::Featured]
    }

    fn scopes(&self, reader: i64) -> Vec<(HomeRowMethod, Option<i64>)> {
        vec![(HomeRowMethod::Featured, Some(reader))]
    }

    async fn ask(&self, scope: &Scope) -> Option<(Vec<Suggestion>, Duration)> {
        let user = scope.user?;
        let ask = |source: Arc<dyn HomeRowSource>, settings: Vec<(String, String)>| async move {
            let asked = UserFeaturedAsked {
                personal_settings: settings.into_iter().collect(),
                user_id: user,
                ..UserFeaturedAsked::default()
            };
            source.user_featured(&asked).await
        };
        let answer = (self.0)
            .ask_for_user(&scope.plugin, user, Self::NOUN, ask)
            .await;
        Some(kept_suggestions(&answered(answer, scope, Self::NOUN)?))
    }

    async fn store(
        &self,
        conn: &mut SqliteConnection,
        scope: &Scope,
        kept: Vec<Suggestion>,
    ) -> Result<(), sqlx::Error> {
        let (plugin, user) = (scope.plugin.as_str(), scope.user);
        let suggestions = to_json(&kept)?;
        sqlx::query!(
            "INSERT INTO featured_suggestions (plugin_id, user_id, suggestions) VALUES (?, ?, ?)
             ON CONFLICT (plugin_id, user_id) DO UPDATE SET suggestions = excluded.suggestions",
            plugin,
            user,
            suggestions
        )
        .execute(conn)
        .await?;
        Ok(())
    }

    async fn delete(
        &self,
        conn: &mut SqliteConnection,
        plugin: &str,
        _method: HomeRowMethod,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "DELETE FROM featured_suggestions WHERE plugin_id = ?",
            plugin
        )
        .execute(conn)
        .await?;
        Ok(())
    }

    async fn plugins(&self, _method: HomeRowMethod) -> Result<Vec<PluginId>, sqlx::Error> {
        let plugins = sqlx::query_scalar!("SELECT DISTINCT plugin_id FROM featured_suggestions")
            .fetch_all(self.0.db.reader())
            .await?;
        Ok(plugins.into_iter().map(PluginId::from).collect())
    }
}

impl Suggestions {
    /// Each Plugin's suggestions for the User `user`.
    async fn stored(&self, user: i64) -> Result<HashMap<PluginId, Vec<Suggestion>>, sqlx::Error> {
        let rows = sqlx::query!(
            "SELECT plugin_id, suggestions FROM featured_suggestions WHERE user_id = ?",
            user
        )
        .fetch_all(self.0.db.reader())
        .await?;
        Ok((rows.into_iter())
            .map(|row| (PluginId::from(row.plugin_id), from_json(&row.suggestions)))
            .collect())
    }
}

/// Deletes what each Plugin suggested for the User `user`, and returns whether there was any.
pub(super) async fn delete_users_own(
    conn: &mut SqliteConnection,
    user: i64,
) -> Result<bool, sqlx::Error> {
    let deleted = sqlx::query!("DELETE FROM featured_suggestions WHERE user_id = ?", user)
        .execute(conn)
        .await?;
    Ok(deleted.rows_affected() > 0)
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
/// its time to live, or [`MIN_TTL`] for none.
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
        Duration::from_secs(u64::from(given.ttl_seconds))
    };
    (suggestions, fresh_for)
}

#[cfg(test)]
#[path = "featured_tests.rs"]
mod tests;
