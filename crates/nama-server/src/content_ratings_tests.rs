use super::{MaximumAge, minimum_age};

#[test]
fn mpaa_ratings_map_to_their_ages() {
    let ages = ["G", "PG", "PG-13", "R", "NC-17"].map(minimum_age);
    assert_eq!(ages, [Some(0), Some(10), Some(13), Some(17), Some(18)]);
}

#[test]
fn us_tv_ratings_map_to_their_ages() {
    let ratings = ["TV-Y", "TV-Y7", "TV-G", "TV-PG", "TV-14", "TV-MA"];
    let ages = ratings.map(minimum_age);
    assert_eq!(
        ages,
        [Some(0), Some(7), Some(0), Some(10), Some(14), Some(17)]
    );
}

#[test]
fn bbfc_ratings_map_to_their_ages() {
    let ages = ["U", "12A", "12", "15", "18", "R18"].map(minimum_age);
    assert_eq!(
        ages,
        [Some(0), Some(12), Some(12), Some(15), Some(18), Some(18)]
    );
}

#[test]
fn fsk_ratings_map_to_their_ages() {
    let ages = ["0", "6", "12", "16", "18", "FSK 16", "FSK16"].map(minimum_age);
    assert_eq!(
        ages,
        [
            Some(0),
            Some(6),
            Some(12),
            Some(16),
            Some(18),
            Some(16),
            Some(16)
        ]
    );
}

#[test]
fn acb_ratings_map_to_their_ages() {
    let ages = ["G", "PG", "M", "MA15+", "R18+", "X18+"].map(minimum_age);
    assert_eq!(
        ages,
        [Some(0), Some(10), Some(15), Some(15), Some(18), Some(18)]
    );
}

#[test]
fn ratings_match_whatever_their_case_spacing_or_country_prefix() {
    let ages = [" pg-13 ", "Rated R", "US:TV-MA", "de:16"].map(minimum_age);
    assert_eq!(ages, [Some(13), Some(17), Some(17), Some(16)]);
}

#[test]
fn unknown_ratings_are_unrated() {
    let ages = ["", "NR", "Unrated", "Not Rated", "banana", "999999999999"].map(minimum_age);
    assert_eq!(ages, [None; 6]);
}

#[test]
fn no_maximum_age_allows_every_title() {
    let none = MaximumAge::default();
    assert!(
        ["NC-17", "banana"]
            .iter()
            .all(|rating| none.allows(Some(rating)))
    );
    assert!(none.allows(None));
}

#[test]
fn a_maximum_age_allows_only_ratings_at_or_under_it() {
    let twelve = MaximumAge {
        age: Some(12),
        allow_unrated: false,
    };
    assert!(twelve.allows(Some("PG")));
    assert!(twelve.allows(Some("12A")));
    assert!(!twelve.allows(Some("PG-13")));
    assert!(!twelve.allows(Some("NR")));
    assert!(!twelve.allows(None));
}

#[test]
fn allowing_unrated_lets_unknown_and_missing_ratings_through() {
    let twelve = MaximumAge {
        age: Some(12),
        allow_unrated: true,
    };
    assert!(twelve.allows(None));
    assert!(twelve.allows(Some("NR")));
    assert!(!twelve.allows(Some("R")));
}
