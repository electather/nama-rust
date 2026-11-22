//! Loading Wikidata's JSON dump: a JSON array holding one entity per line.

use std::io;

use serde::Deserialize;
use sqlx::{PgConnection, PgPool};
use tokio::io::{AsyncBufRead, AsyncBufReadExt as _};

use super::{ExternalId, TitleKind};

/// The properties holding the External IDs kept. An entity naming none is skipped without being
/// parsed, which is most of the dump, since a title without one can't be asked about.
const ID_PROPERTIES: [&str; 4] = [r#""P345""#, r#""P4947""#, r#""P4983""#, r#""P4835""#];

// shortcut: only these classes, not their subclasses, which would need the whole class tree;
// a TMDB or TVDB id also says the kind. Add a class when titles Servers ask about go missing.
/// Wikidata classes whose instances are Movies: film, feature, short, television, animated and
/// documentary film.
const FILM_CLASSES: [&str; 6] = ["Q11424", "Q24869", "Q24862", "Q506240", "Q202866", "Q93204"];
/// Wikidata classes whose instances are Shows: television series, miniseries, animated and anime
/// television series.
const SERIES_CLASSES: [&str; 4] = ["Q5398426", "Q1259759", "Q117467246", "Q63952888"];

/// Titles saved in one statement.
const BATCH: usize = 10_000;

/// Why an import stopped. It leaves the titles as they were.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    /// Reading the dump failed.
    #[error("could not read the dump: {0}")]
    Read(#[from] io::Error),
    /// A line naming an External ID isn't an entity Wikidata writes.
    #[error("line {line} of the dump isn't a Wikidata entity: {source}")]
    Malformed {
        /// Its line number, from 1.
        line: u64,
        /// What doesn't parse.
        source: serde_json::Error,
    },
    /// The dump ends before the `]` closing its array, as a failed download does.
    #[error("the dump ends before its closing `]`, so it was cut short")]
    Truncated,
    /// Saving the titles failed.
    #[error("could not save the titles: {0}")]
    Save(#[from] sqlx::Error),
}

/// Replaces the Hub's titles with the films and series in `dump`, Wikidata's JSON dump of every
/// entity (`latest-all.json`), and returns how many it holds. A title keeps its row across
/// imports; one the dump no longer holds is deleted.
///
/// Runs as the `import-wikidata` subcommand, which the host's scheduler starts weekly, never in
/// the serving process (ADR 0096). All of it is one transaction, so the Hub answers from the last
/// import until this one commits.
///
/// # Errors
///
/// If `dump` can't be read, holds a malformed entity or is cut short, or saving fails.
pub async fn import(db: &PgPool, dump: impl AsyncBufRead + Unpin) -> Result<usize, ImportError> {
    let mut tx = db.begin().await?;
    // Every title's ids are written again, so none the dump dropped are left.
    sqlx::query!("DELETE FROM title_ids")
        .execute(&mut *tx)
        .await?;
    let mut lines = dump.lines();
    let mut line_number = 0;
    let mut batch = Vec::with_capacity(BATCH);
    let mut imported = 0;
    loop {
        let line = lines.next_line().await?.ok_or(ImportError::Truncated)?;
        line_number += 1;
        let entity = line.trim_end().trim_end_matches(',');
        match entity {
            "[" => continue,
            "]" => break,
            _ => {}
        }
        let title = title(entity).map_err(|source| ImportError::Malformed {
            line: line_number,
            source,
        })?;
        batch.extend(title);
        if batch.len() == BATCH {
            save(&mut tx, &batch).await?;
            imported += batch.len();
            batch.clear();
        }
    }
    save(&mut tx, &batch).await?;
    imported += batch.len();
    // `now()` is when the transaction began, so it marks every title this import saved.
    sqlx::query!("DELETE FROM titles WHERE imported_at <> now()")
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(imported)
}

/// A film or series the dump holds, with its External IDs.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Title {
    /// Its Wikidata item, such as `Q83495`.
    pub(super) wikidata_id: String,
    pub(super) kind: TitleKind,
    pub(super) ids: Vec<ExternalId>,
}

/// The title one line's entity is, or `None` if it's neither a film nor a series with an External
/// ID.
pub(super) fn title(entity: &str) -> Result<Option<Title>, serde_json::Error> {
    if !ID_PROPERTIES
        .iter()
        .any(|property| entity.contains(property))
    {
        return Ok(None);
    }
    let entity: Entity = serde_json::from_str(entity)?;
    Ok(entity.into_title())
}

/// Upserts `titles` and inserts their ids.
async fn save(tx: &mut PgConnection, titles: &[Title]) -> Result<(), sqlx::Error> {
    let (wikidata_ids, kinds): (Vec<&str>, Vec<&str>) = titles
        .iter()
        .map(|title| (title.wikidata_id.as_str(), title.kind.as_str()))
        .unzip();
    sqlx::query!(
        "INSERT INTO titles (wikidata_id, kind) SELECT * FROM UNNEST($1::text[], $2::text[])
         ON CONFLICT (wikidata_id) DO UPDATE SET kind = EXCLUDED.kind, imported_at = now()",
        &wikidata_ids as _,
        &kinds as _,
    )
    .execute(&mut *tx)
    .await?;

    let ((owners, namespaces), values): ((Vec<&str>, Vec<&str>), Vec<&str>) = titles
        .iter()
        .flat_map(|title| {
            title.ids.iter().map(|id| {
                (
                    (title.wikidata_id.as_str(), id.namespace.as_str()),
                    id.value.as_str(),
                )
            })
        })
        .unzip();
    sqlx::query!(
        "INSERT INTO title_ids (title_id, namespace, value)
         SELECT titles.id, ids.namespace, ids.value
         FROM UNNEST($1::text[], $2::text[], $3::text[]) AS ids (wikidata_id, namespace, value)
         JOIN titles USING (wikidata_id)",
        &owners as _,
        &namespaces as _,
        &values as _,
    )
    .execute(&mut *tx)
    .await?;
    Ok(())
}

/// The parts of a Wikidata entity the import reads.
#[derive(Deserialize)]
struct Entity {
    id: String,
    #[serde(default)]
    claims: Claims,
}

/// An entity's statements for the properties the import reads; serde skips the rest.
#[derive(Default, Deserialize)]
#[serde(default)]
struct Claims {
    #[serde(rename = "P31")]
    instance_of: Vec<Statement<ItemRef>>,
    #[serde(rename = "P345")]
    imdb: Vec<Statement<String>>,
    #[serde(rename = "P4947")]
    tmdb_movie: Vec<Statement<String>>,
    #[serde(rename = "P4983")]
    tmdb_series: Vec<Statement<String>>,
    #[serde(rename = "P4835")]
    tvdb_series: Vec<Statement<String>>,
}

#[derive(Deserialize)]
struct Statement<V> {
    mainsnak: Snak<V>,
    rank: Rank,
}

#[derive(Deserialize)]
struct Snak<V> {
    /// `None` for a statement of "unknown value" or "no value".
    datavalue: Option<DataValue<V>>,
}

#[derive(Deserialize)]
struct DataValue<V> {
    value: V,
}

#[derive(Deserialize)]
struct ItemRef {
    id: String,
}

/// Ordered best first.
#[derive(Clone, Copy, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
enum Rank {
    Preferred,
    Normal,
    Deprecated,
}

impl Entity {
    fn into_title(self) -> Option<Title> {
        let kind = self.claims.kind()?;
        let Claims {
            imdb,
            tmdb_movie,
            tmdb_series,
            tvdb_series,
            ..
        } = self.claims;
        let tmdb = match kind {
            TitleKind::Movie => tmdb_movie,
            TitleKind::Show => tmdb_series,
        };
        let ids: Vec<_> = [("imdb", imdb), ("tmdb", tmdb), ("tvdb", tvdb_series)]
            .into_iter()
            .filter_map(|(namespace, statements)| {
                Some(ExternalId {
                    namespace: namespace.to_owned(),
                    value: best_value(statements)?,
                })
            })
            .collect();
        (!ids.is_empty()).then_some(Title {
            wikidata_id: self.id,
            kind,
            ids,
        })
    }
}

impl Claims {
    /// `None` when nothing says, or one thing says Movie and another Show.
    fn kind(&self) -> Option<TitleKind> {
        let movie = !self.tmdb_movie.is_empty() || self.is_instance_of(&FILM_CLASSES);
        let show = !self.tmdb_series.is_empty()
            || !self.tvdb_series.is_empty()
            || self.is_instance_of(&SERIES_CLASSES);
        match (movie, show) {
            (true, false) => Some(TitleKind::Movie),
            (false, true) => Some(TitleKind::Show),
            _ => None,
        }
    }

    fn is_instance_of(&self, classes: &[&str]) -> bool {
        self.instance_of
            .iter()
            .filter(|statement| statement.rank != Rank::Deprecated)
            .filter_map(|statement| statement.mainsnak.datavalue.as_ref())
            .any(|class| classes.contains(&class.value.id.as_str()))
    }
}

/// The value of the best-ranked statement Wikidata doesn't deprecate, the first among equals,
/// since a title keeps at most one id per namespace (ADR 0042).
fn best_value(statements: Vec<Statement<String>>) -> Option<String> {
    statements
        .into_iter()
        .filter(|statement| statement.rank != Rank::Deprecated)
        .filter_map(|statement| Some((statement.rank, statement.mainsnak.datavalue?.value)))
        // ADR 0042: a value is never empty.
        .filter(|(_, value)| !value.is_empty())
        .min_by_key(|(rank, _)| *rank)
        .map(|(_, value)| value)
}

#[cfg(test)]
#[path = "import_tests.rs"]
mod tests;
