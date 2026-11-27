//! Films and series, loaded from Wikidata's weekly dump, the External ID crosswalk between them,
//! and the vectors that say which are alike (ADR 0095).

mod vectors;
mod wikidata;

use sqlx::PgPool;

pub use vectors::{PooledCount, RecommendedTitle, SimilarTitle, recommended, similar, train};
pub use wikidata::{ImportError, import};

/// Whether a title is a Movie or a Show, whose External IDs are separate id spaces (ADR 0042).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TitleKind {
    /// A film.
    Movie,
    /// A series.
    Show,
}

impl TitleKind {
    /// Its value in `titles.kind`.
    fn as_str(self) -> &'static str {
        match self {
            Self::Movie => "movie",
            Self::Show => "show",
        }
    }

    /// The kind `titles.kind` holds as `stored`.
    fn stored(stored: &str) -> Self {
        if stored == Self::Movie.as_str() {
            Self::Movie
        } else {
            Self::Show
        }
    }
}

/// A title's id at a metadata source: a namespace, such as `tmdb`, and a value (ADR 0042).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExternalId {
    /// The metadata source, such as `tmdb`, `imdb` or `tvdb`.
    pub namespace: String,
    /// The title's id there, such as `603`.
    pub value: String,
}

impl ExternalId {
    /// The most characters an External ID's text form may have.
    pub const MAX_LENGTH: usize = 100;

    /// The External ID `text` writes as `namespace-value`, or `None` if it isn't one: 1 to
    /// [`Self::MAX_LENGTH`] characters, split at the first `-`, after a Namespace of `a-z` and
    /// `0-9` and before a value that isn't empty (ADR 0042).
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let (namespace, value) = text.split_once('-')?;
        let namespace_is_valid = !namespace.is_empty()
            && (namespace.bytes()).all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit());
        if !namespace_is_valid || value.is_empty() || text.chars().count() > Self::MAX_LENGTH {
            return None;
        }
        Some(Self {
            namespace: namespace.to_owned(),
            value: value.to_owned(),
        })
    }
}

/// The ids in `namespaces`, ordered by namespace, of the `kind` of title that `id` names: the
/// External IDs a Server uses for a title it knows by any other (ADR 0042, 0094).
///
/// Empty when Wikidata lacks the title, or gives it none of `namespaces`.
///
/// # Errors
///
/// If the query fails.
pub async fn crosswalk(
    db: &PgPool,
    kind: TitleKind,
    id: &ExternalId,
    namespaces: &[String],
) -> Result<Vec<ExternalId>, sqlx::Error> {
    // Wikidata can give two titles one id by mistake: this answers for one of them, the same one
    // each time.
    sqlx::query_as!(
        ExternalId,
        "SELECT namespace, value FROM title_ids
         WHERE namespace = ANY($4) AND title_id = (
             SELECT title_ids.title_id FROM title_ids
             JOIN titles ON titles.id = title_ids.title_id
             WHERE titles.kind = $1 AND title_ids.namespace = $2 AND title_ids.value = $3
             ORDER BY titles.wikidata_id LIMIT 1)
         ORDER BY namespace",
        kind.as_str(),
        id.namespace,
        id.value,
        namespaces,
    )
    .fetch_all(db)
    .await
}

#[cfg(test)]
#[path = "titles/titles_tests.rs"]
mod tests;
