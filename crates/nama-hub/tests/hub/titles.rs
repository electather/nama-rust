//! Importing Wikidata's dump, the External ID crosswalk it serves, and the similar titles its
//! vectors find, against a real PostgreSQL.

use nama_hub::{
    db,
    titles::{self, ExternalId, ImportError, PooledCount, SimilarTitle, TitleKind},
};
use sqlx::PgPool;

/// Three films and two series with External IDs, and two people. The Matrix and its sequel share
/// cast, directors, writers, genres, franchise and country; Alien shares one genre and the country
/// with them, and one series shares their director.
const DUMP: &[u8] = include_bytes!("fixtures/wikidata-dump.json");

/// The fixture's first title, now with another TMDB id, and nothing else.
const NEXT_DUMP: &[u8] = br#"[
{"type":"item","id":"Q83495","claims":{"P345":[{"mainsnak":{"snaktype":"value","datavalue":{"value":"tt0133093","type":"string"}},"type":"statement","rank":"normal"}],"P4947":[{"mainsnak":{"snaktype":"value","datavalue":{"value":"604","type":"string"}},"type":"statement","rank":"normal"}]}}
]
"#;

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn imported(db: &PgPool) {
    db::migrate(db).await.unwrap();
    titles::import(db, DUMP).await.unwrap();
}

fn id(namespace: &str, value: &str) -> ExternalId {
    ExternalId {
        namespace: namespace.to_owned(),
        value: value.to_owned(),
    }
}

/// The titles imported and their vectors trained.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn trained(db: &PgPool) {
    imported(db).await;
    titles::train(db, &[]).await.unwrap();
}

/// The pooled counts of `accounts` Accounts that finished both Movies `a` and `b`, the lesser
/// first, as Servers name them.
fn finished_together_by(accounts: i64, a: &str, b: &str) -> Vec<PooledCount> {
    [(a, a), (b, b), (a, b)]
        .map(|(a, b)| PooledCount {
            a: a.to_owned(),
            b: b.to_owned(),
            accounts,
        })
        .into()
}

/// The pooled counts of `accounts` Accounts that finished both The Matrix and Alien.
fn matrix_and_alien_finished_by(accounts: i64) -> Vec<PooledCount> {
    finished_together_by(accounts, "tmdb-348", "tmdb-603")
}

/// The ids of `similar`, most alike first, checking their weights fall from at most 1.0.
fn ids_by_weight(similar: &[SimilarTitle]) -> Vec<ExternalId> {
    let weights: Vec<f32> = similar.iter().map(|title| title.weight).collect();
    // Identical vectors' cosine distance can round below 0, so their weight above 1.0.
    assert!(
        weights.is_sorted_by(|a, b| a >= b) && weights.iter().all(|&w| w > 0.0 && w < 1.001),
        "{weights:?}"
    );
    similar.iter().map(|title| title.id.clone()).collect()
}

fn namespaces(namespaces: &[&str]) -> Vec<String> {
    namespaces
        .iter()
        .map(|&namespace| namespace.to_owned())
        .collect()
}

#[sqlx::test(migrations = false)]
async fn import_keeps_the_films_and_series_with_an_external_id(db: PgPool) {
    db::migrate(&db).await.unwrap();

    let imported = titles::import(&db, DUMP).await.unwrap();

    assert_eq!(imported, 5);
}

#[sqlx::test(migrations = false)]
async fn any_id_of_a_title_answers_in_the_namespaces_the_server_uses(db: PgPool) {
    imported(&db).await;
    let uses = namespaces(&["tmdb", "imdb"]);

    let by_tvdb = titles::crosswalk(&db, TitleKind::Show, &id("tvdb", "81189"), &uses).await;
    let by_imdb = titles::crosswalk(&db, TitleKind::Show, &id("imdb", "tt0903747"), &uses).await;

    let answer = vec![id("imdb", "tt0903747"), id("tmdb", "1396")];
    assert_eq!(by_tvdb.unwrap(), answer);
    assert_eq!(by_imdb.unwrap(), answer);
}

#[sqlx::test(migrations = false)]
async fn a_movie_resolves_by_its_tvdb_id(db: PgPool) {
    imported(&db).await;
    let uses = namespaces(&["tmdb"]);

    let movie = titles::crosswalk(&db, TitleKind::Movie, &id("tvdb", "169"), &uses).await;

    assert_eq!(movie.unwrap(), vec![id("tmdb", "603")]);
}

#[sqlx::test(migrations = false)]
async fn a_movie_and_a_show_with_one_tmdb_id_are_different_titles(db: PgPool) {
    imported(&db).await;
    let tmdb = id("tmdb", "603");
    let uses = namespaces(&["imdb"]);

    let movie = titles::crosswalk(&db, TitleKind::Movie, &tmdb, &uses).await;
    let show = titles::crosswalk(&db, TitleKind::Show, &tmdb, &uses).await;

    assert_eq!(movie.unwrap(), vec![id("imdb", "tt0133093")]);
    assert_eq!(show.unwrap(), vec![id("imdb", "tt0900001")]);
}

#[sqlx::test(migrations = false)]
async fn a_title_wikidata_lacks_gets_no_answer(db: PgPool) {
    imported(&db).await;
    let uses = namespaces(&["imdb", "tmdb"]);

    let person = titles::crosswalk(&db, TitleKind::Movie, &id("imdb", "nm0000401"), &uses).await;
    let unknown = titles::crosswalk(&db, TitleKind::Movie, &id("tmdb", "1"), &uses).await;

    assert_eq!(person.unwrap(), []);
    assert_eq!(unknown.unwrap(), []);
}

#[sqlx::test(migrations = false)]
async fn the_next_dump_replaces_the_titles_and_their_ids(db: PgPool) {
    imported(&db).await;

    titles::import(&db, NEXT_DUMP).await.unwrap();

    let uses = namespaces(&["imdb", "tmdb"]);
    let changed = titles::crosswalk(&db, TitleKind::Movie, &id("imdb", "tt0133093"), &uses).await;
    let dropped = titles::crosswalk(&db, TitleKind::Show, &id("tvdb", "81189"), &uses).await;
    assert_eq!(
        changed.unwrap(),
        vec![id("imdb", "tt0133093"), id("tmdb", "604")]
    );
    assert_eq!(dropped.unwrap(), []);
}

#[sqlx::test(migrations = false)]
async fn a_dump_cut_short_changes_nothing(db: PgPool) {
    imported(&db).await;
    let cut_short = &NEXT_DUMP[..NEXT_DUMP.len() - 2];

    let err = titles::import(&db, cut_short).await.unwrap_err();

    assert!(matches!(err, ImportError::Truncated), "{err}");
    let uses = namespaces(&["tmdb"]);
    let kept = titles::crosswalk(&db, TitleKind::Show, &id("tvdb", "81189"), &uses).await;
    assert_eq!(kept.unwrap(), vec![id("tmdb", "1396")]);
}

#[sqlx::test(migrations = false)]
async fn training_gives_a_vector_to_each_title_sharing_a_fact_with_another(db: PgPool) {
    imported(&db).await;

    let trained = titles::train(&db, &[]).await.unwrap();

    // Not Breaking Bad, which has no facts.
    assert_eq!(trained, 4);
}

#[sqlx::test(migrations = false)]
async fn a_title_answers_the_titles_of_its_kind_most_alike_first(db: PgPool) {
    trained(&db).await;

    let similar = titles::similar(&db, TitleKind::Movie, &[id("tmdb", "603")])
        .await
        .unwrap();

    // Not the series sharing the director, which is a Show.
    assert_eq!(
        ids_by_weight(&similar),
        [id("tmdb", "604"), id("tmdb", "348")]
    );
    assert!(similar[0].weight > 0.9, "{similar:?}");
}

#[sqlx::test(migrations = false)]
async fn similar_titles_answer_in_the_first_namespace_asked_that_they_have(db: PgPool) {
    trained(&db).await;
    let asked = [id("imdb", "tt0133093"), id("tmdb", "603")];

    let similar = titles::similar(&db, TitleKind::Movie, &asked)
        .await
        .unwrap();

    assert_eq!(
        ids_by_weight(&similar),
        [id("imdb", "tt0234215"), id("imdb", "tt0078748")]
    );
}

#[sqlx::test(migrations = false)]
async fn a_title_wikidata_lacks_or_without_facts_gets_no_similar_titles(db: PgPool) {
    trained(&db).await;

    let unknown = titles::similar(&db, TitleKind::Movie, &[id("tmdb", "1")]).await;
    let breaking_bad = titles::similar(&db, TitleKind::Show, &[id("tmdb", "1396")]).await;

    assert_eq!(unknown.unwrap(), []);
    assert_eq!(breaking_bad.unwrap(), []);
}

#[sqlx::test(migrations = false)]
async fn a_new_training_run_replaces_the_previous_runs_vectors(db: PgPool) {
    trained(&db).await;
    assert_eq!(titles::train(&db, &[]).await.unwrap(), 4);

    titles::import(&db, NEXT_DUMP).await.unwrap();
    let retrained = titles::train(&db, &[]).await.unwrap();

    assert_eq!(retrained, 0);
    let similar = titles::similar(&db, TitleKind::Movie, &[id("tmdb", "604")]).await;
    assert_eq!(similar.unwrap(), []);
}

/// The Movies most like The Matrix, once trained with `pooled`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn like_the_matrix_after_training_on(db: &PgPool, pooled: &[PooledCount]) -> Vec<ExternalId> {
    assert_eq!(titles::train(db, pooled).await.unwrap(), 4);
    let similar = titles::similar(db, TitleKind::Movie, &[id("tmdb", "603")])
        .await
        .unwrap();
    ids_by_weight(&similar)
}

#[sqlx::test(migrations = false)]
async fn facts_outweigh_behaviour_while_few_accounts_finished_titles_together(db: PgPool) {
    imported(&db).await;

    let similar = like_the_matrix_after_training_on(&db, &matrix_and_alien_finished_by(20)).await;

    assert_eq!(similar, [id("tmdb", "604"), id("tmdb", "348")]);
}

#[sqlx::test(migrations = false)]
async fn behaviour_outweighs_facts_once_many_accounts_finished_titles_together(db: PgPool) {
    imported(&db).await;

    let similar = like_the_matrix_after_training_on(&db, &matrix_and_alien_finished_by(5000)).await;

    assert_eq!(similar, [id("tmdb", "348"), id("tmdb", "604")]);
}

#[sqlx::test(migrations = false)]
async fn a_new_training_run_replaces_the_behaviour_the_previous_one_learnt(db: PgPool) {
    imported(&db).await;
    like_the_matrix_after_training_on(&db, &matrix_and_alien_finished_by(5000)).await;

    let similar = like_the_matrix_after_training_on(&db, &[]).await;

    assert_eq!(similar, [id("tmdb", "604"), id("tmdb", "348")]);
}

#[sqlx::test(migrations = false)]
async fn a_movie_wikidata_lacks_gets_a_vector_from_behaviour_alone(db: PgPool) {
    imported(&db).await;
    let pooled = finished_together_by(20, "tmdb-603", "tmdb-999");

    assert_eq!(titles::train(&db, &pooled).await.unwrap(), 5);

    let lacking = [id("tmdb", "999")];
    let like_it = titles::similar(&db, TitleKind::Movie, &lacking).await;
    assert_eq!(ids_by_weight(&like_it.unwrap()), [id("tmdb", "603")]);
    let like_the_matrix = titles::similar(&db, TitleKind::Movie, &[id("tmdb", "603")]).await;
    assert!(ids_by_weight(&like_the_matrix.unwrap()).contains(&lacking[0]));
    // Only Movies are pooled, so it isn't a Show.
    let as_a_show = titles::similar(&db, TitleKind::Show, &lacking).await;
    assert_eq!(as_a_show.unwrap(), []);
}
