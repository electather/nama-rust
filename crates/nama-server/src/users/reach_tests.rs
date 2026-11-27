use std::{collections::BTreeSet, sync::Arc, time::Duration};

use futures_util::FutureExt as _;

use super::{Reach, reach};
use crate::{
    content_ratings::MaximumAge,
    db::Database,
    plugins::ExtensionPoints,
    users::{
        LibraryAccess, Permissions, User, Users,
        testing::{insert_user, reach_of},
    },
};

/// The Library a chosen list keeps.
const SEEN: i64 = 1;

const NO_AGE: MaximumAge = MaximumAge {
    age: None,
    allow_unrated: false,
};
const AGE_13_ALLOWING_UNRATED: MaximumAge = MaximumAge {
    age: Some(13),
    allow_unrated: true,
};
const AGE_13_BLOCKING_UNRATED: MaximumAge = MaximumAge {
    age: Some(13),
    allow_unrated: false,
};

/// What `reach` lets through, in the order: a title within the age, one over it, one whose
/// Content Rating isn't known, one without one, an Unmatched Item, each in [`SEEN`], and a
/// missing title, in no Library and with no known rating.
fn judged(reach: &Reach) -> [bool; 6] {
    [
        reach.reaches(SEEN, Some("PG")),
        reach.reaches(SEEN, Some("R")),
        reach.reaches(SEEN, Some("Not a rating")),
        reach.reaches(SEEN, None),
        reach.reaches_unmatched(SEEN),
        reach.allows_unrated(),
    ]
}

#[test]
fn library_access_and_maximum_age_together_decide_what_reaches_a_user() {
    let every = || LibraryAccess::All;
    let chosen = || LibraryAccess::Only(BTreeSet::from([SEEN]));
    let none = || LibraryAccess::Only(BTreeSet::new());
    let (t, f) = (true, false);
    let table = [
        (every(), NO_AGE, [t, t, t, t, t, t]),
        (every(), AGE_13_ALLOWING_UNRATED, [t, f, t, t, t, t]),
        (every(), AGE_13_BLOCKING_UNRATED, [t, f, f, f, f, f]),
        (chosen(), NO_AGE, [t, t, t, t, t, t]),
        (chosen(), AGE_13_ALLOWING_UNRATED, [t, f, t, t, t, t]),
        (chosen(), AGE_13_BLOCKING_UNRATED, [t, f, f, f, f, f]),
        (none(), NO_AGE, [f, f, f, f, f, t]),
        (none(), AGE_13_ALLOWING_UNRATED, [f, f, f, f, f, t]),
        (none(), AGE_13_BLOCKING_UNRATED, [f, f, f, f, f, f]),
    ];
    for (libraries, max_age, expected) in table {
        let row = format!("{libraries:?} {max_age:?}");
        assert_eq!(judged(&reach_of(libraries, max_age)), expected, "{row}");
    }
}

#[test]
fn a_chosen_list_keeps_out_every_other_library() {
    let chosen = reach_of(LibraryAccess::Only(BTreeSet::from([SEEN])), NO_AGE);

    assert!(chosen.sees(SEEN));
    assert!(!chosen.sees(SEEN + 1));
    assert!(!chosen.reaches(SEEN + 1, Some("G")));
    assert!(!chosen.reaches_unmatched(SEEN + 1));
}

#[test]
fn only_every_library_and_no_maximum_age_let_everything_through() {
    let every = reach_of(LibraryAccess::All, NO_AGE);
    let chosen = reach_of(LibraryAccess::Only(BTreeSet::from([SEEN])), NO_AGE);
    let capped = reach_of(LibraryAccess::All, AGE_13_ALLOWING_UNRATED);

    assert!(every.lets_everything());
    assert!(!chosen.lets_everything());
    assert!(!capped.lets_everything());
    assert!(Reach::everything().lets_everything());
}

#[test]
fn kept_ratings_bind_those_within_the_age_and_unrated_ones_with_the_empty_string() {
    let stored = || ["PG", "R", "Not a rating"].map(str::to_owned).to_vec();
    let kept = [NO_AGE, AGE_13_ALLOWING_UNRATED, AGE_13_BLOCKING_UNRATED]
        .map(|max_age| reach_of(LibraryAccess::All, max_age).kept_ratings(stored()));

    assert_eq!(
        kept,
        [
            None,
            Some(vec![
                "PG".to_owned(),
                "Not a rating".to_owned(),
                String::new()
            ]),
            Some(vec!["PG".to_owned()]),
        ]
    );
}

fn admin() -> User {
    User {
        id: 0,
        name: "Admin".to_owned(),
        permissions: Permissions::ADMIN,
        created_at: 0,
    }
}

#[tokio::test]
async fn a_users_reach_is_read_from_their_maximum_age_and_library_access() {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
    let users = Users::new(Arc::clone(&db), ExtensionPoints::default());
    let bob = insert_user(&db, "Bob").await;
    let chosen = LibraryAccess::Only(BTreeSet::new());
    users
        .set_library_access(&admin(), bob, chosen)
        .await
        .unwrap();
    let max_age = AGE_13_BLOCKING_UNRATED;
    users.set_maximum_age(&admin(), bob, max_age).await.unwrap();

    let read = reach(&db, bob).await.unwrap();

    assert_eq!(
        read,
        reach_of(LibraryAccess::Only(BTreeSet::new()), max_age)
    );
}

#[tokio::test]
async fn setting_either_library_access_or_the_maximum_age_ends_the_wait_on_a_reach() {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
    let users = Users::new(Arc::clone(&db), ExtensionPoints::default());
    let bob = insert_user(&db, "Bob").await;

    let (_, access_set) = users.reach_watched(bob).await.unwrap();
    let mut access_set = Box::pin(access_set);
    assert!((&mut access_set).now_or_never().is_none());
    let chosen = LibraryAccess::Only(BTreeSet::new());
    users
        .set_library_access(&admin(), bob, chosen)
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), access_set)
        .await
        .unwrap();

    let (_, age_set) = users.reach_watched(bob).await.unwrap();
    let max_age = AGE_13_ALLOWING_UNRATED;
    users.set_maximum_age(&admin(), bob, max_age).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), age_set)
        .await
        .unwrap();
}
