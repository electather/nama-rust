//! A Linked Server recording its Users' Viewing Events, an Account listing and deleting its own
//! and turning pooling off, and the titles the Hub recommends from them, against a real PostgreSQL.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/hub.rs"]
mod hub;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/mail.rs"]
mod mail;
#[path = "support/refused.rs"]
mod refused;

use axum::http::HeaderMap;
use connectrpc::{ErrorCode, client::HttpClient};
use hub::TestHub;
use mail::{Inbox, fake_mail};
use nama_hub::titles;
use nama_hub_proto::nama::hub::v1::{
    AccountServiceClient, ApproveServerLinkRequest, ApproveUserLinkRequest, CreateAccountRequest,
    DeleteViewingEventRequest, DeviceKind, GetPoolingRequest, LinkServiceClient,
    LinkedServerServiceClient, ListRecommendedTitlesRequest, ListViewingEventsRequest,
    PollServerLinkRequest, RecommendedTitle, RecordViewingEventsRequest, RefusalReason,
    SendSignInLinkRequest, ServerLinkServiceClient, SetPoolingRequest, SignInWithLinkRequest,
    StartServerLinkRequest, StartUserLinkRequest, TitleKind, UserViewingEvent, ViewingEvent,
    ViewingEventServiceClient,
};
use refused::assert_refused;
use sqlx::PgPool;

fn accounts(hub: &TestHub) -> AccountServiceClient<HttpClient> {
    AccountServiceClient::new(HttpClient::plaintext(), hub.client_config())
}

/// The session token a response's `Set-Cookie` carries.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn session_token(headers: &HeaderMap) -> String {
    let cookie = headers.get("set-cookie").unwrap().to_str().unwrap();
    let token = cookie.split(';').next().unwrap();
    token.strip_prefix("nama_hub_session=").unwrap().to_owned()
}

/// Creates an Account with `email`, proves it with a Sign-in Link, and returns the session token
/// the link signed in, as Linking needs.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn create_proven(hub: &TestHub, inbox: &mut Inbox, email: &str) -> String {
    accounts(hub)
        .create_account(CreateAccountRequest {
            email: email.to_owned(),
            password: "correct horse battery".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();
    accounts(hub)
        .send_sign_in_link(SendSignInLinkRequest {
            email: email.to_owned(),
            continue_to: "/".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();
    let used = accounts(hub)
        .sign_in_with_link(SignInWithLinkRequest {
            token: inbox.next_link(email).await,
            ..Default::default()
        })
        .await
        .unwrap();
    session_token(used.headers())
}

/// The Hub's pages' Links, signed in with the session `token`.
fn links(hub: &TestHub, token: &str) -> LinkServiceClient<HttpClient> {
    let config =
        (hub.client_config()).with_default_header("cookie", format!("nama_hub_session={token}"));
    LinkServiceClient::new(HttpClient::plaintext(), config)
}

/// The Hub's pages' history, signed in with the session `token`.
fn history(hub: &TestHub, token: &str) -> ViewingEventServiceClient<HttpClient> {
    let config =
        (hub.client_config()).with_default_header("cookie", format!("nama_hub_session={token}"));
    ViewingEventServiceClient::new(HttpClient::plaintext(), config)
}

/// A Server presenting `credential`.
fn linked_server(hub: &TestHub, credential: &str) -> LinkedServerServiceClient<HttpClient> {
    let config =
        (hub.client_config()).with_default_header("authorization", format!("Bearer {credential}"));
    LinkedServerServiceClient::new(HttpClient::plaintext(), config)
}

/// Links a Server to the Account signed in with `token`, and returns its credential.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn linked(hub: &TestHub, token: &str) -> String {
    let server = ServerLinkServiceClient::new(HttpClient::plaintext(), hub.client_config());
    let started = server
        .start_server_link(StartServerLinkRequest {
            name: "Living room".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned();
    links(hub, token)
        .approve_server_link(ApproveServerLinkRequest {
            code: started.user_code,
            ..Default::default()
        })
        .await
        .unwrap();
    let polled = server
        .poll_server_link(PollServerLinkRequest {
            device_code: started.device_code,
            ..Default::default()
        })
        .await
        .unwrap();
    polled.into_owned().credential
}

/// Links the User `user_id` of the Server presenting `credential` to the Account signed in with
/// `token`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn link_user(hub: &TestHub, credential: &str, user_id: i64, token: &str) {
    let started = linked_server(hub, credential)
        .start_user_link(StartUserLinkRequest {
            user_id,
            name: "Grace".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();
    links(hub, token)
        .approve_user_link(ApproveUserLinkRequest {
            code: started.into_owned().user_code,
            ..Default::default()
        })
        .await
        .unwrap();
}

/// A play of the Movie `external_id` by the User `user_id`, named `key`.
fn played(user_id: i64, key: &str, external_id: &str) -> UserViewingEvent {
    UserViewingEvent {
        user_id,
        key: key.to_owned(),
        event: ViewingEvent {
            external_id: external_id.to_owned(),
            started_at: 1_700_000_000,
            time_of_day_minutes: 1260,
            stop_position_ms: 8_000_000,
            duration_ms: 8_160_000,
            finished: true,
            device_kind: DeviceKind::DEVICE_KIND_TV.into(),
            ..Default::default()
        }
        .into(),
        ..Default::default()
    }
}

/// A play of the Episode `tmdb-63056`, of the Show `tmdb-1399`, by the User `user_id`, named `key`.
fn played_episode(user_id: i64, key: &str) -> UserViewingEvent {
    let mut played = played(user_id, key, "tmdb-63056");
    played.event.modify(|event| {
        event.episode = true;
        "tmdb-1399".clone_into(&mut event.show_external_id);
    });
    played
}

async fn record(
    hub: &TestHub,
    credential: &str,
    events: Vec<UserViewingEvent>,
) -> Result<(), connectrpc::ConnectError> {
    linked_server(hub, credential)
        .record_viewing_events(RecordViewingEventsRequest {
            events,
            ..Default::default()
        })
        .await?;
    Ok(())
}

/// The signed-in Account's Viewing Events.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn listed(hub: &TestHub, token: &str) -> Vec<ViewingEvent> {
    let listed = history(hub, token)
        .list_viewing_events(ListViewingEventsRequest::default())
        .await
        .unwrap();
    listed.into_owned().events
}

/// The External ID of each of the signed-in Account's Viewing Events.
async fn listed_ids(hub: &TestHub, token: &str) -> Vec<String> {
    (listed(hub, token).await.into_iter())
        .map(|event| event.external_id)
        .collect()
}

/// Whether the signed-in Account pools.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn pooling(hub: &TestHub, token: &str) -> bool {
    let got = history(hub, token)
        .get_pooling(GetPoolingRequest::default())
        .await
        .unwrap();
    got.into_owned().pooling
}

/// A Hub with Ada's Account, a Server she Linked, and that Server's User 7 Linked to her.
async fn ada_watching(db: PgPool) -> (TestHub, Inbox, String, String) {
    let (mail, mut inbox) = fake_mail();
    let hub = TestHub::start(db, Some(mail)).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let server = linked(&hub, &ada).await;
    link_user(&hub, &server, 7, &ada).await;
    (hub, inbox, ada, server)
}

#[sqlx::test(migrations = false)]
async fn a_linked_users_play_is_listed_under_their_account(db: PgPool) {
    let (hub, _inbox, ada, server) = ada_watching(db).await;

    record(&hub, &server, vec![played(7, "a", "tmdb-603")])
        .await
        .unwrap();

    let events = listed(&hub, &ada).await;
    let [event] = events.as_slice() else {
        panic!("one Viewing Event: {events:?}");
    };
    let ViewingEvent { id, .. } = event;
    let mut expected = played(7, "a", "tmdb-603").event.into_option().unwrap();
    expected.id.clone_from(id);
    assert_eq!(event, &expected);
}

#[sqlx::test(migrations = false)]
async fn a_user_who_isnt_linked_records_nothing(db: PgPool) {
    let (hub, _inbox, ada, server) = ada_watching(db).await;

    record(&hub, &server, vec![played(8, "a", "tmdb-603")])
        .await
        .unwrap();

    assert_eq!(listed_ids(&hub, &ada).await, Vec::<String>::new());
}

#[sqlx::test(migrations = false)]
async fn an_event_sent_again_is_recorded_once(db: PgPool) {
    let (hub, _inbox, ada, server) = ada_watching(db).await;

    record(&hub, &server, vec![played(7, "a", "tmdb-603")])
        .await
        .unwrap();
    record(&hub, &server, vec![played(7, "a", "tmdb-603")])
        .await
        .unwrap();

    assert_eq!(listed_ids(&hub, &ada).await, ["tmdb-603"]);
}

#[sqlx::test(migrations = false)]
async fn an_episode_sent_again_is_listed_once_with_its_show(db: PgPool) {
    let (hub, _inbox, ada, server) = ada_watching(db).await;

    record(&hub, &server, vec![played_episode(7, "a")])
        .await
        .unwrap();
    record(&hub, &server, vec![played_episode(7, "a")])
        .await
        .unwrap();

    let events = listed(&hub, &ada).await;
    let [event] = events.as_slice() else {
        panic!("one Viewing Event: {events:?}");
    };
    assert!(event.episode);
    assert_eq!(event.external_id, "tmdb-63056");
    assert_eq!(event.show_external_id, "tmdb-1399");
}

#[sqlx::test(migrations = false)]
async fn an_account_lists_and_deletes_only_its_own_events(db: PgPool) {
    let (hub, mut inbox, ada, server) = ada_watching(db).await;
    let bea = create_proven(&hub, &mut inbox, "bea@example.com").await;
    link_user(&hub, &server, 8, &bea).await;
    record(
        &hub,
        &server,
        vec![played(7, "a", "tmdb-603"), played(8, "b", "tmdb-78")],
    )
    .await
    .unwrap();
    let adas = listed(&hub, &ada).await;
    let id = adas.first().map(|event| event.id.clone()).unwrap();

    history(&hub, &bea)
        .delete_viewing_event(DeleteViewingEventRequest {
            id: id.clone(),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(listed_ids(&hub, &ada).await, ["tmdb-603"]);
    assert_eq!(listed_ids(&hub, &bea).await, ["tmdb-78"]);

    history(&hub, &ada)
        .delete_viewing_event(DeleteViewingEventRequest {
            id,
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(listed_ids(&hub, &ada).await, Vec::<String>::new());
}

#[sqlx::test(migrations = false)]
async fn a_server_records_only_under_its_own_users_accounts(db: PgPool) {
    let (hub, mut inbox, ada, _ada_server) = ada_watching(db).await;
    let bea = create_proven(&hub, &mut inbox, "bea@example.com").await;
    let beas_server = linked(&hub, &bea).await;

    // Bea's Server has no User 7 Linked, whatever Ada's Server calls its User 7.
    record(&hub, &beas_server, vec![played(7, "a", "tmdb-603")])
        .await
        .unwrap();

    assert_eq!(listed_ids(&hub, &ada).await, Vec::<String>::new());
    assert_eq!(listed_ids(&hub, &bea).await, Vec::<String>::new());
}

#[sqlx::test(migrations = false)]
async fn an_invalid_event_records_none_of_its_batch(db: PgPool) {
    let (hub, _inbox, ada, server) = ada_watching(db).await;

    let refused = assert_refused(
        record(
            &hub,
            &server,
            vec![played(7, "a", "tmdb-603"), played(7, "b", "")],
        )
        .await,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_VIEWING_EVENT_INVALID,
    );

    assert_eq!(refused.field, "events");
    assert_eq!(listed_ids(&hub, &ada).await, Vec::<String>::new());
}

#[sqlx::test(migrations = false)]
async fn an_episode_without_its_show_is_refused(db: PgPool) {
    let (hub, _inbox, ada, server) = ada_watching(db).await;
    let mut episode = played_episode(7, "a");
    episode.event.modify(|event| event.show_external_id.clear());

    assert_refused(
        record(&hub, &server, vec![episode]).await,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_VIEWING_EVENT_INVALID,
    );
    assert_eq!(listed_ids(&hub, &ada).await, Vec::<String>::new());
}

#[sqlx::test(migrations = false)]
async fn a_movie_naming_a_show_is_refused(db: PgPool) {
    let (hub, _inbox, ada, server) = ada_watching(db).await;
    let mut movie = played(7, "a", "tmdb-603");
    movie
        .event
        .modify(|event| "tmdb-1399".clone_into(&mut event.show_external_id));

    assert_refused(
        record(&hub, &server, vec![movie]).await,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_VIEWING_EVENT_INVALID,
    );
    assert_eq!(listed_ids(&hub, &ada).await, Vec::<String>::new());
}

#[sqlx::test(migrations = false)]
async fn the_history_needs_a_session_and_recording_a_linked_server(db: PgPool) {
    let (hub, _inbox, ada, _server) = ada_watching(db).await;

    assert_refused(
        history(&hub, "not-a-session")
            .list_viewing_events(ListViewingEventsRequest::default())
            .await,
        ErrorCode::Unauthenticated,
        RefusalReason::REFUSAL_REASON_SIGN_IN_REQUIRED,
    );
    assert_refused(
        record(&hub, "not-a-credential", vec![played(7, "a", "tmdb-603")]).await,
        ErrorCode::Unauthenticated,
        RefusalReason::REFUSAL_REASON_SERVER_NOT_LINKED,
    );
    assert_eq!(listed_ids(&hub, &ada).await, Vec::<String>::new());
}

/// A Hub with the fixture's titles and their vectors, Ada's Account, a Server she Linked, and that
/// Server's User 7 Linked to her.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn ada_watching_known_titles(db: PgPool) -> (TestHub, Inbox, String, String) {
    titles::import(&db, &include_bytes!("fixtures/wikidata-dump.json")[..])
        .await
        .unwrap();
    titles::train(&db).await.unwrap();
    ada_watching(db).await
}

/// The titles the Server presenting `credential` gets recommended for its User `user_id`, best
/// first, each as its kind and External ID, such as `movie tmdb-603`.
async fn recommended(
    hub: &TestHub,
    credential: &str,
    user_id: i64,
) -> Result<Vec<String>, connectrpc::ConnectError> {
    let titles = recommended_titles(hub, credential, user_id).await?;
    Ok((titles.into_iter())
        .map(|title| {
            assert!(title.weight > 0.0 && title.weight < 1.001, "{title:?}");
            let kind = if title.kind == TitleKind::TITLE_KIND_SHOW {
                "show"
            } else {
                "movie"
            };
            format!("{kind} {}-{}", title.id.namespace, title.id.value)
        })
        .collect())
}

/// The titles the Server presenting `credential` gets recommended for its User `user_id`.
async fn recommended_titles(
    hub: &TestHub,
    credential: &str,
    user_id: i64,
) -> Result<Vec<RecommendedTitle>, connectrpc::ConnectError> {
    Ok(linked_server(hub, credential)
        .list_recommended_titles(ListRecommendedTitlesRequest {
            user_id,
            namespaces: vec!["tmdb".to_owned(), "imdb".to_owned()],
            ..Default::default()
        })
        .await?
        .into_owned()
        .titles)
}

/// The weight Ada's User 7 is recommended The Matrix Reloaded with.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn sequel_weight(hub: &TestHub, server: &str) -> f32 {
    let titles = recommended_titles(hub, server, 7).await.unwrap();
    (titles.into_iter())
        .find(|title| title.id.value == "604")
        .unwrap()
        .weight
}

/// `titles`, sorted, for a comparison that doesn't depend on the order of titles alike to one.
fn sorted(mut titles: Vec<String>) -> Vec<String> {
    titles.sort();
    titles
}

#[sqlx::test(migrations = false)]
async fn a_linked_user_is_recommended_what_is_nearest_what_they_finished_and_not_that(db: PgPool) {
    let (hub, _inbox, _ada, server) = ada_watching_known_titles(db).await;

    record(&hub, &server, vec![played(7, "a", "tmdb-603")])
        .await
        .unwrap();

    let recommended = recommended(&hub, &server, 7).await.unwrap();
    // The sequel shares nearly every fact; Alien and the series each share one and the country.
    assert_eq!(recommended.first().unwrap(), "movie tmdb-604");
    assert_eq!(
        sorted(recommended),
        ["movie tmdb-348", "movie tmdb-604", "show tmdb-603"]
    );
}

#[sqlx::test(migrations = false)]
async fn a_play_that_didnt_finish_recommends_nothing(db: PgPool) {
    let (hub, _inbox, _ada, server) = ada_watching_known_titles(db).await;
    let mut stopped = played(7, "a", "tmdb-603");
    stopped.event.get_or_insert_default().finished = false;

    record(&hub, &server, vec![stopped]).await.unwrap();

    assert_eq!(
        recommended(&hub, &server, 7).await.unwrap(),
        Vec::<String>::new()
    );
}

#[sqlx::test(migrations = false)]
async fn deleting_a_viewing_event_recomputes_the_taste_vector(db: PgPool) {
    let (hub, _inbox, ada, server) = ada_watching_known_titles(db).await;
    record(
        &hub,
        &server,
        vec![played(7, "a", "tmdb-603"), played(7, "b", "tmdb-348")],
    )
    .await
    .unwrap();
    assert_eq!(
        sorted(recommended(&hub, &server, 7).await.unwrap()),
        ["movie tmdb-604", "show tmdb-603"]
    );
    let with_alien = sequel_weight(&hub, &server).await;
    let alien = (listed(&hub, &ada).await.into_iter())
        .find(|event| event.external_id == "tmdb-348")
        .unwrap();

    history(&hub, &ada)
        .delete_viewing_event(DeleteViewingEventRequest {
            id: alien.id,
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(
        sorted(recommended(&hub, &server, 7).await.unwrap()),
        ["movie tmdb-348", "movie tmdb-604", "show tmdb-603"]
    );
    // Alien, which shares little with the sequel, no longer pulls the taste vector from it.
    assert!(sequel_weight(&hub, &server).await > with_alien);
}

#[sqlx::test(migrations = false)]
async fn a_server_gets_recommendations_only_for_its_own_linked_users(db: PgPool) {
    let (hub, mut inbox, _ada, server) = ada_watching_known_titles(db).await;
    let bea = create_proven(&hub, &mut inbox, "bea@example.com").await;
    let beas_server = linked(&hub, &bea).await;
    record(&hub, &server, vec![played(7, "a", "tmdb-603")])
        .await
        .unwrap();

    // Bea's Server has no User 7 Linked, whatever Ada's Server calls its User 7.
    assert_eq!(
        recommended(&hub, &beas_server, 7).await.unwrap(),
        Vec::<String>::new()
    );
    assert_eq!(
        recommended(&hub, &server, 8).await.unwrap(),
        Vec::<String>::new()
    );
    assert_refused(
        recommended(&hub, "not-a-credential", 7).await,
        ErrorCode::Unauthenticated,
        RefusalReason::REFUSAL_REASON_SERVER_NOT_LINKED,
    );
}

#[sqlx::test(migrations = false)]
async fn an_account_turns_off_only_its_own_pooling(db: PgPool) {
    let (hub, mut inbox, ada, _server) = ada_watching(db).await;
    let bea = create_proven(&hub, &mut inbox, "bea@example.com").await;
    assert!(pooling(&hub, &ada).await);

    history(&hub, &ada)
        .set_pooling(SetPoolingRequest {
            pooling: false,
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(!pooling(&hub, &ada).await);
    assert!(pooling(&hub, &bea).await);
}

#[sqlx::test(migrations = false)]
async fn pooling_needs_a_session(db: PgPool) {
    let (hub, _inbox, ada, _server) = ada_watching(db).await;

    assert_refused(
        history(&hub, "not-a-session")
            .set_pooling(SetPoolingRequest::default())
            .await,
        ErrorCode::Unauthenticated,
        RefusalReason::REFUSAL_REASON_SIGN_IN_REQUIRED,
    );
    assert_refused(
        history(&hub, "not-a-session")
            .get_pooling(GetPoolingRequest::default())
            .await,
        ErrorCode::Unauthenticated,
        RefusalReason::REFUSAL_REASON_SIGN_IN_REQUIRED,
    );
    assert!(pooling(&hub, &ada).await);
}
