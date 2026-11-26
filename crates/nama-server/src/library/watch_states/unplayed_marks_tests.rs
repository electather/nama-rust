use std::sync::Arc;

use super::super::{
    MovieOrEpisode, Progress, WatchStates, Watched,
    tests::{
        alien_at, alien_by_both_ids, alien_id, alien_imdb, made_elsewhere, movie_state,
        watch_states,
    },
};
use crate::{db::Database, plugins::ExtensionPoints};

#[tokio::test]
async fn marking_unplayed_outlasts_a_restart_skips_older_pulls_and_goes_with_its_user() {
    let data = tempfile::tempdir().unwrap();
    let (before, user) = watch_states(&data).await;
    let under_imdb = MovieOrEpisode::movie(&alien_imdb());
    before.mark(user, vec![under_imdb], false).await.unwrap();
    drop(before);
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let watch_states = WatchStates::new(Arc::clone(&db), ExtensionPoints::default());

    // Played before it was marked unplayed, played after, then marked unplayed before that play.
    for (secs_ago, pulled_played, played) in [
        (60, true, None),
        (-60, true, Some(true)),
        (-30, false, Some(true)),
    ] {
        let pulled = made_elsewhere(0, pulled_played, secs_ago);
        (watch_states.take_pulled(user, vec![(alien_by_both_ids(), pulled)]))
            .await
            .unwrap();
        let state = movie_state(&watch_states, user, alien_id()).await;
        assert_eq!(state.map(|state| state.played), played);
    }

    // The newer pulled play dropped the marks, so mark it again for the cascade to have some.
    (watch_states.mark(user, vec![alien_by_both_ids()], false))
        .await
        .unwrap();
    assert_eq!(unplayed_marks(&watch_states, user).await, 2);
    let delete_user = sqlx::query("DELETE FROM users WHERE id = ?").bind(user);
    db.write(|tx| Box::pin(async move { delete_user.execute(&mut **tx).await.map(drop) }))
        .await
        .unwrap();
    assert_eq!(unplayed_marks(&watch_states, user).await, 0);
}

/// How many of the User `user`'s marks of when a title was marked unplayed are kept.
async fn unplayed_marks(watch_states: &WatchStates, user: i64) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM forgotten_watch_states WHERE user_id = ?")
        .bind(user)
        .fetch_one(watch_states.db.reader())
        .await
        .unwrap()
}

#[tokio::test]
async fn an_offline_play_older_than_marking_unplayed_leaves_it_unplayed() {
    let data = tempfile::tempdir().unwrap();
    let (watch_states, user) = watch_states(&data).await;
    let under_imdb = MovieOrEpisode::movie(&alien_imdb());
    (watch_states.mark(user, vec![under_imdb], false))
        .await
        .unwrap();

    let played_before = made_elsewhere(0, true, 60);
    let taken = (watch_states.take_offline(user, vec![(alien_by_both_ids(), played_before)]))
        .await
        .unwrap();

    assert_eq!(taken, []);
    assert_eq!(movie_state(&watch_states, user, alien_id()).await, None);
}

#[tokio::test]
async fn a_pulled_or_offline_change_newer_than_marking_unplayed_is_taken_and_drops_the_mark() {
    let data = tempfile::tempdir().unwrap();
    let (watch_states, user) = watch_states(&data).await;
    let unplayed = made_elsewhere(0, false, 120);
    (watch_states.take_pulled(user, vec![(alien_by_both_ids(), unplayed)]))
        .await
        .unwrap();
    assert_eq!(unplayed_marks(&watch_states, user).await, 2);

    let played_offline = made_elsewhere(0, true, 60);
    let taken = (watch_states.take_offline(user, vec![(alien_by_both_ids(), played_offline)]))
        .await
        .unwrap();
    assert_eq!(taken.len(), 1);
    assert_eq!(unplayed_marks(&watch_states, user).await, 0);

    (watch_states.mark(user, vec![alien_by_both_ids()], false))
        .await
        .unwrap();
    let pulled = made_elsewhere(1_500, false, -60);
    (watch_states.take_pulled(user, vec![(alien_by_both_ids(), pulled.clone())]))
        .await
        .unwrap();
    assert_eq!(
        alien_at(&watch_states, user).await,
        (1_500, pulled.changed_at_ms)
    );
    assert_eq!(unplayed_marks(&watch_states, user).await, 0);
}

#[tokio::test]
async fn marking_unplayed_is_kept_under_every_id_until_played_again() {
    let data = tempfile::tempdir().unwrap();
    let (watch_states, user) = watch_states(&data).await;
    let halfway = Progress::new(1_000, 2_000).unwrap();

    (watch_states.mark(user, vec![alien_by_both_ids()], false))
        .await
        .unwrap();
    assert_eq!(unplayed_marks(&watch_states, user).await, 2);
    (watch_states.report(user, vec![Some(alien_by_both_ids())], halfway))
        .await
        .unwrap();
    assert_eq!(unplayed_marks(&watch_states, user).await, 0);

    let none: Vec<Watched> = Vec::new();
    (watch_states.mark_unplayed(user, vec![alien_by_both_ids()], none))
        .await
        .unwrap();
    assert_eq!(unplayed_marks(&watch_states, user).await, 2);
    (watch_states.mark(user, vec![alien_by_both_ids()], true))
        .await
        .unwrap();
    assert_eq!(unplayed_marks(&watch_states, user).await, 0);
}
