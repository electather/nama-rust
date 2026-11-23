//! Linked Servers pushing and pulling their Users' Subtitle and Audio Preferences, against a real
//! PostgreSQL.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/hub.rs"]
mod hub;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/linking.rs"]
mod linking;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/mail.rs"]
mod mail;
#[path = "support/refused.rs"]
mod refused;

use connectrpc::{ConnectError, ErrorCode};
use hub::TestHub;
use linking::*;
use nama_hub_proto::nama::hub::v1::{
    PullTrackPreferencesRequest, PushTrackPreferencesRequest, RefusalReason, TitleKind,
    TrackPreferenceChange,
};
use refused::assert_refused;
use sqlx::PgPool;

/// The User `user_id`'s Subtitle Preference for Alien, subtitles in `language`, changed at
/// `changed_at_ms`.
fn alien_in(user_id: i64, language: &str, changed_at_ms: i64) -> TrackPreferenceChange {
    TrackPreferenceChange {
        user_id,
        title_kind: TitleKind::TITLE_KIND_MOVIE.into(),
        title_id: "tmdb-348".to_owned(),
        language: language.to_owned(),
        changed_at_ms,
        ..Default::default()
    }
}

/// Pushes `changes` from the Server presenting `credential`.
async fn push(
    hub: &TestHub,
    credential: &str,
    changes: Vec<TrackPreferenceChange>,
) -> Result<(), ConnectError> {
    linked_server(hub, credential)
        .push_track_preferences(PushTrackPreferencesRequest {
            changes,
            ..Default::default()
        })
        .await?;
    Ok(())
}

/// The languages of the Preferences the Server presenting `credential` pulls for its User
/// `user_id`, each with the User it names.
async fn pulled(
    hub: &TestHub,
    credential: &str,
    user_id: i64,
) -> Result<Vec<(i64, String)>, ConnectError> {
    let pulled = linked_server(hub, credential)
        .pull_track_preferences(PullTrackPreferencesRequest {
            user_id,
            ..Default::default()
        })
        .await?
        .into_owned();
    Ok((pulled.preferences.into_iter())
        .map(|preference| (preference.user_id, preference.language))
        .collect())
}

#[sqlx::test(migrations = false)]
async fn a_server_pulls_the_preference_another_server_of_the_account_changed_last(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let (first, second) = (
        linked(&hub, &ada, "Living room").await,
        linked(&hub, &ada, "Cabin").await,
    );
    let first_code = start_user(&hub, &first, 7).await.unwrap();
    approve_user(&hub, &ada, &first_code).await.unwrap();
    let second_code = start_user(&hub, &second, 3).await.unwrap();
    approve_user(&hub, &ada, &second_code).await.unwrap();

    push(&hub, &first, vec![alien_in(7, "eng", 2_000)])
        .await
        .unwrap();
    // Made earlier, though it arrives later.
    push(&hub, &second, vec![alien_in(3, "fre", 1_000)])
        .await
        .unwrap();

    assert_eq!(
        pulled(&hub, &second, 3).await.unwrap(),
        [(3, "eng".to_owned())]
    );
}

#[sqlx::test(migrations = false)]
async fn a_linked_server_pulls_only_the_preferences_of_the_account_its_user_is_linked_to(
    db: PgPool,
) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let bea = create_proven(&hub, &mut inbox, "bea@example.com").await;
    let server = linked(&hub, &ada, "Ada's").await;
    let ada_code = start_user(&hub, &server, 7).await.unwrap();
    approve_user(&hub, &ada, &ada_code).await.unwrap();
    let bea_code = start_user(&hub, &server, 8).await.unwrap();
    approve_user(&hub, &bea, &bea_code).await.unwrap();
    let pushed = vec![
        alien_in(7, "eng", 1_000),
        alien_in(8, "ger", 1_000),
        alien_in(9, "jpn", 1_000),
    ];
    push(&hub, &server, pushed).await.unwrap();

    assert_eq!(
        pulled(&hub, &server, 7).await.unwrap(),
        [(7, "eng".to_owned())]
    );
    assert_eq!(
        pulled(&hub, &server, 9).await.unwrap(),
        [],
        "User 9 isn't Linked"
    );
}

#[sqlx::test(migrations = false)]
async fn a_preference_push_breaking_a_limit_is_refused(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let server = linked(&hub, &ada, "Ada's").await;
    let unnamed_movie = TrackPreferenceChange {
        title_id: String::new(),
        ..alien_in(7, "eng", 1_000)
    };
    let long_language = alien_in(7, &"e".repeat(101), 1_000);

    let without_id = assert_refused(
        push(
            &hub,
            &server,
            vec![alien_in(7, "eng", 1_000), unnamed_movie],
        )
        .await,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_TRACK_PREFERENCE_INVALID,
    );
    assert_eq!(without_id.field, "changes[1].title_id");
    let too_long = assert_refused(
        push(&hub, &server, vec![long_language]).await,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_TRACK_PREFERENCE_INVALID,
    );
    assert_eq!(too_long.field, "changes[0].language");
}
