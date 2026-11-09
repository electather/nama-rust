//! The event bus: the Kernel publishes what happens on it, and it hands each event to every
//! client subscribed through `EventService.Subscribe` and to every Observer at
//! [`points::Events`], and each Request event to every [`Notifier`], for its channel and for the
//! Plugin Connections of the Users told about it.
//!
//! Publishing never waits for a subscriber. Each subscriber may fall up to [`BUFFERED`] events
//! behind; past that its oldest unread events are dropped, and it is sent a `Lagged` in their
//! place.

use std::sync::Arc;

use futures_util::{Stream, StreamExt, future, stream};
use nama_plugin_api::{
    PluginError, PluginId,
    notifier::{DELIVER_TIMEOUT, Notification, Notifier, Recipient, RecipientGone},
    points,
};
use nama_proto::nama::v1::{
    Event, EventType, FollowedShow, Lagged, Permission, Request, RequestState, Requester,
    SubscribeResponse, event, subscribe_response,
};
use tokio::sync::broadcast::{self, error::RecvError};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::{
    connections, db::Database, plugins::Registry, server_settings::ServerUrl, users::User,
};

/// How many events a subscriber may fall behind before it lags. A scan publishes a batch of up to
/// one event per Item of [`crate::scanner`]'s batch of files at once, which this must hold.
const BUFFERED: usize = 4096;

/// How many recipients one Notifier delivers a notification to at once. An Installed Plugin's
/// delivery holds a blocking thread, so a Request with many Users to tell mustn't take one each.
const DELIVERING_AT_ONCE: usize = 4;

/// Publishes events to subscribers and Observers.
///
/// Cloning an `EventBus` gives another handle to the same one.
#[derive(Debug, Clone)]
pub(crate) struct EventBus {
    sender: broadcast::Sender<Event>,
    /// Where the Plugin Connections of the Users a notification is for are.
    db: Arc<Database>,
    plugins: Registry,
    /// What a notification's link to its Request starts with.
    server_url: ServerUrl,
    /// Cancelled when the Server shuts down, which ends every subscription.
    closed: CancellationToken,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Filter {
    /// Only these types; empty means every type.
    pub(crate) types: Vec<EventType>,
    /// Only about these Libraries; empty means every Library, and events about none.
    pub(crate) libraries: Vec<i64>,
    /// Only Request and Followed Show events about those this User is a Requester of; `None` means
    /// every one's.
    pub(crate) requester: Option<i64>,
}

/// What an event is about.
enum About<'a> {
    Library(i64),
    Request(&'a Request),
    FollowedShow(&'a FollowedShow),
}

impl EventBus {
    /// A bus that also tells the Observers and Notifiers enabled in `plugins`, linking
    /// notifications to their Requests at `server_url`, and finding the Users' Plugin Connections
    /// in `db`.
    pub(crate) fn new(db: Arc<Database>, plugins: Registry, server_url: ServerUrl) -> Self {
        Self::with_capacity(db, plugins, server_url, BUFFERED)
    }

    /// A bus on which subscribers may fall `capacity` events behind.
    pub(crate) fn with_capacity(
        db: Arc<Database>,
        plugins: Registry,
        server_url: ServerUrl,
        capacity: usize,
    ) -> Self {
        Self {
            sender: broadcast::channel(capacity).0,
            db,
            plugins,
            server_url,
            closed: CancellationToken::new(),
        }
    }

    /// Publishes `events`, in order, to each subscriber, and as one batch to each Observer. Each
    /// Request event goes to every Notifier too.
    pub(crate) fn publish(&self, events: Vec<Event>) {
        if events.is_empty() {
            return;
        }
        for event in &events {
            // Fails only when nobody is subscribed, which is fine.
            let _: Result<usize, _> = self.sender.send(event.clone());
            self.tell_notifiers(event);
        }
        self.plugins.notify::<points::Events>(events);
    }

    /// Hands `event`, if it is a Request event, to every Notifier for its channel and for the
    /// Plugin Connection to it of each User [`users_to_tell`] picks. The Kernel picks the
    /// recipients so that every Notifier shares the rules (ADR 0021).
    fn tell_notifiers(&self, event: &Event) {
        let Some((kind, About::Request(request))) = classify(event) else {
            return;
        };
        let removed = matches!(
            kind,
            EventType::EVENT_TYPE_REQUEST_CANCELLED | EventType::EVENT_TYPE_REQUEST_WITHDRAWN
        );
        let notification = Arc::new(Notification {
            event: event.clone().into(),
            request: request.clone().into(),
            // A removed Request is no longer in the web app to link to.
            request_url: self
                .server_url
                .request_link(request.id)
                .filter(|_| !removed),
            ..Default::default()
        });
        let db = Arc::clone(&self.db);
        self.plugins
            .call_each_provider(move |plugin, notifier: Arc<dyn Notifier>| {
                deliver(Arc::clone(&db), plugin, notifier, Arc::clone(&notification))
            });
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
        let Some((kind, about)) = classify(event) else {
            return false;
        };
        let wanted = match about {
            About::Library(library) => {
                self.libraries.is_empty() || self.libraries.contains(&library)
            }
            About::Request(request) => self.lets_through_requesters(&request.requesters),
            About::FollowedShow(followed) => self.lets_through_requesters(&followed.requesters),
        };
        (self.types.is_empty() || self.types.contains(&kind)) && wanted
    }

    /// Whether an event about something with `requesters`, and no Library, gets through.
    fn lets_through_requesters(&self, requesters: &[Requester]) -> bool {
        self.libraries.is_empty()
            && self
                .requester
                .is_none_or(|user| requesters.iter().any(|requester| requester.user_id == user))
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

/// Hands `notification` to `plugin`'s `notifier` for its channel and for the Plugin Connection to
/// `plugin` of each User told about it, [`DELIVERING_AT_ONCE`] at a time. A User with none is
/// skipped, and one the Notifier finds gone is disconnected (ADR 0021).
///
/// # Errors
///
/// Each recipient's that failed, together. Reading or deleting Plugin Connections failing is
/// logged, and the others still told.
async fn deliver(
    db: Arc<Database>,
    plugin: PluginId,
    notifier: Arc<dyn Notifier>,
    notification: Arc<Notification>,
) -> Result<(), PluginError> {
    let mut recipients = vec![(None, Recipient::Channel)];
    match connections::connected(&db, &plugin).await {
        Ok(connected) => recipients.extend(connections_to_tell(&notification.event, connected)),
        Err(err) => error!(%plugin, "could not read whom to message about a Request: {err}"),
    }
    let deliveries = recipients.into_iter().map(|(user, recipient)| {
        let (db, plugin, notifier, notification) = (&db, &plugin, &notifier, &notification);
        async move {
            let delivered = deliver_to(notifier.as_ref(), notification, &recipient).await;
            match (delivered, user, recipient) {
                (Ok(()), ..) => None,
                (Err(error), Some(user), Recipient::Connection(connection))
                    if error.is::<RecipientGone>() =>
                {
                    disconnect(db, user, plugin, connection).await;
                    None
                }
                // By id, so that no log names a chat id.
                (Err(error), Some(user), _) => Some(format!("to User {user}: {error}")),
                (Err(error), None, _) => Some(format!("to the channel: {error}")),
            }
        }
    });
    let failed: Vec<String> = stream::iter(deliveries)
        .buffer_unordered(DELIVERING_AT_ONCE)
        .filter_map(future::ready)
        .collect()
        .await;
    if failed.is_empty() {
        Ok(())
    } else {
        Err(failed.join("; ").into())
    }
}

/// The Plugin Connection of each of `connected` that [`users_to_tell`] picks for `event`, with
/// the id of the User whose it is.
fn connections_to_tell(
    event: &Event,
    connected: Vec<(User, String)>,
) -> Vec<(Option<i64>, Recipient)> {
    let (users, connections): (Vec<_>, Vec<_>) = connected.into_iter().unzip();
    let told = users_to_tell(event, &users);
    users
        .iter()
        .zip(connections)
        .filter(|(user, _)| told.contains(&user.id))
        .map(|(user, connection)| (Some(user.id), Recipient::Connection(connection)))
        .collect()
}

/// Deletes `user`'s Plugin Connection to `plugin`, `connection`, which its Notifier found gone.
async fn disconnect(db: &Database, user: i64, plugin: &PluginId, connection: String) {
    match connections::forget(db, user, plugin, connection).await {
        Ok(()) => info!(%plugin, user, "disconnected a User the service says is gone"),
        Err(err) => error!(%plugin, user, "could not disconnect a User who is gone: {err}"),
    }
}

/// Hands `notification` to `notifier` for `recipient`, giving up after [`DELIVER_TIMEOUT`].
async fn deliver_to(
    notifier: &dyn Notifier,
    notification: &Notification,
    recipient: &Recipient,
) -> Result<(), PluginError> {
    let delivered = notifier.deliver(notification, recipient);
    tokio::time::timeout(DELIVER_TIMEOUT, delivered)
        .await
        .unwrap_or_else(|elapsed| {
            Err(format!("timed out after {DELIVER_TIMEOUT:?} ({elapsed})").into())
        })
}

/// An event's type and what it is about.
fn classify(event: &Event) -> Option<(EventType, About<'_>)> {
    use About::{FollowedShow, Library, Request};
    use EventType as T;
    use event::Event as E;
    Some(match event.event.as_ref()? {
        E::ScanStarted(e) => (T::EVENT_TYPE_SCAN_STARTED, Library(e.library_id)),
        E::ScanProgress(e) => (T::EVENT_TYPE_SCAN_PROGRESS, Library(e.library_id)),
        E::ScanFinished(e) => (T::EVENT_TYPE_SCAN_FINISHED, Library(e.library_id)),
        E::LibraryItemAdded(e) => (T::EVENT_TYPE_LIBRARY_ITEM_ADDED, Library(e.library_id)),
        E::LibraryItemRemoved(e) => (T::EVENT_TYPE_LIBRARY_ITEM_REMOVED, Library(e.library_id)),
        E::ItemMatched(e) => (T::EVENT_TYPE_ITEM_MATCHED, Library(e.library_id)),
        E::RequestCreated(e) => (
            T::EVENT_TYPE_REQUEST_CREATED,
            Request(e.request.as_option()?),
        ),
        E::RequesterAdded(e) => (
            T::EVENT_TYPE_REQUESTER_ADDED,
            Request(e.request.as_option()?),
        ),
        E::RequestStateChanged(e) => (
            T::EVENT_TYPE_REQUEST_STATE_CHANGED,
            Request(e.request.as_option()?),
        ),
        E::RequestCancelled(e) => (
            T::EVENT_TYPE_REQUEST_CANCELLED,
            Request(e.request.as_option()?),
        ),
        E::RequesterRemoved(e) => (
            T::EVENT_TYPE_REQUESTER_REMOVED,
            Request(e.request.as_option()?),
        ),
        E::RequestWithdrawn(e) => (
            T::EVENT_TYPE_REQUEST_WITHDRAWN,
            Request(e.request.as_option()?),
        ),
        E::RequestProgress(e) => (
            T::EVENT_TYPE_REQUEST_PROGRESS,
            Request(e.request.as_option()?),
        ),
        E::FollowedShowCreated(e) => (
            T::EVENT_TYPE_FOLLOWED_SHOW_CREATED,
            FollowedShow(e.followed_show.as_option()?),
        ),
        E::FollowedShowRequesterAdded(e) => (
            T::EVENT_TYPE_FOLLOWED_SHOW_REQUESTER_ADDED,
            FollowedShow(e.followed_show.as_option()?),
        ),
        E::FollowedShowStateChanged(e) => (
            T::EVENT_TYPE_FOLLOWED_SHOW_STATE_CHANGED,
            FollowedShow(e.followed_show.as_option()?),
        ),
        E::FollowedShowCancelled(e) => (
            T::EVENT_TYPE_FOLLOWED_SHOW_CANCELLED,
            FollowedShow(e.followed_show.as_option()?),
        ),
        E::FollowedShowRequesterRemoved(e) => (
            T::EVENT_TYPE_FOLLOWED_SHOW_REQUESTER_REMOVED,
            FollowedShow(e.followed_show.as_option()?),
        ),
        E::FollowedShowWithdrawn(e) => (
            T::EVENT_TYPE_FOLLOWED_SHOW_WITHDRAWN,
            FollowedShow(e.followed_show.as_option()?),
        ),
    })
}

/// The IDs of the Users told about `event`, so every Notifier shares the rules (ADR 0021): a
/// Request's Requesters when it is approved, declined, Failed, Partially Available or Available,
/// and each of `users` with `PERMISSION_MANAGE_REQUESTS` about a new Pending Request. Nobody else,
/// and nobody for any other event.
pub(crate) fn users_to_tell(event: &Event, users: &[User]) -> Vec<i64> {
    use RequestState as S;
    use event::Event as E;
    match event.event.as_ref() {
        Some(E::RequestCreated(created)) if created.request.state == S::Pending => users
            .iter()
            .filter(|user| user.permissions.allow(Permission::ManageRequests))
            .map(|user| user.id)
            .collect(),
        // A scan opens a Followed Show's new Season already (Partially) Available. One opened
        // Approved tells nobody: its one Requester just asked for it.
        Some(E::RequestCreated(created))
            if matches!(
                created.request.state.as_known(),
                Some(S::PartiallyAvailable | S::Available)
            ) =>
        {
            requesters(&created.request)
        }
        Some(E::RequestStateChanged(changed))
            if matches!(
                changed.request.state.as_known(),
                Some(S::Approved | S::Declined | S::Failed | S::PartiallyAvailable | S::Available)
            ) =>
        {
            requesters(&changed.request)
        }
        _ => Vec::new(),
    }
}

fn requesters(request: &Request) -> Vec<i64> {
    request
        .requesters
        .iter()
        .map(|requester| requester.user_id)
        .collect()
}

/// An [`Event`] holding `happened`, one of the things that can happen, such as a `ScanStarted`.
pub(crate) fn event(happened: impl Into<event::Event>) -> Event {
    Event {
        event: Some(happened.into()),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use nama_proto::nama::v1::{
        Event, Permission, Request, RequestCancelled, RequestCreated, RequestState,
        RequestStateChanged, Requester, RequesterAdded,
    };

    use super::{event, users_to_tell};
    use crate::users::{Permissions, User};

    /// Users 1 and 2 have a new User's Permissions, 3 has `MANAGE_REQUESTS` and 4 has `ADMIN`.
    fn users() -> Vec<User> {
        let user = |id, permissions| User {
            id,
            name: format!("User {id}"),
            permissions,
            created_at: 0,
        };
        vec![
            user(1, Permissions::NEW_USER),
            user(2, Permissions::NEW_USER),
            user(3, std::iter::once(Permission::ManageRequests).collect()),
            user(4, Permissions::ADMIN),
        ]
    }

    fn request(state: RequestState, requesters: &[i64]) -> Request {
        Request {
            state: state.into(),
            requesters: requesters
                .iter()
                .map(|&user_id| Requester {
                    user_id,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }
    }

    fn created(state: RequestState, requesters: &[i64]) -> Event {
        event(RequestCreated {
            request: request(state, requesters).into(),
            ..Default::default()
        })
    }

    fn moved_to(state: RequestState, requesters: &[i64]) -> Event {
        event(RequestStateChanged {
            request: request(state, requesters).into(),
            ..Default::default()
        })
    }

    #[test]
    fn approving_or_declining_a_request_picks_its_requesters_and_nobody_else() {
        for state in [RequestState::Approved, RequestState::Declined] {
            assert_eq!(
                users_to_tell(&moved_to(state, &[2, 1]), &users()),
                [2, 1],
                "{state:?}"
            );
        }
    }

    #[test]
    fn a_request_failing_or_becoming_available_picks_its_requesters() {
        for state in [
            RequestState::Failed,
            RequestState::PartiallyAvailable,
            RequestState::Available,
        ] {
            assert_eq!(
                users_to_tell(&moved_to(state, &[1]), &users()),
                [1],
                "{state:?}"
            );
        }
    }

    #[test]
    fn a_followed_shows_season_opening_available_picks_its_requesters() {
        for state in [RequestState::PartiallyAvailable, RequestState::Available] {
            assert_eq!(
                users_to_tell(&created(state, &[1, 2]), &users()),
                [1, 2],
                "{state:?}"
            );
        }
    }

    #[test]
    fn a_new_pending_request_picks_every_user_with_manage_requests() {
        let users = users();
        assert_eq!(
            users_to_tell(&created(RequestState::Pending, &[1]), &users),
            [3, 4]
        );
        assert_eq!(
            users_to_tell(&created(RequestState::Pending, &[3]), &users),
            [3, 4],
            "an asker with MANAGE_REQUESTS is told as one of them"
        );
    }

    #[test]
    fn other_request_events_pick_nobody() {
        let pending = request(RequestState::Pending, &[1, 2]);
        let others = [
            event(RequesterAdded {
                request: pending.clone().into(),
                user_id: 2,
                ..Default::default()
            }),
            event(RequestCancelled {
                request: pending.into(),
                ..Default::default()
            }),
            // Opened Approved: its one Requester just asked for it.
            created(RequestState::Approved, &[1]),
            moved_to(RequestState::Processing, &[1]),
        ];
        for other in others {
            assert!(users_to_tell(&other, &users()).is_empty(), "{other:?}");
        }
    }
}
