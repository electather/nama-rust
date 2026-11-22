//! Holding out each MovieLens user's latest ratings, recommending from the rest, and scoring how
//! highly the recommendations rank what was held out.

use std::collections::{HashMap, HashSet};

use nama_hub::titles::SimilarTitle;

use crate::movielens::Rating;

/// How many recommendations are scored: about a row on a home page.
pub(crate) const K: usize = 10;

/// How well recommendations rank users' held-out titles, each a mean over the users.
#[derive(Debug)]
pub(crate) struct Scores {
    /// How many users were scored.
    pub(crate) users: usize,
    /// Of each user's held-out titles, or the first `K` if more, the share among the top `K`.
    pub(crate) recall: f64,
    /// How near the top the held-out titles among the top `K` are, from 0 for none to 1 when they
    /// fill the top places.
    pub(crate) ndcg: f64,
}

/// A MovieLens user's ratings, split in time into what the recommender sees and what it must find.
#[derive(Debug, PartialEq)]
pub(crate) struct User {
    /// The titles they rated before the split, which aren't recommended to them again.
    pub(crate) seen: HashSet<String>,
    /// Of those, the titles they liked, which their recommendations are made from.
    pub(crate) liked: Vec<String>,
    /// The titles they liked after the split, which good recommendations rank highly.
    pub(crate) held_out: HashSet<String>,
}

/// Each user's ratings split into their earlier four fifths and their latest fifth. A user who
/// liked nothing on either side is left out, as nothing could be recommended or found.
pub(crate) fn split(mut ratings: Vec<Rating>) -> Vec<User> {
    ratings.sort_by_key(|rating| (rating.user, rating.time));
    ratings
        .chunk_by(|a, b| a.user == b.user)
        .filter_map(|ratings| {
            let (earlier, latest) = ratings.split_at(ratings.len() - ratings.len() / 5);
            let user = User {
                seen: earlier.iter().map(|rating| rating.title.clone()).collect(),
                liked: liked(earlier).collect(),
                held_out: liked(latest).collect(),
            };
            (!user.liked.is_empty() && !user.held_out.is_empty()).then_some(user)
        })
        .collect()
}

/// Scores each of `users`' top `K` recommendations from `similar`, each liked title's Similar
/// answer, against what they liked after the split.
pub(crate) fn score(users: &[User], similar: &HashMap<String, Vec<SimilarTitle>>) -> Scores {
    let (mut recall, mut ndcg) = (0.0, 0.0);
    for user in users {
        let top = recommend(user, similar);
        recall += recall_at_k(&top, &user.held_out);
        ndcg += ndcg_at_k(&top, &user.held_out);
    }
    let count = as_f64(users.len());
    Scores {
        users: users.len(),
        recall: recall / count,
        ndcg: ndcg / count,
    }
}

/// `user`'s top `K` titles by their weights in the Similar answers of the titles they liked,
/// summed, best first, leaving out what they've seen.
///
/// ponytail: sums Similar answers, as a Server merges its Similar sources; score an Account's
/// taste vector instead once the Hub makes one (#1825).
fn recommend<'a>(user: &User, similar: &'a HashMap<String, Vec<SimilarTitle>>) -> Vec<&'a str> {
    let mut weights: HashMap<&str, f32> = HashMap::new();
    let answers = user.liked.iter().filter_map(|title| similar.get(title));
    for alike in answers.flatten() {
        if !user.seen.contains(&alike.id.value) {
            *weights.entry(&alike.id.value).or_default() += alike.weight;
        }
    }
    let mut ranked: Vec<(&str, f32)> = weights.into_iter().collect();
    // Ties go by id, so a run ranks the same each time.
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(b.0)));
    ranked.into_iter().take(K).map(|(title, _)| title).collect()
}

fn recall_at_k(top: &[&str], held_out: &HashSet<String>) -> f64 {
    let found = top
        .iter()
        .filter(|title| held_out.contains(**title))
        .count();
    as_f64(found) / as_f64(held_out.len().min(K))
}

fn ndcg_at_k(top: &[&str], held_out: &HashSet<String>) -> f64 {
    let gain = |place: usize| 1.0 / (as_f64(place) + 2.0).log2();
    let found: f64 = (0..)
        .zip(top)
        .filter(|(_, title)| held_out.contains(**title))
        .map(|(place, _)| gain(place))
        .sum();
    let best: f64 = (0..held_out.len().min(K)).map(gain).sum();
    found / best
}

#[expect(
    clippy::cast_precision_loss,
    reason = "counts of MovieLens users and titles are far below 2^52"
)]
fn as_f64(count: usize) -> f64 {
    count as f64
}

/// The titles of `ratings` with at least 4 stars, MovieLens's usual line between liked and not.
fn liked(ratings: &[Rating]) -> impl Iterator<Item = String> {
    ratings
        .iter()
        .filter(|rating| rating.stars >= 4.0)
        .map(|rating| rating.title.clone())
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use nama_hub::titles::{ExternalId, SimilarTitle};

    use super::{User, score, split};
    use crate::movielens::Rating;

    fn rating(user: u32, title: &str, stars: f32, time: i64) -> Rating {
        Rating {
            user,
            title: title.to_owned(),
            stars,
            time,
        }
    }

    fn titles<C: FromIterator<String>>(titles: &[&str]) -> C {
        titles.iter().map(|title| (*title).to_owned()).collect()
    }

    #[test]
    fn a_users_latest_fifth_is_held_out_and_only_liked_titles_count() {
        let ratings = vec![
            rating(1, "tt5", 4.5, 50),
            rating(1, "tt1", 4.0, 10),
            rating(1, "tt2", 2.0, 20),
            rating(1, "tt3", 5.0, 30),
            rating(1, "tt4", 4.0, 40),
            rating(1, "tt6", 1.0, 60),
            rating(1, "tt7", 3.0, 70),
            rating(1, "tt8", 3.5, 80),
            rating(1, "tt9", 5.0, 90),
            rating(1, "tt10", 3.0, 100),
        ];

        let users = split(ratings);

        assert_eq!(
            users,
            [User {
                seen: titles(&["tt1", "tt2", "tt3", "tt4", "tt5", "tt6", "tt7", "tt8"]),
                liked: titles(&["tt1", "tt3", "tt4", "tt5"]),
                held_out: titles(&["tt9"]),
            }]
        );
    }

    #[test]
    fn a_user_who_liked_nothing_held_out_is_left_out() {
        let ratings = vec![
            rating(1, "tt1", 5.0, 10),
            rating(1, "tt2", 5.0, 20),
            rating(1, "tt3", 5.0, 30),
            rating(1, "tt4", 5.0, 40),
            rating(1, "tt5", 2.0, 50),
            rating(2, "tt1", 5.0, 10),
            rating(2, "tt2", 5.0, 20),
            rating(2, "tt3", 5.0, 30),
            rating(2, "tt4", 5.0, 40),
            rating(2, "tt5", 5.0, 50),
        ];

        let users = split(ratings);

        assert_eq!(users.len(), 1);
        assert_eq!(
            users.first().map(|user| &user.held_out),
            Some(&titles::<HashSet<String>>(&["tt5"]))
        );
    }

    fn similar(answers: &[(&str, &[(&str, f32)])]) -> HashMap<String, Vec<SimilarTitle>> {
        answers
            .iter()
            .map(|(title, alike)| {
                let alike = alike
                    .iter()
                    .map(|(alike, weight)| SimilarTitle {
                        id: ExternalId {
                            namespace: "imdb".to_owned(),
                            value: (*alike).to_owned(),
                        },
                        weight: *weight,
                    })
                    .collect();
                ((*title).to_owned(), alike)
            })
            .collect()
    }

    fn user(seen: &[&str], liked: &[&str], held_out: &[&str]) -> User {
        User {
            seen: titles(seen),
            liked: titles(liked),
            held_out: titles(held_out),
        }
    }

    #[test]
    fn scores_are_means_over_users_of_recall_and_ndcg_at_ten() {
        // Summed, unseen weights rank c (0.7), h1 (0.5), h2 (0.1): both held out titles found,
        // second and third. The second user's liked title has no answer.
        let users = [
            user(&["a", "b", "x"], &["a", "b"], &["h1", "h2"]),
            user(&["z"], &["z"], &["h1"]),
        ];
        let answers = similar(&[
            ("a", &[("x", 0.9), ("h1", 0.5), ("c", 0.4)]),
            ("b", &[("c", 0.3), ("h2", 0.1)]),
        ]);

        let scores = score(&users, &answers);

        assert_eq!(scores.users, 2);
        assert!((scores.recall - 0.5).abs() < 1e-9, "{scores:?}");
        // (1/log2(3) + 1/log2(4)) / (1 + 1/log2(3)) = 0.693426, halved.
        assert!((scores.ndcg - 0.346_713).abs() < 1e-6, "{scores:?}");
    }

    #[test]
    fn a_held_out_title_below_the_tenth_counts_for_nothing() {
        let users = [user(&["a"], &["a"], &["h"])];
        let answers = similar(&[(
            "a",
            &[
                ("c1", 0.9),
                ("c2", 0.9),
                ("c3", 0.9),
                ("c4", 0.9),
                ("c5", 0.9),
                ("c6", 0.9),
                ("c7", 0.9),
                ("c8", 0.9),
                ("c9", 0.9),
                ("c10", 0.9),
                ("h", 0.8),
            ],
        )]);

        let scores = score(&users, &answers);

        assert!(
            scores.recall.abs() < 1e-9 && scores.ndcg.abs() < 1e-9,
            "{scores:?}"
        );
    }
}
