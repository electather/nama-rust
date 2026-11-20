use std::time::{Duration, Instant};

use nama_plugin_api::{
    contract::{FeaturedSuggestion, FeaturedSuggestions, TitleId, TitleKind},
    home_row::HomeRowMethod,
    metadata::ExternalId,
};

use super::{Suggested, add_heaviest, kept_suggestions};
use crate::home_rows::{
    HomeRows, Kind, Title, lock,
    tests::{SHARED, handle, insert_user, recorded, set_handle, started},
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
    let started = Instant::now();
    while !lock(&home.refreshing_featured).is_empty() {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "still refreshing"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
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
fn a_time_to_live_is_clamped_and_no_suggestions_are_asked_for_again_in_15_minutes() {
    let fresh_for = |suggestions, ttl_seconds| {
        let given = FeaturedSuggestions {
            suggestions,
            ttl_seconds,
            ..FeaturedSuggestions::default()
        };
        kept_suggestions(&given).1
    };

    assert_eq!(
        fresh_for(vec![suggestion("1", 1.0)], 60),
        Duration::from_mins(15)
    );
    assert_eq!(
        fresh_for(vec![suggestion("1", 1.0)], 30 * 24 * 3600),
        Duration::from_hours(7 * 24)
    );
    assert_eq!(fresh_for(vec![], 24 * 3600), Duration::from_mins(15));
}

#[tokio::test]
async fn a_source_giving_only_shared_rows_is_never_asked_for_featured_suggestions() {
    let (_dir, db, home, calls) = recorded(SHARED).await;
    let ada = insert_user(&db, "ada").await;
    set_handle(&db, ada, "1").await;

    let suggested = refreshed(&home, ada).await;

    assert!(calls.featured_asked.lock().unwrap().is_empty());
    assert_eq!(suggested, Suggested::new());
}

#[tokio::test]
async fn each_users_suggestions_are_asked_with_their_own_settings_and_reach_only_them() {
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
    assert_eq!(adas, movies(&[(1, 0.5)]));
    assert_eq!(beas, movies(&[(2, 0.5)]));
}

#[tokio::test]
async fn listing_featured_suggestions_never_waits_on_the_plugin() {
    let (_dir, db, home, calls) = recorded(FEATURED).await;
    let ada = insert_user(&db, "ada").await;
    set_handle(&db, ada, "1").await;
    let held = calls.users_held.lock().await;

    let listed = tokio::time::timeout(Duration::from_secs(5), home.featured(ada)).await;

    assert_eq!(
        listed.unwrap().unwrap(),
        Suggested::new(),
        "none stored yet"
    );
    drop(held);
    assert_eq!(refreshed(&home, ada).await, movies(&[(1, 0.5)]));
}

#[tokio::test]
async fn listing_stale_suggestions_asks_the_plugin_again_and_fresh_ones_do_not() {
    let (_dir, db, home, calls) = recorded(FEATURED).await;
    let ada = insert_user(&db, "ada").await;
    set_handle(&db, ada, "1").await;
    refreshed(&home, ada).await;
    refreshed(&home, ada).await;
    assert_eq!(*calls.featured_asked.lock().unwrap(), [handle("1")]);

    db.write(|tx| {
        Box::pin(async move {
            sqlx::query("UPDATE featured_suggestions SET stale_at = 0")
                .execute(&mut **tx)
                .await
                .map(drop)
        })
    })
    .await
    .unwrap();

    assert_eq!(refreshed(&home, ada).await, movies(&[(1, 0.5)]));
    assert_eq!(
        *calls.featured_asked.lock().unwrap(),
        [handle("1"), handle("1")]
    );
}

#[tokio::test]
async fn changing_personal_settings_drops_the_users_suggestions_and_asks_again() {
    let (_dir, db, home, calls) = recorded(FEATURED).await;
    let ada = insert_user(&db, "ada").await;
    set_handle(&db, ada, "1").await;
    refreshed(&home, ada).await;

    set_handle(&db, ada, "2").await;

    assert_eq!(home.featured(ada).await.unwrap(), Suggested::new());
    assert_eq!(refreshed(&home, ada).await, movies(&[(2, 0.5)]));
    assert_eq!(
        *calls.featured_asked.lock().unwrap(),
        [handle("1"), handle("2")]
    );
}

#[tokio::test]
async fn a_version_no_longer_giving_featured_suggestions_deletes_every_users() {
    let (_dir, db, old_version, _) = recorded(FEATURED).await;
    let ada = insert_user(&db, "ada").await;
    let bea = insert_user(&db, "bea").await;
    set_handle(&db, ada, "1").await;
    set_handle(&db, bea, "2").await;
    refreshed(&old_version, ada).await;
    refreshed(&old_version, bea).await;
    let (new_version, _) = started(&db, SHARED);

    assert_eq!(new_version.featured(ada).await.unwrap(), Suggested::new());

    let stored = new_version.stored_suggestions(bea).await.unwrap();
    assert!(stored.is_empty(), "Bea's are deleted too");
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

    assert!(home.stored_suggestions(ada).await.unwrap().is_empty());
}

#[test]
fn a_title_suggested_twice_keeps_its_heaviest_weight() {
    let mut suggested = Suggested::new();
    let alien = || Title(Kind::Movie, "tmdb-348".to_owned());

    add_heaviest(&mut suggested, vec![(alien(), 0.2)]);
    add_heaviest(&mut suggested, vec![(alien(), 0.7), (alien(), 0.4)]);

    assert_eq!(suggested, movies(&[(348, f64::from(0.7_f32))]));
}
