//! A Plugin's settings, and the schema it declares for them so an admin UI can render a form.
//!
//! The Kernel checks the settings the admin gave against [`Plugin::settings_schema`] before it
//! enables the Plugin, fills in defaults, and hands the result to [`Plugin::register`] through
//! [`Registrar::settings`]. Changing settings means enabling the Plugin again.
//!
//! A [`SettingKind::Choice`] setting is picked from what the Plugin's [`SettingsCheck`] offers, such
//! as a quality profile only the outside service knows. The Kernel runs the check only when an
//! admin asks, never to enable the Plugin, so the outside service being down can't stop it.
//!
//! [`Plugin::settings_schema`]: crate::Plugin::settings_schema
//! [`Plugin::register`]: crate::Plugin::register
//! [`Registrar::settings`]: crate::Registrar::settings

use std::{collections::BTreeMap, fmt};

use serde::{Deserialize, Serialize};
use url::Url;

use crate::{BoxFuture, PluginError};

/// The settings a Plugin takes, in the order a form shows them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SettingsSchema {
    /// Each setting.
    pub fields: Vec<SettingField>,
}

/// One setting a Plugin takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingField {
    /// The key it is stored and configured under, such as `api_key`.
    pub key: String,
    /// A short name for a form's label.
    pub label: String,
    /// What the setting does, for a form's help text.
    pub description: String,
    /// What kind of value it takes.
    pub kind: SettingKind,
    /// Whether the Plugin can't be enabled without it.
    pub required: bool,
    /// The value used when none is given.
    pub default: Option<SettingValue>,
}

/// What kind of value a setting takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SettingKind {
    /// Text.
    Text,
    /// Text that is never shown back or logged, such as an API key.
    Secret,
    /// On or off.
    Bool,
    /// A whole number.
    Integer,
    /// Text picked from the choices the Plugin's [`SettingsCheck`] offers.
    Choice,
    /// An http or https URL with a host, such as a Sonarr address. An Installed Plugin may send
    /// HTTP requests to its host.
    Url,
}

/// A setting's value.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SettingValue {
    /// On or off.
    Bool(bool),
    /// A whole number.
    Integer(i64),
    /// Text, for [`SettingKind::Text`], [`SettingKind::Secret`], [`SettingKind::Choice`] and
    /// [`SettingKind::Url`].
    Text(String),
}

impl SettingValue {
    /// Whether the value is of `kind`.
    #[must_use]
    pub fn is(&self, kind: SettingKind) -> bool {
        match (self, kind) {
            (Self::Text(text), SettingKind::Url) => web_url(text).is_some(),
            (Self::Bool(_), SettingKind::Bool)
            | (Self::Integer(_), SettingKind::Integer)
            | (Self::Text(_), SettingKind::Text | SettingKind::Secret | SettingKind::Choice) => {
                true
            }
            _ => false,
        }
    }
}

// Values may be secrets, so they are never printed.
impl fmt::Debug for SettingValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::Bool(_) => "Bool",
            Self::Integer(_) => "Integer",
            Self::Text(_) => "Text",
        };
        write!(f, "{kind}(..)")
    }
}

/// The settings a Plugin was enabled with, by key.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Settings(BTreeMap<String, SettingValue>);

impl Settings {
    /// Settings with these values.
    #[must_use]
    pub const fn new(values: BTreeMap<String, SettingValue>) -> Self {
        Self(values)
    }

    /// The value of `key`, if it has one.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&SettingValue> {
        self.0.get(key)
    }

    /// The value of `key`, if it is text.
    #[must_use]
    pub fn text(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            SettingValue::Text(text) => Some(text),
            SettingValue::Bool(_) | SettingValue::Integer(_) => None,
        }
    }

    /// The value of `key`, if it is an http or https URL with a host, as a
    /// [`SettingKind::Url`] setting's is.
    #[must_use]
    pub fn url(&self, key: &str) -> Option<Url> {
        web_url(self.text(key)?)
    }

    /// Checks these settings against `schema` and fills in its defaults.
    ///
    /// # Errors
    ///
    /// If a required setting is missing, or a setting has a value of the wrong kind. Settings the
    /// schema doesn't name are kept, so a Plugin's older settings survive an upgrade.
    pub fn resolve(mut self, schema: &SettingsSchema) -> Result<Self, SettingsError> {
        for field in &schema.fields {
            match (self.0.get(&field.key), &field.default) {
                (Some(value), _) if !value.is(field.kind) => {
                    return Err(SettingsError::WrongKind {
                        key: field.key.clone(),
                        expected: field.kind,
                    });
                }
                (None, Some(default)) => {
                    self.0.insert(field.key.clone(), default.clone());
                }
                (None, None) if field.required => {
                    return Err(SettingsError::Missing(field.key.clone()));
                }
                (Some(_) | None, _) => {}
            }
        }
        Ok(self)
    }
}

/// `text` as a URL, if it is an http or https one with a host: one an HTTP client can reach.
fn web_url(text: &str) -> Option<Url> {
    Url::parse(text)
        .ok()
        .filter(|url| matches!(url.scheme(), "http" | "https") && url.has_host())
}

/// Why a Plugin's settings don't fit its schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsError {
    /// A required setting has no value.
    Missing(String),
    /// A setting's value is of the wrong kind.
    WrongKind {
        /// The setting.
        key: String,
        /// The kind the schema declares.
        expected: SettingKind,
    },
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(key) => write!(f, "the setting {key} is required"),
            Self::WrongKind { key, expected } => {
                write!(f, "the setting {key} must be {expected:?}")
            }
        }
    }
}

impl std::error::Error for SettingsError {}

/// One value a [`SettingKind::Choice`] setting may take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    /// What is stored as the setting's text, such as a quality profile's id.
    pub value: String,
    /// What an admin picks it by, such as the profile's name.
    pub label: String,
}

/// The choices for each [`SettingKind::Choice`] setting, by key.
pub type Choices = BTreeMap<String, Vec<Choice>>;

/// A Plugin's check of its settings against the outside service they are for.
pub trait SettingsCheck: Send + Sync {
    /// Checks `settings`, which fit the Plugin's schema except that [`SettingKind::Choice`]
    /// settings may be missing, and returns the choices for each Choice setting. The Kernel gives
    /// up on a check that runs too long.
    ///
    /// # Errors
    ///
    /// When the settings don't work, such as an API key the service refuses, in plain words an
    /// admin can act on.
    fn check<'a>(&'a self, settings: &'a Settings) -> BoxFuture<'a, Result<Choices, PluginError>>;
}

#[cfg(test)]
mod tests {
    use super::{SettingKind, SettingValue, Settings};

    fn text(text: &str) -> SettingValue {
        SettingValue::Text(text.to_owned())
    }

    #[test]
    fn a_url_setting_takes_only_an_http_or_https_url_with_a_host() {
        assert!(text("http://sonarr.lan:8989/").is(SettingKind::Url));
        assert!(text("https://sonarr.example.com").is(SettingKind::Url));

        assert!(!text("sonarr.lan:8989").is(SettingKind::Url));
        assert!(!text("ftp://sonarr.lan/").is(SettingKind::Url));
        assert!(!text("not a url").is(SettingKind::Url));
        assert!(!SettingValue::Integer(8989).is(SettingKind::Url));
    }

    #[test]
    fn a_url_setting_is_read_as_a_url() {
        let settings =
            Settings::new([("sonarr".to_owned(), text("http://sonarr.lan:8989/"))].into());

        let url = settings.url("sonarr");

        assert_eq!(
            url.and_then(|url| url.host_str().map(str::to_owned))
                .as_deref(),
            Some("sonarr.lan")
        );
        assert_eq!(settings.url("missing"), None);
    }
}
