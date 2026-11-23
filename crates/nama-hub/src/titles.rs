//! Films and series, loaded from Wikidata's weekly dump, the External ID crosswalk between them,
//! and the vectors that say which are alike (ADR 0095).

mod vectors;
mod wikidata;

use sqlx::PgPool;

pub use vectors::{PooledCount, SimilarTitle, similar, train};
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
}

/// A title's id at a metadata source: a namespace, such as `tmdb`, and a value (ADR 0042).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExternalId {
    /// The metadata source, such as `tmdb`, `imdb` or `tvdb`.
    pub namespace: String,
    /// The title's id there, such as `603`.
    pub value: String,
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
