use std::{sync::Arc, time::Duration};

use nama_plugin_api::metadata::{ExternalId, ExternalIds};

use super::{Kind, MovieOrEpisode, Progress, WatchState, WatchStates};
use crate::{
    db::{self, Database},
    plugins::Registry,
    users::{Permissions, User, Users},
};

const ALIEN_TMDB_ID: i64 = 348;

/// Watch States on a database in a temp folder, and a User to keep them for.
async fn watch_states(data: &tempfile::TempDir) -> (WatchStates, i64) {
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let watch_states = WatchStates::new(db);
    let user = another_user(&watch_states, "Ada").await;
    (watch_states, user)
}

/// Creates a User called `name` on the database of `watch_states`, and gives their id.
async fn another_user(watch_states: &WatchStates, name: &str) -> i64 {
    let root = User {
        id: 0,
        name: "Root".to_owned(),
        permissions: Permissions::ADMIN,
        created_at: 0,
    };
    let plugins = Registry::new();
    plugins
        .enable(Arc::new(nama_plugin_local_auth::LocalAuth))
        .unwrap();
    Users::new(Arc::clone(&watch_states.db), plugins.extension_points())
        .create(&root, name, "correct horse battery staple")
        .await
        .unwrap()
}

fn alien() -> MovieOrEpisode {
    MovieOrEpisode::movie(&ExternalId::tmdb(ALIEN_TMDB_ID))
}

/// The User `user`'s Watch State of the Movie `id`, if they've started it.
async fn movie_state(watch_states: &WatchStates, user: i64, id: ExternalId) -> Option<WatchState> {
    let title = (Kind::Movie, ExternalIds::from_iter([id]));
    let states = watch_states.of_titles(Some(user), &[title]).await.unwrap();
    states[0].first().map(|&(_, state)| state)
}

fn alien_id() -> ExternalId {
    ExternalId::tmdb(ALIEN_TMDB_ID)
}

#[tokio::test]
async fn a_movie_once_played_stays_played_when_played_again_from_the_start() {
    let data = tempfile::tempdir().unwrap();
    let (watch_states, user) = watch_states(&data).await;
    let near_the_end = Progress::new(1_950, 2_000).unwrap();
    let near_the_start = Progress::new(100, 2_000).unwrap();

    for progress in [near_the_end, near_the_start] {
        watch_states
            .report(user, vec![Some(alien())], progress)
            .await
            .unwrap();
    }

    let state = movie_state(&watch_states, user, alien_id()).await.unwrap();
    assert_eq!((state.position_ms, state.played), (100, true));
}

#[tokio::test]
async fn marking_a_movie_unplayed_forgets_its_position() {
    let data = tempfile::tempdir().unwrap();
    let (watch_states, user) = watch_states(&data).await;
    let halfway = Progress::new(1_000, 2_000).unwrap();
    watch_states
        .report(user, vec![Some(alien())], halfway)
        .await
        .unwrap();
    watch_states.mark(user, vec![alien()], true).await.unwrap();

    watch_states.mark(user, vec![alien()], false).await.unwrap();

    assert_eq!(movie_state(&watch_states, user, alien_id()).await, None);
}

#[tokio::test]
async fn a_watch_state_kept_against_a_made_up_id_is_read_back() {
    let data = tempfile::tempdir().unwrap();
    let (watch_states, user) = watch_states(&data).await;
    let akira = ExternalId::new("anidb", "m-1").unwrap();
    let halfway = Progress::new(1_000, 2_000).unwrap();

    watch_states
        .report(user, vec![Some(MovieOrEpisode::movie(&akira))], halfway)
        .await
        .unwrap();

    let state = movie_state(&watch_states, user, akira).await.unwrap();
    assert_eq!((state.position_ms, state.played), (1_000, false));
}

/// Each User's Watch State `states` gives of one title, as the User, position and whether played.
fn read(states: &[(i64, WatchState)]) -> Vec<(i64, i64, bool)> {
    (states.iter())
        .map(|(user, state)| (*user, state.position_ms, state.played))
        .collect()
}

#[tokio::test]
async fn a_users_watch_states_under_two_of_a_titles_ids_keep_the_newer() {
    let data = tempfile::tempdir().unwrap();
    let (watch_states, ada) = watch_states(&data).await;
    let imdb = ExternalId::new("imdb", "tt0078748").unwrap();
    let tmdb = ExternalId::tmdb(ALIEN_TMDB_ID);
    let halfway = Progress::new(1_000, 2_000).unwrap();
    let near_the_end = Progress::new(1_950, 2_000).unwrap();
    let report = |id: &ExternalId, progress| {
        watch_states.report(ada, vec![Some(MovieOrEpisode::movie(id))], progress)
    };
    report(&imdb, halfway).await.unwrap();
    // Later, under the other id, as after the Library's title source order changed. `imdb`
    // sorts first, so only being more recent keeps `tmdb`'s.
    db::advance_clock(Duration::from_secs(2));
    report(&tmdb, near_the_end).await.unwrap();
    let not_alien = MovieOrEpisode::movie(&ExternalId::tmdb(1));
    let an_episode = MovieOrEpisode::episode(&imdb);
    watch_states
        .mark(ada, vec![not_alien, an_episode], true)
        .await
        .unwrap();

    let ids = ExternalIds::from_iter([tmdb, imdb]);
    let states = watch_states
        .of_titles(None, &[(Kind::Movie, ids)])
        .await
        .unwrap();

    let [alien] = states.as_slice() else {
        panic!("{states:?}");
    };
    assert_eq!(read(alien), [(ada, 1_950, true)]);
}

#[tokio::test]
async fn every_users_watch_states_of_a_pages_titles_come_in_its_order_by_user() {
    let data = tempfile::tempdir().unwrap();
    let (watch_states, ada) = watch_states(&data).await;
    let bo = another_user(&watch_states, "Bo").await;
    let imdb = ExternalId::new("imdb", "tt0078748").unwrap();
    let lies = ExternalId::tmdb(1_002);
    let halfway = Progress::new(1_000, 2_000).unwrap();
    (watch_states.report(bo, vec![Some(alien())], halfway))
        .await
        .unwrap();
    let alien_under_imdb = MovieOrEpisode::movie(&imdb);
    let lies_episode = MovieOrEpisode::episode(&lies);
    watch_states
        .mark(ada, vec![alien_under_imdb, lies_episode.clone()], true)
        .await
        .unwrap();
    watch_states
        .mark(bo, vec![lies_episode], true)
        .await
        .unwrap();

    let page = [
        (Kind::Episode, ExternalIds::from_iter([lies])),
        (Kind::Movie, ExternalIds::from_iter([ExternalId::tmdb(1)])),
        (
            Kind::Movie,
            ExternalIds::from_iter([ExternalId::tmdb(ALIEN_TMDB_ID), imdb]),
        ),
    ];
    let states = watch_states.of_titles(None, &page).await.unwrap();

    let listed: Vec<_> = states.iter().map(|states| read(states)).collect();
    assert_eq!(
        listed,
        [
            vec![(ada, 0, true), (bo, 0, true)],
            vec![],
            vec![(ada, 0, true), (bo, 1_000, false)],
        ]
    );
}

#[tokio::test]
async fn watch_states_of_one_user_leave_out_every_other_users() {
    let data = tempfile::tempdir().unwrap();
    let (watch_states, ada) = watch_states(&data).await;
    let bo = another_user(&watch_states, "Bo").await;
    let halfway = Progress::new(1_000, 2_000).unwrap();
    (watch_states.report(bo, vec![Some(alien())], halfway))
        .await
        .unwrap();
    watch_states.mark(ada, vec![alien()], true).await.unwrap();
    let title = (Kind::Movie, ExternalIds::from_iter([alien_id()]));

    let of_bo = (watch_states.of_titles(Some(bo), std::slice::from_ref(&title)))
        .await
        .unwrap();
    let of_nobody = (watch_states.of_titles(Some(bo + ada + 1), &[title]))
        .await
        .unwrap();

    assert_eq!(read(&of_bo[0]), [(bo, 1_000, false)]);
    assert_eq!(read(&of_nobody[0]), []);
}

fn shares(position_ms: u64, duration_ms: u64, episodes: usize) -> Vec<Option<(i64, i64)>> {
    let progress = Progress::new(position_ms, duration_ms).unwrap();
    progress
        .split(episodes)
        .into_iter()
        .map(|share| share.map(|share| (share.position_ms, share.duration_ms)))
        .collect()
}

#[test]
fn a_two_episode_file_fills_the_first_episodes_half_then_the_seconds() {
    assert_eq!(
        shares(40, 60, 2),
        [Some((30, 30)), Some((10, 30))],
        "40 minutes into an hour"
    );
    assert_eq!(
        shares(30, 60, 2),
        [Some((30, 30)), None],
        "the second not reached"
    );
    assert_eq!(
        shares(0, 60, 2),
        [Some((0, 30)), None],
        "the first is reached at 0"
    );
}

#[test]
fn shares_that_dont_divide_evenly_still_add_up_to_the_duration() {
    assert_eq!(
        shares(1_000, 1_000, 3),
        [Some((333, 333)), Some((333, 333)), Some((334, 334))]
    );
    assert_eq!(
        shares(1, 1, 2),
        [None, Some((1, 1))],
        "a share too short to hold any"
    );
}

#[test]
fn a_position_past_the_duration_is_the_end_and_a_zero_duration_is_refused() {
    assert_eq!(Progress::new(5_000, 1_000), Progress::new(1_000, 1_000));
    assert_eq!(Progress::new(0, 0), None);
    assert_eq!(Progress::new(0, u64::MAX), None);
}
