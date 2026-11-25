//! When a Linked User's Watch State, Track Preferences and Watchlist move between the Server and
//! the Hub, through the Kernel's [`Linking`] (ADR 0008, ADR 0056, ADR 0092, ADR 0105).
//!
//! Each trigger is one call on [`Syncing`], and what each runs is decided only here:
//!
//! | Trigger                                | Pushes                    | Then pulls                | Then syncs the Watchlist |
//! |----------------------------------------|---------------------------|---------------------------|--------------------------|
//! | A User signed in                       |                           | Watch State, Preferences  | yes                      |
//! | A User's Link is new                   | Watch State, Preferences  | Watch State, Preferences  | yes                      |
//! | The Server's Link is new               | every User's of both      |                           |                          |
//! | A User changed their Watchlist         |                           |                           | yes                      |
//! | Each scan's Matching                   |                           | each User's Watch State   | each User's              |
//! | Every minute                           | what waits for the Hub    |                           | each User's that failed  |
//!
//! A Link is new when a look finds it Linked that the last didn't, or first since the Server
//! started, as [`Linking`] tells it, so a restart pushes once more what a missed push left out. A
//! push queues what it pushes for the Hub, as the owners of Watch State and Track Preferences
//! queue each change in the write that makes it. Every pull sends what waits first, so a push comes
//! before its pull, and the minute's send takes the rest, such as what waited while the Hub was
//! unreachable. A Watchlist is synced whole, so one whose sync failed is synced again each minute
//! until it goes; a restart forgets which, as the first look at each Link syncs it anyway. A
//! Server with no Plugin that links its Users does no sync work at all but that minute's send,
//! which forgets what waits, as none of its Users can be Linked.

use std::{
    collections::HashSet,
    future::Future,
    mem,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use nama_proto::nama::v1::{Event, event};
use tokio::{
    sync::broadcast::{self, error::RecvError},
    task::JoinHandle,
    time::MissedTickBehavior,
};
use tracing::{Instrument, Span};

use crate::{
    browse::Browse,
    db::Database,
    diagnostics,
    events::EventBus,
    link::{self, Linking, Whose},
    playback::TrackPreferences,
    plugins::ExtensionPoints,
    users::Users,
    watchlist,
};

/// How often what waits for the Hub is sent.
const SEND_EVERY: Duration = Duration::from_mins(1);

/// Runs each trigger's sync with the Hub.
#[derive(Debug, Clone)]
pub(crate) struct Syncing {
    linking: Linking,
    browse: Browse,
    preferences: TrackPreferences,
    db: Arc<Database>,
    users: Users,
    /// The Users whose last Watchlist sync failed.
    watchlists_unsynced: Arc<Mutex<HashSet<i64>>>,
}

impl Syncing {
    /// Syncs what `db` holds of `users`, telling the Observers of `points` of what it takes.
    pub(crate) fn new(db: Arc<Database>, points: ExtensionPoints, users: Users) -> Self {
        Self {
            browse: Browse::new(Arc::clone(&db), points.clone()),
            preferences: TrackPreferences::new(Arc::clone(&db), points),
            linking: users.linking().clone(),
            db,
            users,
            watchlists_unsynced: Arc::default(),
        }
    }

    /// Sends what waits for the Hub every [`SEND_EVERY`], and syncs again each Watchlist whose
    /// sync failed, until the task is aborted. A failure is logged: what didn't go waits for the
    /// next.
    pub(crate) fn keep_sending(&self) -> JoinHandle<()> {
        let sync = self.clone();
        tokio::spawn(async move {
            let mut ticks = tokio::time::interval(SEND_EVERY);
            // A send slower than a minute is followed by one more, not by the ticks it missed.
            ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
            loop {
                ticks.tick().await;
                let span = diagnostics::following_current(
                    tracing::info_span!(parent: None, "send_to_hub"),
                );
                sync.send_waiting().instrument(span).await;
            }
        })
    }

    /// [`Syncing::keep_sending`]'s one send.
    async fn send_waiting(&self) {
        let _: Result<(), link::Error> = self.linking.send_queued().await;
        let unsynced = mem::take(&mut *self.unsynced_watchlists());
        for user_id in unsynced {
            self.sync_watchlist(user_id).await;
        }
    }

    /// [`Syncing::watchlists_unsynced`], locked.
    fn unsynced_watchlists(&self) -> MutexGuard<'_, HashSet<i64>> {
        (self.watchlists_unsynced.lock()).unwrap_or_else(PoisonError::into_inner)
    }

    /// The User `user_id` signed in: pulls their Watch State and Track Preferences, then syncs
    /// their Watchlist. Must be called within a Tokio runtime.
    pub(crate) fn signed_in(&self, user_id: i64) {
        let span = tracing::info_span!(parent: None, "sync_after_sign_in");
        self.soon(span, move |sync| async move {
            sync.pull_watch_states(user_id).await;
            sync.pull_track_preferences(user_id).await;
            sync.sync_watchlist(user_id).await;
        });
    }

    /// Follows each new Link, pushing what it says, until the task is aborted. Started before
    /// serving, so it hears the first look.
    pub(crate) fn follow_links(&self) -> JoinHandle<()> {
        tokio::spawn(self.clone().push_each_new_link(self.linking.follow()))
    }

    /// [`Syncing::follow_links`]'s loop over the new Links `followed`.
    async fn push_each_new_link(self, mut followed: broadcast::Receiver<Whose>) {
        loop {
            match followed.recv().await {
                Ok(Whose::Server) => self.server_linked(),
                Ok(Whose::User(user_id)) => self.user_linked(user_id),
                // ponytail: a Link missed here is pushed at the first look after the next restart;
                // list every Linked User here if Links ever come faster than pushes.
                Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => return,
            }
        }
    }

    /// The User `user_id`'s Link is new: pushes their whole Watch State and Track Preferences,
    /// dated as stored, since what they did before they Linked was never pushed, then pulls both,
    /// then syncs their Watchlist.
    fn user_linked(&self, user_id: i64) {
        let span = tracing::info_span!(parent: None, "sync_after_user_link");
        self.soon(span, move |sync| async move {
            sync.push_watch_states(user_id).await;
            sync.push_track_preferences(user_id).await;
            sync.pull_watch_states(user_id).await;
            sync.pull_track_preferences(user_id).await;
            sync.sync_watchlist(user_id).await;
        });
    }

    /// The Server's Link is new: pushes every User's Watch State and Track Preferences, dated as
    /// stored.
    fn server_linked(&self) {
        let span = tracing::info_span!(parent: None, "push_after_server_link");
        self.soon(span, move |sync| async move {
            let listed = match sync.users.list().await {
                Ok(listed) => listed,
                Err(err) => {
                    diagnostics::failed_with(&err, &Span::current());
                    tracing::error!(
                        "listing Users to push their Watch State and Track Preferences failed: \
                         {err}"
                    );
                    return;
                }
            };
            for user in listed {
                sync.push_watch_states(user.id).await;
                sync.push_track_preferences(user.id).await;
            }
        });
    }

    /// The User `user_id` changed their Watchlist: syncs it. Must be called within a Tokio
    /// runtime.
    pub(crate) fn watchlist_changed(&self, user_id: i64) {
        let span = tracing::info_span!(parent: None, "sync_watchlist");
        self.soon(span, move |sync| async move {
            sync.sync_watchlist(user_id).await;
        });
    }

    /// Follows each scan's Matching, after which it pulls every User's Watch State, so a title a
    /// scan added shows what they did with it on their other Servers (ADR 0008), and syncs their
    /// Watchlist, so a sync that failed is tried again (ADR 0056), until the task is aborted.
    /// Started before the scans, so it sees their Matching finish.
    pub(crate) fn follow_scans(&self, events: &EventBus) -> JoinHandle<()> {
        tokio::spawn(self.clone().pull_after_each_scan(events.follow()))
    }

    /// [`Syncing::follow_scans`]'s loop over the events `followed`.
    async fn pull_after_each_scan(self, mut followed: broadcast::Receiver<Event>) {
        loop {
            match followed.recv().await {
                Ok(Event {
                    event: Some(event::Event::MatchingFinished(_)),
                    ..
                })
                // The events it missed may have held a scan's.
                | Err(RecvError::Lagged(_)) => {}
                Ok(_) => continue,
                Err(RecvError::Closed) => return,
            }
            if self.linking.has_hub() {
                self.pull_every_user().await;
            }
        }
    }

    /// Runs `work` in a task of its own, in `span`, so a slow or unreachable service holds up
    /// nothing, unless the Server's config names no Hub.
    fn soon<F>(&self, span: Span, work: impl FnOnce(Self) -> F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        if self.linking.has_hub() {
            let span = diagnostics::following_current(span);
            tokio::spawn(work(self.clone()).instrument(span));
        }
    }

    /// Pulls each User's Watch State and syncs their Watchlist, one User at a time, as each reads
    /// every Library.
    async fn pull_every_user(&self) {
        let listed = match self.users.list().await {
            Ok(listed) => listed,
            Err(err) => {
                diagnostics::failed_with(&err, &Span::current());
                tracing::error!("listing Users to sync them failed: {err}");
                return;
            }
        };
        for user in listed {
            self.pull_watch_states(user.id).await;
            self.sync_watchlist(user.id).await;
        }
    }

    /// Queues the User `user_id`'s whole Watch State for the Hub, each change dated as stored, so a
    /// pull after it sends it first. It goes only for a Linked User, and the later of a title's
    /// changes wins (ADR 0008).
    async fn push_watch_states(&self, user_id: i64) {
        let changes = match self.browse.watch_state_changes(user_id).await {
            Ok(changes) => changes,
            Err(err) => {
                diagnostics::failed_with(&err, &Span::current());
                tracing::error!(user = user_id, "reading a User's Watch State failed: {err}");
                return;
            }
        };
        let queued = (self.db)
            .write(move |tx| {
                Box::pin(async move { link::queue_watch_states(tx, user_id, &changes).await })
            })
            .await;
        if let Err(err) = queued {
            tracing::error!(user = user_id, "queuing a User's Watch State failed: {err}");
        }
    }

    /// Queues each of the User `user_id`'s Track Preferences for the Hub, dated as stored, as
    /// [`Syncing::push_watch_states`] does their Watch State, and for the same reasons. The change
    /// made last of each wins (ADR 0092).
    async fn push_track_preferences(&self, user_id: i64) {
        let stored = match self.preferences.changes_of_user(user_id).await {
            Ok(stored) => stored,
            Err(err) => {
                diagnostics::failed_with(&err, &Span::current());
                tracing::error!(
                    user = user_id,
                    "reading a User's Track Preferences failed: {err}"
                );
                return;
            }
        };
        let queued = (self.db)
            .write(move |tx| {
                Box::pin(async move { link::queue_track_preferences(tx, user_id, &stored).await })
            })
            .await;
        if let Err(err) = queued {
            tracing::error!(
                user = user_id,
                "queuing a User's Track Preferences failed: {err}"
            );
        }
    }

    /// Sends what waits for the Hub, so a push comes before its pull: whether it all went. A
    /// failure is logged by the Link.
    async fn sent(&self) -> bool {
        self.linking.send_queued().await.is_ok()
    }

    /// Takes the User `user_id`'s Watch State from the Hub, where it is newer than the Server's
    /// own, once what waits for it is sent. A failure is logged: the next sign-in or scan pulls
    /// again.
    async fn pull_watch_states(&self, user_id: i64) {
        if !self.sent().await {
            return;
        }
        let Some(pulled) = self.linking.watch_states(user_id).await else {
            return;
        };
        if let Err(err) = self.browse.take_pulled(user_id, pulled).await {
            diagnostics::failed_with(&err, &Span::current());
            tracing::error!(
                user = user_id,
                "taking a User's pulled Watch State failed: {err}"
            );
        }
    }

    /// Takes the User `user_id`'s Track Preferences from the Hub, each where it is newer than the
    /// Server's own, once what waits for it is sent. A failure is logged: the next sign-in pulls
    /// again.
    async fn pull_track_preferences(&self, user_id: i64) {
        if !self.sent().await {
            return;
        }
        let Some(pulled) = self.linking.track_preferences(user_id).await else {
            return;
        };
        if let Err(err) = self.preferences.take_pulled(user_id, pulled).await {
            diagnostics::failed_with(&err, &Span::current());
            tracing::error!(
                user = user_id,
                "taking a User's pulled Track Preferences failed: {err}"
            );
        }
    }

    /// Hands the User `user_id`'s whole Watchlist to the Hub, and takes what it answers where that
    /// is newer than the Server's own. A failure is logged, and the next minute's send syncs again,
    /// sending what this didn't.
    async fn sync_watchlist(&self, user_id: i64) {
        let entries = match watchlist::entries(&self.db, user_id).await {
            Ok(entries) => entries,
            Err(err) => {
                diagnostics::failed_with(&err, &Span::current());
                tracing::error!(
                    user = user_id,
                    "reading a User's Watchlist to sync failed: {err}"
                );
                return;
            }
        };
        let synced = match self.linking.sync_watchlist(user_id, entries).await {
            Ok(Some(synced)) => synced,
            Ok(None) => return,
            Err(_logged) => {
                self.unsynced_watchlists().insert(user_id);
                return;
            }
        };
        if let Err(err) = watchlist::take_synced(&self.db, user_id, synced).await {
            diagnostics::failed_with(&err, &Span::current());
            tracing::error!(
                user = user_id,
                "taking a User's synced Watchlist failed: {err}"
            );
        }
    }
}
