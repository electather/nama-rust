use std::collections::{BTreeMap, HashMap};

use uuid::Uuid;

use super::{ExternalId, PooledCount, Trained, behaviours, blend, embed, taste};

/// The vector of a title with `facts`, each with how many titles share it.
fn vector(facts: &[(&str, i64)]) -> BTreeMap<u64, f32> {
    let facts: Vec<(String, i64)> = facts
        .iter()
        .map(|&(fact, sharing)| (fact.to_owned(), sharing))
        .collect();
    embed(facts.iter().map(|(fact, sharing)| (fact, *sharing))).unwrap()
}

fn cosine(a: &BTreeMap<u64, f32>, b: &BTreeMap<u64, f32>) -> f32 {
    let dot: f32 = a
        .iter()
        .filter_map(|(dimension, x)| Some(x * b.get(dimension)?))
        .sum();
    let norm = |v: &BTreeMap<u64, f32>| v.values().map(|x| x * x).sum::<f32>().sqrt();
    dot / (norm(a) * norm(b))
}

#[test]
fn sharing_a_rare_fact_makes_titles_more_alike_than_sharing_a_common_one() {
    let title = vector(&[("director:Q9545711", 3), ("country:Q30", 5000)]);
    let same_director = vector(&[("director:Q9545711", 3), ("genre:Q200092", 900)]);
    let same_country = vector(&[("country:Q30", 5000), ("genre:Q200092", 900)]);

    assert!(cosine(&title, &same_director) > cosine(&title, &same_country) + 0.3);
}

#[test]
fn titles_sharing_no_fact_are_not_alike_at_all() {
    let title = vector(&[
        ("cast:Q43416", 40),
        ("director:Q9545711", 3),
        ("genre:Q471839", 2000),
        ("country:Q30", 5000),
    ]);
    let other = vector(&[
        ("cast:Q4547", 30),
        ("director:Q56005", 25),
        ("genre:Q200092", 900),
        ("country:Q145", 1200),
    ]);

    assert_eq!(cosine(&title, &other), 0.0);
}

#[test]
fn a_title_without_facts_has_no_vector() {
    assert_eq!(embed([]), None);
}

#[test]
fn a_taste_vector_sums_the_finished_titles_vectors_each_at_length_one() {
    // 3-4-5: the first is 5 long, the second 2.
    let finished = ["{1:3,2:4}/1000000000", "{2:2}/1000000000"];

    let taste = taste(finished).unwrap();

    let expected = BTreeMap::from([(1, 0.6), (2, 1.8)]);
    assert_eq!(
        taste.keys().collect::<Vec<_>>(),
        expected.keys().collect::<Vec<_>>()
    );
    for (dimension, value) in expected {
        assert!((taste[&dimension] - value).abs() < 1e-6, "{taste:?}");
    }
}

#[test]
fn an_account_that_finished_nothing_has_no_taste_vector() {
    assert_eq!(taste([]), None);
}

#[test]
fn a_taste_vector_past_what_pgvector_holds_keeps_its_heaviest_values() {
    // One finished title whose value in each dimension is that dimension's number.
    let values: Vec<String> = (1..=16_001)
        .map(|dimension| format!("{dimension}:{dimension}"))
        .collect();
    let finished = format!("{{{}}}/1000000000", values.join(","));

    let taste = taste([finished.as_str()]).unwrap();

    assert_eq!(taste.len(), 16_000);
    assert!(!taste.contains_key(&1), "the lightest is dropped");
}

const TITLE: Trained = Trained::Wikidata(Uuid::from_u128(1));
const FINISHED_WITH_IT: Trained = Trained::Wikidata(Uuid::from_u128(3));

fn pooled(a: &str, b: &str, accounts: i64) -> PooledCount {
    PooledCount {
        a: a.to_owned(),
        b: b.to_owned(),
        accounts,
    }
}

/// The Movies the pooled counts below name, by the External IDs they name them by.
fn movies() -> HashMap<String, Trained> {
    [
        ("tmdb-603", TITLE),
        ("imdb-tt0133093", TITLE),
        ("tmdb-348", FINISHED_WITH_IT),
    ]
    .into_iter()
    .map(|(id, title)| (id.to_owned(), title))
    .collect()
}

/// How alike `TITLE` is to a title sharing its facts and to one `accounts` Accounts finished with
/// it, which has no facts.
fn alike_when_finished_together_by(accounts: i64) -> (f32, f32) {
    let facts = vector(&[("director:Q9545711", 3), ("genre:Q471839", 2000)]);
    let mut behaviours = behaviours(
        &[
            pooled("tmdb-603", "tmdb-603", accounts),
            pooled("tmdb-348", "tmdb-348", accounts),
            pooled("tmdb-348", "tmdb-603", accounts),
        ],
        &movies(),
    );
    let title = blend(Some(facts.clone()), behaviours.remove(&TITLE)).unwrap();
    let finished_with_it = blend(None, behaviours.remove(&FINISHED_WITH_IT)).unwrap();
    (cosine(&title, &facts), cosine(&title, &finished_with_it))
}

#[test]
fn facts_outweigh_behaviour_while_few_accounts_finished_a_title() {
    let (to_facts, to_finished_with_it) = alike_when_finished_together_by(20);

    assert!(
        to_facts > to_finished_with_it,
        "{to_facts} {to_finished_with_it}"
    );
}

#[test]
fn behaviour_outweighs_facts_once_many_accounts_finished_a_title() {
    let (to_facts, to_finished_with_it) = alike_when_finished_together_by(5000);

    assert!(
        to_finished_with_it > to_facts,
        "{to_facts} {to_finished_with_it}"
    );
}

#[test]
fn a_title_without_facts_gets_a_vector_from_behaviour_alone() {
    let (_, to_finished_with_it) = alike_when_finished_together_by(20);

    assert!(to_finished_with_it > 0.0);
}

#[test]
fn a_title_finished_only_with_itself_under_another_id_has_no_behaviour() {
    let behaviours = behaviours(
        &[
            pooled("tmdb-603", "tmdb-603", 20),
            pooled("imdb-tt0133093", "imdb-tt0133093", 20),
            pooled("imdb-tt0133093", "tmdb-603", 20),
        ],
        &movies(),
    );

    assert!(behaviours.is_empty());
}

#[test]
fn a_title_without_facts_or_behaviour_has_no_vector() {
    assert_eq!(blend(None, None), None);
}

#[test]
fn a_movie_wikidata_lacks_gets_behaviour_under_its_external_id() {
    let lacking = Trained::Lacking(ExternalId {
        namespace: "tmdb".to_owned(),
        value: "999".to_owned(),
    });
    let mut movies = movies();
    movies.insert("tmdb-999".to_owned(), lacking.clone());

    let behaviours = behaviours(
        &[
            pooled("tmdb-603", "tmdb-603", 20),
            pooled("tmdb-999", "tmdb-999", 20),
            pooled("tmdb-603", "tmdb-999", 20),
        ],
        &movies,
    );

    assert!(behaviours.contains_key(&lacking));
    assert!(behaviours.contains_key(&TITLE));
}
