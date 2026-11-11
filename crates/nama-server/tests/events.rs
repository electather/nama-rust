//! Live updates through the generated `EventService` client, over a real socket.

#[path = "support/refused.rs"]
mod refused;
#[path = "support/server.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod server;

use std::sync::Arc;

use connectrpc::{ErrorCode, client::HttpClient};
use nama_plugin_api::{
    BoxFuture, Manifest, Observer, Plugin, PluginId, PointKey, Registrar, points,
};
use nama_proto::nama::v1::{
    Event, EventType, LibraryItemAdded, LibraryKind, ListUnmatchedItemsRequest, MatchServiceClient,
    MatchingFinished, ProbingFinished, RefusalReason, ScanFinished, ScanProgress, ScanStarted,
    SubscribeRequest, event,
};
use nama_server::plugins::Registry;
use refused::assert_refused;
use server::{Library, Subscription, TestServer, WAIT, subscribe_with};
use tokio::sync::mpsc;

const MOVIES: LibraryKind = LibraryKind::LIBRARY_KIND_MOVIES;

/// The events on `subscription` up to and including the next `ProbingFinished`.
async fn until_probing_finished(subscription: &mut Subscription) -> Vec<Event> {
    let mut events = Vec::new();
    loop {
        let next = subscription.next().await;
        let finished = matches!(next, event::Event::ProbingFinished(_));
        events.push(event(next));
        if finished {
            return events;
        }
    }
}

/// The event `event`, as the Server sends it.
fn event(event: impl Into<event::Event>) -> Event {
    Event {
        event: Some(event.into()),
        ..Default::default()
    }
}

#[tokio::test]
async fn a_subscriber_receives_a_scans_start_progress_added_items_and_finishes_in_order() {
    let server = TestServer::builder().start().await;
    let media = server
        .create_library(MOVIES, &["Alien (1979)/Alien (1979).mkv"])
        .await;
    let library = media.id;
    let mut subscription =
        subscribe_with(&server.client_config(), SubscribeRequest::default()).await;

    server.scan(library).await;

    assert_eq!(
        until_probing_finished(&mut subscription).await,
        [
            event(ScanStarted {
                library_id: library,
                ..Default::default()
            }),
            event(ScanProgress {
                library_id: library,
                files_done: 1,
                files_total: 1,
                ..Default::default()
            }),
            event(LibraryItemAdded {
                library_id: library,
                // The first Item in a fresh database.
                item_id: 1,
                path: media.path("Alien (1979)/Alien (1979).mkv"),
                ..Default::default()
            }),
            event(ScanFinished {
                library_id: library,
                added: 1,
                ..Default::default()
            }),
            // No metadata Provider is enabled, and the Server has no ffprobe.
            event(MatchingFinished {
                library_id: library,
                undecided: 1,
                ..Default::default()
            }),
            event(ProbingFinished {
                library_id: library,
                ..Default::default()
            }),
        ]
    );
}

/// A test Plugin whose Observer forwards each batch of events it is told.
struct Forward(mpsc::UnboundedSender<Vec<Event>>);

impl Plugin for Forward {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            extension_points: vec![PointKey::observe::<points::Events>()],
            ..Manifest::new(PluginId::from_static("forward"))
        }
    }

    fn register(&self, registrar: &mut Registrar) {
        registrar.observe::<points::Events>(ForwardObserver(self.0.clone()));
    }
}

/// The Observer [`Forward`] registers.
struct ForwardObserver(mpsc::UnboundedSender<Vec<Event>>);

impl Observer<points::Events> for ForwardObserver {
    fn observe<'a>(&'a self, event: &'a Vec<Event>) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            let _: Result<(), _> = self.0.send(event.clone());
        })
    }
}

/// `batches` in an order that doesn't depend on the order they came in.
fn sorted(mut batches: Vec<Vec<Event>>) -> Vec<Vec<Event>> {
    batches.sort_by_key(|batch| format!("{batch:?}"));
    batches
}

#[tokio::test]
async fn an_observer_is_told_the_same_events_a_subscriber_receives_in_one_batch_per_write() {
    let (tx, mut observed) = mpsc::unbounded_channel();
    let plugins = Registry::new();
    plugins.enable(Arc::new(Forward(tx))).unwrap();
    let server = TestServer::builder().plugins(plugins).start().await;
    let media = server
        .create_library(
            MOVIES,
            &[
                "Alien (1979)/Alien (1979).mkv",
                "Heat (1995)/Heat (1995).mkv",
            ],
        )
        .await;
    let mut subscription =
        subscribe_with(&server.client_config(), SubscribeRequest::default()).await;

    server.scan(media.id).await;
    let streamed = until_probing_finished(&mut subscription).await;

    // The start, then the one write's progress and both Items, then the finishes.
    let [started, progress, first, second, scanned, matched, probed] =
        <[Event; 7]>::try_from(streamed).unwrap();
    let written = vec![
        vec![started],
        vec![progress, first, second],
        vec![scanned],
        vec![matched],
        vec![probed],
    ];
    // Batches may reach an Observer in any order, so compare them as sets.
    let mut told = Vec::new();
    while told.len() < written.len() {
        let batch = tokio::time::timeout(WAIT, observed.recv())
            .await
            .expect("timed out waiting for the Observer")
            .expect("the Observer is gone");
        told.push(batch);
    }
    assert_eq!(sorted(told), sorted(written));
}

#[tokio::test]
async fn a_subscriber_receives_only_the_event_types_and_libraries_it_asks_for() {
    let server = TestServer::builder().start().await;
    let ignored_media = server
        .create_library(MOVIES, &["Alien (1979)/Alien (1979).mkv"])
        .await;
    let wanted_media = server
        .create_library(MOVIES, &["Heat (1995)/Heat (1995).mkv"])
        .await;
    let (ignored, wanted) = (ignored_media.id, wanted_media.id);
    let mut everything = subscribe_with(&server.client_config(), SubscribeRequest::default()).await;
    let mut filtered = subscribe_with(
        &server.client_config(),
        SubscribeRequest {
            types: vec![EventType::EVENT_TYPE_SCAN_FINISHED.into()],
            library_ids: vec![wanted],
            ..Default::default()
        },
    )
    .await;

    // One scan after the other, so the first's events come before the second's.
    server.scan(ignored).await;
    until_probing_finished(&mut everything).await;
    server.scan(wanted).await;

    assert_eq!(
        event(filtered.next().await),
        event(ScanFinished {
            library_id: wanted,
            added: 1,
            ..Default::default()
        })
    );
}

#[tokio::test]
async fn subscribing_to_an_unspecified_event_type_is_an_invalid_argument() {
    let server = TestServer::builder().start().await;
    let mut refused = subscribe_with(
        &server.client_config(),
        SubscribeRequest {
            types: vec![EventType::EVENT_TYPE_UNSPECIFIED.into()],
            ..Default::default()
        },
    )
    .await;

    let message = refused.message().await;
    let refusal = assert_refused(
        message,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_UNKNOWN_ENUM_VALUE,
    );
    assert_eq!(refusal.field, "types");
}

#[tokio::test]
async fn shutting_down_the_server_ends_every_subscription() {
    let server = TestServer::builder().start().await;
    let mut subscription =
        subscribe_with(&server.client_config(), SubscribeRequest::default()).await;

    drop(server);

    let ended = subscription.message().await;
    assert!(matches!(ended, Ok(None)), "{ended:?}");
}

/// Whether `event` is a `ProbingFinished`.
fn is_probing_finished(event: &event::Event) -> bool {
    matches!(event, event::Event::ProbingFinished(_))
}

/// The `MatchingFinished` and `ProbingFinished` that end `events`, and whatever came before.
#[expect(clippy::panic, reason = "a test helper fails its test by panicking")]
fn split_finishes(
    mut events: Vec<event::Event>,
) -> (Vec<event::Event>, MatchingFinished, ProbingFinished) {
    let (
        Some(event::Event::ProbingFinished(probed)),
        Some(event::Event::MatchingFinished(matched)),
    ) = (events.pop(), events.pop())
    else {
        panic!("the scan didn't end with Matching then Probing finishing: {events:?}");
    };
    (events, *matched, *probed)
}

/// A Library over Alien, which the fake TMDB Matches, and Home Movies, which it doesn't.
async fn alien_and_home_movies(server: &TestServer) -> Library {
    server
        .create_library(
            MOVIES,
            &[
                "Alien (1979)/Alien (1979).mkv",
                "Home Movies (2019)/Home Movies (2019).mkv",
            ],
        )
        .await
}

#[tokio::test]
async fn matching_finished_comes_once_after_a_scans_items_are_matched_or_unmatched() {
    let server = TestServer::builder().tmdb().start().await;
    let media = alien_and_home_movies(&server).await;

    let events = server.scan_until(media.id, is_probing_finished).await;
    let unmatched = MatchServiceClient::new(HttpClient::plaintext(), server.client_config())
        .list_unmatched_items(ListUnmatchedItemsRequest {
            library_id: media.id,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .items;

    let (before, matched, _) = split_finishes(events);
    assert_eq!(
        matched,
        MatchingFinished {
            library_id: media.id,
            matched: 1,
            unmatched: 1,
            ..Default::default()
        }
    );
    assert!(
        (before.iter()).any(|event| matches!(event, event::Event::ItemMatched(_))),
        "{before:?}"
    );
    assert!(
        !(before.iter()).any(|event| matches!(event, event::Event::MatchingFinished(_))),
        "{before:?}"
    );
    let paths: Vec<_> = unmatched.iter().map(|item| item.path.as_str()).collect();
    assert_eq!(
        paths,
        [media.path("Home Movies (2019)/Home Movies (2019).mkv")]
    );
}

#[tokio::test]
async fn matching_finished_comes_with_the_items_undecided_when_the_metadata_provider_is_down() {
    let mut server = TestServer::builder().tmdb().start().await;
    let media = alien_and_home_movies(&server).await;
    server.stop_tmdb();

    let events = server.scan_until(media.id, is_probing_finished).await;

    let (_, matched, _) = split_finishes(events);
    assert_eq!(
        matched,
        MatchingFinished {
            library_id: media.id,
            undecided: 2,
            ..Default::default()
        }
    );
}

#[tokio::test]
async fn a_scan_that_finds_nothing_new_still_finishes_matching_and_probing() {
    let server = TestServer::builder().tmdb().start().await;
    let media = server
        .matched_library(MOVIES, &["Alien (1979)/Alien (1979).mkv"])
        .await;

    let events = server.scan_until(media.id, is_probing_finished).await;

    let (_, matched, probed) = split_finishes(events);
    assert_eq!(
        (matched, probed),
        (
            MatchingFinished {
                library_id: media.id,
                ..Default::default()
            },
            ProbingFinished {
                library_id: media.id,
                ..Default::default()
            }
        )
    );
}

#[tokio::test]
async fn without_ffprobe_probing_finished_follows_matching_finished_with_nothing_probed() {
    let server = TestServer::builder().tmdb().start().await;
    let media = alien_and_home_movies(&server).await;

    let events = server.scan_until(media.id, is_probing_finished).await;

    let (_, _, probed) = split_finishes(events);
    assert_eq!(
        probed,
        ProbingFinished {
            library_id: media.id,
            ..Default::default()
        }
    );
}
