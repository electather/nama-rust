//! A Plugin's settings, and the schema it declares for them so an admin UI can render a form.
//!
//! The Kernel checks the settings the admin gave against [`Manifest::settings`] before it
//! enables the Plugin, fills in defaults, and hands the result to [`Plugin::register`] through
//! [`Registrar::settings`]. Changing settings means enabling the Plugin again.
//!
//! A [`SettingKind::Choice`] setting is picked from what the Plugin's [`SettingsCheck`] offers, such
//! as a quality profile only the outside service knows. The Kernel runs the check only when an
//! admin asks, never to enable the Plugin, so the outside service being down can't stop it.
//!
//! [`Manifest::settings`]: crate::Manifest::settings
//! [`Plugin::register`]: crate::Plugin::register
//! [`Registrar::settings`]: crate::Registrar::settings

use std::{
    collections::{BTreeMap, HashSet},
    fmt,
};

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
    /// The least an [`SettingKind::Integer`] setting may be, inclusive. Only for that kind.
    pub min: Option<i64>,
    /// The most an [`SettingKind::Integer`] setting may be, inclusive. Only for that kind.
    pub max: Option<i64>,
    /// The choices a [`SettingKind::Choices`] setting offers, in the order a form shows them,
    /// unless the Plugin's [`SettingsCheck`] offers its own for it. Only for that kind.
    pub choices: Vec<Choice>,
}

impl SettingField {
    /// Whether `number` is within the setting's [`min`](Self::min) and [`max`](Self::max).
    #[must_use]
    pub fn within_bounds(&self, number: i64) -> bool {
        self.min.is_none_or(|min| number >= min) && self.max.is_none_or(|max| number <= max)
    }

    /// Why the Kernel refuses a Plugin that declares this setting, if it does.
    #[must_use]
    pub fn flaw(&self) -> Option<SettingFlaw> {
        // First, since the bounds and Choices rules assume the default is of the setting's kind.
        self.kind_flaw()
            .or_else(|| self.bounds_flaw())
            .or_else(|| self.choices_flaw())
    }

    fn kind_flaw(&self) -> Option<SettingFlaw> {
        let default = self.default.as_ref()?;
        (!default.is(self.kind)).then(|| SettingFlaw::DefaultWrongKind {
            key: self.key.clone(),
            kind: self.kind,
        })
    }

    fn bounds_flaw(&self) -> Option<SettingFlaw> {
        let key = || self.key.clone();
        if self.kind != SettingKind::Integer && (self.min.is_some() || self.max.is_some()) {
            return Some(SettingFlaw::BoundsOnAnotherKind { key: key() });
        }
        if let (Some(min), Some(max)) = (self.min, self.max)
            && min > max
        {
            return Some(SettingFlaw::BoundsCrossed {
                key: key(),
                min,
                max,
            });
        }
        match self.default {
            Some(SettingValue::Integer(default)) if !self.within_bounds(default) => {
                Some(SettingFlaw::DefaultOutOfBounds { key: key() })
            }
            _ => None,
        }
    }

    fn choices_flaw(&self) -> Option<SettingFlaw> {
        let key = || self.key.clone();
        if self.kind != SettingKind::Choices && !self.choices.is_empty() {
            return Some(SettingFlaw::ChoicesOnAnotherKind { key: key() });
        }
        let mut listed = HashSet::new();
        if let Some(choice) = self
            .choices
            .iter()
            .find(|choice| !listed.insert(&choice.value))
        {
            return Some(SettingFlaw::ChoiceRepeated {
                key: key(),
                value: choice.value.clone(),
            });
        }
        match &self.default {
            Some(SettingValue::Texts(default))
                if default.iter().any(|value| !listed.contains(value)) =>
            {
                Some(SettingFlaw::DefaultNotAChoice { key: key() })
            }
            _ => None,
        }
    }
}

impl SettingsSchema {
    /// [`SettingFlaw::NoChoices`] for the first [`SettingKind::Choices`] setting with no choices
    /// of its own, unless `offers_choices`, that is the Plugin has a [`SettingsCheck`] to offer
    /// some.
    #[must_use]
    pub fn unoffered_choices(&self, offers_choices: bool) -> Option<SettingFlaw> {
        if offers_choices {
            return None;
        }
        let unoffered = self
            .fields
            .iter()
            .find(|field| field.kind == SettingKind::Choices && field.choices.is_empty())?;
        Some(SettingFlaw::NoChoices {
            key: unoffered.key.clone(),
        })
    }
}

/// A flaw in a setting a Plugin declares, for which the Kernel refuses the Plugin whatever values
/// it is given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingFlaw {
    /// A default that isn't of the setting's [`kind`](SettingField::kind).
    DefaultWrongKind {
        /// The setting's key.
        key: String,
        /// The setting's kind.
        kind: SettingKind,
    },
    /// A [`min`](SettingField::min) or [`max`](SettingField::max) on a setting that isn't
    /// [`SettingKind::Integer`].
    BoundsOnAnotherKind {
        /// The setting's key.
        key: String,
    },
    /// A `min` over its `max`.
    BoundsCrossed {
        /// The setting's key.
        key: String,
        /// Its `min`.
        min: i64,
        /// Its `max`.
        max: i64,
    },
    /// A default outside the setting's bounds.
    DefaultOutOfBounds {
        /// The setting's key.
        key: String,
    },
    /// [`choices`](SettingField::choices) on a setting that isn't [`SettingKind::Choices`].
    ChoicesOnAnotherKind {
        /// The setting's key.
        key: String,
    },
    /// A choice's value listed twice.
    ChoiceRepeated {
        /// The setting's key.
        key: String,
        /// The value listed twice.
        value: String,
    },
    /// A default holding a value that isn't among the setting's choices.
    DefaultNotAChoice {
        /// The setting's key.
        key: String,
    },
    /// A [`SettingKind::Choices`] setting with no choices, from a Plugin with no
    /// [`SettingsCheck`] to offer some.
    NoChoices {
        /// The setting's key.
        key: String,
    },
}

// Words a Built-in's refusal. An Installed Plugin's refusal names its wire Manifest's fields and
// exports instead, such as `min_value`.
impl fmt::Display for SettingFlaw {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DefaultWrongKind { key, kind } => {
                write!(f, "its setting {key}'s default isn't {kind}")
            }
            Self::BoundsOnAnotherKind { key } => write!(
                f,
                "its setting {key} has min or max, which only an Integer setting may"
            ),
            Self::BoundsCrossed { key, min, max } => {
                write!(f, "its setting {key}'s min {min} is over its max {max}")
            }
            Self::DefaultOutOfBounds { key } => {
                write!(f, "its setting {key}'s default is outside its bounds")
            }
            Self::ChoicesOnAnotherKind { key } => write!(
                f,
                "its setting {key} has choices, which only a Choices setting may"
            ),
            Self::ChoiceRepeated { key, value } => {
                write!(f, "its setting {key} lists the choice {value:?} twice")
            }
            Self::DefaultNotAChoice { key } => write!(
                f,
                "its setting {key}'s default holds a value that isn't among its choices"
            ),
            Self::NoChoices { key } => write!(
                f,
                "its setting {key} is Choices with no choices, and it has no settings check to \
                 offer some"
            ),
        }
    }
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
    /// The ids of Libraries, which an admin picks from the Server's Library list. A Library
    /// deleted later stays in the value, so a Plugin ignores an id the Kernel no longer lists.
    Libraries,
    /// Several values, unlike [`SettingKind::Choice`], each picked from the setting's
    /// [`choices`](SettingField::choices) or those its [`SettingsCheck`] offers. A value no longer
    /// offered stays in the value, so a Plugin ignores one it doesn't know.
    Choices,
}

// The noun phrase a refusal ends with, "the setting url must be an http or https URL".
impl fmt::Display for SettingKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Text | Self::Secret => "text",
            Self::Bool => "true or false",
            Self::Integer => "a whole number",
            Self::Choice => "one of its choices",
            Self::Url => "an http or https URL",
            Self::Libraries => "a list of Library ids",
            Self::Choices => "a list of its choices",
        })
    }
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
    /// Library ids, for [`SettingKind::Libraries`].
    Libraries(Vec<i64>),
    /// The picked choices' values, for [`SettingKind::Choices`], in the order the choices are
    /// listed.
    // Stored as `{"texts": [..]}`: untagged, a bare `[]` would read back as no Library ids.
    #[serde(with = "tagged_texts")]
    Texts(Vec<String>),
}

mod tagged_texts {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Serialize, Deserialize)]
    struct Tagged<T> {
        texts: T,
    }

    pub(super) fn serialize<S: Serializer>(texts: &[String], to: S) -> Result<S::Ok, S::Error> {
        Tagged { texts }.serialize(to)
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(from: D) -> Result<Vec<String>, D::Error> {
        Tagged::deserialize(from).map(|tagged| tagged.texts)
    }
}

impl From<Vec<i64>> for SettingValue {
    fn from(ids: Vec<i64>) -> Self {
        Self::Libraries(ids)
    }
}

impl SettingValue {
    /// Whether the value is of `kind`.
    #[must_use]
    pub fn is(&self, kind: SettingKind) -> bool {
        match (self, kind) {
            (Self::Text(text), SettingKind::Url) => web_url(text).is_some(),
            (Self::Bool(_), SettingKind::Bool)
            | (Self::Integer(_), SettingKind::Integer)
            | (Self::Libraries(_), SettingKind::Libraries)
            | (Self::Texts(_), SettingKind::Choices)
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
            Self::Libraries(_) => "Libraries",
            Self::Texts(_) => "Texts",
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
            SettingValue::Bool(_)
            | SettingValue::Integer(_)
            | SettingValue::Libraries(_)
            | SettingValue::Texts(_) => None,
        }
    }

    /// The value of `key`, if it holds Library ids, as a [`SettingKind::Libraries`] setting's
    /// does.
    #[must_use]
    pub fn library_ids(&self, key: &str) -> Option<&[i64]> {
        match self.get(key)? {
            SettingValue::Libraries(ids) => Some(ids),
            SettingValue::Bool(_)
            | SettingValue::Integer(_)
            | SettingValue::Text(_)
            | SettingValue::Texts(_) => None,
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
    /// If a required setting is missing, or holds an empty list of choices, or else a setting has a
    /// value of the wrong kind or an integer outside its bounds. Settings the schema doesn't name
    /// are kept, so a Plugin's older settings survive an upgrade.
    pub fn resolve(mut self, schema: &SettingsSchema) -> Result<Self, SettingsError> {
        // Missing before wrong kinds, whatever the order of the fields: the Plugins list offers
        // Set up exactly when this refuses with `Missing`.
        let missing = schema.fields.iter().find(|field| {
            let value = self.0.get(&field.key).or(field.default.as_ref());
            field.required
                && value.is_none_or(
                    |value| matches!(value, SettingValue::Texts(picked) if picked.is_empty()),
                )
        });
        if let Some(field) = missing {
            return Err(SettingsError::Missing(field.key.clone()));
        }
        for field in &schema.fields {
            match (self.0.get(&field.key), &field.default) {
                (Some(value), _) if !value.is(field.kind) => {
                    return Err(SettingsError::WrongKind {
                        key: field.key.clone(),
                        expected: field.kind,
                    });
                }
                (Some(SettingValue::Integer(number)), _) if !field.within_bounds(*number) => {
                    return Err(SettingsError::OutOfRange {
                        key: field.key.clone(),
                        min: field.min,
                        max: field.max,
                    });
                }
                (None, Some(default)) => {
                    self.0.insert(field.key.clone(), default.clone());
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
    /// An integer setting's value is outside the bounds its schema declares.
    OutOfRange {
        /// The setting.
        key: String,
        /// Its least value, if it has one.
        min: Option<i64>,
        /// Its most value, if it has one.
        max: Option<i64>,
    },
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(key) => write!(f, "the setting {key} is required"),
            Self::WrongKind { key, expected } => {
                write!(f, "the setting {key} must be {expected}")
            }
            Self::OutOfRange { key, min, max } => match (min, max) {
                (Some(min), Some(max)) => {
                    write!(f, "the setting {key} must be from {min} to {max}")
                }
                (Some(min), None) => write!(f, "the setting {key} must be at least {min}"),
                (None, Some(max)) => write!(f, "the setting {key} must be at most {max}"),
                (None, None) => write!(f, "the setting {key} is out of range"),
            },
        }
    }
}

impl std::error::Error for SettingsError {}

/// One value a [`SettingKind::Choice`] or [`SettingKind::Choices`] setting may take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    /// What is stored as the setting's text, or among its texts, such as a quality profile's id.
    pub value: String,
    /// What an admin picks it by, such as the profile's name.
    pub label: String,
}

/// The choices for each [`SettingKind::Choice`] or [`SettingKind::Choices`] setting, by key.
pub type Choices = BTreeMap<String, Vec<Choice>>;

/// `values` as a [`SettingKind::Choices`] setting keeps them: each once, in the order `choices`
/// lists them, leaving out any it doesn't list.
#[must_use]
pub fn in_listed_order(choices: &[Choice], values: &[String]) -> Vec<String> {
    let mut kept = HashSet::new();
    choices
        .iter()
        .filter(|choice| values.contains(&choice.value) && kept.insert(&choice.value))
        .map(|choice| choice.value.clone())
        .collect()
}

/// A Plugin's check of its settings against the outside service they are for.
pub trait SettingsCheck: Send + Sync {
    /// Checks `settings`, which fit the Plugin's schema except that [`SettingKind::Choice`]
    /// settings may be missing, and returns the choices for each Choice setting and for each
    /// Choices setting whose [`choices`](SettingField::choices) it replaces. The Kernel gives up
    /// on a check that runs too long.
    ///
    /// # Errors
    ///
    /// When the settings don't work, such as an API key the service refuses, in plain words an
    /// admin can act on.
    fn check<'a>(&'a self, settings: &'a Settings) -> BoxFuture<'a, Result<Choices, PluginError>>;
}

/// A Plugin's check of one User's Personal settings, run when that User saves them.
pub trait PersonalSettingsCheck: Send + Sync {
    /// Checks `personal`, one User's Personal settings as they would be saved, each value as text
    /// under its key as a [`Recipient::User`](crate::notifier::Recipient::User) carries them.
    /// `settings` are the Plugin's own saved settings. The Kernel gives up on a check that runs too
    /// long.
    ///
    /// # Errors
    ///
    /// When the values won't work, such as a topic the service doesn't allow, in plain words the
    /// User can act on.
    fn check<'a>(
        &'a self,
        settings: &'a Settings,
        personal: &'a BTreeMap<String, String>,
    ) -> BoxFuture<'a, Result<(), PluginError>>;
}

#[cfg(test)]
mod tests {
    use super::{
        SettingField, SettingFlaw, SettingKind, SettingValue, Settings, SettingsError,
        SettingsSchema,
    };

    fn text(text: &str) -> SettingValue {
        SettingValue::Text(text.to_owned())
    }

    fn required(key: &str, kind: SettingKind) -> SettingField {
        SettingField {
            key: key.to_owned(),
            label: key.to_owned(),
            description: String::new(),
            kind,
            required: true,
            default: None,
            min: None,
            max: None,
            choices: Vec::new(),
        }
    }

    fn days(min: Option<i64>, max: Option<i64>) -> SettingsSchema {
        SettingsSchema {
            fields: vec![SettingField {
                min,
                max,
                ..required("days", SettingKind::Integer)
            }],
        }
    }

    fn resolve_days(schema: &SettingsSchema, days: i64) -> Result<Settings, SettingsError> {
        Settings::new([("days".to_owned(), SettingValue::Integer(days))].into()).resolve(schema)
    }

    fn out_of_range(min: Option<i64>, max: Option<i64>) -> SettingsError {
        SettingsError::OutOfRange {
            key: "days".to_owned(),
            min,
            max,
        }
    }

    #[test]
    fn an_integer_outside_its_bounds_is_refused_naming_the_setting() {
        let schema = days(Some(0), Some(365));

        assert_eq!(
            resolve_days(&schema, -1),
            Err(out_of_range(Some(0), Some(365)))
        );
        assert_eq!(
            resolve_days(&schema, 366),
            Err(out_of_range(Some(0), Some(365)))
        );
    }

    #[test]
    fn an_integer_equal_to_either_bound_is_taken() {
        let schema = days(Some(0), Some(365));

        assert_eq!(resolve_days(&schema, 0).err(), None);
        assert_eq!(resolve_days(&schema, 365).err(), None);
    }

    #[test]
    fn an_integer_without_bounds_takes_any_value() {
        let schema = days(None, None);

        assert_eq!(resolve_days(&schema, i64::MIN).err(), None);
        assert_eq!(resolve_days(&schema, i64::MAX).err(), None);
    }

    #[test]
    fn a_missing_setting_is_reported_before_one_out_of_range() {
        let mut schema = days(Some(0), None);
        schema.fields.push(required("api_key", SettingKind::Secret));

        assert_eq!(
            resolve_days(&schema, -5),
            Err(SettingsError::Missing("api_key".to_owned()))
        );
    }

    #[test]
    fn an_integer_out_of_range_is_refused_in_plain_words() {
        let refusals = [
            out_of_range(Some(0), None),
            out_of_range(None, Some(10)),
            out_of_range(Some(1), Some(10)),
        ]
        .map(|err| err.to_string());

        assert_eq!(
            refusals,
            [
                "the setting days must be at least 0",
                "the setting days must be at most 10",
                "the setting days must be from 1 to 10",
            ]
        );
    }

    #[test]
    fn a_missing_setting_is_reported_even_after_one_of_the_wrong_kind() {
        let schema = SettingsSchema {
            fields: vec![
                required("port", SettingKind::Integer),
                required("api_key", SettingKind::Secret),
            ],
        };
        let settings = Settings::new([("port".to_owned(), text("8989"))].into());

        assert_eq!(
            settings.resolve(&schema),
            Err(SettingsError::Missing("api_key".to_owned()))
        );
    }

    #[test]
    fn a_setting_of_the_wrong_kind_is_refused_in_plain_words() {
        let err = SettingsError::WrongKind {
            key: "sonarr".to_owned(),
            expected: SettingKind::Url,
        };

        assert_eq!(
            err.to_string(),
            "the setting sonarr must be an http or https URL"
        );
    }

    #[test]
    fn a_default_is_a_flaw_only_when_not_of_its_settings_kind() {
        let flaw = |kind, default| {
            SettingField {
                default: Some(default),
                ..required("port", kind)
            }
            .flaw()
        };
        let wrong_kind = |kind| {
            Some(SettingFlaw::DefaultWrongKind {
                key: "port".to_owned(),
                kind,
            })
        };

        assert_eq!(
            flaw(SettingKind::Integer, text("8989")),
            wrong_kind(SettingKind::Integer)
        );
        assert_eq!(
            flaw(SettingKind::Url, text("sonarr.lan")),
            wrong_kind(SettingKind::Url)
        );
        assert_eq!(
            flaw(SettingKind::Integer, SettingValue::Integer(8989)),
            None
        );
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
    fn a_libraries_setting_takes_only_library_ids() {
        assert!(SettingValue::from(vec![1, 2]).is(SettingKind::Libraries));
        assert!(SettingValue::Libraries(Vec::new()).is(SettingKind::Libraries));

        assert!(!text("1,2").is(SettingKind::Libraries));
        assert!(!SettingValue::Integer(1).is(SettingKind::Libraries));
        assert!(!SettingValue::Libraries(vec![1]).is(SettingKind::Text));
    }

    #[test]
    fn a_libraries_setting_is_read_as_ids() {
        let settings =
            Settings::new([("keep".to_owned(), SettingValue::Libraries(vec![4, 2]))].into());

        assert_eq!(settings.library_ids("keep"), Some([4, 2].as_slice()));
        assert_eq!(settings.text("keep"), None);
        assert_eq!(settings.library_ids("missing"), None);
    }

    #[test]
    fn a_choices_setting_takes_only_picked_values() {
        assert!(SettingValue::Texts(vec!["G".to_owned()]).is(SettingKind::Choices));
        assert!(SettingValue::Texts(Vec::new()).is(SettingKind::Choices));

        assert!(!text("G").is(SettingKind::Choices));
        assert!(!SettingValue::Libraries(Vec::new()).is(SettingKind::Choices));
        assert!(!SettingValue::Texts(Vec::new()).is(SettingKind::Text));
    }

    #[test]
    fn a_required_choices_setting_with_none_picked_is_missing() {
        let schema = SettingsSchema {
            fields: vec![required("ratings", SettingKind::Choices)],
        };
        let none = Settings::new([("ratings".to_owned(), SettingValue::Texts(Vec::new()))].into());

        assert_eq!(
            none.resolve(&schema),
            Err(SettingsError::Missing("ratings".to_owned()))
        );
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
