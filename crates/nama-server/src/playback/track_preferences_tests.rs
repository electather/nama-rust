use std::{
    future,
    sync::Arc,
    task::{Context, Waker},
    time::Duration,
};

use nama_plugin_api::{
    BoxFuture, Manifest, Observer, Plugin, PluginError, PluginId, PointKey, Registrar,
    metadata::{ExternalId, ExternalIds},
    points::{
        TrackDescription, TrackPreference, TrackPreferenceChange, TrackPreferenceChanged,
        TrackPreferenceFor,
    },
};
use tokio::sync::mpsc;

use super::{
    Description, Preferred, SubtitlePreference, TrackPreferences, is_language, same_language,
};
use crate::{
    db::{Database, now_ms},
    library::{
        Contents, Decided, Metadata,
        fixture::{MatchedMovie, tmdb_movie},
    },
    plugins::{ExtensionPoints, Registry},
    users::{Permissions, User, Users},
};

/// Track Preferences on a database in a temp folder, and a User to keep them for.
async fn track_preferences(data: &tempfile::TempDir) -> (TrackPreferences, i64) {
    track_preferences_telling(data, ExtensionPoints::default()).await
}

/// [`track_preferences`], telling the Observers of `points`.
async fn track_preferences_telling(
    data: &tempfile::TempDir,
    points: ExtensionPoints,
) -> (TrackPreferences, i64) {
    let db = Arc::new(Database::open(data.path()).await.unwrap());
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
    let user = Users::new(Arc::clone(&db), plugins.extension_points())
        .create(&root, "Ada", "correct horse battery staple")
        .await
        .unwrap();
    (TrackPreferences::new(db, points), user)
}

/// A Plugin that hands each Track Preference change it's told of to the test.
#[derive(Clone)]
struct Hears(mpsc::UnboundedSender<TrackPreferenceChange>);

impl Plugin for Hears {
    fn manifest(&self) -> Manifest {
        Manifest {
            extension_points: vec![PointKey::observe::<TrackPreferenceChanged>()],
            ..Manifest::new(PluginId::from_static("hears"))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.observe::<TrackPreferenceChanged>(self.clone());
        Ok(())
    }
}

impl Observer<TrackPreferenceChanged> for Hears {
    fn observe<'a>(
        &'a self,
        event: &'a TrackPreferenceChange,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        let sent = self.0.send(event.clone());
        Box::pin(future::ready(
            sent.map_err(|_unsent| "the test stopped listening".into()),
        ))
    }
}

/// Each `track_preferences` row, as `(scope, title_id, kind, off, language, forced, sdh,
/// title)`.
async fn rows(
    preferences: &TrackPreferences,
) -> Vec<(
    String,
    String,
    String,
    i64,
    Option<String>,
    i64,
    i64,
    Option<String>,
)> {
    sqlx::query_as(
        "SELECT scope, title_id, kind, off, language, forced, sdh, title
             FROM track_preferences",
    )
    .fetch_all(preferences.db.reader())
    .await
    .unwrap()
}

#[tokio::test]
async fn a_users_own_audio_language_is_kept_as_their_untitled_audio_row_until_the_files_default() {
    let data = tempfile::tempdir().unwrap();
    let (preferences, user) = track_preferences(&data).await;

    preferences
        .keep_users_audio(user, Some("de".to_owned()))
        .await
        .unwrap();
    let kept = rows(&preferences).await;
    let read = preferences.users_audio(user).await.unwrap();
    preferences.keep_users_audio(user, None).await.unwrap();

    let german = (
        "user".to_owned(),
        String::new(),
        "audio".to_owned(),
        0,
        Some("de".to_owned()),
        0,
        0,
        None,
    );
    assert_eq!(kept, [german]);
    assert_eq!(read.as_deref(), Some("de"));
    assert_eq!(rows(&preferences).await, []);
    assert_eq!(preferences.users_audio(user).await.unwrap(), None);
}

#[tokio::test]
async fn the_users_own_audio_preference_reaches_a_version_of_an_unmatched_item() {
    let (data, media) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (preferences, user) = track_preferences(&data).await;
    let alien = MatchedMovie::new(&preferences.db, media.path(), tmdb_movie(348, "Alien")).await;
    Contents::new(Arc::clone(&preferences.db))
        .record(vec![(alien.item, Decided::Unmatched(None))])
        .await
        .unwrap();

    preferences
        .keep_users_audio(user, Some("de".to_owned()))
        .await
        .unwrap();

    assert_eq!(
        preferences.audio(user, alien.version).await.unwrap(),
        Some(Preferred {
            preference: Description {
                language: Some("de".to_owned()),
                ..Description::default()
            },
            users_own: true,
            foreign_audio_only: false,
        })
    );
}

/// Full English subtitles.
fn english() -> SubtitlePreference {
    SubtitlePreference::Like(Description {
        language: Some("en".to_owned()),
        ..Description::default()
    })
}

#[tokio::test]
async fn a_movies_subtitle_preference_wants_its_subtitles_over_any_audio_whatever_the_users_own() {
    let (data, media) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (preferences, user) = track_preferences(&data).await;
    let alien = MatchedMovie::new(&preferences.db, media.path(), tmdb_movie(348, "Alien")).await;
    let movie = preferences.kept_for(alien.version).await.unwrap().unwrap();

    preferences
        .keep_users_subtitle(user, english(), true)
        .await
        .unwrap();
    preferences
        .keep_subtitle(user, movie, english())
        .await
        .unwrap();

    assert_eq!(
        preferences.subtitle(user, alien.version).await.unwrap(),
        Some(Preferred {
            preference: english(),
            users_own: false,
            foreign_audio_only: false,
        })
    );
}

#[tokio::test]
async fn the_users_own_subtitles_only_over_foreign_audio_reach_a_version_of_an_unmatched_item() {
    let (data, media) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (preferences, user) = track_preferences(&data).await;
    let alien = MatchedMovie::new(&preferences.db, media.path(), tmdb_movie(348, "Alien")).await;
    Contents::new(Arc::clone(&preferences.db))
        .record(vec![(alien.item, Decided::Unmatched(None))])
        .await
        .unwrap();

    preferences
        .keep_users_subtitle(user, english(), true)
        .await
        .unwrap();

    assert_eq!(
        preferences.subtitle(user, alien.version).await.unwrap(),
        Some(Preferred {
            preference: english(),
            users_own: true,
            foreign_audio_only: true,
        })
    );
}

/// Keeps English subtitles as the User `user`'s own, as the Server did before the switch
/// "Only when the audio is in another language" existed.
async fn keep_english_subtitles_as_before_the_switch(preferences: &TrackPreferences, user: i64) {
    (preferences.db)
        .write(move |tx| {
            Box::pin(async move {
                let insert = sqlx::query(
                    "INSERT INTO track_preferences
                             (user_id, scope, title_id, kind, off, language, forced, sdh)
                         VALUES (?, 'user', '', 'subtitle', 0, 'en', 0, 0)",
                );
                insert.bind(user).execute(&mut **tx).await
            })
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn a_users_own_subtitles_kept_before_the_condition_start_over_any_audio_until_it_is_set() {
    let data = tempfile::tempdir().unwrap();
    let (preferences, user) = track_preferences(&data).await;
    keep_english_subtitles_as_before_the_switch(&preferences, user).await;
    let foreign_audio_only = async || {
        let preferred = preferences.users_subtitle(user).await.unwrap().unwrap();
        preferred.foreign_audio_only
    };

    let before = foreign_audio_only().await;
    preferences
        .keep_users_subtitle(user, english(), true)
        .await
        .unwrap();

    assert!(!before);
    assert!(foreign_audio_only().await);
}

/// Full English subtitles for the Movie `tmdb-348`, as another Server changed them at
/// `changed_at_ms`.
fn pulled_english_for_alien(changed_at_ms: i64) -> TrackPreference {
    TrackPreference {
        kept_for: TrackPreferenceFor::Movie(vec!["tmdb-348".to_owned()]),
        like: Some(TrackDescription {
            language: Some("en".to_owned()),
            ..TrackDescription::default()
        }),
        changed_at_ms,
        ..TrackPreference::default()
    }
}

#[tokio::test]
async fn a_pulled_movies_subtitle_preference_reaches_its_versions_as_a_description() {
    let (data, media) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (preferences, user) = track_preferences(&data).await;
    let alien = MatchedMovie::new(&preferences.db, media.path(), tmdb_movie(348, "Alien")).await;

    let pulled = pulled_english_for_alien(now_ms());
    preferences.take_pulled(user, vec![pulled]).await.unwrap();

    assert_eq!(
        preferences.subtitle(user, alien.version).await.unwrap(),
        Some(Preferred {
            preference: english(),
            users_own: false,
            foreign_audio_only: false,
        })
    );
}

#[tokio::test]
async fn a_preference_pulled_under_any_of_a_movies_ids_reaches_its_versions() {
    let (data, media) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (preferences, user) = track_preferences(&data).await;
    let imdb = ExternalId::new("imdb", "tt0078748").unwrap();
    let known_by_both = Metadata {
        ids: ExternalIds::from_iter([ExternalId::tmdb(348), imdb.clone()]),
        ..tmdb_movie(348, "Alien")
    };
    let alien = MatchedMovie::new(&preferences.db, media.path(), known_by_both).await;

    let pulled = TrackPreference {
        kept_for: TrackPreferenceFor::Movie(vec![imdb.to_string()]),
        ..pulled_english_for_alien(now_ms())
    };
    preferences.take_pulled(user, vec![pulled]).await.unwrap();

    let preferred = preferences.subtitle(user, alien.version).await.unwrap();
    assert_eq!(
        preferred.map(|preferred| preferred.preference),
        Some(english())
    );
}

#[tokio::test]
async fn a_keep_for_a_movie_tells_every_id_it_is_known_by() {
    let (data, media) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (preferences, user) = track_preferences(&data).await;
    let known_by_both = Metadata {
        ids: ExternalIds::from_iter([
            ExternalId::tmdb(348),
            ExternalId::new("imdb", "tt0078748").unwrap(),
        ]),
        ..tmdb_movie(348, "Alien")
    };
    let alien = MatchedMovie::new(&preferences.db, media.path(), known_by_both).await;
    let movie = preferences.kept_for(alien.version).await.unwrap().unwrap();

    let kept = preferences
        .keep_subtitle(user, movie, english())
        .await
        .unwrap();

    assert_eq!(
        kept.kept_for,
        TrackPreferenceFor::Movie(vec!["imdb-tt0078748".to_owned(), "tmdb-348".to_owned()])
    );
}

#[tokio::test]
async fn a_pulled_preference_is_taken_only_where_newer_than_the_servers_own() {
    let (data, media) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (preferences, user) = track_preferences(&data).await;
    let alien = MatchedMovie::new(&preferences.db, media.path(), tmdb_movie(348, "Alien")).await;
    let movie = preferences.kept_for(alien.version).await.unwrap().unwrap();
    let off = preferences
        .keep_subtitle(user, movie, SubtitlePreference::Off)
        .await
        .unwrap();
    let preference = async || preferences.subtitle(user, alien.version).await.unwrap();

    let older = pulled_english_for_alien(off.changed_at_ms - 1);
    preferences.take_pulled(user, vec![older]).await.unwrap();
    let after_older = preference().await;
    let newer = pulled_english_for_alien(off.changed_at_ms + 1);
    preferences.take_pulled(user, vec![newer]).await.unwrap();

    let preferred = |wanted| {
        Some(Preferred {
            preference: wanted,
            users_own: false,
            foreign_audio_only: false,
        })
    };
    assert_eq!(after_older, preferred(SubtitlePreference::Off));
    assert_eq!(preference().await, preferred(english()));
}

#[tokio::test]
async fn a_users_own_audio_left_to_the_files_on_another_server_forgets_it_here() {
    let data = tempfile::tempdir().unwrap();
    let (preferences, user) = track_preferences(&data).await;
    let german = preferences
        .keep_users_audio(user, Some("de".to_owned()))
        .await
        .unwrap();

    let files_own = TrackPreference {
        audio: true,
        changed_at_ms: german.changed_at_ms + 1,
        ..TrackPreference::default()
    };
    preferences
        .take_pulled(user, vec![files_own])
        .await
        .unwrap();

    assert_eq!(preferences.users_audio(user).await.unwrap(), None);
}

#[tokio::test]
async fn a_users_preferences_read_as_the_changes_that_kept_them_once_per_id_dated_as_stored() {
    let (data, media) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (preferences, user) = track_preferences(&data).await;
    let known_by_both = Metadata {
        ids: ExternalIds::from_iter([
            ExternalId::tmdb(348),
            ExternalId::new("imdb", "tt0078748").unwrap(),
        ]),
        ..tmdb_movie(348, "Alien")
    };
    let alien = MatchedMovie::new(&preferences.db, media.path(), known_by_both).await;
    let movie = preferences.kept_for(alien.version).await.unwrap().unwrap();
    let japanese = Description {
        language: Some("ja".to_owned()),
        ..Description::default()
    };

    let own = preferences
        .keep_users_subtitle(user, english(), true)
        .await
        .unwrap();
    let movies = preferences.keep_audio(user, movie, japanese).await.unwrap();
    let pulled = pulled_english_for_alien(1_000);
    (preferences.take_pulled(user, vec![pulled.clone()]))
        .await
        .unwrap();

    let read = preferences.changes_of_user(user).await.unwrap();

    let under = |id: &str| TrackPreference {
        kept_for: TrackPreferenceFor::Movie(vec![id.to_owned()]),
        ..movies.clone()
    };
    let expected = [own, under("imdb-tt0078748"), under("tmdb-348"), pulled];
    assert_eq!(read.len(), expected.len(), "{read:?}");
    for change in expected {
        assert!(read.contains(&change), "{change:?} not in {read:?}");
    }
}

#[tokio::test]
async fn each_keep_tells_what_it_kept_and_for_what() {
    let (data, media) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (preferences, user) = track_preferences(&data).await;
    let alien = MatchedMovie::new(&preferences.db, media.path(), tmdb_movie(348, "Alien")).await;
    let movie = preferences.kept_for(alien.version).await.unwrap().unwrap();

    let own = preferences
        .keep_users_subtitle(user, english(), true)
        .await
        .unwrap();
    let movies = preferences
        .keep_subtitle(user, movie, SubtitlePreference::Off)
        .await
        .unwrap();
    let files_audio = preferences.keep_users_audio(user, None).await.unwrap();

    let english = TrackDescription {
        language: Some("en".to_owned()),
        ..TrackDescription::default()
    };
    assert_eq!(
        (own.kept_for, own.like, own.foreign_audio_only),
        (TrackPreferenceFor::User, Some(english), true)
    );
    assert_eq!(
        (movies.kept_for, movies.like, movies.audio),
        (
            TrackPreferenceFor::Movie(vec!["tmdb-348".to_owned()]),
            None,
            false
        )
    );
    assert_eq!(
        (files_audio.kept_for, files_audio.like, files_audio.audio),
        (TrackPreferenceFor::User, None, true)
    );
}

#[test]
fn a_language_is_named_by_any_iso_639_code_of_one_language() {
    for code in ["en", "eng", "ger", "deu", "DE", "tlh"] {
        assert!(is_language(code), "{code}");
    }
    for code in ["klingon", "und", "mul", "zxx", "mis", "pt-BR", ""] {
        assert!(!is_language(code), "{code}");
    }
}

#[test]
fn languages_match_whichever_iso_639_code_names_them() {
    for (a, b) in [
        ("eng", "en"),
        ("ger", "deu"),
        ("ger", "de"),
        ("fre", "fra"),
        ("chi", "zh"),
        ("per", "fa"),
        ("ENG", "en"),
        ("pt-BR", "pt-br"),
    ] {
        assert!(same_language(Some(a), Some(b)), "{a} is {b}");
    }
}

#[test]
fn different_languages_and_a_missing_one_dont_match() {
    assert!(!same_language(Some("eng"), Some("ger")));
    assert!(!same_language(Some("pt-BR"), Some("pt")));
    assert!(!same_language(Some("eng"), None));
    assert!(!same_language(None, Some("eng")));
    assert!(same_language(None, None));
    assert!(same_language(Some("und"), None));
    assert!(!same_language(Some("und"), Some("eng")));
}

#[tokio::test]
async fn a_preference_whose_caller_stops_waiting_is_still_kept_and_told() {
    let data = tempfile::tempdir().unwrap();
    let (heard, mut hearing) = mpsc::unbounded_channel();
    let plugins = Registry::new();
    plugins.enable(Arc::new(Hears(heard))).unwrap();
    let (preferences, user) = track_preferences_telling(&data, plugins.extension_points()).await;

    // Polled once, which queues the write, then dropped, as when a client hangs up (ADR 0029).
    let mut keeping = Box::pin(preferences.keep_users_audio(user, Some("de".to_owned())));
    let polled = keeping
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()));
    assert!(polled.is_pending(), "the keep finished at once");
    drop(keeping);
    let told = tokio::time::timeout(Duration::from_secs(5), hearing.recv()).await;

    let told = told.expect("no Observer was told").unwrap();
    let language = told.preference.like.and_then(|like| like.language);
    assert_eq!((told.user_id, language.as_deref()), (user, Some("de")));
    let kept = preferences.users_audio(user).await.unwrap();
    assert_eq!(kept.as_deref(), Some("de"));
}
