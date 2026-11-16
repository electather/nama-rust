//! Deciding which search result, if any, a file's title and year Match.
//!
//! Each candidate gets a confidence from 0 to 1: how alike the titles are (the candidate's
//! title or original title, whichever is closer) times how well the years agree. A candidate
//! Matches only above [`THRESHOLD`], and only if every other candidate within [`MARGIN`] of it
//! has at most a [`VOTE_RATIO`]th of its votes: two titles that fit about as well and are about
//! as well known are ambiguous, and an ambiguous file stays Unmatched rather than risk the wrong
//! Match.

use nama_plugin_api::metadata::ExternalId;

use crate::library::Candidate;

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
pub(super) enum Decision {
    /// One candidate fits confidently.
    Match(ExternalId),
    /// More than one candidate fits about as well: these, best fit first.
    Ambiguous(Vec<Candidate>),
    /// No candidate fits well enough.
    NoMatch,
}

pub(super) fn decide(title: &str, year: Option<i32>, candidates: &[Candidate]) -> Decision {
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
    let contenders = others
        .iter()
        .take_while(|(score, _)| best_score - score < MARGIN)
        .filter(|(_, other)| !outvotes(best, other))
        .map(|(_, other)| *other);
    let tied: Vec<Candidate> = std::iter::once(*best).chain(contenders).cloned().collect();
    if tied.len() > 1 {
        Decision::Ambiguous(tied)
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
#[path = "confidence_tests.rs"]
mod tests;
