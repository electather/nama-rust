//! Title vectors (ADR 0095): each title's Wikidata facts and the Movies Accounts finished with it
//! hashed into a sparse vector, which pgvector searches for the titles most like one.

use std::{
    collections::{BTreeMap, HashMap},
    hash::{DefaultHasher, Hash, Hasher as _},
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

/// How many Accounts finish a title before its behaviour weighs as much as its facts; fewer, and
/// its facts weigh more, more, and its behaviour does.
///
/// ponytail: a guess. Tune it once Viewing Events can be measured against, which MovieLens can't
/// stand in for (ADR 0095).
const ACCOUNTS_WEIGHING_AS_MUCH_AS_FACTS: f32 = 100.0;

/// The most titles a Similar answer holds: as many as the Server keeps of one (ADR 0084).
const MOST_SIMILAR: i64 = 100;

/// A title like another, by its id in a namespace the asking Server uses.
#[derive(Clone, Debug, PartialEq)]
pub struct SimilarTitle {
    /// Its id in the first of the asked namespaces the Hub knows it by.
    pub id: ExternalId,
    /// How alike, above 0.0 and at most 1.0: their vectors' cosine similarity.
    pub weight: f32,
}

/// How many Accounts finished both Movies of a pair, or one Movie when `a` is `b`: a pooled count
/// (ADR 0093).
#[derive(Debug, PartialEq, Eq)]
pub struct PooledCount {
    /// One Movie's External ID as Servers send it, such as `tmdb-603`.
    pub a: String,
    /// The other's, or `a` again.
    pub b: String,
    /// How many Accounts finished both.
    pub accounts: i64,
}

/// A title a vector is trained for.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Trained {
    /// One Wikidata has.
    Wikidata(Uuid),
    /// A Movie Wikidata lacks, by the External ID Servers pool it by, which only behaviour trains.
    Lacking(ExternalId),
}

/// What pooled counts say about a title.
#[derive(Debug)]
struct Behaviour {
    /// How many Accounts finished it.
    accounts: i64,
    /// A dimension of its own, and one for each title Accounts finished with it.
    vector: BTreeMap<u64, f32>,
}

/// Builds every title's vector from its facts as the last import left them and from `pooled`,
/// which holds only counts of enough Accounts to read, in a new training run that replaces the
/// previous run's vectors at once, and returns how many titles have one. A Movie Wikidata lacks
/// gets one from `pooled` alone.
///
/// Runs as the `train-vectors` subcommand, after each import, never in the serving process (ADR
/// 0096). Until it commits, the Hub answers from the previous run.
///
/// # Errors
///
/// If a query fails. The previous run's vectors stay.
pub async fn train(db: &PgPool, pooled: &[PooledCount]) -> Result<usize, sqlx::Error> {
    let mut tx = db.begin().await?;
    let mut behaviours = behaviours(pooled, &movies_of(&mut tx, pooled).await?);
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
        let facts = embed(title.facts.iter().zip(title.sharing));
        let title = Trained::Wikidata(title.title_id);
        let Some(vector) = blend(facts, behaviours.remove(&title)) else {
            continue;
        };
        batch.push((title, vector));
        if batch.len() == VECTORS_PER_STATEMENT {
            save(&mut tx, run, &batch).await?;
            trained += batch.len();
            batch.clear();
        }
    }
    save(&mut tx, run, &batch).await?;
    trained += batch.len();
    let behaviour_alone: Vec<_> = (behaviours.into_iter())
        .map(|(title, behaviour)| (title, behaviour.vector))
        .collect();
    for chunk in behaviour_alone.chunks(VECTORS_PER_STATEMENT) {
        save(&mut tx, run, chunk).await?;
        trained += chunk.len();
    }
    tx.commit().await?;
    Ok(trained)
}

/// The Movies `pooled` names, each by the External IDs it names them by: the title the last import
/// left, or the External ID itself when Wikidata lacks the Movie.
async fn movies_of(
    tx: &mut PgConnection,
    pooled: &[PooledCount],
) -> Result<HashMap<String, Trained>, sqlx::Error> {
    // Servers name a title `namespace-value`, as `title_ids` splits it.
    let named: HashMap<&str, ExternalId> = (pooled.iter())
        .flat_map(|count| [count.a.as_str(), count.b.as_str()])
        .filter_map(|id| {
            let (namespace, value) = id.split_once('-')?;
            let external_id = ExternalId {
                namespace: namespace.to_owned(),
                value: value.to_owned(),
            };
            Some((id, external_id))
        })
        .collect();
    let (namespaces, values): (Vec<&str>, Vec<&str>) = (named.values())
        .map(|id| (id.namespace.as_str(), id.value.as_str()))
        .unzip();
    let movies = sqlx::query!(
        "SELECT DISTINCT ON (title_ids.namespace, title_ids.value)
             title_ids.namespace, title_ids.value, titles.id
         FROM UNNEST($1::text[], $2::text[]) AS pooled (namespace, value)
         JOIN title_ids ON title_ids.namespace = pooled.namespace
             AND title_ids.value = pooled.value
         JOIN titles ON titles.id = title_ids.title_id
         WHERE titles.kind = $3
         ORDER BY title_ids.namespace, title_ids.value, titles.wikidata_id",
        &namespaces as _,
        &values as _,
        TitleKind::Movie.as_str(),
    )
    .fetch_all(tx)
    .await?;
    let in_wikidata: HashMap<ExternalId, Uuid> = (movies.into_iter())
        .map(|movie| {
            let id = ExternalId {
                namespace: movie.namespace,
                value: movie.value,
            };
            (id, movie.id)
        })
        .collect();
    Ok((named.into_iter())
        .map(|(id, external_id)| {
            let movie = match in_wikidata.get(&external_id) {
                Some(&title) => Trained::Wikidata(title),
                None => Trained::Lacking(external_id),
            };
            (id.to_owned(), movie)
        })
        .collect())
}

/// What `pooled` says about each of the titles `movies` names, by their External IDs. A title has
/// behaviour once Accounts finished it with another: a count of it alone tells nothing about which
/// titles are like it.
///
/// ponytail: an Account that finished one title under two of its ids, on two Servers, counts twice
/// for it; count Accounts per title rather than per External ID if that ever skews it.
fn behaviours(
    pooled: &[PooledCount],
    movies: &HashMap<String, Trained>,
) -> HashMap<Trained, Behaviour> {
    let mut alone: HashMap<&Trained, i64> = HashMap::new();
    let mut pairs: HashMap<(&Trained, &Trained), i64> = HashMap::new();
    for count in pooled {
        let (Some(a), Some(b)) = (movies.get(&count.a), movies.get(&count.b)) else {
            continue;
        };
        if count.a == count.b {
            *alone.entry(a).or_default() += count.accounts;
        } else if a != b {
            *pairs.entry((a.min(b), a.max(b))).or_default() += count.accounts;
        }
    }
    let mut behaviours: HashMap<Trained, Behaviour> = HashMap::new();
    for ((a, b), together) in pairs {
        // An Account counted for a pair was counted for each of its Movies.
        let (Some(&a_alone), Some(&b_alone)) = (alone.get(a), alone.get(b)) else {
            continue;
        };
        #[expect(
            clippy::cast_precision_loss,
            reason = "a count of Accounts needn't be exact to weigh a pair"
        )]
        let weight = together as f32 / (a_alone as f32 * b_alone as f32).sqrt();
        for (title, accounts, other) in [(a, a_alone, b), (b, b_alone, a)] {
            let behaviour = behaviours
                .entry(title.clone())
                .or_insert_with(|| Behaviour {
                    accounts,
                    // Its own dimension is what makes two titles finished together alike, not only
                    // two finished with the same others.
                    vector: BTreeMap::from([(dimension(title), 1.0)]),
                });
            behaviour.vector.insert(dimension(other), weight);
        }
    }
    behaviours
}

/// A title's vector from its `facts` and its `behaviour`, behaviour weighing more the more
/// Accounts finished it. `None` without either.
fn blend(
    facts: Option<BTreeMap<u64, f32>>,
    behaviour: Option<Behaviour>,
) -> Option<BTreeMap<u64, f32>> {
    let Some(behaviour) = behaviour else {
        return facts;
    };
    let Some(facts) = facts else {
        return Some(behaviour.vector);
    };
    #[expect(
        clippy::cast_precision_loss,
        reason = "a count of Accounts needn't be exact to weigh behaviour"
    )]
    let accounts = behaviour.accounts as f32;
    let weight = accounts / (accounts + ACCOUNTS_WEIGHING_AS_MUCH_AS_FACTS);
    // Each scaled to its share of a unit vector, as cosine similarity ignores a vector's length.
    let mut blended = scaled(facts, 1.0 - weight);
    for (dimension, value) in scaled(behaviour.vector, weight) {
        *blended.entry(dimension).or_insert(0.0) += value;
    }
    Some(blended)
}

fn scaled(mut vector: BTreeMap<u64, f32>, length: f32) -> BTreeMap<u64, f32> {
    let norm = vector
        .values()
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt();
    for value in vector.values_mut() {
        *value *= length / norm;
    }
    vector
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
        #[expect(
            clippy::cast_precision_loss,
            reason = "a count of titles needn't be exact to weigh a fact"
        )]
        let sharing = sharing as f32;
        *vector.entry(dimension(fact)).or_insert(0.0) += 1.0 / sharing.ln_1p();
    }
    (!vector.is_empty()).then_some(vector)
}

/// The dimension `key`, a fact or a title, is hashed to. A fact is a `String` and a title a
/// `Trained`, which hash apart.
fn dimension(key: &impl Hash) -> u64 {
    // Rust may change `DefaultHasher` between releases, which is harmless: a training run hashes
    // every key itself.
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    // pgvector numbers a `sparsevec`'s dimensions from 1.
    hasher.finish() % DIMENSIONS + 1
}

/// Inserts `vectors`, each a title's, as made by training run `run`.
async fn save(
    tx: &mut PgConnection,
    run: Uuid,
    vectors: &[(Trained, BTreeMap<u64, f32>)],
) -> Result<(), sqlx::Error> {
    let mut titles = Vec::with_capacity(vectors.len());
    let mut namespaces = Vec::with_capacity(vectors.len());
    let mut values = Vec::with_capacity(vectors.len());
    let mut embeddings = Vec::with_capacity(vectors.len());
    for (title, vector) in vectors {
        let (title, namespace, value) = match title {
            Trained::Wikidata(title) => (Some(*title), None, None),
            Trained::Lacking(id) => (None, Some(id.namespace.as_str()), Some(id.value.as_str())),
        };
        titles.push(title);
        namespaces.push(namespace);
        values.push(value);
        embeddings.push(as_pgvector(vector));
    }
    sqlx::query!(
        "INSERT INTO title_vectors (title_id, namespace, value, training_run_id, embedding)
         SELECT title_id, namespace, value, $1, embedding::sparsevec
         FROM UNNEST($2::uuid[], $3::text[], $4::text[], $5::text[])
             AS vectors (title_id, namespace, value, embedding)",
        run,
        &titles as _,
        &namespaces as _,
        &values as _,
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
/// its id in the first of `ids`' namespaces it has (ADR 0042). A Movie Wikidata lacks is named,
/// and answered, by the External ID Servers pooled it by. Empty when the title has no vector. Only
/// titles sharing a fact with it, or finished with it or with a title it was finished with, are
/// answered.
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
    // A vector without a `title_id` is a Movie Wikidata lacks, under its External ID, and so is
    // asked for and answered only when `$1` is `$6`, the Movie kind.
    let found = sqlx::query!(
        r#"WITH asked AS (
               SELECT title_vectors.id, title_vectors.embedding
               FROM UNNEST($2::text[], $3::text[]) AS asked_ids (namespace, value)
               LEFT JOIN title_ids ON title_ids.namespace = asked_ids.namespace
                   AND title_ids.value = asked_ids.value
               LEFT JOIN titles ON titles.id = title_ids.title_id AND titles.kind = $1
               JOIN title_vectors ON title_vectors.title_id = titles.id
                   OR (title_vectors.namespace = asked_ids.namespace
                       AND title_vectors.value = asked_ids.value AND $1 = $6::text)
               ORDER BY titles.wikidata_id NULLS LAST LIMIT 1),
           nearest AS (
               SELECT alike.title_id, alike.namespace, alike.value,
                      alike.embedding <=> asked.embedding AS distance
               FROM asked
               JOIN title_vectors AS alike ON alike.id <> asked.id
               LEFT JOIN titles ON titles.id = alike.title_id
               WHERE alike.embedding <=> asked.embedding < $4
                   AND (titles.kind = $1
                        AND EXISTS (SELECT 1 FROM title_ids
                                    WHERE title_ids.title_id = alike.title_id
                                    AND title_ids.namespace = ANY($2))
                        OR $1 = $6::text AND alike.namespace = ANY($2))
               ORDER BY distance, alike.title_id, alike.namespace, alike.value LIMIT $5)
           SELECT COALESCE(id.namespace, nearest.namespace) AS "namespace!",
                  COALESCE(id.value, nearest.value) AS "value!",
                  (1 - nearest.distance)::real AS "weight!"
           FROM nearest
           LEFT JOIN LATERAL (
               SELECT title_ids.namespace, title_ids.value FROM title_ids
               WHERE title_ids.title_id = nearest.title_id AND title_ids.namespace = ANY($2)
               ORDER BY array_position($2, title_ids.namespace) LIMIT 1) AS id ON true
           ORDER BY nearest.distance, nearest.title_id, nearest.namespace, nearest.value"#,
        kind.as_str(),
        &namespaces as _,
        &values as _,
        // Under 1: sharing at least one dimension, as every value is positive.
        1.0_f64,
        MOST_SIMILAR,
        TitleKind::Movie.as_str(),
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
