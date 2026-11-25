//! Viewing Events (ADR 0093): each play of a Matched title by a Linked User, queued for the Hub
//! under the External ID its Watch State is kept by, which the minute's send of what waits for the
//! Hub takes there (`src/sync.rs`). A play is noted as `PlaybackStarted` tells it and recorded once
//! `PlaybackEnded` does, one Viewing Event for each title of its Version whose Watch State it
//! changed: a file holding two Episodes may have stopped in the first. Whether its User is Linked
//! is asked as it ends, so a play from before they Linked never goes, and again as it's sent.

use std::{collections::HashMap, sync::Arc};

use jiff::{Timestamp, tz::TimeZone};
use nama_plugin_api::{
    contract::DeviceKind,
    metadata::{ExternalId, ExternalIds},
};
use nama_proto::nama::v1::{Event, Playback, event};
use tokio::{
    sync::broadcast::{self, error::RecvError},
    task::JoinHandle,
};
use tracing::{Instrument as _, Span};

use crate::{
    db::{self, Database},
    diagnostics,
    events::EventBus,
    library::{Contents, TitlesIn, WatchState, WatchStateKind, WatchStates},
    link::{self, Linking, ViewingEvent},
    plugins::ExtensionPoints,
    users::Users,
};

/// Records Linked Users' plays as Viewing Events.
#[derive(Debug)]
pub(crate) struct ViewingEvents {
    db: Arc<Database>,
    contents: Contents,
    watch_states: WatchStates,
    users: Users,
    linking: Linking,
    /// Each plan playing now, by its id.
    open: HashMap<u64, Open>,
}

/// A play not ended yet, and when it started.
#[derive(Debug)]
struct Open {
    playback: Playback,
    started: Started,
}

/// When a play started, by the Server's clock.
#[derive(Debug, Clone, Copy)]
struct Started {
    /// In seconds since the Unix epoch.
    at: i64,
    /// In the Server's time zone.
    time_of_day_minutes: i32,
}

impl Started {
    fn now() -> Self {
        let at = db::now();
        let wall_clock =
            TimeZone::system().to_datetime(Timestamp::from_second(at).unwrap_or_default());
        Self {
            at,
            time_of_day_minutes: i32::from(wall_clock.hour()) * 60 + i32::from(wall_clock.minute()),
        }
    }
}

/// A Matched title a Version holds, by the ids its Watch State is found under.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Title {
    kind: WatchStateKind,
    /// Its id as browsing identifies it, which the Hub knows it by.
    id: ExternalId,
    ids: ExternalIds,
    /// An Episode's Show's id, `None` if its Match gives the Show none.
    show: Option<ExternalId>,
}

impl ViewingEvents {
    /// Records the plays of `users` on what `db` holds.
    pub(crate) fn new(db: Arc<Database>, points: ExtensionPoints, users: Users) -> Self {
        Self {
            contents: Contents::new(Arc::clone(&db)),
            watch_states: WatchStates::new(Arc::clone(&db), points),
            linking: users.linking().clone(),
            db,
            users,
            open: HashMap::new(),
        }
    }

    /// Follows the plays `events` publishes, recording each as it ends, until the task is aborted;
    /// at once done when the Server's config names no Hub (ADR 0002). Started before serving, so
    /// it sees every play start.
    pub(crate) fn follow_plays(self, events: &EventBus) -> JoinHandle<()> {
        let followed = events.follow();
        tokio::spawn(async move {
            if self.linking.has_hub() {
                self.record_each_play(followed).await;
            }
        })
    }

    /// [`ViewingEvents::follow_plays`]'s loop over the events `followed`.
    async fn record_each_play(mut self, mut followed: broadcast::Receiver<Event>) {
        loop {
            match followed.recv().await {
                Ok(Event {
                    event: Some(event::Event::PlaybackStarted(started)),
                    ..
                }) => {
                    let playback = started.playback.into_option().unwrap_or_default();
                    self.started(playback).await;
                }
                Ok(Event {
                    event: Some(event::Event::PlaybackEnded(ended)),
                    ..
                }) => {
                    let playback = ended.playback.into_option().unwrap_or_default();
                    self.ended(playback.plan_id).await;
                }
                // shortcut: a play whose start the lag missed goes unrecorded, and one whose end it
                // missed stays open until the Server restarts; ask the running plans instead if
                // that ever shows.
                Ok(_) | Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => return,
            }
        }
    }

    /// Notes when `playback` started, ending the plays its User left open on a title it plays.
    async fn started(&mut self, playback: Playback) {
        let open = Open {
            playback: playback.clone(),
            started: Started::now(),
        };
        self.open.insert(playback.plan_id, open);
        self.end_left_open(&playback).await;
    }

    /// Ends now each earlier play `later`'s User left open on a title it plays. One left without
    /// stopping, as when its tab closed, ends only once its player goes 60 seconds without asking
    /// for it, by when `later`'s reports have replaced the Watch State it last reported (#2051).
    async fn end_left_open(&mut self, later: &Playback) {
        let earlier: Vec<(u64, i64)> = (self.open.values())
            .map(|open| &open.playback)
            .filter(|earlier| earlier.user_id == later.user_id && earlier.plan_id < later.plan_id)
            .map(|earlier| (earlier.plan_id, earlier.version_id))
            .collect();
        if earlier.is_empty() {
            return;
        }
        let Some(later_titles) = self.titles_of(later.version_id).await else {
            return;
        };
        for (plan_id, version_id) in earlier {
            let Some(titles) = self.titles_of(version_id).await else {
                continue;
            };
            if titles.iter().any(|title| plays_alike(title, &later_titles)) {
                self.ended(plan_id).await;
            }
        }
    }

    /// Ends the play `plan_id`, recording it.
    async fn ended(&mut self, plan_id: u64) {
        // None for one that started before the Server did, or that a later play ended.
        let Some(open) = self.open.remove(&plan_id) else {
            return;
        };
        let span = diagnostics::following_current(tracing::info_span!(
            parent: None,
            "record_viewing_events",
            user = open.playback.user_id
        ));
        self.record(&open).instrument(span).await;
    }

    /// Queues a Viewing Event for each title `open` reached, if its User is Linked. A failure is
    /// logged: the play goes unrecorded.
    async fn record(&self, open: &Open) {
        let user_id = open.playback.user_id;
        // Not, when no answer came: logged by the Link.
        if !matches!(self.linking.is_user_linked(user_id).await, Ok(true)) {
            return;
        }
        let viewed = match self.viewing_events(open).await {
            Ok(viewed) => viewed,
            Err(err) => {
                diagnostics::failed_with(&err, &Span::current());
                tracing::error!("reading a play's Viewing Events failed: {err}");
                return;
            }
        };
        let queuing = self
            .db
            .write(move |tx| Box::pin(link::queue_viewing_events(tx, user_id, viewed)));
        if let Err(err) = queuing.await {
            diagnostics::failed_with(&err, &Span::current());
            tracing::error!("queuing a play's Viewing Events failed: {err}");
        }
    }

    /// A Viewing Event for each Matched title of `open`'s Version whose Watch State its User
    /// changed since it started.
    async fn viewing_events(&self, open: &Open) -> Result<Vec<ViewingEvent>, sqlx::Error> {
        let Playback {
            user_id,
            device_id,
            version_id,
            ..
        } = open.playback;
        let titles = titles_in(&self.contents, version_id).await?;
        if titles.is_empty() {
            return Ok(Vec::new());
        }
        let asked: Vec<_> = (titles.iter())
            .map(|title| (title.kind, title.ids.clone()))
            .collect();
        let states = self.watch_states.of_titles(Some(user_id), &asked).await?;
        let devices = self.users.device_kinds(&[device_id]).await?;
        let device_kind =
            (devices.first()).map_or(DeviceKind::DEVICE_KIND_UNSPECIFIED, |device| device.2);
        Ok((titles.into_iter().zip(states))
            .filter_map(|(title, states)| {
                let (_, state) = states.into_iter().next()?;
                viewed(title, state, open.started, device_kind)
            })
            .collect())
    }

    /// The titles the Version `version_id` holds; `None`, logged, when they couldn't be read.
    async fn titles_of(&self, version_id: i64) -> Option<Vec<Title>> {
        titles_in(&self.contents, version_id)
            .await
            .inspect_err(|err| tracing::error!("reading a played Version's titles failed: {err}"))
            .ok()
    }
}

/// The Matched titles the Version `version_id` holds, in order.
async fn titles_in(contents: &Contents, version_id: i64) -> Result<Vec<Title>, sqlx::Error> {
    Ok(match contents.titles_in(version_id).await? {
        TitlesIn::NoSuchVersion => Vec::new(),
        TitlesIn::Movie(movie) => (movie.into_iter())
            .map(|(id, ids)| Title {
                kind: WatchStateKind::Movie,
                id,
                ids,
                show: None,
            })
            .collect(),
        TitlesIn::Episodes(episodes) => (episodes.into_iter().flatten())
            .map(|(id, ids, show)| Title {
                kind: WatchStateKind::Episode,
                id,
                ids,
                show,
            })
            .collect(),
    })
}

/// Whether `title` is one of `titles`, as its Watch State is kept.
fn plays_alike(title: &Title, titles: &[Title]) -> bool {
    (titles.iter()).any(|other| other.kind == title.kind && other.id == title.id)
}

/// The Viewing Event of a play that `started` on a Device of `device_kind` and left `title` at
/// `state`, if it changed it since. An Episode's names its Show, and one whose Show has no id has
/// none.
fn viewed(
    title: Title,
    state: WatchState,
    started: Started,
    device_kind: DeviceKind,
) -> Option<ViewingEvent> {
    // Watch State keeps whole seconds.
    if state.updated_at < started.at {
        return None;
    }
    let show_id = match title.kind {
        WatchStateKind::Movie => None,
        WatchStateKind::Episode => Some(title.show?.to_string()),
    };
    Some(ViewingEvent {
        id: title.id.to_string(),
        show_id,
        started_at: started.at,
        time_of_day_minutes: started.time_of_day_minutes,
        stop_position_ms: state.position_ms,
        duration_ms: state.duration_ms,
        device_kind,
    })
}

#[cfg(test)]
#[path = "viewing_tests.rs"]
mod tests;
