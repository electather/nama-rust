//! Title vectors (ADR 0095): each title's Wikidata facts hashed into a vector, which pgvector
//! searches for the titles most like one.

use std::hash::{DefaultHasher, Hash as _, Hasher as _};

use futures_util::TryStreamExt as _;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::{ExternalId, TitleKind};

/// `title_vectors.embedding`'s dimensions.
const DIMENSIONS: usize = 256;

const VECTORS_PER_STATEMENT: usize = 1_000;

/// The most titles a Similar answer holds: as many as the Server keeps of one (ADR 0084).
const MOST_SIMILAR: i64 = 100;

/// The least weight a similar title has. Facts hashed to one dimension make titles sharing no
/// fact look slightly alike, and this keeps them out.
const LEAST_WEIGHT: f32 = 0.1;

/// A title like another, by its id in a namespace the asking Server uses.
#[derive(Clone, Debug, PartialEq)]
pub struct SimilarTitle {
    /// Its id in the first of the asked namespaces the Hub knows it by.
    pub id: ExternalId,
    /// How alike, from 0.1 to 1.0: their vectors' cosine similarity.
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
    // A fact only one title has makes it like no other, so it's left out. Read on a connection of
    // its own while `tx` writes.
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
/// dimension, with a sign, weighing more the fewer titles share it. `None` when the facts cancel
/// out, since a zero vector is like nothing.
///
/// shortcut: every property weighs the same, and only how rare a fact is weighs it; weigh
/// directors, cast and the rest apart once the offline evaluation (#1826) can measure it.
pub(super) fn embed<'a>(facts: impl IntoIterator<Item = (&'a String, i64)>) -> Option<Vec<f32>> {
    let mut vector = vec![0.0_f32; DIMENSIONS];
    for (fact, sharing) in facts {
        // Rust may change `DefaultHasher` between releases, which is harmless: a training run
        // hashes every title's facts itself.
        let mut hasher = DefaultHasher::new();
        fact.hash(&mut hasher);
        let hash = hasher.finish();
        let sign = if hash >> 63 == 0 { 1.0 } else { -1.0 };
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_precision_loss,
            reason = "the remainder is below `DIMENSIONS`, and a count of titles needn't be exact \
                      to weigh a fact"
        )]
        let (dimension, sharing) = ((hash % DIMENSIONS as u64) as usize, sharing as f32);
        if let Some(value) = vector.get_mut(dimension) {
            *value += sign / sharing.ln_1p();
        }
    }
    vector
        .iter()
        .any(|value| value.abs() > f32::EPSILON)
        .then_some(vector)
}

/// Inserts `vectors`, each a title's, as made by training run `run`.
async fn save(
    tx: &mut PgConnection,
    run: Uuid,
    vectors: &[(Uuid, Vec<f32>)],
) -> Result<(), sqlx::Error> {
    let (titles, embeddings): (Vec<Uuid>, Vec<String>) = vectors
        .iter()
        .map(|(title, vector)| (*title, as_pgvector(vector)))
        .unzip();
    sqlx::query!(
        "INSERT INTO title_vectors (title_id, training_run_id, embedding)
         SELECT title_id, $1, embedding::vector
         FROM UNNEST($2::uuid[], $3::text[]) AS vectors (title_id, embedding)",
        run,
        &titles as _,
        &embeddings as _,
    )
    .execute(tx)
    .await?;
    Ok(())
}

/// `vector` in pgvector's text form, such as `[0.5,-1]`.
fn as_pgvector(vector: &[f32]) -> String {
    let values: Vec<String> = vector.iter().map(f32::to_string).collect();
    format!("[{}]", values.join(","))
}

/// The titles of `kind` most like the one `ids` names, by any of them, most alike first, each by
/// its id in the first of `ids`' namespaces it has (ADR 0042). Empty when Wikidata lacks the title
/// or it has no vector.
///
/// ponytail: compares every vector of the kind, which is exact and fast enough at Wikidata's
/// size; add an HNSW index once a Similar answer is measured slow.
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
               SELECT similar.title_id, similar.embedding <=> asked.embedding AS distance
               FROM asked
               JOIN title_vectors AS similar ON similar.title_id <> asked.title_id
               JOIN titles ON titles.id = similar.title_id
               WHERE titles.kind = $1 AND similar.embedding <=> asked.embedding < $4
               ORDER BY distance, similar.title_id LIMIT $5)
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
        f64::from(1.0 - LEAST_WEIGHT),
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

#[cfg(test)]
#[path = "vectors_tests.rs"]
mod tests;
