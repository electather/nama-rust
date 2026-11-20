use std::{
    collections::HashMap,
    future::ready,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
    time::{Duration, Instant},
};

use nama_plugin_api::{
    PluginId,
    contract::{SharedRowsAsked, UserFeaturedAsked, UserRowsAsked},
    home_row::{HomeRowMethod, HomeRowSource},
};
use sqlx::SqliteConnection;
use tempfile::TempDir;

use super::{Adapter, Refreshes, Scope};
use crate::{
    db::{self, Database},
    home_rows::{
        HomeRows,
        tests::{Calls, SHARED, handle, insert_user, recorded, set_handle, started},
    },
};

const ALL: &[HomeRowMethod] = &[
    HomeRowMethod::Shared,
    HomeRowMethod::User,
    HomeRowMethod::Featured,
];

/// An adapter asking the Recorded source by every method, and keeping in memory, by scope, which
/// answer of its each was: 1 for its first, and so on.
#[derive(Clone)]
struct Probe {
    home: HomeRows,
    /// How long each answer stays fresh, `None` to keep none, as an adapter does on a failure.
    fresh_for: Option<Duration>,
    answers: Arc<AtomicU32>,
    stored: Arc<Mutex<HashMap<Scope, u32>>>,
}

impl Adapter for Probe {
    type Kept = u32;
    const NOUN: &'static str = "probes";

    fn methods(&self) -> &'static [HomeRowMethod] {
        ALL
    }

    async fn ask(&self, scope: &Scope) -> Option<(u32, Duration)> {
        let (plugin, user) = (&scope.plugin, scope.user.unwrap_or_default());
        let answered = match scope.method {
            HomeRowMethod::Shared => {
                let ask = |source: Arc<dyn HomeRowSource>| async move {
                    source.shared_rows(&SharedRowsAsked::default()).await
                };
                self.home.points.ask(plugin, ask).await.map(drop)
            }
            HomeRowMethod::User => {
                let ask = |source: Arc<dyn HomeRowSource>, settings: Vec<(String, String)>| async move {
                    let asked = UserRowsAsked {
                        personal_settings: settings.into_iter().collect(),
                        user_id: user,
                        ..UserRowsAsked::default()
                    };
                    source.user_rows(&asked).await
                };
                let answer = self.home.ask_for_user(plugin, user, Self::NOUN, ask);
                answer.await.map(drop)
            }
            HomeRowMethod::Featured => {
                let ask = |source: Arc<dyn HomeRowSource>, settings: Vec<(String, String)>| async move {
                    let asked = UserFeaturedAsked {
                        personal_settings: settings.into_iter().collect(),
                        user_id: user,
                        ..UserFeaturedAsked::default()
                    };
                    source.user_featured(&asked).await
                };
                let answer = self.home.ask_for_user(plugin, user, Self::NOUN, ask);
                answer.await.map(drop)
            }
        };
        answered?;
        let nth = self.answers.fetch_add(1, Ordering::SeqCst) + 1;
        Some((nth, self.fresh_for?))
    }

    fn store(
        &self,
        _conn: &mut SqliteConnection,
        scope: &Scope,
        kept: u32,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send {
        self.stored.lock().unwrap().insert(scope.clone(), kept);
        ready(Ok(()))
    }

    fn delete(
        &self,
        _conn: &mut SqliteConnection,
        plugin: &str,
        method: HomeRowMethod,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send {
        let mut stored = self.stored.lock().unwrap();
        stored.retain(|scope, _| (scope.plugin.as_str(), scope.method) != (plugin, method));
        ready(Ok(()))
    }

    fn plugins(
        &self,
        method: HomeRowMethod,
    ) -> impl Future<Output = Result<Vec<PluginId>, sqlx::Error>> + Send {
        let stored = self.stored.lock().unwrap();
        let scopes = stored.keys().filter(|scope| scope.method == method);
        ready(Ok(scopes.map(|scope| scope.plugin.clone()).collect()))
    }
}

impl Probe {
    /// The answer stored for the Recorded source's scope by `method` that `user` sees.
    fn answer(&self, method: HomeRowMethod, user: i64) -> Option<u32> {
        self.stored
            .lock()
            .unwrap()
            .get(&scope(method, user))
            .copied()
    }
}

/// The Recorded source's scope by `method` that `user` sees.
fn scope(method: HomeRowMethod, user: i64) -> Scope {
    Scope {
        plugin: PluginId::from_static("rows"),
        method,
        user: (method != HomeRowMethod::Shared).then_some(user),
    }
}

/// Refreshes over a fresh database, with the Recorded source giving answers by `gives`, and a
/// [`Probe`] of it whose answers are fresh for an hour.
async fn probed(
    gives: &'static [HomeRowMethod],
) -> (TempDir, Arc<Database>, Refreshes, Probe, Arc<Calls>) {
    let (dir, db, home, calls) = recorded(gives).await;
    let probe = Probe {
        home: home.clone(),
        fresh_for: Some(Duration::from_hours(1)),
        answers: Arc::default(),
        stored: Arc::default(),
    };
    (dir, db, home.refreshes, probe, calls)
}

/// Refreshes what is stale of `user`'s, and waits for the refreshes to finish.
async fn visit(refreshes: &Refreshes, probe: &Probe, user: i64) {
    refreshes.refresh_stale(probe, user).await.unwrap();
    refreshes.settled().await;
}

/// How often the Recorded source was asked for its answer by `method`.
fn times_asked(calls: &Calls, method: HomeRowMethod) -> usize {
    let asked = calls.asked();
    match method {
        HomeRowMethod::Shared => asked.iter().filter(|asked| asked.is_none()).count(),
        HomeRowMethod::User => asked.iter().filter(|asked| asked.is_some()).count(),
        HomeRowMethod::Featured => calls.featured_asked.lock().unwrap().len(),
    }
}

/// Waits until the Recorded source was asked `times` by each method.
async fn asked_by_each(calls: &Calls, times: usize) {
    let started = Instant::now();
    while !ALL
        .iter()
        .all(|&method| times_asked(calls, method) == times)
    {
        assert!(started.elapsed() < Duration::from_secs(10), "never asked");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn make_every_put_off_stale(db: &Database) {
    db.write(|tx| {
        Box::pin(async move {
            sqlx::query("UPDATE home_row_refreshes SET stale_at = 0")
                .execute(&mut **tx)
                .await?;
            sqlx::query("UPDATE featured_refreshes SET stale_at = 0")
                .execute(&mut **tx)
                .await
                .map(drop)
        })
    })
    .await
    .unwrap();
}

/// When each put-off goes stale, of every Plugin and User.
async fn stale_ats(db: &Database) -> Vec<i64> {
    sqlx::query_scalar(
        "SELECT stale_at FROM home_row_refreshes UNION ALL SELECT stale_at FROM featured_refreshes",
    )
    .fetch_all(db.reader())
    .await
    .unwrap()
}

/// How many put-offs by `method` are stored, every User's.
async fn put_offs(db: &Database, method: HomeRowMethod) -> i64 {
    let sql = match method {
        HomeRowMethod::Shared => "SELECT count(*) FROM home_row_refreshes WHERE user_id IS NULL",
        HomeRowMethod::User => "SELECT count(*) FROM home_row_refreshes WHERE user_id IS NOT NULL",
        HomeRowMethod::Featured => "SELECT count(*) FROM featured_refreshes",
    };
    sqlx::query_scalar(sql)
        .fetch_one(db.reader())
        .await
        .unwrap()
}

#[tokio::test]
async fn a_never_refreshed_scope_is_refreshed_in_the_background_without_waiting_on_the_plugin() {
    let (_dir, db, refreshes, probe, calls) = probed(ALL).await;
    let ada = insert_user(&db, "ada").await;
    let held = calls.held.lock().await;

    let refreshing = refreshes.refresh_stale(&probe, ada);
    tokio::time::timeout(Duration::from_secs(5), refreshing)
        .await
        .unwrap()
        .unwrap();

    for &method in ALL {
        assert_eq!(probe.answer(method, ada), None, "{method:?} isn't yet");
    }
    drop(held);
    refreshes.settled().await;
    for &method in ALL {
        assert!(probe.answer(method, ada).is_some(), "{method:?} is stored");
    }
}

#[tokio::test]
async fn a_fresh_scope_is_not_asked_again_until_it_goes_stale() {
    let (_dir, db, refreshes, probe, calls) = probed(ALL).await;
    let ada = insert_user(&db, "ada").await;

    visit(&refreshes, &probe, ada).await;
    visit(&refreshes, &probe, ada).await;
    for &method in ALL {
        assert_eq!(times_asked(&calls, method), 1, "{method:?} is fresh");
    }
    make_every_put_off_stale(&db).await;
    visit(&refreshes, &probe, ada).await;

    for &method in ALL {
        assert_eq!(times_asked(&calls, method), 2, "{method:?} went stale");
    }
}

#[tokio::test]
async fn a_failed_answer_is_asked_for_again_only_after_15_minutes() {
    let (_dir, db, refreshes, mut probe, calls) = probed(ALL).await;
    probe.fresh_for = None;
    let ada = insert_user(&db, "ada").await;
    let before = db::now();

    visit(&refreshes, &probe, ada).await;
    visit(&refreshes, &probe, ada).await;

    for &method in ALL {
        assert_eq!(times_asked(&calls, method), 1, "{method:?} is put off");
        assert_eq!(probe.answer(method, ada), None);
    }
    let stale_ats = stale_ats(&db).await;
    assert_eq!(stale_ats.len(), 3);
    let in_15_minutes = before + 900..=db::now() + 900;
    assert!(stale_ats.iter().all(|at| in_15_minutes.contains(at)));
}

#[tokio::test]
async fn freshness_is_clamped_to_between_15_minutes_and_a_week() {
    let (_dir, db, refreshes, mut probe, _) = probed(ALL).await;
    let ada = insert_user(&db, "ada").await;
    let fresh_between = |least: i64, most: i64| move |at: &i64| (least..=most).contains(at);

    probe.fresh_for = Some(Duration::from_mins(1));
    let before = db::now();
    visit(&refreshes, &probe, ada).await;
    let in_15_minutes = fresh_between(before + 900, db::now() + 900);
    assert!(stale_ats(&db).await.iter().all(in_15_minutes));

    make_every_put_off_stale(&db).await;
    probe.fresh_for = Some(Duration::from_hours(30 * 24));
    let restarted = db::now();
    visit(&refreshes, &probe, ada).await;
    let in_a_week = fresh_between(restarted + 604_800, db::now() + 604_800);
    let stale_ats = stale_ats(&db).await;
    assert_eq!(stale_ats.len(), 3);
    assert!(stale_ats.iter().all(in_a_week));
}

#[tokio::test]
async fn an_answer_asked_before_the_user_changed_their_personal_settings_is_dropped() {
    let (_dir, db, refreshes, probe, calls) = probed(ALL).await;
    let ada = insert_user(&db, "ada").await;
    set_handle(&db, ada, "ada").await;
    let held = calls.held.lock().await;
    refreshes.refresh_stale(&probe, ada).await.unwrap();
    asked_by_each(&calls, 1).await;

    set_handle(&db, ada, "ada-2").await;
    drop(held);
    refreshes.settled().await;

    assert!(probe.answer(HomeRowMethod::Shared, ada).is_some());
    assert_eq!(probe.answer(HomeRowMethod::User, ada), None, "dropped");
    assert_eq!(probe.answer(HomeRowMethod::Featured, ada), None, "dropped");
    visit(&refreshes, &probe, ada).await;
    assert_eq!(calls.asked().last(), Some(&Some(handle("ada-2"))));
    let featured_asked = calls.featured_asked.lock().unwrap().clone();
    assert_eq!(featured_asked.last(), Some(&handle("ada-2")));
    assert!(probe.answer(HomeRowMethod::User, ada).is_some());
    assert!(probe.answer(HomeRowMethod::Featured, ada).is_some());
}

#[tokio::test]
async fn the_old_answer_stays_stored_until_a_new_one_replaces_it() {
    let (_dir, db, refreshes, probe, calls) = probed(ALL).await;
    let ada = insert_user(&db, "ada").await;
    visit(&refreshes, &probe, ada).await;
    let answers = |method| probe.answer(method, ada).unwrap();
    let olds: Vec<u32> = ALL.iter().map(|&method| answers(method)).collect();
    make_every_put_off_stale(&db).await;
    let held = calls.held.lock().await;

    refreshes.refresh_stale(&probe, ada).await.unwrap();
    asked_by_each(&calls, 2).await;

    for (&method, &old) in ALL.iter().zip(&olds) {
        assert_eq!(probe.answer(method, ada), Some(old), "{method:?} is kept");
    }
    drop(held);
    refreshes.settled().await;
    for (&method, &old) in ALL.iter().zip(&olds) {
        assert!(
            probe.answer(method, ada) > Some(old),
            "{method:?} is replaced"
        );
    }
}

#[tokio::test]
async fn one_refresh_per_scope_runs_at_a_time() {
    let (_dir, db, refreshes, probe, calls) = probed(ALL).await;
    let ada = insert_user(&db, "ada").await;
    let held = calls.held.lock().await;

    for &method in ALL {
        refreshes.refresh(&probe, scope(method, ada)).await;
        refreshes.refresh(&probe, scope(method, ada)).await;
    }
    drop(held);
    refreshes.settled().await;

    for &method in ALL {
        assert_eq!(times_asked(&calls, method), 1, "{method:?} asked once");
    }
}

#[tokio::test]
async fn a_plugin_no_longer_giving_a_method_has_its_answers_by_it_deleted_for_every_user() {
    let others: [(HomeRowMethod, &'static [HomeRowMethod]); 3] = [
        (
            HomeRowMethod::Shared,
            &[HomeRowMethod::User, HomeRowMethod::Featured],
        ),
        (
            HomeRowMethod::User,
            &[HomeRowMethod::Shared, HomeRowMethod::Featured],
        ),
        (
            HomeRowMethod::Featured,
            &[HomeRowMethod::Shared, HomeRowMethod::User],
        ),
    ];
    for (gone, still_given) in others {
        let (_dir, db, old_version, probe, _) = probed(ALL).await;
        let ada = insert_user(&db, "ada").await;
        let bea = insert_user(&db, "bea").await;
        visit(&old_version, &probe, ada).await;
        visit(&old_version, &probe, bea).await;
        let (new_version, _) = started(&db, still_given);

        new_version.refreshes.forget(&probe).await.unwrap();

        for user in [ada, bea] {
            assert_eq!(probe.answer(gone, user), None, "{gone:?} is deleted");
            let kept = still_given
                .iter()
                .all(|&kept| probe.answer(kept, user).is_some());
            assert!(kept, "{still_given:?} are kept");
        }
        assert_eq!(put_offs(&db, gone).await, 0, "{gone:?}'s put-offs too");
        for &kept in still_given {
            assert!(put_offs(&db, kept).await > 0, "{kept:?}'s put-offs stay");
        }
    }
}

#[tokio::test]
async fn a_plugin_is_never_asked_for_a_method_it_doesnt_give() {
    let (_dir, db, refreshes, probe, calls) = probed(SHARED).await;
    let ada = insert_user(&db, "ada").await;

    visit(&refreshes, &probe, ada).await;
    refreshes
        .refresh(&probe, scope(HomeRowMethod::User, ada))
        .await;
    refreshes
        .refresh(&probe, scope(HomeRowMethod::Featured, ada))
        .await;
    refreshes.settled().await;

    assert_eq!(times_asked(&calls, HomeRowMethod::Shared), 1);
    assert_eq!(times_asked(&calls, HomeRowMethod::User), 0);
    assert_eq!(times_asked(&calls, HomeRowMethod::Featured), 0);
    assert_eq!(probe.answer(HomeRowMethod::User, ada), None);
}
