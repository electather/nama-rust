use std::{collections::BTreeMap, future, sync::Arc, time::Duration};

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    contract::{
        ImportAsked, Imported, LinkPolled, LinkStarted, PollLinkAsked, ScrobbleAsked, Scrobbled,
        StartLinkAsked, SyncedWatchState, TitleId, TitleRefKind,
    },
    metadata::{ExternalId, ExternalIds},
    watch_sync::WatchSync,
};
use tempfile::TempDir;
use tokio::sync::mpsc;

use super::WatchSyncImports;
use crate::{
    db::{Database, now_ms},
    library::{
        MovieOrEpisode, Progress, WatchState, WatchStateKind, WatchStates,
        fixture::{MatchedMovie, matched_episodes_file, tmdb_movie},
        import_cursor,
    },
    log_capture::CapturedLog,
    plugins::Registry,
    users::{personal_settings_of, replace_personal_settings},
};

const TRACKER: PluginId = PluginId::from_static("tracker");
const ALIEN_TMDB_ID: i64 = 348;

/// A Watch sync Plugin that hands the test each scrobble it's told, answering it with `renewed`
/// or failing it if `down`, and answers each import with `imported`.
#[derive(Clone)]
struct Tracker {
    scrobbled: mpsc::UnboundedSender<ScrobbleAsked>,
    renewed: Scrobbled,
    imported: Imported,
    down: bool,
}

impl Plugin for Tracker {
    fn manifest(&self) -> Manifest {
        Manifest {
            extension_points: vec![PointKey::provide::<dyn WatchSync>()],
            ..Manifest::new(TRACKER)
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.provide::<dyn WatchSync>(Priority::DEFAULT, Arc::new(self.clone()));
        Ok(())
    }
}

impl WatchSync for Tracker {
    fn scrobble<'a>(
        &'a self,
        asked: &'a ScrobbleAsked,
    ) -> BoxFuture<'a, Result<Scrobbled, PluginError>> {
        let _heard = self.scrobbled.send(asked.clone());
        let answer = if self.down {
            Err("the tracker is down".into())
        } else {
            Ok(self.renewed.clone())
        };
        Box::pin(future::ready(answer))
    }

    fn import<'a>(
        &'a self,
        _asked: &'a ImportAsked,
    ) -> BoxFuture<'a, Result<Imported, PluginError>> {
        Box::pin(future::ready(Ok(self.imported.clone())))
    }

    fn start_link<'a>(
        &'a self,
        _asked: &'a StartLinkAsked,
    ) -> BoxFuture<'a, Result<LinkStarted, PluginError>> {
        Box::pin(future::ready(Err("these tests link no one".into())))
    }

    fn poll_link<'a>(
        &'a self,
        _asked: &'a PollLinkAsked,
    ) -> BoxFuture<'a, Result<LinkPolled, PluginError>> {
        Box::pin(future::ready(Err("these tests link no one".into())))
    }
}

/// A database with `tracker` enabled, and Ada, who linked it, and Bob, who didn't.
struct Linked {
    db: Arc<Database>,
    plugins: Registry,
    watch_states: WatchStates,
    ada: i64,
    bob: i64,
    _data: TempDir,
}

impl Linked {
    async fn with(tracker: Tracker) -> Self {
        let data = tempfile::tempdir().unwrap();
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let plugins = Registry::new();
        plugins.enable(Arc::new(tracker)).unwrap();
        let (ada, bob) = (insert_user(&db, "ada").await, insert_user(&db, "bob").await);
        let token = vec![("token".to_owned(), "ada-token".to_owned(), true)];
        (replace_personal_settings(&db, ada, &TRACKER, token).await).unwrap();
        Self {
            watch_states: WatchStates::new(Arc::clone(&db), plugins.extension_points()),
            db,
            plugins,
            ada,
            bob,
            _data: data,
        }
    }

    async fn mark_played(&self, user: i64, title: MovieOrEpisode) {
        (self.watch_states.mark(user, vec![title], true))
            .await
            .unwrap();
    }

    /// Gives Ada a setting beside her token that no renewal may touch.
    async fn add_adas_username(&self) {
        let settings = vec![
            ("token".to_owned(), "ada-token".to_owned(), true),
            ("username".to_owned(), "ada".to_owned(), false),
        ];
        (replace_personal_settings(&self.db, self.ada, &TRACKER, settings).await).unwrap();
    }

    async fn adas_settings(&self) -> BTreeMap<String, (String, bool)> {
        (personal_settings_of(&self.db, self.ada, &TRACKER).await).unwrap()
    }

    async fn alien_state(&self, user: i64) -> Option<WatchState> {
        let alien = (WatchStateKind::Movie, ExternalIds::from_iter([alien_id()]));
        let states = (self.watch_states.of_titles(Some(user), &[alien]))
            .await
            .unwrap();
        states[0].first().map(|&(_, state)| state)
    }
}

/// A Tracker that answers each import with `imported`, and the scrobbles it hears.
fn tracker(imported: Imported) -> (Tracker, mpsc::UnboundedReceiver<ScrobbleAsked>) {
    let (scrobbled, heard) = mpsc::unbounded_channel();
    let tracker = Tracker {
        scrobbled,
        renewed: Scrobbled::default(),
        imported,
        down: false,
    };
    (tracker, heard)
}

async fn insert_user(db: &Database, name: &'static str) -> i64 {
    db.write(move |tx| {
        Box::pin(async move {
            sqlx::query_scalar(
                "INSERT INTO users (username, display_username, permissions, created_at,
                                    updated_at)
                 VALUES (?, ?, 0, 0, 0) RETURNING id",
            )
            .bind(name)
            .bind(name)
            .fetch_one(&mut **tx)
            .await
        })
    })
    .await
    .unwrap()
}

fn alien_id() -> ExternalId {
    ExternalId::tmdb(ALIEN_TMDB_ID)
}

/// Alien as the tracker knows it, played or not as of `changed_at_ms`.
fn alien_on_tracker(played: bool, changed_at_ms: i64) -> SyncedWatchState {
    SyncedWatchState {
        kind: TitleRefKind::TITLE_REF_KIND_MOVIE.into(),
        ids: vec![TitleId {
            namespace: "tmdb".to_owned(),
            value: ALIEN_TMDB_ID.to_string(),
            ..TitleId::default()
        }],
        played,
        changed_at_ms,
        ..SyncedWatchState::default()
    }
}

async fn next(heard: &mut mpsc::UnboundedReceiver<ScrobbleAsked>) -> ScrobbleAsked {
    let next = tokio::time::timeout(Duration::from_secs(5), heard.recv()).await;
    next.expect("nothing was scrobbled").unwrap()
}

/// Whether anything more is scrobbled within a moment, which is long enough for a task already
/// spawned to send it.
async fn quiet(heard: &mut mpsc::UnboundedReceiver<ScrobbleAsked>) -> bool {
    tokio::time::timeout(Duration::from_millis(200), heard.recv())
        .await
        .is_err()
}

#[tokio::test]
async fn a_movie_and_an_episode_marked_played_are_scrobbled_with_the_users_personal_settings() {
    let (tracker, mut heard) = tracker(Imported::default());
    let linked = Linked::with(tracker).await;

    linked
        .mark_played(linked.ada, MovieOrEpisode::movie(&alien_id()))
        .await;
    let movie = next(&mut heard).await;
    let episode_id = ExternalId::tmdb(62_085);
    (linked.mark_played(linked.ada, MovieOrEpisode::episode(&episode_id))).await;
    let episode = next(&mut heard).await;

    for (asked, kind) in [
        (&movie, TitleRefKind::TITLE_REF_KIND_MOVIE),
        (&episode, TitleRefKind::TITLE_REF_KIND_EPISODE),
    ] {
        assert_eq!(asked.user_id, linked.ada);
        assert_eq!(asked.personal_settings["token"], "ada-token");
        assert_eq!(asked.personal_settings.len(), 1);
        let change = &asked.changes[0];
        assert_eq!((change.kind, change.played), (kind.into(), true));
        let numbers = (
            &change.show_ids,
            change.season_number,
            change.episode_number,
        );
        assert_eq!(numbers, (&vec![], None, None), "no Library has it");
    }
    assert_eq!(movie.changes[0].ids[0].value, ALIEN_TMDB_ID.to_string());
}

#[tokio::test]
async fn an_episode_in_a_library_is_scrobbled_with_its_shows_ids_season_and_number() {
    let (tracker, mut heard) = tracker(Imported::default());
    let linked = Linked::with(tracker).await;
    let media = tempfile::tempdir().unwrap();
    let show_ids = ExternalIds::from_iter([ExternalId::tmdb(70_523), ExternalId::tvdb(121_361)]);
    let second = ExternalId::tmdb(1_350_210);
    matched_episodes_file(&linked.db, media.path(), show_ids.clone(), [1, 1_350_210]).await;

    (linked.mark_played(linked.ada, MovieOrEpisode::episode(&second))).await;
    let asked = next(&mut heard).await;

    let change = &asked.changes[0];
    let told_show: ExternalIds = (change.show_ids.iter())
        .map(|id| ExternalId::new(&id.namespace, id.value.clone()).unwrap())
        .collect();
    assert_eq!(told_show, show_ids);
    assert_eq!(
        (change.season_number, change.episode_number),
        (Some(1), Some(2))
    );
}

#[tokio::test]
async fn a_movie_sharing_a_library_episodes_id_is_scrobbled_without_numbers() {
    let (tracker, mut heard) = tracker(Imported::default());
    let linked = Linked::with(tracker).await;
    let media = tempfile::tempdir().unwrap();
    let show_ids = ExternalIds::from_iter([ExternalId::tmdb(70_523)]);
    matched_episodes_file(
        &linked.db,
        media.path(),
        show_ids,
        [ALIEN_TMDB_ID, 1_350_210],
    )
    .await;

    (linked.mark_played(linked.ada, MovieOrEpisode::movie(&alien_id()))).await;
    let asked = next(&mut heard).await;

    let change = &asked.changes[0];
    let numbers = (
        &change.show_ids,
        change.season_number,
        change.episode_number,
    );
    assert_eq!(numbers, (&vec![], None, None));
}

#[tokio::test]
async fn a_user_without_personal_settings_is_not_scrobbled() {
    let (tracker, mut heard) = tracker(Imported::default());
    let linked = Linked::with(tracker).await;

    linked
        .mark_played(linked.bob, MovieOrEpisode::movie(&alien_id()))
        .await;

    assert!(quiet(&mut heard).await, "Bob's change was scrobbled");
}

#[tokio::test]
async fn a_failing_scrobble_is_logged_and_counted_and_the_mark_still_stands() {
    let (tracker, mut heard) = tracker(Imported::default());
    let linked = Linked::with(Tracker {
        down: true,
        ..tracker
    })
    .await;
    let log = CapturedLog::start();

    linked
        .mark_played(linked.ada, MovieOrEpisode::movie(&alien_id()))
        .await;
    next(&mut heard).await;
    let points = linked.plugins.extension_points();
    let counted = || points.failing(&TRACKER, PointKey::provide::<dyn WatchSync>());
    let waiting = async {
        while counted().is_none() {
            tokio::task::yield_now().await;
        }
    };
    (tokio::time::timeout(Duration::from_secs(5), waiting).await).expect("never counted");
    let failing = counted().unwrap();

    assert_eq!(
        (failing.last_error.as_str(), failing.in_a_row),
        ("the tracker is down", 1)
    );
    assert!(
        log.finish()
            .contains("scrobbling a User's Watch State failed")
    );
    let stored = linked.alien_state(linked.ada).await;
    assert!(stored.is_some_and(|state| state.played));
}

#[tokio::test]
async fn an_import_takes_what_is_newer_ignores_what_is_older_and_scrobbles_nothing_back() {
    let folder = tempfile::tempdir().unwrap();
    let older_than_any = alien_on_tracker(false, 1_000);
    let (tracker, mut heard) = tracker(Imported {
        changes: vec![older_than_any],
        cursor: "page-2".to_owned(),
        ..Imported::default()
    });
    let linked = Linked::with(tracker.clone()).await;
    MatchedMovie::new(
        &linked.db,
        folder.path(),
        tmdb_movie(ALIEN_TMDB_ID, "Alien"),
    )
    .await;
    let started = Progress::new(100, 2_000).unwrap();
    (linked.watch_states)
        .report(
            linked.ada,
            vec![Some(MovieOrEpisode::movie(&alien_id()))],
            started,
        )
        .await
        .unwrap();
    next(&mut heard).await;
    let imports = WatchSyncImports::new(Arc::clone(&linked.db), linked.plugins.extension_points());

    imports.clone().import_each_user(TRACKER).await;
    let after_older = linked.alien_state(linked.ada).await.unwrap();
    assert!(linked.plugins.disable(&TRACKER).await);
    let played_later = alien_on_tracker(true, now_ms() + 60_000);
    let newer = Tracker {
        imported: Imported {
            changes: vec![played_later],
            cursor: "page-3".to_owned(),
            ..Imported::default()
        },
        ..tracker
    };
    linked.plugins.enable(Arc::new(newer)).unwrap();
    imports.import_each_user(TRACKER).await;
    let after_newer = linked.alien_state(linked.ada).await.unwrap();

    assert_eq!((after_older.played, after_older.position_ms), (false, 100));
    assert!(after_newer.played);
    let cursor = import_cursor(&linked.db, linked.ada, &TRACKER).await;
    assert_eq!(cursor.unwrap(), "page-3");
    assert!(
        quiet(&mut heard).await,
        "an imported change was scrobbled back"
    );
}

/// What the tracker answers having renewed Ada's token: her token, and a username that isn't a
/// secret and a refresh token she never set, which are dropped.
fn renewed_tokens<M: FromIterator<(String, String)>>() -> M {
    [
        ("token", "renewed-token"),
        ("username", "mallory"),
        ("refresh", "refresh-2"),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_owned(), value.to_owned()))
    .collect()
}

/// Ada's settings once only her token was renewed.
fn adas_renewed_settings() -> BTreeMap<String, (String, bool)> {
    BTreeMap::from([
        ("token".to_owned(), ("renewed-token".to_owned(), true)),
        ("username".to_owned(), ("ada".to_owned(), false)),
    ])
}

#[tokio::test]
async fn a_scrobble_keeps_the_token_it_renewed_and_leaves_the_users_other_settings() {
    let (tracker, mut heard) = tracker(Imported::default());
    let renewed = Scrobbled {
        personal_settings: renewed_tokens(),
        ..Scrobbled::default()
    };
    let linked = Linked::with(Tracker { renewed, ..tracker }).await;
    linked.add_adas_username().await;

    (linked.mark_played(linked.ada, MovieOrEpisode::movie(&alien_id()))).await;
    next(&mut heard).await;
    let renewing = async {
        while linked.adas_settings().await["token"].0 == "ada-token" {
            tokio::task::yield_now().await;
        }
    };
    (tokio::time::timeout(Duration::from_secs(5), renewing).await).expect("never renewed");

    assert_eq!(linked.adas_settings().await, adas_renewed_settings());
}

#[tokio::test]
async fn an_import_keeps_the_token_it_renewed_even_when_taking_its_changes_fails() {
    let (tracker, _heard) = tracker(Imported {
        changes: vec![alien_on_tracker(true, now_ms())],
        cursor: "page-2".to_owned(),
        personal_settings: renewed_tokens(),
        ..Imported::default()
    });
    let linked = Linked::with(tracker).await;
    linked.add_adas_username().await;
    // Taking the changes reads every Library first, so this makes it fail.
    let drop_libraries = sqlx::query("DROP TABLE libraries");
    (linked.db)
        .write(|tx| Box::pin(drop_libraries.execute(&mut **tx)))
        .await
        .unwrap();
    let imports = WatchSyncImports::new(Arc::clone(&linked.db), linked.plugins.extension_points());

    imports.import_each_user(TRACKER).await;

    let cursor = import_cursor(&linked.db, linked.ada, &TRACKER).await;
    assert_eq!(cursor.unwrap(), "", "taking the changes didn't fail");
    assert_eq!(linked.adas_settings().await, adas_renewed_settings());
}
