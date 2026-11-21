use std::time::Duration;

use nama_plugin_api::{
    contract::{FeaturedSuggestion, FeaturedSuggestions, TitleId, TitleKind},
    home_row::HomeRowMethod,
    metadata::ExternalId,
};

use super::{Suggested, Suggestions, add_heaviest, kept_suggestions};
use crate::home_rows::{
    HomeRows, Kind, Title,
    tests::{handle, insert_user, recorded, set_handle},
};

const FEATURED: &[HomeRowMethod] = &[HomeRowMethod::Shared, HomeRowMethod::Featured];

/// The suggestion of the Movie of TMDB id `tmdb` at `weight`.
fn suggestion(tmdb: &str, weight: f32) -> FeaturedSuggestion {
    FeaturedSuggestion {
        kind: TitleKind::TITLE_KIND_MOVIE.into(),
        id: TitleId {
            namespace: "tmdb".to_owned(),
            value: tmdb.to_owned(),
            ..TitleId::default()
        }
        .into(),
        weight,
        ..FeaturedSuggestion::default()
    }
}

/// Plugin suggestions of the Movies of `tmdb` ids, each at its weight.
fn movies(tmdb: &[(i64, f64)]) -> Suggested {
    (tmdb.iter())
        .map(|&(tmdb, weight)| ((false, ExternalId::tmdb(tmdb)), weight))
        .collect()
}

/// Lists `user`'s Featured suggestions, waits for the refreshes that starts, and lists them again.
async fn refreshed(home: &HomeRows, user: i64) -> Suggested {
    home.featured(user).await.unwrap();
    home.refreshes.settled().await;
    home.featured(user).await.unwrap()
}

#[test]
fn weights_are_clamped_unreadable_titles_dropped_and_the_first_20_kept() {
    let mut given: Vec<FeaturedSuggestion> = vec![
        suggestion("1", 1.5),
        suggestion("2", -0.5),
        suggestion("-0", -0.0),
        suggestion("3", f32::NAN),
        suggestion("", 0.5),
        FeaturedSuggestion {
            kind: TitleKind::TITLE_KIND_UNSPECIFIED.into(),
            ..suggestion("4", 0.5)
        },
    ];
    given.extend((5..30).map(|n| suggestion(&n.to_string(), 0.5)));

    let (kept, _) = kept_suggestions(&FeaturedSuggestions {
        suggestions: given,
        ..FeaturedSuggestions::default()
    });

    let movie = |tmdb: &str| Title(Kind::Movie, format!("tmdb-{tmdb}"));
    assert_eq!(kept[..2], [(movie("1"), 1.0), (movie("2"), 0.0)]);
    assert!(kept[2].1.is_sign_positive(), "-0.0 is kept as 0.0");
    assert_eq!(kept.len(), 3 + 14, "the first 20, of which 3 don't read");
    assert_eq!(kept.last(), Some(&(movie("18"), 0.5)));
}

#[test]
fn no_suggestions_are_asked_for_again_in_15_minutes() {
    let given = FeaturedSuggestions {
        ttl_seconds: 24 * 3600,
        ..FeaturedSuggestions::default()
    };

    assert_eq!(kept_suggestions(&given).1, Duration::from_mins(15));
}

#[tokio::test]
async fn each_users_suggestions_are_asked_with_their_id_and_settings_and_reach_only_them() {
    let (_dir, db, home, calls) = recorded(FEATURED).await;
    let ada = insert_user(&db, "ada").await;
    let bea = insert_user(&db, "bea").await;
    set_handle(&db, ada, "1").await;
    set_handle(&db, bea, "2").await;

    let adas = refreshed(&home, ada).await;
    let beas = refreshed(&home, bea).await;

    assert_eq!(
        *calls.featured_asked.lock().unwrap(),
        [handle("1"), handle("2")]
    );
    assert_eq!(*calls.user_ids.lock().unwrap(), [ada, bea]);
    assert_eq!(adas, movies(&[(1, 0.5)]));
    assert_eq!(beas, movies(&[(2, 0.5)]));
}

#[tokio::test]
async fn deleting_a_user_deletes_their_suggestions() {
    let (_dir, db, home, _) = recorded(FEATURED).await;
    let ada = insert_user(&db, "ada").await;
    set_handle(&db, ada, "1").await;
    refreshed(&home, ada).await;

    db.write(move |tx| {
        Box::pin(async move {
            sqlx::query("DELETE FROM users WHERE id = ?")
                .bind(ada)
                .execute(&mut **tx)
                .await
                .map(drop)
        })
    })
    .await
    .unwrap();

    let stored = Suggestions(home.clone()).stored(ada).await.unwrap();
    assert!(stored.is_empty());
}

#[tokio::test]
async fn changing_personal_settings_lists_the_old_suggestions_until_the_plugin_answers_again() {
    let (_dir, db, home, calls) = recorded(FEATURED).await;
    let ada = insert_user(&db, "ada").await;
    set_handle(&db, ada, "1").await;
    refreshed(&home, ada).await;

    set_handle(&db, ada, "2").await;

    assert_eq!(home.featured(ada).await.unwrap(), movies(&[(1, 0.5)]));
    // Not `refreshed`: listing again while that listing's refresh is ending reads the old
    // suggestions and asks the Plugin a second time (#1506).
    home.refreshes.settled().await;
    assert_eq!(home.featured(ada).await.unwrap(), movies(&[(2, 0.5)]));
    assert_eq!(
        *calls.featured_asked.lock().unwrap(),
        [handle("1"), handle("2")]
    );
}

#[test]
fn a_title_suggested_twice_keeps_its_heaviest_weight() {
    let mut suggested = Suggested::new();
    let alien = || Title(Kind::Movie, "tmdb-348".to_owned());

    add_heaviest(&mut suggested, vec![(alien(), 0.2)]);
    add_heaviest(&mut suggested, vec![(alien(), 0.7), (alien(), 0.4)]);

    assert_eq!(suggested, movies(&[(348, f64::from(0.7_f32))]));
}
