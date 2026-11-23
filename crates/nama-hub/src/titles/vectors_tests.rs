use std::collections::BTreeMap;

use super::{embed, taste};

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
