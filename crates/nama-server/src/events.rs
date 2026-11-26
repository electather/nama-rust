//! The event bus: the Kernel publishes what happens on it, and it hands each event to every
//! client subscribed through `EventService.Subscribe` and to every Observer at
//! [`points::Events`], and each Request event to every [`Notifier`], for its channel and for the
//! Plugin Connections and Personal settings of the Users told about it, whose inbox it goes in too,
//! and from there to their browsers by Web Push. Each of them is sent an `InboxChanged` once it is
//! stored. Each Link the Server keeps or forgets goes to Observers and Notifiers alone, the Admins
//! told of it (ADR 0105).
//!
//! Publishing never waits for a subscriber. Each subscriber may fall up to [`BUFFERED`] events
//! behind; past that its oldest unread events are dropped, and it is sent a `Lagged` in their
//! place.

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use futures_util::{Stream, StreamExt, stream};
use nama_plugin_api::{
    notifier::{Notification, Notifier},
    points::{self, ObservedEvents},
};
use nama_plugin_proto::nama::plugin::v1 as wire;
use nama_proto::nama::v1::{
    Event, EventType, FollowedShow, InboxChanged, InboxNotification, Lagged, NotificationKind,
    Permission, Request, RequestState, SubscribeResponse, event, subscribe_response,
};
use tokio::{
    sync::{
        broadcast::{self, error::RecvError},
        mpsc,
    },
    task::{JoinError, JoinHandle, JoinSet},
};
use tokio_util::sync::CancellationToken;
use tracing::{error, warn};

use self::{
    notifying::{Audience, deliver},
    ratings::Ratings,
};
use crate::{
    content_ratings::MaximumAge,
    db::Database,
    link::{LinkChange, Linking, Whose},
    plugins::{self, ExtensionPoints},
    server_settings::{ServerUrl, web_push_key},
    users::{self, LibraryAccess, User, every_notification_turned_off, every_user, store_in_inbox},
    web_push::{Pusher, push_to_subscriptions},
};

mod notifying;
mod ratings;
pub(crate) use ratings::{Rated, Removed};

/// How many events a subscriber may fall behind before it lags. A scan publishes a batch of up to
/// one event per Item of [`crate::scanner`]'s batch of files at once, which this must hold.
const BUFFERED: usize = 4096;

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
    /// The one task that stores personal notifications in inboxes, so they are stored in the
    /// order they were published, and pushes them to their Users' browsers.
    inboxes: Arc<Mutex<Inboxes>>,
    /// The Requests and Followed Shows removed last, oldest first, [`BUFFERED`] at most. Each
    /// removal publishes an event after it, so an event about one removed before those is one
    /// every subscriber has read or lagged past. Their ids are never reused (`AUTOINCREMENT`), so
    /// a rating kept here is never another row's.
    removed: Arc<Mutex<VecDeque<Removed>>>,
}

/// The task that stores personal notifications in inboxes and pushes them, as
/// [`EventBus::publish`] feeds it.
#[derive(Debug, Default)]
enum Inboxes {
    /// Started by the first personal notification, so building an [`EventBus`] needs no runtime.
    #[default]
    NotStarted,
    Storing(mpsc::UnboundedSender<Event>, JoinHandle<()>),
    /// [`EventBus::shutdown`] stopped it: the database is about to close.
    Stopped,
}

/// What a subscriber asked to narrow their events to. It only narrows what their [`Viewer`] sees.
#[derive(Debug, Clone, Default)]
pub(crate) struct Filter {
    /// Only these types; empty means every type.
    pub(crate) types: Vec<EventType>,
    /// Only about these Libraries; empty means every Library, and events about none.
    pub(crate) libraries: Vec<i64>,
}

/// Which events a subscriber may see, and what of them, from their Permissions and the Libraries
/// they see. Only this module decides that (see the crate's `AGENTS.md`, "One owner for who sees
/// an Event").
#[derive(Debug)]
struct Viewer {
    /// Who they are, who alone sees their inbox's events, and who gets events about their own
    /// Requests and Followed Shows whatever [`Self::max_age`] says, as the list shows them those.
    user: i64,
    /// Only Library events about these Libraries.
    libraries: LibraryAccess,
    /// Only Request and Followed Show events about those this User is a Requester of; `None` means
    /// every one's.
    requester: Option<i64>,
    /// Only events about others' Requests and Followed Shows, and `ItemMatched` events, of titles
    /// this lets through.
    max_age: MaximumAge,
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
    /// The User whose inbox it is.
    Inbox(i64),
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
            inboxes: Arc::default(),
            removed: Arc::default(),
        }
    }

    /// Keeps the Content Rating of each of `removed`, which a write is removing, for subscribers
    /// to judge events about them by once their rows are gone. Called inside that write, before it
    /// commits, so no subscriber finds a row gone before its rating is kept here.
    pub(crate) fn removing(&self, removed: Vec<Removed>) {
        let mut kept = lock(&self.removed);
        kept.extend(removed);
        let over = kept.len().saturating_sub(BUFFERED);
        kept.drain(..over);
    }

    /// Publishes `events`, in order, to each subscriber, and those of the kinds the Plugin contract
    /// has as one batch to each Observer. Each Request event goes to every Notifier too, and each
    /// personal notification in the inbox of each User told.
    pub(crate) fn publish(&self, events: Vec<Event>) {
        if events.is_empty() {
            return;
        }
        let batch = plugins::observed(&events);
        self.queue_for_inboxes(&events);
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
        let Some((_, About::Request(request))) = classify(event) else {
            return;
        };
        let request_url = request_link(&self.server_url, event);
        let Some(notification) = plugins::notification(event, request, request_url) else {
            return;
        };
        self.tell_each_notifier(Audience::Request(event.clone()), notification);
    }

    /// Hands `notification` to every Notifier, for its channel and for whom `audience` picks.
    fn tell_each_notifier(&self, audience: Audience, notification: Notification) {
        let told = Arc::new((audience, notification));
        let db = Arc::clone(&self.db);
        self.plugins
            .tell_each(move |plugin, notifier: Arc<dyn Notifier>| {
                deliver(Arc::clone(&db), plugin, notifier, Arc::clone(&told))
            });
    }

    /// Tells Plugins of each Link `linking` keeps or forgets from now on, until it keeps no more.
    pub(crate) fn announce_links(&self, linking: &Linking) -> JoinHandle<()> {
        tokio::spawn(self.clone().announce_each(linking.follow_changes()))
    }

    /// [`EventBus::announce_links`]'s loop over the Link changes `changes` brings.
    async fn announce_each(self, mut changes: broadcast::Receiver<LinkChange>) {
        loop {
            match changes.recv().await {
                Ok(change) => self.announce(change).await,
                Err(RecvError::Lagged(missed)) => {
                    warn!(
                        missed,
                        "Plugins weren't told of some Links kept or forgotten"
                    );
                }
                Err(RecvError::Closed) => return,
            }
        }
    }

    /// Hands `change` to every Observer and every Notifier, for its channel and the Admins. Never
    /// to a client: the client API has no twin of it, as Settings shows each Link.
    async fn announce(&self, change: LinkChange) {
        let Some(event) = link_event(&self.db, change).await else {
            return;
        };
        let batch = ObservedEvents {
            events: vec![event.clone()],
            ..Default::default()
        };
        self.plugins.notify::<points::Events>(batch);
        let notification = Notification {
            event: event.into(),
            ..Default::default()
        };
        self.tell_each_notifier(Audience::Admins, notification);
    }

    /// Queues each of `events` that makes a personal notification for the inbox of each User
    /// [`users_to_tell`] picks, whether or not a Notifier reaches them.
    fn queue_for_inboxes(&self, events: &[Event]) {
        let mut personal = (events.iter())
            .filter(|event| notification_kind(event).is_some())
            .peekable();
        if personal.peek().is_none() {
            return;
        }
        // Held while sending, so two `publish`es at once queue their events in one order.
        let mut inboxes = lock(&self.inboxes);
        if matches!(*inboxes, Inboxes::NotStarted) {
            // Unbounded, as `publish` can't wait; it carries one event per Request change.
            let (sender, receiver) = mpsc::unbounded_channel();
            let storing = tokio::spawn(store_and_push_each(
                Arc::clone(&self.db),
                self.server_url.clone(),
                self.sender.clone(),
                receiver,
            ));
            *inboxes = Inboxes::Storing(sender, storing);
        }
        let Inboxes::Storing(sender, _) = &*inboxes else {
            return;
        };
        for event in personal {
            // Fails only if the task panicked, which `shutdown` logs.
            let _: Result<(), _> = sender.send(event.clone());
        }
    }

    /// Stores and pushes the personal notifications published so far and stops taking any more:
    /// once nothing publishes, and before the database closes, as the task holds it.
    pub(crate) async fn shutdown(&self) {
        let stopped = std::mem::replace(&mut *lock(&self.inboxes), Inboxes::Stopped);
        let Inboxes::Storing(sender, storing) = stopped else {
            return;
        };
        drop(sender);
        if let Err(err) = storing.await
            && err.is_panic()
        {
            error!("storing notifications in inboxes panicked: {err}");
        }
    }

    /// Every event published from now on, and each `InboxChanged`, whole, for a Kernel task that
    /// acts on them, such as pulling Watch State after a scan; one that falls [`BUFFERED`] behind
    /// misses the oldest. Never for a client, who sees only what [`Self::subscribe`] lets through.
    pub(crate) fn follow(&self) -> broadcast::Receiver<Event> {
        self.sender.subscribe()
    }

    /// The events published from now on that `user`, who sees `libraries` and has the Maximum Age
    /// `max_age`, may see and `filter` lets through, as `user` may see them, with a `Lagged`
    /// wherever the subscriber fell too far behind. Ends when the bus is closed.
    pub(crate) fn subscribe(
        &self,
        user: &User,
        libraries: LibraryAccess,
        max_age: MaximumAge,
        filter: Filter,
    ) -> impl Stream<Item = SubscribeResponse> + Send + use<> {
        // Fixed for the stream's life: `api::events` ends it when its User's Permissions, Library
        // access or Maximum Age change.
        self.stream(Viewer::of(user, libraries, max_age), filter)
    }

    /// [`Self::subscribe`]'s stream, for `viewer`.
    fn stream(
        &self,
        viewer: Viewer,
        filter: Filter,
    ) -> impl Stream<Item = SubscribeResponse> + Send + use<> {
        let receiver = self.sender.subscribe();
        let ratings = Ratings {
            db: Arc::clone(&self.db),
            removed: Arc::clone(&self.removed),
        };
        stream::unfold(
            (receiver, viewer, filter, ratings),
            |(mut receiver, viewer, filter, ratings)| async move {
                let message = next_message(&mut receiver, &viewer, &filter, &ratings).await?;
                let response = SubscribeResponse {
                    message: Some(message),
                    ..Default::default()
                };
                Some((response, (receiver, viewer, filter, ratings)))
            },
        )
        .take_until(self.closed.clone().cancelled_owned())
    }

    /// [`Self::subscribe`] as an Admin, who sees every event whole but another User's
    /// `InboxChanged`.
    #[cfg(test)]
    pub(crate) fn subscribe_as_admin(
        &self,
        filter: Filter,
    ) -> impl Stream<Item = SubscribeResponse> + Send + use<> {
        let admin = User {
            id: 0,
            name: "Admin".to_owned(),
            permissions: users::Permissions::ADMIN,
            created_at: 0,
        };
        let viewer = Viewer::of(&admin, LibraryAccess::All, MaximumAge::default());
        self.stream(viewer, filter)
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
            About::Request(_) | About::FollowedShow(_) | About::Player(_) | About::Inbox(_) => None,
        };
        (self.types.is_empty() || self.types.contains(&kind))
            && (self.libraries.is_empty()
                || library.is_some_and(|library| self.libraries.contains(&library)))
    }
}

/// The next event from `receiver` that `viewer` may see and `filter` lets through, as `viewer`
/// may see it, or a `Lagged` if events were dropped first. `None` once nothing more can be
/// published. Reads the Content Ratings `viewer`'s Maximum Age asks for from `ratings`.
async fn next_message(
    receiver: &mut broadcast::Receiver<Event>,
    viewer: &Viewer,
    filter: &Filter,
    ratings: &Ratings,
) -> Option<subscribe_response::Message> {
    loop {
        match receiver.recv().await {
            Ok(event) if filter.lets_through(&event) => {
                if let Some(seen) = viewer.sees(event)
                    && viewer.allows_title_of(ratings, &seen).await
                {
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

/// [`store_in_inboxes`] for each event `published` sends, in order, logging each that fails, then
/// tells each User it stored one for with an `InboxChanged` on `subscribers`, and pushes what it
/// stored to their browsers, linking to the Requests at `server_url`. Each event's pushes run on
/// their own, so a slow push service holds up no other notification.
async fn store_and_push_each(
    db: Arc<Database>,
    server_url: ServerUrl,
    subscribers: broadcast::Sender<Event>,
    mut published: mpsc::UnboundedReceiver<Event>,
) {
    let mut pusher = None;
    let mut pushing = JoinSet::new();
    while let Some(happened) = published.recv().await {
        let stored = match store_in_inboxes(&db, &happened).await {
            Ok(stored) if !stored.is_empty() => stored,
            Ok(_) => continue,
            Err(err) => {
                error!("could not store a Request notification in the inbox: {err}");
                continue;
            }
        };
        // Straight on the channel: this task holds no `EventBus` to `publish` through, and no
        // Observer or Notifier needs it.
        for &(user_id, _) in &stored {
            let changed = InboxChanged {
                user_id,
                ..Default::default()
            };
            // Fails only when nobody is subscribed, which is fine.
            let _: Result<usize, _> = subscribers.send(event(changed));
        }
        if pusher.is_none() {
            pusher = make_pusher(&db).await;
        }
        if let Some(pusher) = pusher.clone() {
            let link = request_link(&server_url, &happened);
            let db = Arc::clone(&db);
            pushing.spawn(async move { push_to_subscriptions(&db, &pusher, stored, link).await });
        }
        while let Some(pushed) = pushing.try_join_next() {
            log_panic(pushed);
        }
    }
    // Before the database closes: a push deletes a subscription that's gone.
    while let Some(pushed) = pushing.join_next().await {
        log_panic(pushed);
    }
}

fn log_panic(pushed: Result<(), JoinError>) {
    if let Err(err) = pushed {
        error!("pushing notifications panicked: {err}");
    }
}

/// What pushes signed with the Server's VAPID key, or `None`, logged, if it can't be read or made.
async fn make_pusher(db: &Database) -> Option<Pusher> {
    let key = (web_push_key(db).await)
        .inspect_err(|err| {
            error!("could not read the Server's Web Push key, so none pushed: {err}");
        })
        .ok()?;
    (Pusher::new(key))
        .inspect_err(|err| error!("could not start Web Push, so none pushed: {err}"))
        .ok()
}

/// Stores `event`, if it makes a personal notification, in the inbox of each User
/// [`users_to_tell`] picks, and returns each notification stored with its User.
async fn store_in_inboxes(
    db: &Database,
    event: &Event,
) -> Result<Vec<(i64, InboxNotification)>, sqlx::Error> {
    let Some((kind, request)) = notification_kind(event) else {
        return Ok(Vec::new());
    };
    let users = every_user(db).await?;
    let turned_off = match every_notification_turned_off(db).await {
        Ok(turned_off) => turned_off,
        // As `deliver` does: keeping a kind someone turned off beats missing one they want.
        Err(err) => {
            error!("could not read who turned notifications off, so stored for all: {err}");
            Vec::new()
        }
    };
    let told = users_to_tell(event, &users, &turned_off);
    store_in_inbox(db, told, kind, request).await
}

/// Where the web app lists `event`'s Request, at `server_url`, or `None` once the event removed
/// it, as it's then no longer there to link to.
fn request_link(server_url: &ServerUrl, event: &Event) -> Option<String> {
    let removed = matches!(
        classify(event),
        Some((
            EventType::EVENT_TYPE_REQUEST_CANCELLED | EventType::EVENT_TYPE_REQUEST_WITHDRAWN,
            _
        ))
    );
    server_url.requests_link().filter(|_| !removed)
}

/// The contract's Event for `change`, naming its User as `db` has them, or `None` once they are
/// gone or, logged, can't be read.
async fn link_event(db: &Database, change: LinkChange) -> Option<wire::Event> {
    let happened: wire::event::Event = match change {
        LinkChange::Linked(Whose::Server) => wire::ServerLinked::default().into(),
        LinkChange::Unlinked(Whose::Server) => wire::ServerUnlinked::default().into(),
        LinkChange::Linked(Whose::User(user_id)) => wire::UserLinked {
            user_id,
            name: name_of(db, user_id).await?,
            ..Default::default()
        }
        .into(),
        LinkChange::Unlinked(Whose::User(user_id)) => wire::UserUnlinked {
            user_id,
            name: name_of(db, user_id).await?,
            ..Default::default()
        }
        .into(),
    };
    Some(wire::Event {
        event: Some(happened),
        ..Default::default()
    })
}

/// The name of the User `user_id`, or `None` once they are gone or, logged, `db` can't be read.
async fn name_of(db: &Database, user_id: i64) -> Option<String> {
    let users = (every_user(db).await)
        .inspect_err(|err| error!(user = user_id, "could not read whose Link changed: {err}"))
        .ok()?;
    let named = users.into_iter().find(|user| user.id == user_id);
    named.map(|user| user.name)
}

/// An event's type and what it is about.
fn classify(event: &Event) -> Option<(EventType, About<'_>)> {
    use About::{FollowedShow, Inbox, Library, Player, Request};
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
        E::InboxChanged(e) => (T::EVENT_TYPE_INBOX_CHANGED, Inbox(e.user_id)),
    })
}

impl Viewer {
    fn of(user: &User, libraries: LibraryAccess, max_age: MaximumAge) -> Self {
        Self {
            user: user.id,
            libraries,
            requester: user.whose_requests(),
            max_age,
            player: user.whose_playing(),
            sees_paths: user.permissions.allow(Permission::ManageLibraries),
        }
    }

    /// Whether this viewer's Maximum Age lets `event` reach them: one about another User's Request
    /// or Followed Show, or any `ItemMatched`, of a title over it doesn't, as their lists leave
    /// those out. One whose rating can't be read, logged, doesn't either.
    async fn allows_title_of(&self, ratings: &Ratings, event: &Event) -> bool {
        if self.max_age.lets_every_title() {
            return true;
        }
        if let Some(event::Event::ItemMatched(matched)) = &event.event {
            return match ratings.of_match(matched.item_id, &matched.title).await {
                Ok(rating) => self.max_age.allows(rating.as_deref()),
                Err(err) => {
                    warn!(
                        item = matched.item_id,
                        "a subscriber with a Maximum Age wasn't told of a Match whose Content \
                         Rating couldn't be read: {err}"
                    );
                    false
                }
            };
        }
        let row = match classify(event) {
            Some((_, About::Request(request)))
                if !users::is_requester(self.user, &request.requesters) =>
            {
                Rated::Request(request.id)
            }
            Some((_, About::FollowedShow(followed)))
                if !users::is_requester(self.user, &followed.requesters) =>
            {
                Rated::FollowedShow(followed.id)
            }
            _ => return true,
        };
        match ratings.of(row).await {
            Ok(Some(rating)) => self.max_age.allows(rating.as_deref()),
            Ok(None) => false,
            Err(err) => {
                error!("could not read the Content Rating of {row:?} for a subscriber: {err}");
                false
            }
        }
    }

    /// `event` as this viewer may see it, or `None` if they may not see it at all. A Library event
    /// reaches those who see its Library, as `ListLibraries` lists it to them, but with the path
    /// of a file left out for those who may not see `folders` there.
    fn sees(&self, mut event: Event) -> Option<Event> {
        let (_, about) = classify(&event)?;
        let visible = match about {
            About::Library(library) => self.libraries.sees(library),
            About::Request(request) => users::may_see(self.requester, &request.requesters),
            About::FollowedShow(followed) => users::may_see(self.requester, &followed.requesters),
            About::Player(user) => self.player.is_none_or(|player| player == user),
            // Admins included: an inbox is its User's alone.
            About::Inbox(user) => self.user == user,
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
/// Request's Requesters for each [`notification_kind`] but a new Pending Request, and each of
/// `users` with `PERMISSION_MANAGE_REQUESTS` for that, but for those `turned_off` lists with that
/// kind. Nobody for any other event.
pub(crate) fn users_to_tell(
    event: &Event,
    users: &[User],
    turned_off: &[(i64, NotificationKind)],
) -> Vec<i64> {
    let Some((kind, request)) = notification_kind(event) else {
        return Vec::new();
    };
    let candidates = if kind == NotificationKind::NewPending {
        users
            .iter()
            .filter(|user| user.permissions.allow(Permission::ManageRequests))
            .map(|user| user.id)
            .collect()
    } else {
        requesters(request)
    };
    candidates
        .into_iter()
        .filter(|&user| !turned_off.contains(&(user, kind)))
        .collect()
}

/// The kind of personal notification `event` makes, if any, with its Request: a Request approved,
/// declined, Failed, Partially Available or Available, cancelled by deleting media it waited on,
/// or new and Pending.
fn notification_kind(event: &Event) -> Option<(NotificationKind, &Request)> {
    use RequestState as S;
    use event::Event as E;
    match event.event.as_ref() {
        Some(E::RequestCreated(created)) if created.request.state == S::Pending => {
            Some((NotificationKind::NewPending, &created.request))
        }
        // A scan opens a Followed Show's new Season already (Partially) Available. One opened
        // Approved tells nobody: its one Requester just asked for it.
        Some(E::RequestCreated(created))
            if matches!(
                created.request.state.as_known(),
                Some(S::PartiallyAvailable | S::Available)
            ) =>
        {
            let kind = moved_to_kind(created.request.state.as_known()?)?;
            Some((kind, &created.request))
        }
        Some(E::RequestStateChanged(changed)) => {
            let kind = moved_to_kind(changed.request.state.as_known()?)?;
            Some((kind, &changed.request))
        }
        Some(E::RequestCancelled(cancelled)) if cancelled.deleted_by.is_some() => {
            Some((NotificationKind::CancelledByDeletion, &cancelled.request))
        }
        _ => None,
    }
}

/// The kind telling Requesters their Request moved to `state`, if any.
const fn moved_to_kind(state: RequestState) -> Option<NotificationKind> {
    use NotificationKind as K;
    use RequestState as S;
    match state {
        S::Approved => Some(K::Approved),
        S::Declined => Some(K::Declined),
        S::Failed => Some(K::Failed),
        S::PartiallyAvailable => Some(K::PartiallyAvailable),
        S::Available => Some(K::Available),
        S::Unspecified | S::Pending | S::Processing => None,
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

// No code that could panic runs while one is held, so a poisoned lock's value is still whole.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
#[path = "events/events_tests.rs"]
mod tests;
