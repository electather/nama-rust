//! Loading Wikidata's JSON dump: a JSON array holding one entity per line.

use std::io;

use serde::Deserialize;
use sqlx::{PgConnection, PgPool};
use tokio::io::{AsyncBufRead, AsyncBufReadExt as _};

use super::{ExternalId, TitleKind};

/// The properties holding the External IDs kept. An entity naming none is skipped without being
/// parsed, which is most of the dump, since a title without one can't be asked about.
const ID_PROPERTIES: [&str; 5] = [
    r#""P345""#,
    r#""P4947""#,
    r#""P4983""#,
    r#""P12196""#,
    r#""P4835""#,
];

// ponytail: only these classes, not their subclasses, which would need the whole class tree;
// a TMDB or TVDB id also says the kind. Add a class when titles Servers ask about go missing.
/// Wikidata classes whose instances are Movies: film, feature, short, television, animated and
/// documentary film.
const FILM_CLASSES: [&str; 6] = ["Q11424", "Q24869", "Q24862", "Q506240", "Q202866", "Q93204"];
/// Wikidata classes whose instances are Shows: television series, miniseries, animated and anime
/// television series.
const SERIES_CLASSES: [&str; 4] = ["Q5398426", "Q1259759", "Q117467246", "Q63952888"];

const TITLES_PER_STATEMENT: usize = 10_000;

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
/// ponytail: that transaction lasts hours, and a deploy's migration touching these tables waits
/// for it; deploy such a migration between imports. Stage into a table of its own and swap once
/// imports must not hold locks that long.
///
/// # Errors
///
/// If `dump` can't be read, holds a malformed entity or is cut short, or saving fails.
pub async fn import(db: &PgPool, dump: impl AsyncBufRead + Unpin) -> Result<usize, ImportError> {
    let mut tx = db.begin().await?;
    // Every title's ids and facts are written again, so none the dump dropped are left.
    sqlx::query!("DELETE FROM title_ids")
        .execute(&mut *tx)
        .await?;
    sqlx::query!("DELETE FROM title_facts")
        .execute(&mut *tx)
        .await?;
    let mut lines = dump.lines();
    let mut line_number = 0;
    let mut batch = Vec::with_capacity(TITLES_PER_STATEMENT);
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
        let title = parse_title(entity).map_err(|source| ImportError::Malformed {
            line: line_number,
            source,
        })?;
        batch.extend(title);
        if batch.len() == TITLES_PER_STATEMENT {
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

/// A film or series the dump holds, with its External IDs and the facts its vector is built from.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Title {
    /// Its Wikidata item, such as `Q83495`.
    pub(super) wikidata_id: String,
    pub(super) kind: TitleKind,
    pub(super) ids: Vec<ExternalId>,
    /// Sorted, each once.
    pub(super) facts: Vec<Fact>,
}

/// Something Wikidata says about a title: an item, such as a person or a genre, and what it is to
/// the title.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Fact {
    /// Such as `director`: `title_facts.property`.
    pub(super) property: &'static str,
    /// Its Wikidata item, such as `Q9545711`.
    pub(super) item: String,
}

/// The title one line's entity is, or `None` if it's neither a film nor a series with an External
/// ID.
pub(super) fn parse_title(entity: &str) -> Result<Option<Title>, serde_json::Error> {
    if !ID_PROPERTIES
        .iter()
        .any(|property| entity.contains(property))
    {
        return Ok(None);
    }
    let entity: Entity = serde_json::from_str(entity)?;
    Ok(entity.into_title())
}

/// Upserts `titles` and inserts their ids and facts.
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

    let ((holders, properties), items): ((Vec<&str>, Vec<&str>), Vec<&str>) = titles
        .iter()
        .flat_map(|title| {
            title.facts.iter().map(|fact| {
                (
                    (title.wikidata_id.as_str(), fact.property),
                    fact.item.as_str(),
                )
            })
        })
        .unzip();
    sqlx::query!(
        "INSERT INTO title_facts (title_id, property, item)
         SELECT titles.id, facts.property, facts.item
         FROM UNNEST($1::text[], $2::text[], $3::text[]) AS facts (wikidata_id, property, item)
         JOIN titles USING (wikidata_id)",
        &holders as _,
        &properties as _,
        &items as _,
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
    #[serde(rename = "P12196")]
    tvdb_movie: Vec<Statement<String>>,
    #[serde(rename = "P4835")]
    tvdb_series: Vec<Statement<String>>,
    #[serde(rename = "P161")]
    cast: Vec<Statement<ItemRef>>,
    #[serde(rename = "P57")]
    directors: Vec<Statement<ItemRef>>,
    #[serde(rename = "P58")]
    writers: Vec<Statement<ItemRef>>,
    #[serde(rename = "P136")]
    genres: Vec<Statement<ItemRef>>,
    #[serde(rename = "P921")]
    subjects: Vec<Statement<ItemRef>>,
    #[serde(rename = "P166")]
    awards: Vec<Statement<ItemRef>>,
    #[serde(rename = "P8345")]
    franchises: Vec<Statement<ItemRef>>,
    /// "Part of the series", which is how most films name their franchise.
    #[serde(rename = "P179")]
    series: Vec<Statement<ItemRef>>,
    #[serde(rename = "P495")]
    countries: Vec<Statement<ItemRef>>,
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
        let claims = self.claims;
        // Each kind's TMDB and TVDB ids: both number Movies and Shows apart (ADR 0042).
        let film_ids = (best_value(claims.tmdb_movie), best_value(claims.tvdb_movie));
        let series_ids = (
            best_value(claims.tmdb_series),
            best_value(claims.tvdb_series),
        );
        // An id Wikidata deprecates says no kind, as it is often one put on the wrong title.
        let movie = film_ids != (None, None) || is_instance_of(&claims.instance_of, &FILM_CLASSES);
        let show =
            series_ids != (None, None) || is_instance_of(&claims.instance_of, &SERIES_CLASSES);
        let (kind, ids_of_kind) = match (movie, show) {
            (true, false) => (TitleKind::Movie, film_ids),
            (false, true) => (TitleKind::Show, series_ids),
            _ => return None,
        };
        let ids: Vec<_> = [
            ("imdb", best_value(claims.imdb)),
            ("tmdb", ids_of_kind.0),
            ("tvdb", ids_of_kind.1),
        ]
        .into_iter()
        .filter_map(|(namespace, value)| {
            Some(ExternalId {
                namespace: namespace.to_owned(),
                value: value?,
            })
        })
        .collect();
        if ids.is_empty() {
            return None;
        }
        let mut facts: Vec<Fact> = [
            ("cast", claims.cast),
            ("director", claims.directors),
            ("writer", claims.writers),
            ("genre", claims.genres),
            ("subject", claims.subjects),
            ("award", claims.awards),
            ("franchise", claims.franchises),
            ("franchise", claims.series),
            ("country", claims.countries),
        ]
        .into_iter()
        .flat_map(|(property, statements)| {
            items(statements).map(move |item| Fact { property, item })
        })
        .collect();
        facts.sort_unstable();
        facts.dedup();
        Some(Title {
            wikidata_id: self.id,
            kind,
            ids,
            facts,
        })
    }
}

/// The items `statements` name, but those Wikidata deprecates.
fn items(statements: Vec<Statement<ItemRef>>) -> impl Iterator<Item = String> {
    statements
        .into_iter()
        .filter(|statement| statement.rank != Rank::Deprecated)
        .filter_map(|statement| Some(statement.mainsnak.datavalue?.value.id))
}

fn is_instance_of(instance_of: &[Statement<ItemRef>], classes: &[&str]) -> bool {
    instance_of
        .iter()
        .filter(|statement| statement.rank != Rank::Deprecated)
        .filter_map(|statement| statement.mainsnak.datavalue.as_ref())
        .any(|class| classes.contains(&class.value.id.as_str()))
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
#[path = "wikidata_tests.rs"]
mod tests;
