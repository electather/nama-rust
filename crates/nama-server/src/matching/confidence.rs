//! Deciding which search result, if any, a file's title and year Match.
//!
//! Each candidate gets a confidence from 0 to 1: how alike the titles are (the candidate's
//! title or original title, whichever is closer) times how well the years agree. A candidate
//! Matches only above [`THRESHOLD`], and only if every other candidate within [`MARGIN`] of it
//! has at most a [`VOTE_RATIO`]th of its votes: two titles that fit about as well and are about
//! as well known are ambiguous, and an ambiguous file stays Unmatched rather than risk the wrong
//! Match.

use nama_plugin_api::metadata::{Candidate, ExternalId};

/// The least confidence a Match needs.
const THRESHOLD: f64 = 0.85;

/// How far ahead of the next candidate the best must be to Match on its fit alone.
const MARGIN: f64 = 0.1;

/// How many times the votes of each candidate within [`MARGIN`] the best needs to Match anyway.
/// A bare "The Office" (the US show with 5.6 times the UK one's votes) stays Unmatched: in a
/// Sonarr Library that folder is the UK show.
const VOTE_RATIO: u64 = 10;

/// What a search's results say about a title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Decision {
    /// One candidate fits confidently.
    Match(ExternalId),
    /// More than one candidate fits about as well.
    Ambiguous,
    /// No candidate fits well enough.
    NoMatch,
}

pub(crate) fn decide(title: &str, year: Option<i32>, candidates: &[Candidate]) -> Decision {
    // Every candidate is scored, not just those above the threshold: a runner-up just below it
    // still fits about as well as the best and so makes the best ambiguous.
    let mut scored: Vec<(f64, &Candidate)> = candidates
        .iter()
        .map(|candidate| (confidence(title, year, candidate), candidate))
        .collect();
    // On equal fit the better-known candidate is the best, whatever order the search gave.
    scored.sort_by(|a, b| {
        b.0.total_cmp(&a.0)
            .then(b.1.vote_count.cmp(&a.1.vote_count))
    });
    let Some(((best_score, best), others)) = scored.split_first() else {
        return Decision::NoMatch;
    };
    if *best_score < THRESHOLD {
        return Decision::NoMatch;
    }
    let contested = others
        .iter()
        .take_while(|(score, _)| best_score - score < MARGIN)
        .any(|(_, other)| !outvotes(best, other));
    if contested {
        Decision::Ambiguous
    } else {
        Decision::Match(best.id.clone())
    }
}

/// Whether `best` is far better known than `other`, counting no votes as one.
fn outvotes(best: &Candidate, other: &Candidate) -> bool {
    u64::from(best.vote_count) >= VOTE_RATIO * u64::from(other.vote_count.max(1))
}

fn confidence(title: &str, year: Option<i32>, candidate: &Candidate) -> f64 {
    let wanted = normalize(title);
    let title_score = std::iter::once(&candidate.title)
        .chain(&candidate.original_title)
        .map(|other| similarity(&wanted, &normalize(other)))
        .fold(0.0, f64::max);
    title_score * year_agreement(year, candidate.year)
}

/// How well a file's year agrees with a candidate's. Release dates differ by country, so a year
/// either side still counts.
fn year_agreement(wanted: Option<i32>, found: Option<i32>) -> f64 {
    match (wanted, found) {
        (None, _) => 1.0,
        (Some(_), None) => 0.8,
        (Some(wanted), Some(found)) => match wanted.abs_diff(found) {
            0 => 1.0,
            1 => 0.95,
            _ => 0.0,
        },
    }
}

/// A title in lowercase, without common accents, with `&` as `and` and anything but letters
/// and digits as single spaces, so case, accents and punctuation don't count.
fn normalize(title: &str) -> String {
    let title: String = title
        .to_lowercase()
        .chars()
        .map(fold_accent)
        .collect::<String>()
        .replace('&', " and ")
        .replace('ß', "ss")
        .replace('æ', "ae")
        .replace('œ', "oe");
    title
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// A lowercase Latin letter without its accent.
fn fold_accent(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'ç' => 'c',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'ñ' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' => 'o',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        'ý' | 'ÿ' => 'y',
        other => other,
    }
}

/// How alike two normalized titles are, from 0 to 1: 1 if they are the same, and otherwise the
/// square of the Sørensen–Dice coefficient of their letter pairs, ignoring spaces. Squaring
/// keeps near-misses on short titles, such as Alien and Aliens, below [`THRESHOLD`] while a
/// typo in a long title still fits.
fn similarity(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }
    let pairs = |text: &str| -> Vec<(char, char)> {
        let letters: Vec<char> = text.chars().filter(|c| *c != ' ').collect();
        letters
            .iter()
            .copied()
            .zip(letters.iter().copied().skip(1))
            .collect()
    };
    let a = pairs(a);
    let mut b = pairs(b);
    let total = a.len() + b.len();
    if total == 0 {
        return 0.0;
    }
    let mut shared = 0_usize;
    for pair in &a {
        if let Some(found) = b.iter().position(|other| other == pair) {
            b.swap_remove(found);
            shared += 1;
        }
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "title lengths are far below where f64 loses precision"
    )]
    let dice = (2 * shared) as f64 / total as f64;
    dice * dice
}

#[cfg(test)]
mod tests {
    use nama_plugin_api::metadata::{Candidate, ExternalId};

    use super::{Decision, decide};

    fn candidate(id: i64, title: &str, year: Option<i32>) -> Candidate {
        Candidate {
            id: ExternalId::Tmdb(id),
            title: title.to_owned(),
            original_title: None,
            year,
            poster: None,
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
            Decision::Match(ExternalId::Tmdb(348))
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
            Decision::Match(ExternalId::Tmdb(438_631))
        );
        assert_eq!(
            decide("Dune", Some(1984), &found),
            Decision::Match(ExternalId::Tmdb(841))
        );
    }

    #[test]
    fn a_year_off_by_one_still_matches() {
        let found = [candidate(438_631, "Dune", Some(2021))];

        assert_eq!(
            decide("Dune", Some(2020), &found),
            Decision::Match(ExternalId::Tmdb(438_631))
        );
    }

    #[test]
    fn case_and_punctuation_do_not_count() {
        let found = [candidate(945_961, "Alien: Romulus", Some(2024))];

        assert_eq!(
            decide("alien romulus", Some(2024), &found),
            Decision::Match(ExternalId::Tmdb(945_961))
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
            Decision::Match(ExternalId::Tmdb(387))
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
            Decision::Match(ExternalId::Tmdb(194))
        );
        assert_eq!(
            decide(
                "The Lord of the Rings - The Fellowshp of the Ring",
                Some(2001),
                &found
            ),
            Decision::Match(ExternalId::Tmdb(120))
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
            Decision::Match(ExternalId::Tmdb(329_865))
        );
        assert_eq!(
            decide("Oblivion", Some(2013), &oblivion),
            Decision::Match(ExternalId::Tmdb(75_612))
        );
        assert_eq!(
            decide("Band of Brothers", None, &band_of_brothers),
            Decision::Match(ExternalId::Tmdb(4613))
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

        assert_eq!(decide("The Office", None, &the_office), Decision::Ambiguous);
        assert_eq!(
            decide("Hamlet", Some(2000), &barely_rated),
            Decision::Ambiguous
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
            Decision::Match(ExternalId::Tmdb(120))
        );
        assert_eq!(
            decide(typo, Some(2001), &fellowship(2_501)),
            Decision::Ambiguous
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
}
