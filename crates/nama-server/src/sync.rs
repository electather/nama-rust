//! When a Linked User's Watch State, Track Preferences and Watchlist move between the Server and
//! the enabled Plugin that links it, such as the Hub Plugin (ADR 0008, ADR 0056, ADR 0092).
//!
//! Each trigger is one call on [`Syncing`], and what each runs is decided only here:
//!
//! | Trigger                                | Pushes                    | Then pulls                | Then syncs the Watchlist |
//! |----------------------------------------|---------------------------|---------------------------|--------------------------|
//! | A User signed in                       |                           | Watch State, Preferences  | yes                      |
//! | A User's Link read or started: Linked  | Watch State, Preferences  | Watch State, Preferences  | yes                      |
//! | The Server's Link read: Linked         | every User's of both      |                           |                          |
//! | A User changed their Watchlist         |                           |                           | yes                      |
//! | Each scan's Matching                   |                           | each User's Watch State   | each User's              |
//!
//! A push comes before its pull, every call to the Plugin gives up after [`SYNC_TIMEOUT`], and a
//! Server no Plugin links does no sync work at all. Telling Observers of a User's own changes is
//! for the Watch State and Track Preference owners, not this.

use std::{future::Future, sync::Arc, time::Duration};

use nama_plugin_api::{
    PluginError,
    points::{
        TrackPreferenceChange, TrackPreferenceChanged, WatchStateChanges, WatchStatesChanged,
    },
    server_link::ServerLinker,
};
use nama_proto::nama::v1::{Event, event};
use tokio::{
    sync::broadcast::{self, error::RecvError},
    task::JoinHandle,
};
use tracing::{Instrument, Span};

use crate::{
    browse::Browse, db::Database, diagnostics, events::EventBus, playback::TrackPreferences,
    plugins::ExtensionPoints, users::Users, watchlist,
};

/// How long a call to the Plugin that links the Server may take, and how long a pull waits for
/// Observers to queue the push before it.
const SYNC_TIMEOUT: Duration = Duration::from_mins(1);

/// Runs each trigger's sync with the enabled Plugin that links the Server.
#[derive(Debug, Clone)]
pub(crate) struct Syncing {
    points: ExtensionPoints,
    browse: Browse,
    preferences: TrackPreferences,
    db: Arc<Database>,
    users: Users,
}

impl Syncing {
    /// Syncs what `db` holds of `users` through the Plugins of `points`.
    pub(crate) fn new(db: Arc<Database>, points: ExtensionPoints, users: Users) -> Self {
        Self {
            browse: Browse::new(Arc::clone(&db), points.clone()),
            preferences: TrackPreferences::new(Arc::clone(&db), points.clone()),
            points,
            db,
            users,
        }
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

    /// The User `user_id`'s Link was read or started and found Linked: pushes their whole Watch
    /// State and Track Preferences, dated as stored, since what they did before they Linked was
    /// never pushed, then pulls both, then syncs their Watchlist. Must be called within a Tokio
    /// runtime.
    // ponytail: pushes their whole Watch State and Track Preferences on each look at their Link, as
    // the pull already reads the Account's whole; have the Plugin say when a Link is new if that
    // grows slow.
    pub(crate) fn user_linked(&self, user_id: i64) {
        let span = tracing::info_span!(parent: None, "sync_after_user_link");
        self.soon(span, move |sync| async move {
            sync.push_watch_states(user_id).await;
            sync.push_track_preferences(user_id).await;
            sync.pull_watch_states(user_id).await;
            sync.pull_track_preferences(user_id).await;
            sync.sync_watchlist(user_id).await;
        });
    }

    /// The Server's Link was read and found Linked: pushes every User's Watch State and Track
    /// Preferences, dated as stored. Must be called within a Tokio runtime.
    // ponytail: reads every User's whole Watch State and Track Preferences on each look at a Linked
    // Server; have the Plugin say when a Link is new if that grows slow.
    pub(crate) fn server_linked(&self) {
        let span = tracing::info_span!(parent: None, "push_after_server_link");
        self.soon(span, move |sync| async move {
            let listed = match sync.users.list().await {
                Ok(listed) => listed,
                Err(err) => {
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
            if self.has_linker() {
                self.pull_every_user().await;
            }
        }
    }

    /// Whether an enabled Plugin links the Server; most Servers have none, and sync nothing.
    fn has_linker(&self) -> bool {
        !self.points.providers::<dyn ServerLinker>().is_empty()
    }

    /// Runs `work` in a task of its own, in `span`, so a slow or unreachable service holds up
    /// nothing, unless no Plugin links the Server.
    fn soon<F>(&self, span: Span, work: impl FnOnce(Self) -> F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        if self.has_linker() {
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
                tracing::error!("listing Users to sync them failed: {err}");
                return;
            }
        };
        for user in listed {
            self.pull_watch_states(user.id).await;
            self.sync_watchlist(user.id).await;
        }
    }

    /// Tells the Plugins that observe Watch State, such as the Hub's, of the User `user_id`'s
    /// whole Watch State, each change dated as stored, and waits until they have it queued, so a
    /// pull after it sends it first. The Plugin queues it only for a Linked User, and the Hub keeps
    /// the later of a title's changes (ADR 0008).
    async fn push_watch_states(&self, user_id: i64) {
        let changes = match self.browse.watch_state_changes(user_id).await {
            Ok(changes) if changes.is_empty() => return,
            Ok(changes) => changes,
            Err(err) => {
                tracing::error!(user = user_id, "reading a User's Watch State failed: {err}");
                return;
            }
        };
        let told = (self.points)
            .notify_and_wait::<WatchStatesChanged>(WatchStateChanges { user_id, changes });
        wait_until_told(told, user_id, "Watch State").await;
    }

    /// Tells the Plugins that observe Track Preferences, such as the Hub's, of each of the User
    /// `user_id`'s, dated as stored, as [`Syncing::push_watch_states`] does their Watch State, and
    /// for the same reasons. The Hub keeps the change made last of each (ADR 0092).
    async fn push_track_preferences(&self, user_id: i64) {
        let stored = match self.preferences.changes_of_user(user_id).await {
            Ok(stored) => stored,
            Err(err) => {
                tracing::error!(
                    user = user_id,
                    "reading a User's Track Preferences failed: {err}"
                );
                return;
            }
        };
        // One at a time, so the Hub Plugin asks the Hub about the User's Link once, not once each,
        // and in a task of its own, so the rest are still told after waiting on them times out.
        let points = self.points.clone();
        let telling = async move {
            for preference in stored {
                let change = TrackPreferenceChange {
                    user_id,
                    preference,
                };
                points
                    .notify_and_wait::<TrackPreferenceChanged>(change)
                    .await;
            }
        };
        let told = tokio::spawn(telling.in_current_span());
        wait_until_told(told, user_id, "Track Preferences").await;
    }

    /// Takes the User `user_id`'s Watch State from the Plugin that links the Server, where it is
    /// newer than the Server's own. A failure is logged: the next sign-in or scan pulls again.
    async fn pull_watch_states(&self, user_id: i64) {
        let pull = async |linker: Arc<dyn ServerLinker>| linker.watch_states(user_id).await;
        let Some(pulled) = self.pulled(user_id, "Watch State", pull).await else {
            return;
        };
        if let Err(err) = self.browse.take_pulled(user_id, pulled).await {
            tracing::error!(
                user = user_id,
                "taking a User's pulled Watch State failed: {err}"
            );
        }
    }

    /// Takes the User `user_id`'s Track Preferences from the Plugin that links the Server, each
    /// where it is newer than the Server's own. A failure is logged: the next sign-in pulls again.
    async fn pull_track_preferences(&self, user_id: i64) {
        let pull = async |linker: Arc<dyn ServerLinker>| linker.track_preferences(user_id).await;
        let Some(pulled) = self.pulled(user_id, "Track Preferences", pull).await else {
            return;
        };
        if let Err(err) = self.preferences.take_pulled(user_id, pulled).await {
            tracing::error!(
                user = user_id,
                "taking a User's pulled Track Preferences failed: {err}"
            );
        }
    }

    /// What `pull` gets of the User `user_id`'s `what` from the Plugin that links the Server,
    /// `None` without one, or when it fails, which is logged.
    async fn pulled<T, F>(
        &self,
        user_id: i64,
        what: &'static str,
        pull: impl Fn(Arc<dyn ServerLinker>) -> F,
    ) -> Option<T>
    where
        F: Future<Output = Result<T, PluginError>>,
    {
        let answer = (self.points)
            .ask_first::<dyn ServerLinker, _, _, _, _>(async |linker| {
                within_sync_timeout(pull(linker)).await
            })
            .await?;
        let plugin = &answer.plugin;
        answer
            .result
            .inspect_err(|_failure| {
                // Ids only: `ask_first` already puts the failure's message on the Plugin's page.
                tracing::warn!(%plugin, user = user_id, "could not pull a User's {what}");
            })
            .ok()
    }

    /// Hands the User `user_id`'s whole Watchlist to the Plugin that links the Server, and takes
    /// what it answers where that is newer than the Server's own. A failure is logged: the next
    /// change, sign-in or scan syncs again, sending what this didn't.
    async fn sync_watchlist(&self, user_id: i64) {
        let entries = match watchlist::entries(&self.db, user_id).await {
            Ok(entries) => entries,
            Err(err) => {
                tracing::error!(
                    user = user_id,
                    "reading a User's Watchlist to sync failed: {err}"
                );
                return;
            }
        };
        let Some(answer) = (self.points)
            .ask_first::<dyn ServerLinker, _, _, _, _>(async |linker| {
                within_sync_timeout(linker.sync_watchlist(user_id, entries.clone())).await
            })
            .await
        else {
            return;
        };
        let Ok(synced) = answer.result else {
            // Ids only: `ask_first` already puts the failure's message on the Plugin's page.
            let plugin = &answer.plugin;
            tracing::warn!(%plugin, user = user_id, "could not sync a User's Watchlist");
            return;
        };
        if let Err(err) = watchlist::take_synced(&self.db, user_id, synced).await {
            tracing::error!(
                user = user_id,
                "taking a User's synced Watchlist failed: {err}"
            );
        }
    }
}

/// What `call` to the Plugin that links the Server answers, or a failure once it has taken
/// [`SYNC_TIMEOUT`], so a Plugin that never answers can't hold up every later scan's pulls.
async fn within_sync_timeout<T>(
    call: impl Future<Output = Result<T, PluginError>>,
) -> Result<T, PluginError> {
    tokio::time::timeout(SYNC_TIMEOUT, call)
        .await
        .unwrap_or_else(|_elapsed| Err("it didn't answer in time".into()))
}

/// Waits until `told`, Observers taking the User `user_id`'s `what`, but no longer than
/// [`SYNC_TIMEOUT`], so an Observer that never answers can't hold up the pull; the Plugin's Sync
/// Job sends what it queues later.
async fn wait_until_told(told: impl Future, user_id: i64, what: &str) {
    if tokio::time::timeout(SYNC_TIMEOUT, told).await.is_err() {
        tracing::warn!(
            user = user_id,
            "Observers took too long to take a User's {what}"
        );
    }
}
