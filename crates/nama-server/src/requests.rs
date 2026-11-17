//! Requests: Users asking for a Movie, or a Season of a Show, that isn't on the Server to be added
//! to it (ADR 0012).
//!
//! Followed Shows sit beside them: Users asking for a whole Show, whose later Seasons get Season
//! Requests as a Library scan Matches their Episodes, never by asking a title source.
//!
//! This area is the only code that reads or writes `requests`, `requesters`, `followed_shows`
//! and `followed_show_requesters`. It hands Requests out as their proto messages, since those are
//! also what its events carry.

use std::{
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use nama_plugin_api::{PluginId, metadata::ExternalId};
use tokio::{sync::mpsc::UnboundedReceiver, task::JoinHandle};

use crate::{
    db::Database,
    events::EventBus,
    library::{Contents, TitleSources},
    page,
    plugins::ExtensionPoints,
};

mod apply;
mod asked;
mod asking;
mod availability;
mod deciding;
mod followed;
mod hand_off;
mod listing;
mod lookup;
mod progress;
mod qualities;
mod quality;
mod state;
#[cfg(test)]
mod testing;

pub(crate) use asked::Asked;
pub(crate) use availability::{make_available, make_seasons_available};
pub(crate) use deciding::RequestTitle;
use hand_off::HandOff;
pub(crate) use hand_off::{missing, unmet};
pub(crate) use lookup::{Found, FoundShow, Hit};
use progress::Progress;
pub(crate) use qualities::{
    ASKED as QUALITY_ASKED_FIELD, CHANGED as QUALITY_CHANGED_FIELD, Invalid as QualitiesInvalid,
    PROFILE as QUALITY_PROFILE_FIELD, QUALITY_PROFILE, Refused as QualityRefused,
    check as check_qualities, list as list_qualities, replace as replace_qualities,
};

/// Why a Request couldn't be made, found or changed.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    /// There is no such Request or Followed Show, or none the caller may see.
    #[error("there is no {0} {1}")]
    NoSuch(Asked, i64),
    #[error("the {0} {1} isn't Pending")]
    NotPending(Asked, i64),
    #[error("the Request {0} isn't Failed")]
    NotFailed(i64),
    #[error("you aren't a Requester of the {0} {1}")]
    NotARequester(Asked, i64),
    #[error("a Library already has the Movie {0}")]
    InLibrary(ExternalId),
    #[error("every aired Episode of Season {season} of the Show {show} is in a Library")]
    SeasonInLibrary { show: ExternalId, season: u32 },
    #[error("the title source has no Movie {0}")]
    NoSuchMovie(ExternalId),
    #[error("the title source has no Show {0}")]
    NoSuchShow(ExternalId),
    #[error("the title source has no Season {season} of the Show {show}")]
    NoSuchSeason { show: ExternalId, season: u32 },
    /// No title source is enabled, or each failed.
    #[error("no title source could answer")]
    Unavailable,
    #[error(transparent)]
    Quality(#[from] qualities::Refused),
    #[error(transparent)]
    PageTokenInvalid(#[from] page::TokenInvalid),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// The Server's Requests, which publishes what changes about them.
#[derive(Debug, Clone)]
pub(crate) struct Requests {
    db: Arc<Database>,
    contents: Contents,
    /// Looks up what a new Request asks for: a Movie, or a Show's Seasons.
    metadata: TitleSources,
    /// What [`Self::apply`] publishes the changes its writes make on.
    events: EventBus,
    /// Where the Fulfillers are.
    plugins: ExtensionPoints,
    /// Its way to hand Requests to the Fulfillers.
    hand_off: HandOff,
    /// What the Fulfillers say they are downloading for the Requests they have.
    progress: Progress,
    /// The loop [`Requests::start`] spawns to hand off as Fulfillers come up.
    handing_off: Arc<Mutex<Option<JoinHandle<()>>>>,
    /// The loop [`Requests::start`] spawns to ask the Fulfillers about progress.
    following: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl Requests {
    /// The Requests in `db`, looking titles up through the title sources and handing Approved
    /// ones to the Fulfillers in `plugins`, publishing on `events`.
    pub(crate) fn new(db: Arc<Database>, events: EventBus, plugins: ExtensionPoints) -> Self {
        let progress = Progress::new(Arc::clone(&db), events.clone(), plugins.clone());
        Self {
            contents: Contents::new(Arc::clone(&db)),
            metadata: TitleSources::new(plugins.clone()),
            hand_off: HandOff::new(
                Arc::clone(&db),
                plugins.clone(),
                progress.became_processing(),
            ),
            progress,
            events,
            plugins,
            db,
            handing_off: Arc::default(),
            following: Arc::default(),
        }
    }

    /// Starts, in the background so a slow Fulfiller doesn't hold up serving, handing each
    /// Approved Request, oldest first, to its Fulfiller whenever a Plugin `came_up` names provides
    /// one, and asking the Fulfillers how the Requests they have that aren't Available yet are
    /// progressing, every `progress_every` while there are any and at once when one becomes
    /// Processing. A failed hand-off is only logged, and leaves the rest waiting. Call it once.
    pub(crate) fn start(&self, came_up: UnboundedReceiver<PluginId>, progress_every: Duration) {
        let requests = self.clone();
        let handing_off =
            tokio::spawn(async move { requests.as_fulfillers_come_up(came_up).await });
        let asking = self.progress.clone();
        let following = tokio::spawn(async move { asking.follow(progress_every).await });
        *lock(&self.handing_off) = Some(handing_off);
        *lock(&self.following) = Some(following);
    }

    /// Hands off the waiting Requests whenever a Plugin `came_up` names provides a Fulfiller, as
    /// [`Self::start`] describes, until `came_up` closes.
    async fn as_fulfillers_come_up(&self, came_up: UnboundedReceiver<PluginId>) {
        self.hand_off.as_fulfillers_come_up(self, came_up).await;
    }

    /// Stops what [`Self::start`] started, logging a panic from either loop. Does nothing before
    /// it.
    ///
    /// The Requests not handed off yet stay Approved, so the next start hands them off. The one
    /// being handed off now runs on in a task of its own (`HandOff::request`), which nothing waits
    /// for: if the runtime stops first, a Request the Fulfiller took stays Approved and the next
    /// start hands it over again, as for an approval in flight (ADR 0029). Progress is kept in
    /// memory only, so stopping mid-poll loses nothing.
    pub(crate) async fn shutdown(&self) {
        stop(&self.handing_off, "handing the waiting Requests off").await;
        stop(&self.following, "asking the Fulfillers about progress").await;
    }
}

/// Aborts the loop `task` holds, if any, and waits for it, logging that `doing` panicked if it did.
async fn stop(task: &Mutex<Option<JoinHandle<()>>>, doing: &str) {
    let Some(handle) = lock(task).take() else {
        return;
    };
    handle.abort();
    if let Err(err) = handle.await
        && err.is_panic()
    {
        tracing::error!("{doing} panicked: {err}");
    }
}

/// Locks `mutex`, taking its value even if poisoned: nothing panics while one is held, so it is
/// still whole.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
