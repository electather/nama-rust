use std::sync::Arc;

use super::{
    Description, Preferred, SubtitlePreference, TrackPreferences, is_language, same_language,
};
use crate::{
    db::Database,
    library::contents::{
        Contents, Decided,
        fixture::{MatchedMovie, tmdb_movie},
    },
    plugins::Registry,
    users::{Permissions, User, Users},
};

/// Track Preferences on a database in a temp folder, and a User to keep them for.
async fn track_preferences(data: &tempfile::TempDir) -> (TrackPreferences, i64) {
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
    (TrackPreferences::new(db), user)
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

// An Unmatched Item's Version can't be planned yet (`Contents::version_file`), so this is
// where the User's own is seen to reach one.
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
