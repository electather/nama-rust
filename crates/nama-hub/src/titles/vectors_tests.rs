use std::collections::{BTreeMap, HashMap};

use uuid::Uuid;

use super::{PooledCount, behaviours, blend, embed};

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

const TITLE: Uuid = Uuid::from_u128(1);
const FINISHED_WITH_IT: Uuid = Uuid::from_u128(3);

fn pooled(a: &str, b: &str, accounts: i64) -> PooledCount {
    PooledCount {
        a: a.to_owned(),
        b: b.to_owned(),
        accounts,
    }
}

/// The Movies the pooled counts below name, by the External IDs they name them by.
fn movies() -> HashMap<String, Uuid> {
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
