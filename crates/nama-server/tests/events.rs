//! Live updates through the generated `EventService` client, over a real socket.
#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test helpers fail their test by panicking"
)]

#[path = "support/server.rs"]
mod server;

use std::{sync::Arc, time::Duration};

use connectrpc::{
    ErrorCode,
    client::{HttpClient, ServerStream},
};
use nama_plugin_api::{BoxFuture, Observer, Plugin, PluginId, Registrar, points};
use nama_proto::nama::v1::{
    __buffa::view::SubscribeResponseView, CreateLibraryRequest, Event, EventServiceClient,
    EventType, LibraryItemAdded, LibraryKind, LibraryServiceClient, ScanFinished,
    ScanLibraryRequest, ScanProgress, ScanStarted, SubscribeRequest, SubscribeResponse, event,
    subscribe_response,
};
use nama_server::{config::Config, plugins::Registry};
use server::TestServer;
use tempfile::TempDir;
use tokio::sync::mpsc;

/// How long a test waits for an event before failing.
const WAIT: Duration = Duration::from_secs(10);

/// A subscription to the Server's events.
type Subscription = ServerStream<hyper::body::Incoming, SubscribeResponseView<'static>>;

/// A Server, clients for its `LibraryService` and `EventService`, and a scratch directory for
/// media folders.
struct Fixture {
    libraries: LibraryServiceClient<HttpClient>,
    events: EventServiceClient<HttpClient>,
    media: TempDir,
    _server: TestServer,
}

impl Fixture {
    async fn start(plugins: Registry) -> Self {
        let server = TestServer::start(Config::default(), plugins).await;
        Self {
            libraries: LibraryServiceClient::new(HttpClient::plaintext(), server.client_config()),
            events: EventServiceClient::new(HttpClient::plaintext(), server.client_config()),
            media: tempfile::tempdir().unwrap(),
            _server: server,
        }
    }

    /// Creates a Movies Library named `name` whose folder holds `files`, and returns its id.
    async fn library(&self, name: &str, files: &[&str]) -> i64 {
        let folder = self.media.path().canonicalize().unwrap().join(name);
        for file in files {
            let path = folder.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"video").unwrap();
        }
        std::fs::create_dir_all(&folder).unwrap();
        self.libraries
            .create_library(CreateLibraryRequest {
                name: name.to_owned(),
                kind: LibraryKind::LIBRARY_KIND_MOVIES.into(),
                folders: vec![folder.to_str().unwrap().to_owned()],
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned()
            .library
            .into_option()
            .unwrap()
            .id
    }

    /// The absolute path of `file` in the folder of the Library named `name`.
    fn path(&self, name: &str, file: &str) -> String {
        let folder = self.media.path().canonicalize().unwrap().join(name);
        folder.join(file).to_str().unwrap().to_owned()
    }

    async fn scan(&self, id: i64) {
        self.libraries
            .scan_library(ScanLibraryRequest {
                id,
                ..Default::default()
            })
            .await
            .unwrap();
    }

    async fn subscribe(&self, request: SubscribeRequest) -> Subscription {
        self.events.subscribe(request).await.unwrap()
    }
}

/// The next message on `subscription`.
async fn next(subscription: &mut Subscription) -> subscribe_response::Message {
    tokio::time::timeout(WAIT, subscription.message::<SubscribeResponse>())
        .await
        .expect("timed out waiting for an event")
        .unwrap()
        .expect("the subscription ended")
        .to_owned_message()
        .message
        .expect("a message with nothing in it")
}

/// The events on `subscription` up to and including the next `ScanFinished`.
async fn until_scan_finished(subscription: &mut Subscription) -> Vec<Event> {
    let mut events = Vec::new();
    loop {
        let subscribe_response::Message::Event(event) = next(subscription).await else {
            panic!("the subscription lagged");
        };
        let finished = matches!(event.event, Some(event::Event::ScanFinished(_)));
        events.push(*event);
        if finished {
            return events;
        }
    }
}

fn event(event: impl Into<event::Event>) -> Event {
    Event {
        event: Some(event.into()),
        ..Default::default()
    }
}

#[tokio::test]
async fn a_subscriber_receives_a_scans_start_progress_added_items_and_finish_in_order() {
    let fx = Fixture::start(Registry::new()).await;
    let library = fx
        .library("Movies", &["Alien (1979)/Alien (1979).mkv"])
        .await;
    let mut subscription = fx.subscribe(SubscribeRequest::default()).await;

    fx.scan(library).await;

    assert_eq!(
        until_scan_finished(&mut subscription).await,
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
                path: fx.path("Movies", "Alien (1979)/Alien (1979).mkv"),
                ..Default::default()
            }),
            event(ScanFinished {
                library_id: library,
                added: 1,
                ..Default::default()
            }),
        ]
    );
}

/// A test Plugin whose Observer forwards each batch of events it is told.
struct Forward(mpsc::UnboundedSender<Vec<Event>>);

impl Plugin for Forward {
    fn id(&self) -> PluginId {
        PluginId::from_static("forward")
    }

    fn version(&self) -> &'static str {
        "0.0.0"
    }

    fn register(&self, registrar: &mut Registrar) {
        registrar.observe::<points::Events>(ForwardObserver(self.0.clone()));
    }
}

struct ForwardObserver(mpsc::UnboundedSender<Vec<Event>>);

impl Observer<points::Events> for ForwardObserver {
    fn observe<'a>(&'a self, event: &'a Vec<Event>) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            let _: Result<(), _> = self.0.send(event.clone());
        })
    }
}

/// `events` in an order that doesn't depend on the order they came in.
fn sorted(mut events: Vec<Event>) -> Vec<Event> {
    events.sort_by_key(|event| format!("{event:?}"));
    events
}

#[tokio::test]
async fn an_observer_is_told_the_same_events_a_subscriber_receives() {
    let (tx, mut observed) = mpsc::unbounded_channel();
    let plugins = Registry::new();
    plugins.enable(Arc::new(Forward(tx))).unwrap();
    let fx = Fixture::start(plugins).await;
    let library = fx
        .library(
            "Movies",
            &[
                "Alien (1979)/Alien (1979).mkv",
                "Heat (1995)/Heat (1995).mkv",
            ],
        )
        .await;
    let mut subscription = fx.subscribe(SubscribeRequest::default()).await;

    fx.scan(library).await;
    let streamed = until_scan_finished(&mut subscription).await;

    // Batches may reach an Observer in any order, so wait for as many events and compare them as
    // sets.
    let mut told = Vec::new();
    while told.len() < streamed.len() {
        let batch = tokio::time::timeout(WAIT, observed.recv())
            .await
            .expect("timed out waiting for the Observer")
            .expect("the Observer is gone");
        told.extend(batch);
    }
    assert_eq!(sorted(told), sorted(streamed));
}

#[tokio::test]
async fn a_subscriber_receives_only_the_event_types_and_libraries_it_asks_for() {
    let fx = Fixture::start(Registry::new()).await;
    let ignored = fx
        .library("Ignored", &["Alien (1979)/Alien (1979).mkv"])
        .await;
    let wanted = fx.library("Wanted", &["Heat (1995)/Heat (1995).mkv"]).await;
    let mut everything = fx.subscribe(SubscribeRequest::default()).await;
    let mut filtered = fx
        .subscribe(SubscribeRequest {
            types: vec![EventType::EVENT_TYPE_SCAN_FINISHED.into()],
            library_ids: vec![wanted],
            ..Default::default()
        })
        .await;

    // One scan after the other, so the first's events come before the second's.
    fx.scan(ignored).await;
    until_scan_finished(&mut everything).await;
    fx.scan(wanted).await;

    assert_eq!(
        next(&mut filtered).await,
        subscribe_response::Message::Event(Box::new(event(ScanFinished {
            library_id: wanted,
            added: 1,
            ..Default::default()
        })))
    );
}

#[tokio::test]
async fn subscribing_to_an_unspecified_event_type_is_an_invalid_argument() {
    let fx = Fixture::start(Registry::new()).await;
    let mut refused = fx
        .subscribe(SubscribeRequest {
            types: vec![EventType::EVENT_TYPE_UNSPECIFIED.into()],
            ..Default::default()
        })
        .await;

    let error = tokio::time::timeout(WAIT, refused.message::<SubscribeResponse>())
        .await
        .expect("timed out waiting for the refusal")
        .expect_err("subscribed with an unspecified event type");
    assert_eq!(error.code, ErrorCode::InvalidArgument);
}

#[tokio::test]
async fn shutting_down_the_server_ends_every_subscription() {
    let fx = Fixture::start(Registry::new()).await;
    let mut subscription = fx.subscribe(SubscribeRequest::default()).await;

    drop(fx);

    let ended = tokio::time::timeout(WAIT, subscription.message::<SubscribeResponse>())
        .await
        .expect("the subscription outlived the Server");
    assert!(matches!(ended, Ok(None)), "{ended:?}");
}
