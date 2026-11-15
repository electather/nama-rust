//! The event bus: the Kernel publishes what happens on it, and it hands each event to every
//! client subscribed through `EventService.Subscribe` and to every Observer at
//! [`points::Events`], and each Request event to every [`Notifier`], for its channel and for the
//! Plugin Connections and Personal settings of the Users told about it.
//!
//! Publishing never waits for a subscriber. Each subscriber may fall up to [`BUFFERED`] events
//! behind; past that its oldest unread events are dropped, and it is sent a `Lagged` in their
//! place.

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use futures_util::{Stream, StreamExt, future, stream};
use nama_plugin_api::{
    PluginError, PluginId,
    notifier::{DELIVER_TIMEOUT, Notification, Notifier, Recipient, RecipientGone},
    points,
};
use nama_proto::nama::v1::{
    Event, EventType, FollowedShow, Lagged, Permission, Request, RequestState, SubscribeResponse,
    event, subscribe_response,
};
use tokio::sync::broadcast::{self, error::RecvError};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::{
    connections,
    db::Database,
    playback,
    plugins::{self, ExtensionPoints, Failure, ProviderError},
    requests,
    server_settings::ServerUrl,
    users::{User, personal_settings},
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
    plugins: ExtensionPoints,
    /// What a notification's link to its Request starts with.
    server_url: ServerUrl,
    /// Cancelled when the Server shuts down, which ends every subscription.
    closed: CancellationToken,
}

/// What a subscriber asked to narrow their events to. It only narrows what their [`Viewer`] sees.
#[derive(Debug, Clone, Default)]
pub(crate) struct Filter {
    /// Only these types; empty means every type.
    pub(crate) types: Vec<EventType>,
    /// Only about these Libraries; empty means every Library, and events about none.
    pub(crate) libraries: Vec<i64>,
}

/// Which events a subscriber may see, and what of them, from their Permissions. Only this module
/// decides that (see the crate's `AGENTS.md`, "One owner for who sees an Event").
#[derive(Debug)]
struct Viewer {
    /// Only Request and Followed Show events about those this User is a Requester of; `None` means
    /// every one's.
    requester: Option<i64>,
    /// Only playback events about this User's playing; `None` means every User's.
    player: Option<i64>,
    /// Whether they see where a Library's files are on the Server.
    sees_paths: bool,
}

/// What an event is about.
enum About<'a> {
    Library(i64),
    Request(&'a Request),
    FollowedShow(&'a FollowedShow),
    /// The User playing.
    Player(i64),
}

impl EventBus {
    /// A bus that also tells the Observers and Notifiers enabled in `plugins`, linking
    /// notifications to their Requests at `server_url`, and finding the Users' Plugin Connections
    /// in `db`.
    pub(crate) fn new(db: Arc<Database>, plugins: ExtensionPoints, server_url: ServerUrl) -> Self {
        Self::with_capacity(db, plugins, server_url, BUFFERED)
    }

    /// A bus on which subscribers may fall `capacity` events behind.
    pub(crate) fn with_capacity(
        db: Arc<Database>,
        plugins: ExtensionPoints,
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

    /// Publishes `events`, in order, to each subscriber, and those of the kinds the Plugin contract
    /// has as one batch to each Observer. Each Request event goes to every Notifier too.
    pub(crate) fn publish(&self, events: Vec<Event>) {
        if events.is_empty() {
            return;
        }
        let batch = plugins::observed(&events);
        for event in events {
            // Fails only when nobody is subscribed, which is fine.
            let _: Result<usize, _> = self.sender.send(event.clone());
            self.tell_notifiers(&event);
        }
        if let Some(batch) = batch {
            self.plugins.notify::<points::Events>(batch);
        }
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
        // A removed Request is no longer in the web app to link to.
        let request_url = (self.server_url.request_link(request.id)).filter(|_| !removed);
        let Some(notification) = plugins::notification(event, request, request_url) else {
            return;
        };
        let told = Arc::new((event.clone(), notification));
        let db = Arc::clone(&self.db);
        self.plugins
            .tell_each(move |plugin, notifier: Arc<dyn Notifier>| {
                deliver(Arc::clone(&db), plugin, notifier, Arc::clone(&told))
            });
    }

    /// The events published from now on that `user` may see and `filter` lets through, as `user`
    /// may see them, with a `Lagged` wherever the subscriber fell too far behind. Ends when the bus
    /// is closed.
    pub(crate) fn subscribe(
        &self,
        user: &User,
        filter: Filter,
    ) -> impl Stream<Item = SubscribeResponse> + Send + use<> {
        let receiver = self.sender.subscribe();
        // ponytail: Permissions as the stream opened, like any request's, so one who loses
        // `VIEW_REQUESTS` sees everyone's Request events until they resubscribe. End streams on a
        // Permission change if that matters.
        let viewer = Viewer::of(user);
        stream::unfold(
            (receiver, viewer, filter),
            |(mut receiver, viewer, filter)| async move {
                let message = next_message(&mut receiver, &viewer, &filter).await?;
                let response = SubscribeResponse {
                    message: Some(message),
                    ..Default::default()
                };
                Some((response, (receiver, viewer, filter)))
            },
        )
        .take_until(self.closed.clone().cancelled_owned())
    }

    /// [`Self::subscribe`] as an Admin, who sees every event whole.
    #[cfg(test)]
    pub(crate) fn subscribe_as_admin(
        &self,
        filter: Filter,
    ) -> impl Stream<Item = SubscribeResponse> + Send + use<> {
        let admin = User {
            id: 0,
            name: "Admin".to_owned(),
            permissions: crate::users::Permissions::ADMIN,
            created_at: 0,
        };
        self.subscribe(&admin, filter)
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
        let library = match about {
            About::Library(library) => Some(library),
            About::Request(_) | About::FollowedShow(_) | About::Player(_) => None,
        };
        (self.types.is_empty() || self.types.contains(&kind))
            && (self.libraries.is_empty()
                || library.is_some_and(|library| self.libraries.contains(&library)))
    }
}

/// The next event from `receiver` that `viewer` may see and `filter` lets through, as `viewer`
/// may see it, or a `Lagged` if events were dropped first. `None` once nothing more can be
/// published.
async fn next_message(
    receiver: &mut broadcast::Receiver<Event>,
    viewer: &Viewer,
    filter: &Filter,
) -> Option<subscribe_response::Message> {
    loop {
        match receiver.recv().await {
            Ok(event) if filter.lets_through(&event) => {
                if let Some(seen) = viewer.sees(event) {
                    return Some(seen.into());
                }
            }
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

/// Hands `told`'s notification to `plugin`'s `notifier` for its channel, and for the Plugin
/// Connection to `plugin` and the Personal settings for it of each User told about its event,
/// [`DELIVERING_AT_ONCE`] at a time. A User with neither is skipped, and one the Notifier finds
/// gone is disconnected (ADR 0021), or, for Personal settings, only logged (ADR 0046).
///
/// # Errors
///
/// Each recipient that failed, with the Notifier's error, that User's secret Personal settings
/// blanked. Reading Personal settings or reading or deleting Plugin Connections failing is logged,
/// and the others still told.
async fn deliver(
    db: Arc<Database>,
    plugin: PluginId,
    notifier: Arc<dyn Notifier>,
    told: Arc<(Event, Notification)>,
) -> Result<(), Undelivered> {
    let (event, notification) = &*told;
    let mut recipients = vec![(None, Recipient::Channel)];
    match connections::connected(&db, &plugin).await {
        Ok(connected) => recipients.extend(connections_to_tell(event, connected)),
        Err(err) => error!(%plugin, "could not read whom to message about a Request: {err}"),
    }
    let mut secrets = HashMap::new();
    match personal_settings::recipients(&db, &plugin).await {
        Ok(personal) => recipients.extend(personal_to_tell(event, personal, &mut secrets)),
        Err(err) => error!(%plugin, "could not read whose Personal settings to use: {err}"),
    }
    let deliveries = recipients.into_iter().map(|(user, recipient)| {
        let (db, plugin, notifier, secrets) = (&db, &plugin, &notifier, &secrets);
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
                (Err(error), Some(user), Recipient::User(_)) if error.is::<RecipientGone>() => {
                    // The User set these values themself; only they can change them.
                    info!(%plugin, user, "a Notifier says a User's Personal settings reach nobody");
                    None
                }
                (Err(error), user, _) => {
                    let theirs = user.and_then(|user| secrets.get(&user));
                    Some((
                        user,
                        without(error.to_string(), theirs.map_or(&[], Vec::as_slice)),
                    ))
                }
            }
        }
    });
    let failed: Vec<_> = stream::iter(deliveries)
        .buffer_unordered(DELIVERING_AT_ONCE)
        .filter_map(future::ready)
        .collect()
        .await;
    if failed.is_empty() {
        Ok(())
    } else {
        Err(Undelivered(failed))
    }
}

/// The recipients a Notifier didn't reach, each with its error: a User by id, or the channel.
struct Undelivered(Vec<(Option<i64>, String)>);

impl ProviderError<()> for Undelivered {
    fn failure(self) -> Failure {
        let each = self.0.into_iter().map(|(user, error)| match user {
            // By id, so that no log names a chat id.
            Some(user) => format!("to User {user}: {error}"),
            None => format!("to the channel: {error}"),
        });
        Failure {
            kind: (),
            message: each.collect::<Vec<_>>().join("; "),
        }
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

/// The Personal settings of each of `personal` that [`users_to_tell`] picks for `event`, with the
/// id of the User whose they are. Each picked User's secret values go in `secrets`.
fn personal_to_tell(
    event: &Event,
    personal: Vec<(User, BTreeMap<String, String>, Vec<String>)>,
    secrets: &mut HashMap<i64, Vec<String>>,
) -> Vec<(Option<i64>, Recipient)> {
    let users: Vec<User> = personal.iter().map(|(user, ..)| user.clone()).collect();
    let told = users_to_tell(event, &users);
    personal
        .into_iter()
        .filter(|(user, ..)| told.contains(&user.id))
        .map(|(user, values, theirs)| {
            secrets.insert(user.id, theirs);
            (Some(user.id), Recipient::User(values))
        })
        .collect()
}

/// `message` with each of `secrets` blanked.
fn without(mut message: String, secrets: &[String]) -> String {
    for secret in secrets.iter().filter(|secret| !secret.is_empty()) {
        message = message.replace(secret.as_str(), "…");
    }
    message
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
    use About::{FollowedShow, Library, Player, Request};
    use EventType as T;
    use event::Event as E;
    Some(match event.event.as_ref()? {
        E::ScanStarted(e) => (T::EVENT_TYPE_SCAN_STARTED, Library(e.library_id)),
        E::ScanProgress(e) => (T::EVENT_TYPE_SCAN_PROGRESS, Library(e.library_id)),
        E::ScanFinished(e) => (T::EVENT_TYPE_SCAN_FINISHED, Library(e.library_id)),
        E::LibraryItemAdded(e) => (T::EVENT_TYPE_LIBRARY_ITEM_ADDED, Library(e.library_id)),
        E::LibraryItemRemoved(e) => (T::EVENT_TYPE_LIBRARY_ITEM_REMOVED, Library(e.library_id)),
        E::ItemMatched(e) => (T::EVENT_TYPE_ITEM_MATCHED, Library(e.library_id)),
        E::MatchingFinished(e) => (T::EVENT_TYPE_MATCHING_FINISHED, Library(e.library_id)),
        E::ProbingFinished(e) => (T::EVENT_TYPE_PROBING_FINISHED, Library(e.library_id)),
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
        E::PlaybackStarted(e) => (
            T::EVENT_TYPE_PLAYBACK_STARTED,
            Player(e.playback.as_option()?.user_id),
        ),
        E::PlaybackEnded(e) => (
            T::EVENT_TYPE_PLAYBACK_ENDED,
            Player(e.playback.as_option()?.user_id),
        ),
        E::PlaybackChanged(e) => (
            T::EVENT_TYPE_PLAYBACK_CHANGED,
            Player(e.playback.as_option()?.user_id),
        ),
    })
}

impl Viewer {
    fn of(user: &User) -> Self {
        Self {
            requester: requests::visible_to(user),
            player: playback::visible_to(user),
            sees_paths: user.permissions.allow(Permission::ManageLibraries),
        }
    }

    /// `event` as this viewer may see it, or `None` if they may not see it at all. A Library event
    /// reaches everyone, as `ListLibraries` lists every Library, but with the path of a file left
    /// out for those who may not see `folders` there.
    fn sees(&self, mut event: Event) -> Option<Event> {
        let (_, about) = classify(&event)?;
        let visible = match about {
            About::Library(_) => true,
            About::Request(request) => requests::may_see(self.requester, &request.requesters),
            About::FollowedShow(followed) => {
                requests::may_see(self.requester, &followed.requesters)
            }
            About::Player(user) => self.player.is_none_or(|player| player == user),
        };
        if !visible {
            return None;
        }
        if !self.sees_paths
            && let Some(event::Event::LibraryItemAdded(added)) = &mut event.event
        {
            added.path.clear();
        }
        Some(event)
    }
}

/// The IDs of the Users told about `event`, so every Notifier shares the rules (ADR 0021): a
/// Request's Requesters when it is approved, declined, Failed, Partially Available or Available,
/// or cancelled by deleting media it waited on, and each of `users` with
/// `PERMISSION_MANAGE_REQUESTS` about a new Pending Request. Nobody else, and nobody for any other
/// event.
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
        Some(E::RequestCancelled(cancelled)) if cancelled.deleted_by.is_some() => {
            requesters(&cancelled.request)
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
#[path = "events_tests.rs"]
mod tests;
