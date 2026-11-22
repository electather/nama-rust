//! Title vectors (ADR 0095): each title's Wikidata facts hashed into a sparse vector, which
//! pgvector searches for the titles most like one.

use std::{
    collections::BTreeMap,
    hash::{DefaultHasher, Hash as _, Hasher as _},
};

use futures_util::TryStreamExt as _;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::{ExternalId, TitleKind};

/// `title_vectors.embedding`'s dimensions, the most pgvector allows a `sparsevec`. Two facts
/// hashed to one dimension make titles sharing neither look alike, so rare facts that collide
/// would crowd out real answers; among a billion, two facts share one about once per billion
/// pairs.
const DIMENSIONS: u64 = 1_000_000_000;

const VECTORS_PER_STATEMENT: usize = 1_000;

/// The most titles a Similar or recommended answer holds: as many as the Server keeps of a
/// Similar one (ADR 0084).
const MOST_SIMILAR: i64 = 100;

/// A title recommended to an Account, by its id in a namespace the asking Server uses.
#[derive(Clone, Debug, PartialEq)]
pub struct RecommendedTitle {
    /// Whether it is a Movie or a Show.
    pub kind: TitleKind,
    /// Its id in the first of the asked namespaces the Hub knows it by.
    pub id: ExternalId,
    /// How near the taste vector, above 0.0 and at most 1.0: their cosine similarity.
    pub weight: f32,
}

/// A title like another, by its id in a namespace the asking Server uses.
#[derive(Clone, Debug, PartialEq)]
pub struct SimilarTitle {
    /// Its id in the first of the asked namespaces the Hub knows it by.
    pub id: ExternalId,
    /// How alike, above 0.0 and at most 1.0: their vectors' cosine similarity.
    pub weight: f32,
}

/// Builds every title's vector from its facts as the last import left them, in a new training
/// run that replaces the previous run's vectors at once, and returns how many titles have one.
///
/// Runs as the `train-vectors` subcommand, after each import, never in the serving process (ADR
/// 0096). Until it commits, the Hub answers from the previous run.
///
/// # Errors
///
/// If a query fails. The previous run's vectors stay.
pub async fn train(db: &PgPool) -> Result<usize, sqlx::Error> {
    let mut tx = db.begin().await?;
    sqlx::query!("DELETE FROM title_vectors")
        .execute(&mut *tx)
        .await?;
    let run = sqlx::query_scalar!("INSERT INTO training_runs DEFAULT VALUES RETURNING id")
        .fetch_one(&mut *tx)
        .await?;
    // A fact only one title has makes it like no other, so it's left out. Streamed on a
    // connection of its own, since `tx`'s is busy with `save` between rows.
    let mut titles = sqlx::query!(
        r#"SELECT title_id AS "title_id!", array_agg(property || ':' || item) AS "facts!",
                  array_agg(sharing) AS "sharing!"
           FROM (SELECT title_id, property, item,
                        count(*) OVER (PARTITION BY property, item) AS sharing
                 FROM title_facts) AS facts
           WHERE sharing > 1
           GROUP BY title_id"#
    )
    .fetch(db);
    let mut batch = Vec::with_capacity(VECTORS_PER_STATEMENT);
    let mut trained = 0;
    while let Some(title) = titles.try_next().await? {
        let Some(vector) = embed(title.facts.iter().zip(title.sharing)) else {
            continue;
        };
        batch.push((title.title_id, vector));
        if batch.len() == VECTORS_PER_STATEMENT {
            save(&mut tx, run, &batch).await?;
            trained += batch.len();
            batch.clear();
        }
    }
    save(&mut tx, run, &batch).await?;
    trained += batch.len();
    tx.commit().await?;
    Ok(trained)
}

/// A title's vector from its facts, each with how many titles share it: every fact hashed to one
/// of `DIMENSIONS`, weighing more the fewer titles share it. `None` without facts, since a zero
/// vector is like nothing.
///
/// ponytail: every property weighs the same, and only how rare a fact is weighs it; weigh
/// directors, cast and the rest apart once the offline evaluation (#1826) can measure it.
pub(super) fn embed<'a>(
    facts: impl IntoIterator<Item = (&'a String, i64)>,
) -> Option<BTreeMap<u64, f32>> {
    let mut vector = BTreeMap::new();
    for (fact, sharing) in facts {
        // Rust may change `DefaultHasher` between releases, which is harmless: a training run
        // hashes every title's facts itself.
        let mut hasher = DefaultHasher::new();
        fact.hash(&mut hasher);
        // pgvector numbers a `sparsevec`'s dimensions from 1.
        let dimension = hasher.finish() % DIMENSIONS + 1;
        #[expect(
            clippy::cast_precision_loss,
            reason = "a count of titles needn't be exact to weigh a fact"
        )]
        let sharing = sharing as f32;
        *vector.entry(dimension).or_insert(0.0) += 1.0 / sharing.ln_1p();
    }
    (!vector.is_empty()).then_some(vector)
}

/// Inserts `vectors`, each a title's, as made by training run `run`.
async fn save(
    tx: &mut PgConnection,
    run: Uuid,
    vectors: &[(Uuid, BTreeMap<u64, f32>)],
) -> Result<(), sqlx::Error> {
    let (titles, embeddings): (Vec<Uuid>, Vec<String>) = vectors
        .iter()
        .map(|(title, vector)| (*title, as_pgvector(vector)))
        .unzip();
    sqlx::query!(
        "INSERT INTO title_vectors (title_id, training_run_id, embedding)
         SELECT title_id, $1, embedding::sparsevec
         FROM UNNEST($2::uuid[], $3::text[]) AS vectors (title_id, embedding)",
        run,
        &titles as _,
        &embeddings as _,
    )
    .execute(tx)
    .await?;
    Ok(())
}

/// `vector` in pgvector's `sparsevec` text form, such as `{1:0.5,7:2}/1000000000`.
fn as_pgvector(vector: &BTreeMap<u64, f32>) -> String {
    let values: Vec<String> = vector
        .iter()
        .map(|(dimension, value)| format!("{dimension}:{value}"))
        .collect();
    format!("{{{}}}/{DIMENSIONS}", values.join(","))
}

/// The titles of `kind` most like the one `ids` names, by any of them, most alike first, each by
/// its id in the first of `ids`' namespaces it has (ADR 0042). Empty when Wikidata lacks the title
/// or it has no vector. Only titles sharing a fact with it are answered.
///
/// ponytail: compares every vector of the kind, which is exact and fast enough at Wikidata's
/// size; add an HNSW index once a Similar answer is measured slow. pgvector's HNSW indexes a
/// `sparsevec` of at most 1,000 non-zero values, so a title with more facts needs them capped.
///
/// # Errors
///
/// If the query fails.
pub async fn similar(
    db: &PgPool,
    kind: TitleKind,
    ids: &[ExternalId],
) -> Result<Vec<SimilarTitle>, sqlx::Error> {
    let (namespaces, values): (Vec<&str>, Vec<&str>) = ids
        .iter()
        .map(|id| (id.namespace.as_str(), id.value.as_str()))
        .unzip();
    let found = sqlx::query!(
        r#"WITH asked AS (
               SELECT title_vectors.title_id, title_vectors.embedding FROM title_ids
               JOIN titles ON titles.id = title_ids.title_id
               JOIN title_vectors ON title_vectors.title_id = titles.id
               JOIN UNNEST($2::text[], $3::text[]) AS asked_ids (namespace, value)
                   ON asked_ids.namespace = title_ids.namespace
                   AND asked_ids.value = title_ids.value
               WHERE titles.kind = $1
               ORDER BY titles.wikidata_id LIMIT 1),
           nearest AS (
               SELECT alike.title_id, alike.embedding <=> asked.embedding AS distance
               FROM asked
               JOIN title_vectors AS alike ON alike.title_id <> asked.title_id
               JOIN titles ON titles.id = alike.title_id
               WHERE titles.kind = $1 AND alike.embedding <=> asked.embedding < $4
                   AND EXISTS (SELECT 1 FROM title_ids
                               WHERE title_ids.title_id = alike.title_id
                               AND title_ids.namespace = ANY($2))
               ORDER BY distance, alike.title_id LIMIT $5)
           SELECT id.namespace AS "namespace!", id.value AS "value!",
                  (1 - nearest.distance)::real AS "weight!"
           FROM nearest
           CROSS JOIN LATERAL (
               SELECT namespace, value FROM title_ids
               WHERE title_ids.title_id = nearest.title_id AND namespace = ANY($2)
               ORDER BY array_position($2, namespace) LIMIT 1) AS id
           ORDER BY nearest.distance, nearest.title_id"#,
        kind.as_str(),
        &namespaces as _,
        &values as _,
        // Under 1: sharing at least one fact, as every value is positive.
        1.0_f64,
        MOST_SIMILAR,
    )
    .fetch_all(db)
    .await?;
    Ok(found
        .into_iter()
        .map(|similar| SimilarTitle {
            id: ExternalId {
                namespace: similar.namespace,
                value: similar.value,
            },
            weight: similar.weight,
        })
        .collect())
}

/// The titles nearest the taste vector of an Account that `finished` the titles these name, best
/// first, leaving out those, each by its id in the first of `namespaces` it has (ADR 0042). Empty
/// when none of `finished` has a vector. Only titles sharing a fact with one of them are answered.
///
/// ponytail: the taste vector is built on each call, never stored, so deleting a Viewing Event or
/// a new training run changes the next answer; store it once building it is measured slow.
///
/// # Errors
///
/// If a query fails.
pub async fn recommended(
    db: &PgPool,
    finished: &[(TitleKind, ExternalId)],
    namespaces: &[String],
) -> Result<Vec<RecommendedTitle>, sqlx::Error> {
    let (kinds, (finished_namespaces, values)): (Vec<&str>, (Vec<&str>, Vec<&str>)) = finished
        .iter()
        .map(|(kind, id)| (kind.as_str(), (id.namespace.as_str(), id.value.as_str())))
        .unzip();
    let finished_vectors = sqlx::query!(
        r#"SELECT DISTINCT title_vectors.title_id AS "title_id!",
                  title_vectors.embedding::text AS "embedding!"
           FROM UNNEST($1::text[], $2::text[], $3::text[]) AS finished (kind, namespace, value)
           JOIN title_ids ON title_ids.namespace = finished.namespace
               AND title_ids.value = finished.value
           JOIN titles ON titles.id = title_ids.title_id AND titles.kind = finished.kind
           JOIN title_vectors ON title_vectors.title_id = titles.id"#,
        &kinds as _,
        &finished_namespaces as _,
        &values as _,
    )
    .fetch_all(db)
    .await?;
    let Some(taste) = taste(
        finished_vectors
            .iter()
            .map(|title| title.embedding.as_str()),
    ) else {
        return Ok(Vec::new());
    };
    let finished_titles: Vec<Uuid> = finished_vectors
        .iter()
        .map(|title| title.title_id)
        .collect();
    let found = sqlx::query!(
        r#"WITH nearest AS (
               SELECT title_vectors.title_id, titles.kind,
                      title_vectors.embedding <=> $1::text::sparsevec AS distance
               FROM title_vectors
               JOIN titles ON titles.id = title_vectors.title_id
               WHERE title_vectors.title_id <> ALL($2)
                   AND title_vectors.embedding <=> $1::text::sparsevec < $4
                   AND EXISTS (SELECT 1 FROM title_ids
                               WHERE title_ids.title_id = title_vectors.title_id
                               AND title_ids.namespace = ANY($3))
               ORDER BY distance, title_vectors.title_id LIMIT $5)
           SELECT nearest.kind AS "kind!", id.namespace AS "namespace!", id.value AS "value!",
                  (1 - nearest.distance)::real AS "weight!"
           FROM nearest
           CROSS JOIN LATERAL (
               SELECT namespace, value FROM title_ids
               WHERE title_ids.title_id = nearest.title_id AND namespace = ANY($3)
               ORDER BY array_position($3, namespace) LIMIT 1) AS id
           ORDER BY nearest.distance, nearest.title_id"#,
        as_pgvector(&taste),
        &finished_titles,
        namespaces,
        // Under 1: sharing at least one fact, as every value is positive.
        1.0_f64,
        MOST_SIMILAR,
    )
    .fetch_all(db)
    .await?;
    Ok(found
        .into_iter()
        .map(|recommended| RecommendedTitle {
            kind: TitleKind::stored(&recommended.kind),
            id: ExternalId {
                namespace: recommended.namespace,
                value: recommended.value,
            },
            weight: recommended.weight,
        })
        .collect())
}

/// The taste vector of an Account that finished titles whose vectors, in pgvector's `sparsevec`
/// text form, are `finished`: their sum, each at length 1 so a title with many facts weighs no more
/// than one with few. `None` without any.
///
/// ponytail: every finished title weighs the same, however often or long ago; weigh by recency
/// once the offline evaluation (#1826) can measure it.
pub(super) fn taste<'a>(finished: impl IntoIterator<Item = &'a str>) -> Option<BTreeMap<u64, f32>> {
    let mut taste = BTreeMap::new();
    for vector in finished {
        let vector = from_pgvector(vector);
        let length = vector
            .iter()
            .map(|(_, value)| value * value)
            .sum::<f32>()
            .sqrt();
        for (dimension, value) in vector {
            *taste.entry(dimension).or_insert(0.0) += value / length;
        }
    }
    (!taste.is_empty()).then_some(taste)
}

/// The dimensions and values of `vector`, in the text form [`as_pgvector`] writes and pgvector
/// prints.
fn from_pgvector(vector: &str) -> Vec<(u64, f32)> {
    let values = vector
        .split_once('}')
        .map_or("", |(values, _dimensions)| values.trim_start_matches('{'));
    values
        .split(',')
        .filter_map(|entry| {
            let (dimension, value) = entry.split_once(':')?;
            Some((dimension.parse().ok()?, value.parse().ok()?))
        })
        .collect()
}

#[cfg(test)]
#[path = "vectors_tests.rs"]
mod tests;
