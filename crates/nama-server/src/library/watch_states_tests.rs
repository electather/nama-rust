use std::{
    future,
    sync::Arc,
    task::{Context, Waker},
    time::Duration,
};

use nama_plugin_api::{
    BoxFuture, Manifest, Observer, Plugin, PluginError, PluginId, PointKey, Registrar,
    metadata::{ExternalId, ExternalIds},
    points::{WatchStateChange, WatchStateChanges, WatchStatesChanged},
};
use tokio::sync::mpsc;

use super::{Kind, MovieOrEpisode, Progress, WatchState, WatchStates, Watched};
use crate::{
    db::{self, Database},
    plugins::{ExtensionPoints, Registry},
    users::{Permissions, User, Users},
};

const ALIEN_TMDB_ID: i64 = 348;

/// Watch States on a database in a temp folder, and a User to keep them for.
async fn watch_states(data: &tempfile::TempDir) -> (WatchStates, i64) {
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let watch_states = WatchStates::new(db, ExtensionPoints::default());
    let user = another_user(&watch_states, "Ada").await;
    (watch_states, user)
}

/// Watch States as [`watch_states`] gives, whose Observers' news the test hears.
async fn heard_watch_states(
    data: &tempfile::TempDir,
) -> (WatchStates, i64, mpsc::UnboundedReceiver<WatchStateChanges>) {
    let (heard, hearing) = mpsc::unbounded_channel();
    let plugins = Registry::new();
    plugins.enable(Arc::new(Hears(heard))).unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let watch_states = WatchStates::new(db, plugins.extension_points());
    let user = another_user(&watch_states, "Ada").await;
    (watch_states, user, hearing)
}

/// A Plugin that hands each Watch State change it's told of to the test.
#[derive(Clone)]
struct Hears(mpsc::UnboundedSender<WatchStateChanges>);

impl Plugin for Hears {
    fn manifest(&self) -> Manifest {
        Manifest {
            extension_points: vec![PointKey::observe::<WatchStatesChanged>()],
            ..Manifest::new(PluginId::from_static("hears"))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.observe::<WatchStatesChanged>(self.clone());
        Ok(())
    }
}

impl Observer<WatchStatesChanged> for Hears {
    fn observe<'a>(
        &'a self,
        event: &'a WatchStateChanges,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        let sent = self.0.send(event.clone());
        Box::pin(future::ready(
            sent.map_err(|_unsent| "the test stopped listening".into()),
        ))
    }
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
async fn a_movie_played_again_keeps_when_it_first_became_played() {
    let data = tempfile::tempdir().unwrap();
    let (watch_states, user) = watch_states(&data).await;
    let near_the_end = Progress::new(1_950, 2_000).unwrap();
    watch_states
        .report(user, vec![Some(alien())], near_the_end)
        .await
        .unwrap();
    let first = movie_state(&watch_states, user, alien_id()).await.unwrap();

    db::advance_clock(Duration::from_secs(2));
    watch_states
        .report(user, vec![Some(alien())], near_the_end)
        .await
        .unwrap();
    db::advance_clock(Duration::from_secs(2));
    watch_states.mark(user, vec![alien()], true).await.unwrap();

    let again = movie_state(&watch_states, user, alien_id()).await.unwrap();
    assert_eq!(first.played_at, Some(first.updated_at));
    assert_eq!(again.played_at, first.played_at);
    assert!(again.updated_at > first.updated_at, "{again:?}");
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

fn alien_imdb() -> ExternalId {
    ExternalId::new("imdb", "tt0078748").unwrap()
}

/// Alien under its TMDB id, known by its IMDb id too.
fn alien_by_both_ids() -> Watched {
    let ids = ExternalIds::from_iter([alien_id(), alien_imdb()]);
    alien().known_as(&ids)
}

#[tokio::test]
async fn progress_on_a_movie_played_under_another_of_its_ids_is_stored_played() {
    let data = tempfile::tempdir().unwrap();
    let (watch_states, user) = watch_states(&data).await;
    let under_imdb = MovieOrEpisode::movie(&alien_imdb());
    watch_states
        .mark(user, vec![under_imdb], true)
        .await
        .unwrap();
    let near_the_start = Progress::new(100, 2_000).unwrap();

    (watch_states.report(user, vec![Some(alien_by_both_ids())], near_the_start))
        .await
        .unwrap();

    let state = movie_state(&watch_states, user, alien_id()).await.unwrap();
    assert_eq!((state.position_ms, state.played), (100, true));
}

#[tokio::test]
async fn marking_a_movie_unplayed_forgets_it_under_every_id() {
    let data = tempfile::tempdir().unwrap();
    let (watch_states, user) = watch_states(&data).await;
    let halfway = Progress::new(1_000, 2_000).unwrap();
    for id in [alien_id(), alien_imdb()] {
        (watch_states.report(user, vec![Some(MovieOrEpisode::movie(&id))], halfway))
            .await
            .unwrap();
    }

    (watch_states.mark(user, vec![alien_by_both_ids()], false))
        .await
        .unwrap();

    assert_eq!(movie_state(&watch_states, user, alien_id()).await, None);
    assert_eq!(movie_state(&watch_states, user, alien_imdb()).await, None);
}

/// Whether each of the User `user`'s Watch States is hidden from Continue Watching.
async fn hidden(watch_states: &WatchStates, user: i64) -> Vec<bool> {
    let states = watch_states.of_user(user).await.unwrap();
    states.into_iter().map(|(_, _, _, hidden)| hidden).collect()
}

#[tokio::test]
async fn hiding_a_movie_hides_only_that_users_and_leaves_its_watch_state_alone() {
    let data = tempfile::tempdir().unwrap();
    let (watch_states, ada) = watch_states(&data).await;
    let bob = another_user(&watch_states, "Bob").await;
    let halfway = Progress::new(1_000, 2_000).unwrap();
    for user in [ada, bob] {
        (watch_states.report(user, vec![Some(alien())], halfway))
            .await
            .unwrap();
    }
    let before = movie_state(&watch_states, ada, alien_id()).await;

    watch_states.hide(ada, vec![alien()], true).await.unwrap();

    assert_eq!(movie_state(&watch_states, ada, alien_id()).await, before);
    assert_eq!(hidden(&watch_states, ada).await, [true]);
    assert_eq!(hidden(&watch_states, bob).await, [false]);
}

#[tokio::test]
async fn each_write_returns_the_watch_state_it_left_under_every_id_preferred_first() {
    let data = tempfile::tempdir().unwrap();
    let (watch_states, user) = watch_states(&data).await;
    let ids = ExternalIds::from_iter([ExternalId::new("imdb", "tt0078748").unwrap(), alien_id()]);
    let known = || alien().known_as(&ids);

    watch_states
        .report(
            user,
            vec![Some(known())],
            Progress::new(1_950, 2_000).unwrap(),
        )
        .await
        .unwrap();
    let rewound = (watch_states.report(
        user,
        vec![Some(known())],
        Progress::new(100, 2_000).unwrap(),
    ))
    .await
    .unwrap();
    let marked = watch_states.mark(user, vec![known()], true).await.unwrap();
    let unplayed = watch_states.mark(user, vec![known()], false).await.unwrap();

    let [rewound] = rewound.as_slice() else {
        panic!("{rewound:?}")
    };
    assert_eq!(rewound.ids, ["tmdb-348", "imdb-tt0078748"]);
    assert!(!rewound.episode);
    assert_eq!(
        (rewound.played, rewound.position_ms, rewound.duration_ms),
        (true, 100, 2_000)
    );
    assert_eq!(
        (
            marked[0].played,
            marked[0].position_ms,
            marked[0].duration_ms
        ),
        (true, 100, 2_000)
    );
    assert_eq!(
        (
            unplayed[0].played,
            unplayed[0].position_ms,
            unplayed[0].duration_ms
        ),
        (false, 0, 0)
    );
}

/// A change to Alien made on another Server, or offline on a Device: at `position_ms` of 2000, or
/// played, made `secs_ago` seconds ago, which may be negative.
fn made_elsewhere(position_ms: u64, played: bool, secs_ago: i64) -> WatchStateChange {
    WatchStateChange {
        episode: false,
        ids: vec![alien_imdb().to_string()],
        played,
        position_ms,
        duration_ms: if position_ms == 0 && !played {
            0
        } else {
            2_000
        },
        changed_at_ms: (db::now() - secs_ago) * 1000,
    }
}

#[tokio::test]
async fn a_pulled_change_is_taken_only_when_newer_than_the_servers_own_under_any_id() {
    let data = tempfile::tempdir().unwrap();
    let (watch_states, user) = watch_states(&data).await;
    let halfway = Progress::new(1_000, 2_000).unwrap();
    (watch_states.report(
        user,
        vec![Some(MovieOrEpisode::movie(&alien_imdb()))],
        halfway,
    ))
    .await
    .unwrap();

    let older = made_elsewhere(500, false, 60);
    (watch_states.take_pulled(user, vec![(alien_by_both_ids(), older)]))
        .await
        .unwrap();
    let own = movie_state(&watch_states, user, alien_imdb())
        .await
        .unwrap();
    assert_eq!(own.position_ms, 1_000);
    assert_eq!(movie_state(&watch_states, user, alien_id()).await, None);

    let newer = made_elsewhere(1_500, false, -60);
    (watch_states.take_pulled(user, vec![(alien_by_both_ids(), newer.clone())]))
        .await
        .unwrap();
    let state = movie_state(&watch_states, user, alien_id()).await.unwrap();
    assert_eq!(
        (state.position_ms, state.updated_at * 1000),
        (1_500, newer.changed_at_ms)
    );
}

#[tokio::test]
async fn a_pulled_change_marked_played_on_another_server_stays_played_here() {
    let data = tempfile::tempdir().unwrap();
    let (watch_states, user) = watch_states(&data).await;

    let played = made_elsewhere(0, true, 60);
    (watch_states.take_pulled(user, vec![(alien_by_both_ids(), played)]))
        .await
        .unwrap();

    let state = movie_state(&watch_states, user, alien_id()).await.unwrap();
    assert!(state.played);
    assert_eq!(state.played_at, Some(state.updated_at));
}

#[tokio::test]
async fn a_newer_pulled_change_marking_it_unplayed_forgets_it_under_every_id() {
    let data = tempfile::tempdir().unwrap();
    let (watch_states, user) = watch_states(&data).await;
    let halfway = Progress::new(1_000, 2_000).unwrap();
    (watch_states.report(user, vec![Some(alien())], halfway))
        .await
        .unwrap();

    let unplayed = made_elsewhere(0, false, -60);
    (watch_states.take_pulled(user, vec![(alien_by_both_ids(), unplayed)]))
        .await
        .unwrap();

    assert_eq!(movie_state(&watch_states, user, alien_id()).await, None);
}

#[tokio::test]
async fn of_a_titles_pulled_changes_under_several_ids_only_the_newest_is_taken_in_any_order() {
    let data = tempfile::tempdir().unwrap();
    let (watch_states, user) = watch_states(&data).await;
    let played_earlier = made_elsewhere(0, true, 120);
    let unplayed_later = WatchStateChange {
        ids: vec![alien_id().to_string()],
        ..made_elsewhere(0, false, 60)
    };

    for pulled in [
        vec![played_earlier.clone(), unplayed_later.clone()],
        vec![unplayed_later, played_earlier],
    ] {
        let taken = pulled
            .into_iter()
            .map(|change| (alien_by_both_ids(), change));
        (watch_states.take_pulled(user, taken.collect()))
            .await
            .unwrap();

        assert_eq!(movie_state(&watch_states, user, alien_id()).await, None);
    }
}

#[tokio::test]
async fn a_change_whose_caller_stops_waiting_is_still_stored_and_told() {
    let data = tempfile::tempdir().unwrap();
    let (watch_states, user, mut hearing) = heard_watch_states(&data).await;

    // Polled once, which sends the write on its way, then dropped, as when a client hangs up
    // (ADR 0029).
    let mut marking = Box::pin(watch_states.mark(user, vec![alien()], true));
    let polled = marking
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()));
    assert!(polled.is_pending(), "the mark finished at once");
    drop(marking);
    let told = tokio::time::timeout(Duration::from_secs(5), hearing.recv()).await;

    let told = told.expect("no Observer was told").unwrap();
    assert_eq!((told.user_id, told.changes[0].played), (user, true));
    let stored = movie_state(&watch_states, user, alien_id()).await;
    assert!(stored.is_some_and(|state| state.played));
}

/// Where Alien stands for `user`: its position and when it last changed, in milliseconds.
async fn alien_at(watch_states: &WatchStates, user: i64) -> (i64, i64) {
    let state = movie_state(watch_states, user, alien_id()).await.unwrap();
    (state.position_ms, state.updated_at * 1000)
}

#[tokio::test]
async fn an_offline_change_is_stored_with_its_own_time_only_when_newer_than_the_stored_one() {
    let data = tempfile::tempdir().unwrap();
    let (watch_states, user) = watch_states(&data).await;
    let stored = made_elsewhere(1_000, false, 120);
    (watch_states.take_offline(user, vec![(alien_by_both_ids(), stored.clone())]))
        .await
        .unwrap();
    let stored_at = alien_at(&watch_states, user).await;

    let older = made_elsewhere(500, false, 180);
    let same_second = WatchStateChange {
        changed_at_ms: stored_at.1 + 999,
        ..made_elsewhere(700, false, 0)
    };
    let newer = made_elsewhere(1_500, false, 60);
    for ignored in [older, same_second] {
        (watch_states.take_offline(user, vec![(alien_by_both_ids(), ignored)]))
            .await
            .unwrap();
        assert_eq!(alien_at(&watch_states, user).await, stored_at);
    }
    (watch_states.take_offline(user, vec![(alien_by_both_ids(), newer.clone())]))
        .await
        .unwrap();

    assert_eq!(stored_at, (1_000, stored.changed_at_ms));
    assert_eq!(
        alien_at(&watch_states, user).await,
        (1_500, newer.changed_at_ms)
    );
}

#[tokio::test]
async fn a_newer_offline_change_to_a_played_movie_keeps_it_played() {
    let data = tempfile::tempdir().unwrap();
    let (watch_states, user) = watch_states(&data).await;
    let played = made_elsewhere(1_950, true, 120);
    let rewound = made_elsewhere(500, false, 60);

    for change in [played, rewound] {
        (watch_states.take_offline(user, vec![(alien_by_both_ids(), change)]))
            .await
            .unwrap();
    }

    let state = movie_state(&watch_states, user, alien_id()).await.unwrap();
    assert_eq!((state.played, state.position_ms), (true, 500));
}

#[tokio::test]
async fn observers_hear_each_offline_change_stored_and_none_ignored() {
    let data = tempfile::tempdir().unwrap();
    let (watch_states, user, mut hearing) = heard_watch_states(&data).await;
    let mut hear = async || {
        let told = tokio::time::timeout(Duration::from_secs(5), hearing.recv()).await;
        told.expect("no Observer was told").unwrap().changes
    };
    let stored = made_elsewhere(1_000, false, 120);
    let older = made_elsewhere(500, false, 180);
    let newer = made_elsewhere(1_500, false, 60);

    (watch_states.take_offline(user, vec![(alien_by_both_ids(), stored)]))
        .await
        .unwrap();
    let told_stored = hear().await;
    let ignored = (watch_states.take_offline(user, vec![(alien_by_both_ids(), older)]))
        .await
        .unwrap();
    (watch_states.take_offline(user, vec![(alien_by_both_ids(), newer.clone())]))
        .await
        .unwrap();
    let told_newer = hear().await;

    assert_eq!(told_stored.len(), 1);
    assert_eq!(ignored, []);
    let [told] = told_newer.as_slice() else {
        panic!("{told_newer:?}")
    };
    assert_eq!(told.ids, ["tmdb-348", "imdb-tt0078748"]);
    assert_eq!(
        (told.position_ms, told.changed_at_ms),
        (1_500, newer.changed_at_ms)
    );
}

#[tokio::test]
async fn an_offline_change_dated_in_the_future_is_taken_as_made_now() {
    let data = tempfile::tempdir().unwrap();
    let (watch_states, user) = watch_states(&data).await;
    let from_the_future = made_elsewhere(1_500, false, -3_600);

    let taken = (watch_states.take_offline(user, vec![(alien_by_both_ids(), from_the_future)]))
        .await
        .unwrap();

    let after = db::now_ms();
    assert!(taken[0].changed_at_ms <= after, "{taken:?} after {after}");
    assert!(alien_at(&watch_states, user).await.1 <= after);
}

#[tokio::test]
async fn offline_changes_sent_out_of_order_are_taken_in_the_order_they_were_made() {
    let data = tempfile::tempdir().unwrap();
    let (watch_states, user) = watch_states(&data).await;
    let played = made_elsewhere(1_950, true, 120);
    let rewound = made_elsewhere(500, false, 60);

    let sent = [rewound.clone(), played].map(|change| (alien_by_both_ids(), change));
    let taken = watch_states.take_offline(user, sent.into()).await.unwrap();

    assert_eq!(taken.len(), 2);
    let state = movie_state(&watch_states, user, alien_id()).await.unwrap();
    assert_eq!(
        (state.played, state.position_ms, state.updated_at * 1000),
        (true, 500, rewound.changed_at_ms)
    );
}
