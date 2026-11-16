use nama_plugin_api::metadata::{ExternalId, ExternalIds};

use crate::library::Candidate;

use super::{Decision, decide};

fn candidate(id: i64, title: &str, year: Option<i32>) -> Candidate {
    Candidate {
        id: ExternalId::tmdb(id),
        ids: ExternalIds::from_iter([ExternalId::tmdb(id)]),
        title: title.to_owned(),
        original_title: None,
        year,
        poster: None,
        origin_country: Vec::new(),
        vote_count: 0,
    }
}

fn with_votes(vote_count: u32, candidate: Candidate) -> Candidate {
    Candidate {
        vote_count,
        ..candidate
    }
}

#[test]
fn the_title_from_the_files_year_matches() {
    let found = [
        candidate(348, "Alien", Some(1979)),
        candidate(945_961, "Alien: Romulus", Some(2024)),
        candidate(679, "Aliens", Some(1986)),
    ];

    assert_eq!(
        decide("Alien", Some(1979), &found),
        Decision::Match(ExternalId::tmdb(348))
    );
}

#[test]
fn the_year_tells_a_remake_from_the_original() {
    let found = [
        candidate(841, "Dune", Some(1984)),
        candidate(438_631, "Dune", Some(2021)),
    ];

    assert_eq!(
        decide("Dune", Some(2021), &found),
        Decision::Match(ExternalId::tmdb(438_631))
    );
    assert_eq!(
        decide("Dune", Some(1984), &found),
        Decision::Match(ExternalId::tmdb(841))
    );
}

#[test]
fn a_year_off_by_one_still_matches() {
    let found = [candidate(438_631, "Dune", Some(2021))];

    assert_eq!(
        decide("Dune", Some(2020), &found),
        Decision::Match(ExternalId::tmdb(438_631))
    );
}

#[test]
fn case_and_punctuation_do_not_count() {
    let found = [candidate(945_961, "Alien: Romulus", Some(2024))];

    assert_eq!(
        decide("alien romulus", Some(2024), &found),
        Decision::Match(ExternalId::tmdb(945_961))
    );
}

#[test]
fn the_original_title_matches_too() {
    let found = [Candidate {
        original_title: Some("Das Boot".to_owned()),
        ..candidate(387, "The Boat", Some(1981))
    }];

    assert_eq!(
        decide("Das Boot", Some(1981), &found),
        Decision::Match(ExternalId::tmdb(387))
    );
}

#[test]
fn accents_do_not_count_and_a_typo_in_a_long_title_still_matches() {
    let found = [
        candidate(194, "Amélie", Some(2001)),
        candidate(
            120,
            "The Lord of the Rings: The Fellowship of the Ring",
            Some(2001),
        ),
    ];

    assert_eq!(
        decide("Amelie", Some(2001), &found),
        Decision::Match(ExternalId::tmdb(194))
    );
    assert_eq!(
        decide(
            "The Lord of the Rings - The Fellowshp of the Ring",
            Some(2001),
            &found
        ),
        Decision::Match(ExternalId::tmdb(120))
    );
}

#[test]
fn a_far_better_known_title_breaks_a_tie() {
    let arrival = [
        with_votes(4, candidate(420_634, "Arrival", Some(2016))),
        with_votes(18_000, candidate(329_865, "Arrival", Some(2016))),
    ];
    let oblivion = [
        with_votes(11_000, candidate(75_612, "Oblivion", Some(2013))),
        with_votes(2, candidate(254_986, "Oblivion", Some(2013))),
    ];
    let band_of_brothers = [
        with_votes(3_400, candidate(4613, "Band of Brothers", Some(2001))),
        with_votes(3, candidate(96_157, "Band of Brothers", Some(2019))),
    ];

    assert_eq!(
        decide("Arrival", Some(2016), &arrival),
        Decision::Match(ExternalId::tmdb(329_865))
    );
    assert_eq!(
        decide("Oblivion", Some(2013), &oblivion),
        Decision::Match(ExternalId::tmdb(75_612))
    );
    assert_eq!(
        decide("Band of Brothers", None, &band_of_brothers),
        Decision::Match(ExternalId::tmdb(4613))
    );
}

#[test]
fn a_tie_between_titles_less_than_ten_times_apart_in_votes_stays_ambiguous() {
    let the_office = [
        with_votes(5_682, candidate(2316, "The Office", Some(2005))),
        with_votes(1_016, candidate(2996, "The Office", Some(2001))),
    ];
    let barely_rated = [
        with_votes(1, candidate(1, "Hamlet", Some(2000))),
        with_votes(0, candidate(2, "Hamlet", Some(2000))),
    ];

    assert_eq!(
        decide("The Office", None, &the_office),
        Decision::Ambiguous(the_office.to_vec())
    );
    assert_eq!(
        decide("Hamlet", Some(2000), &barely_rated),
        Decision::Ambiguous(barely_rated.to_vec())
    );
}

#[test]
fn an_ambiguous_title_ties_only_the_titles_that_contest_the_best() {
    let us = with_votes(5_682, candidate(2316, "The Office", Some(2005)));
    let uk = with_votes(1_016, candidate(2996, "The Office", Some(2001)));
    let outvoted = with_votes(100, candidate(3, "The Office", Some(2005)));
    let unlike = with_votes(9_000, candidate(4, "Office Space", Some(2005)));

    assert_eq!(
        decide(
            "The Office",
            None,
            &[outvoted, uk.clone(), unlike, us.clone()]
        ),
        Decision::Ambiguous(vec![us, uk])
    );
}

#[test]
fn a_runner_up_just_below_the_threshold_needs_outvoting_too() {
    // The file's typo fits the first title at about 0.92 and the second at about 0.84:
    // within the margin of each other, though only the first clears the threshold.
    let fellowship = |votes| {
        [
            with_votes(
                25_000,
                candidate(
                    120,
                    "The Lord of the Rings: The Fellowship of the Ring",
                    Some(2001),
                ),
            ),
            with_votes(
                votes,
                candidate(
                    9,
                    "Lord of the Rings: The Fellowship of the Ring",
                    Some(2001),
                ),
            ),
        ]
    };
    let typo = "The Lord of the Rings - The Fellowshp of the Ring";

    assert_eq!(
        decide(typo, Some(2001), &fellowship(2_500)),
        Decision::Match(ExternalId::tmdb(120))
    );
    assert_eq!(
        decide(typo, Some(2001), &fellowship(2_501)),
        Decision::Ambiguous(fellowship(2_501).to_vec())
    );
}

#[test]
fn a_title_that_only_looks_alike_does_not_match() {
    let found = [
        candidate(679, "Aliens", Some(1979)),
        candidate(1, "Alien Nation", Some(1979)),
    ];

    assert_eq!(decide("Alien", Some(1979), &found), Decision::NoMatch);
    assert_eq!(decide("Alien", Some(1979), &[]), Decision::NoMatch);
}

#[test]
fn the_right_title_from_another_year_does_not_match() {
    let found = [candidate(841, "Dune", Some(1984))];

    assert_eq!(decide("Dune", Some(2021), &found), Decision::NoMatch);
}
