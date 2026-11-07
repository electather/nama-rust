//! The event bus: the Kernel publishes what happens on it, and it hands each event to every
//! client subscribed through `EventService.Subscribe` and to every Observer at
//! [`points::Events`].
//!
//! Publishing never waits for a subscriber. Each subscriber may fall up to [`BUFFERED`] events
//! behind; past that its oldest unread events are dropped, and it is sent a `Lagged` in their
//! place.

use futures_util::{Stream, StreamExt, stream};
use nama_plugin_api::points;
use nama_proto::nama::v1::{
    Event, EventType, Lagged, SubscribeResponse, event, subscribe_response,
};
use tokio::sync::broadcast::{self, error::RecvError};
use tokio_util::sync::CancellationToken;

use crate::plugins::Registry;

/// How many events a subscriber may fall behind before it lags. A scan publishes a batch of up to
/// one event per Item of [`crate::scanner`]'s batch of files at once, which this must hold.
const BUFFERED: usize = 4096;

/// Publishes events to subscribers and Observers.
///
/// Cloning an `EventBus` gives another handle to the same one.
#[derive(Debug, Clone)]
pub(crate) struct EventBus {
    sender: broadcast::Sender<Event>,
    plugins: Registry,
    /// Cancelled when the Server shuts down, which ends every subscription.
    closed: CancellationToken,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Filter {
    /// Only these types; empty means every type.
    pub(crate) types: Vec<EventType>,
    /// Only about these Libraries; empty means every Library.
    pub(crate) libraries: Vec<i64>,
}

impl EventBus {
    /// A bus that also tells the Observers enabled in `plugins`.
    pub(crate) fn new(plugins: Registry) -> Self {
        Self::with_capacity(plugins, BUFFERED)
    }

    /// A bus on which subscribers may fall `capacity` events behind.
    pub(crate) fn with_capacity(plugins: Registry, capacity: usize) -> Self {
        Self {
            sender: broadcast::channel(capacity).0,
            plugins,
            closed: CancellationToken::new(),
        }
    }

    /// Publishes `events`, in order, to each subscriber, and as one batch to each Observer.
    pub(crate) fn publish(&self, events: Vec<Event>) {
        if events.is_empty() {
            return;
        }
        for event in &events {
            // Fails only when nobody is subscribed, which is fine.
            let _: Result<usize, _> = self.sender.send(event.clone());
        }
        self.plugins.notify::<points::Events>(events);
    }

    /// The events published from now on that `filter` lets through, with a `Lagged` wherever the
    /// subscriber fell too far behind. Ends when the bus is closed.
    pub(crate) fn subscribe(
        &self,
        filter: Filter,
    ) -> impl Stream<Item = SubscribeResponse> + Send + 'static {
        let receiver = self.sender.subscribe();
        stream::unfold((receiver, filter), |(mut receiver, filter)| async move {
            let message = next_message(&mut receiver, &filter).await?;
            let response = SubscribeResponse {
                message: Some(message),
                ..Default::default()
            };
            Some((response, (receiver, filter)))
        })
        .take_until(self.closed.clone().cancelled_owned())
    }

    /// Ends every subscription, as the Server shuts down, so none holds up the shutdown.
    pub(crate) fn close(&self) {
        self.closed.cancel();
    }
}

impl Filter {
    fn lets_through(&self, event: &Event) -> bool {
        let Some((kind, library)) = classify(event) else {
            return false;
        };
        (self.types.is_empty() || self.types.contains(&kind))
            && (self.libraries.is_empty() || self.libraries.contains(&library))
    }
}

/// The next event from `receiver` that `filter` lets through, or a `Lagged` if events were
/// dropped first. `None` once nothing more can be published.
async fn next_message(
    receiver: &mut broadcast::Receiver<Event>,
    filter: &Filter,
) -> Option<subscribe_response::Message> {
    loop {
        match receiver.recv().await {
            Ok(event) if filter.lets_through(&event) => return Some(event.into()),
            Ok(_) => {}
            Err(RecvError::Lagged(dropped)) => {
                let lagged = Lagged {
                    dropped,
                    ..Default::default()
                };
                return Some(lagged.into());
            }
            Err(RecvError::Closed) => return None,
        }
    }
}

/// An event's type and the Library it is about.
fn classify(event: &Event) -> Option<(EventType, i64)> {
    use event::Event as E;
    Some(match event.event.as_ref()? {
        E::ScanStarted(e) => (EventType::EVENT_TYPE_SCAN_STARTED, e.library_id),
        E::ScanProgress(e) => (EventType::EVENT_TYPE_SCAN_PROGRESS, e.library_id),
        E::ScanFinished(e) => (EventType::EVENT_TYPE_SCAN_FINISHED, e.library_id),
        E::LibraryItemAdded(e) => (EventType::EVENT_TYPE_LIBRARY_ITEM_ADDED, e.library_id),
        E::LibraryItemRemoved(e) => (EventType::EVENT_TYPE_LIBRARY_ITEM_REMOVED, e.library_id),
        E::ItemMatched(e) => (EventType::EVENT_TYPE_ITEM_MATCHED, e.library_id),
    })
}

/// An [`Event`] holding `happened`, one of the things that can happen, such as a `ScanStarted`.
pub(crate) fn event(happened: impl Into<event::Event>) -> Event {
    Event {
        event: Some(happened.into()),
        ..Default::default()
    }
}
