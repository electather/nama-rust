//! Checking that each Library a Plugin's settings name by id exists when they are saved. A Library
//! deleted later stays in the value: the Kernel just stops listing it, and the Plugin ignores it.

use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
};

use nama_plugin_api::settings::{SettingKind, SettingValue, SettingsSchema};

use super::{ChangeError, PluginSettings};
use crate::library::libraries::{self, Libraries};

impl PluginSettings {
    /// Refuses `settings` if a Libraries setting `schema` declares names an id no Library has.
    pub(super) async fn check_library_ids(
        &self,
        schema: &SettingsSchema,
        settings: &BTreeMap<String, SettingValue>,
    ) -> Result<(), ChangeError> {
        let names_some = libraries_settings(schema, settings)
            .any(|(_, value)| matches!(value, SettingValue::Libraries(ids) if !ids.is_empty()));
        if !names_some {
            return Ok(());
        }
        let known = Libraries::new(Arc::clone(&self.db))
            .list()
            .await
            .map_err(list_failed)?
            .into_iter()
            .map(|library| library.id)
            .collect();
        match unknown_library(schema, settings, &known) {
            Some((key, id)) => Err(ChangeError::UnknownLibrary { key, id }),
            None => Ok(()),
        }
    }
}

/// `err`, from listing the Libraries, as a failed change.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "listing only reads, so it fails with nothing but a database error"
)]
fn list_failed(err: libraries::Error) -> ChangeError {
    match err {
        libraries::Error::Database(err) => ChangeError::Database(err),
        other => ChangeError::Database(sqlx::Error::Protocol(other.to_string())),
    }
}

/// The first Libraries setting in `schema`'s order naming an id `known` lacks, with that id.
fn unknown_library(
    schema: &SettingsSchema,
    settings: &BTreeMap<String, SettingValue>,
    known: &HashSet<i64>,
) -> Option<(String, i64)> {
    libraries_settings(schema, settings).find_map(|(key, value)| match value {
        SettingValue::Libraries(ids) => ids
            .iter()
            .find(|id| !known.contains(id))
            .map(|&id| (key.to_owned(), id)),
        SettingValue::Bool(_) | SettingValue::Integer(_) | SettingValue::Text(_) => None,
    })
}

/// Each Libraries setting `schema` declares that `settings` gives a value, by key.
fn libraries_settings<'a>(
    schema: &'a SettingsSchema,
    settings: &'a BTreeMap<String, SettingValue>,
) -> impl Iterator<Item = (&'a str, &'a SettingValue)> {
    schema
        .fields
        .iter()
        .filter(|field| field.kind == SettingKind::Libraries)
        .filter_map(|field| Some((field.key.as_str(), settings.get(&field.key)?)))
}

#[cfg(test)]
#[path = "library_ids_tests.rs"]
mod tests;
