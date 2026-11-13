//! A title's ids as namespace and value pairs (ADR 0042).

use std::{cmp::Ordering, fmt, str::FromStr};

/// The namespace of TMDB ids, whose values are numbers.
pub const TMDB: &str = "tmdb";
/// The namespace of IMDb ids, whose values look like `tt0078748`.
pub const IMDB: &str = "imdb";
/// The namespace of TVDB ids, whose values are numbers.
pub const TVDB: &str = "tvdb";

/// An id of a title in a metadata source: a namespace such as `tmdb` and a value its owner
/// defines. Movies and Shows have separate id spaces, so a kind travels beside it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExternalId {
    namespace: String,
    value: String,
}

/// Why a namespace and value aren't an [`ExternalId`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidId {
    /// The namespace is empty or has a character other than `a-z` and `0-9`.
    Namespace(String),
    /// The value is empty.
    EmptyValue,
    /// The text has no `-` between a namespace and a value.
    NoSeparator,
}

impl fmt::Display for InvalidId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Namespace(namespace) => write!(
                f,
                "the namespace {namespace:?} isn't one or more of a-z and 0-9"
            ),
            Self::EmptyValue => f.write_str("the id's value is empty"),
            Self::NoSeparator => f.write_str("the id has no `-` after its namespace"),
        }
    }
}

impl std::error::Error for InvalidId {}

impl ExternalId {
    /// The id `value` in `namespace`.
    ///
    /// # Errors
    ///
    /// When `namespace` isn't one or more of `a-z` and `0-9`, or `value` is empty.
    pub fn new(namespace: &str, value: impl Into<String>) -> Result<Self, InvalidId> {
        let well_formed = !namespace.is_empty()
            && namespace
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
        if !well_formed {
            return Err(InvalidId::Namespace(namespace.to_owned()));
        }
        let value = value.into();
        if value.is_empty() {
            return Err(InvalidId::EmptyValue);
        }
        Ok(Self {
            namespace: namespace.to_owned(),
            value,
        })
    }

    /// The TMDB id `id`.
    #[must_use]
    pub fn tmdb(id: i64) -> Self {
        Self::number(TMDB, id)
    }

    /// The TVDB id `id`.
    #[must_use]
    pub fn tvdb(id: i64) -> Self {
        Self::number(TVDB, id)
    }

    fn number(namespace: &str, id: i64) -> Self {
        Self {
            namespace: namespace.to_owned(),
            value: id.to_string(),
        }
    }

    /// Transitional: the task `id-set-type` deletes this once callers use [`Self::tmdb`].
    #[doc(hidden)]
    #[expect(
        non_snake_case,
        reason = "stands in for the old enum variant until its callers move"
    )]
    #[must_use]
    pub fn Tmdb(id: i64) -> Self {
        Self::tmdb(id)
    }

    /// Transitional: the task `id-set-type` deletes this once callers use [`Self::tvdb`].
    #[doc(hidden)]
    #[expect(
        non_snake_case,
        reason = "stands in for the old enum variant until its callers move"
    )]
    #[must_use]
    pub fn Tvdb(id: i64) -> Self {
        Self::tvdb(id)
    }

    /// Its namespace, such as `tmdb`.
    #[must_use]
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// Its value, which only its namespace's owner interprets.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }
}

impl fmt::Display for ExternalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.namespace, self.value)
    }
}

impl FromStr for ExternalId {
    type Err = InvalidId;

    /// Reads `namespace-value`, split at the first `-`.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (namespace, value) = text.split_once('-').ok_or(InvalidId::NoSeparator)?;
        Self::new(namespace, value)
    }
}

impl Ord for ExternalId {
    // Shorter values first so numeric ids such as `tmdb-9` and `tmdb-10` sort as numbers.
    fn cmp(&self, other: &Self) -> Ordering {
        self.namespace
            .cmp(&other.namespace)
            .then_with(|| self.value.len().cmp(&other.value.len()))
            .then_with(|| self.value.cmp(&other.value))
    }
}

impl PartialOrd for ExternalId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Every id a title has, in the metadata sources a Provider knows about, at most one per
/// namespace. Watch State syncs by these (ADR 0008).
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct ExternalIds {
    /// The TMDB id.
    pub tmdb: Option<i64>,
    /// The IMDb id, such as `tt0078748`.
    pub imdb: Option<String>,
    /// The TVDB id.
    pub tvdb: Option<i64>,
}

impl ExternalIds {
    /// Its id in `namespace`, if it has one.
    #[must_use]
    pub fn get(&self, namespace: &str) -> Option<ExternalId> {
        match namespace {
            TMDB => self.tmdb.map(ExternalId::tmdb),
            TVDB => self.tvdb.map(ExternalId::tvdb),
            IMDB => self.imdb.as_ref().map(|imdb| ExternalId {
                namespace: IMDB.to_owned(),
                value: imdb.clone(),
            }),
            _ => None,
        }
    }

    /// Adds `id`, in place of any id it has in the same namespace.
    pub fn insert(&mut self, id: ExternalId) {
        // While the three fields last, an id they can't hold (another namespace, or a TMDB or
        // TVDB value that isn't a number) is dropped; the task `id-set-type` keeps every one.
        match id.namespace() {
            TMDB => {
                if let Ok(tmdb) = id.value.parse() {
                    self.tmdb = Some(tmdb);
                }
            }
            TVDB => {
                if let Ok(tvdb) = id.value.parse() {
                    self.tvdb = Some(tvdb);
                }
            }
            IMDB => self.imdb = Some(id.value),
            _ => {}
        }
    }

    /// Its ids, in namespace order.
    pub fn iter(&self) -> impl Iterator<Item = ExternalId> + '_ {
        [IMDB, TMDB, TVDB]
            .into_iter()
            .filter_map(|namespace| self.get(namespace))
    }

    /// Whether it has no ids.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.iter().next().is_none()
    }
}

impl FromIterator<ExternalId> for ExternalIds {
    fn from_iter<I: IntoIterator<Item = ExternalId>>(iter: I) -> Self {
        let mut set = Self::default();
        for id in iter {
            set.insert(id);
        }
        set
    }
}

#[cfg(test)]
#[path = "ids_tests.rs"]
mod tests;
