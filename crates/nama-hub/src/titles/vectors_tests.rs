use std::collections::BTreeMap;

use super::embed;

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
