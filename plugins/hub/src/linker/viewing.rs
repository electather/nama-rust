//! Viewing Events (ADR 0093): each play of a Matched title by a Linked User, sent to the Hub under
//! the External ID its Watch State is kept by. The Server records none of its own: it queues only
//! those the Hub hasn't had yet, in the Plugin's Store, and [`SEND_JOB`] sends them once the Hub
//! is back.

use std::{
    collections::HashMap,
    hash::Hash,
    sync::{Arc, Mutex, PoisonError},
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use buffa::{EnumValue, Message as _};
use nama_hub_proto::nama::hub::v1::{
    DeviceKind as HubDeviceKind, RecordViewingEventsRequest, UserViewingEvent, ViewingEvent,
};
use nama_plugin_api::{
    BoxFuture, Observer, PluginError,
    automation::functions::{ListDevices, ListVersionTitles, ListWatchStates},
    contract::{
        DeviceKind, DevicesAsked, Playback, TitleId, TitleRef, TitleRefKind, TitleWatchStates,
        VersionTitles, VersionTitlesAsked, WatchStatesAsked, event::Event as Happened,
    },
    job::Job,
    points::{Events, ObservedEvents},
};
use ring::rand::{SecureRandom as _, SystemRandom};

use super::{
    Linker,
    queue::{Queue, Queued},
};
use crate::hub::limits;

/// The id of the Job that sends the Viewing Events the Hub hasn't had yet.
pub(super) const SEND_JOB: &str = "send-viewing-events";

impl Queued for UserViewingEvent {
    const STORE_KEY: &'static str = "viewing-event-queue";
    const PER_CALL: usize = limits::VIEWING_EVENTS;
    const OLD_STORE_KEY: Option<&'static str> = Some("viewing-events");

    /// Its random key, so no Viewing Event ever replaces another.
    fn key(&self) -> impl Eq + Hash + '_ {
        &self.key
    }

    fn changed_at_ms(&self) -> i64 {
        (self.event.as_option()).map_or(0, |event| event.started_at.saturating_mul(1000))
    }

    /// An older Hub Plugin kept them as the request that sends them.
    fn from_old(old: &[u8]) -> Result<Vec<Self>, PluginError> {
        Ok(RecordViewingEventsRequest::decode_from_slice(old)?.events)
    }
}

/// The Job that sends the Viewing Events the Hub hasn't had yet, which the Kernel runs every 5
/// minutes.
pub(crate) fn send_job() -> Job {
    Job {
        id: SEND_JOB.to_owned(),
        name: "Send Viewing Events".to_owned(),
        description: "Sends the Hub the Viewing Events of plays it was unreachable for.".to_owned(),
        schedule: Some("*/5 * * * *".to_owned()),
    }
}

/// A play not ended yet, and when it started.
struct Open {
    playback: Playback,
    started: Started,
}

/// When a play started, by the Server's clock.
#[derive(Clone, Copy)]
struct Started {
    /// In seconds since the Unix epoch.
    at: i64,
    /// In the Server's time zone.
    time_of_day_minutes: i32,
}

impl Started {
    fn now() -> Self {
        let now = jiff::ZonedDateTime::now();
        Self {
            at: now.timestamp().as_second(),
            time_of_day_minutes: i32::from(now.hour()) * 60 + i32::from(now.minute()),
        }
    }
}

/// The Observer of plays and the sender of [`SEND_JOB`]'s Viewing Events. Clones share one state, so the Observer
/// and the Job can each hold one.
#[derive(Clone)]
pub(crate) struct ViewingEvents {
    linker: Arc<Linker>,
    /// Each plan playing now, by its id.
    open: Arc<Mutex<HashMap<u64, Open>>>,
    unsent: Arc<Queue<UserViewingEvent>>,
}

impl ViewingEvents {
    pub(crate) fn new(linker: Arc<Linker>) -> Self {
        Self {
            unsent: Arc::new(Queue::new(Arc::clone(&linker.kernel))),
            linker,
            open: Arc::default(),
        }
    }

    /// Keeps a Viewing Event for each title the play `playback` reached, if its User is Linked, and
    /// tries sending them. Called when it ends, or when a later play ends it (`end_left_open`).
    async fn ended(&self, playback: &Playback) -> Result<(), PluginError> {
        let open = self.open_lock().remove(&playback.plan_id);
        // None for one that started before the Plugin was enabled, or that a later play ended.
        let Some(Open { started, .. }) = open else {
            return Ok(());
        };
        if !self.linker.user_linked(playback.user_id).await? {
            return Ok(());
        }
        let events = self.viewing_events(playback, started).await?;
        if events.is_empty() {
            return Ok(());
        }
        self.unsent.add(events).await?;
        // Unsent, they wait for the Job.
        let _unsent = self.send().await;
        Ok(())
    }

    /// A Viewing Event for each Matched title of `playback`'s Version whose Watch State its User
    /// changed since it `started`: a file holding two Episodes may have stopped in the first.
    async fn viewing_events(
        &self,
        playback: &Playback,
        started: Started,
    ) -> Result<Vec<UserViewingEvent>, PluginError> {
        let kernel = &self.linker.kernel;
        let listed = (kernel.ask::<ListVersionTitles>(VersionTitlesAsked {
            version_id: playback.version_id,
            ..Default::default()
        }))
        .await?;
        if listed.titles.is_empty() {
            return Ok(Vec::new());
        }
        let states = (kernel.ask::<ListWatchStates>(WatchStatesAsked {
            titles: listed.titles.clone(),
            user_id: Some(playback.user_id),
            ..Default::default()
        }))
        .await?;
        let devices = (kernel.ask::<ListDevices>(DevicesAsked {
            device_ids: vec![playback.device_id],
            ..Default::default()
        }))
        .await?;
        let device_kind = (devices.devices.first())
            .map_or(HubDeviceKind::DEVICE_KIND_UNSPECIFIED, |device| {
                hub_device_kind(device.kind)
            });
        (states.titles.iter())
            .filter_map(|title| viewed(title, &listed, started, device_kind))
            .map(|event| {
                Ok(UserViewingEvent {
                    user_id: playback.user_id,
                    key: random_key()?,
                    event: event.into(),
                    ..Default::default()
                })
            })
            .collect()
    }

    /// Sends the Hub the Viewing Events it hasn't had yet.
    pub(super) async fn send(&self) -> Result<(), PluginError> {
        let push = |page| -> BoxFuture<'_, _> {
            Box::pin(self.linker.call(|hub| hub.record_viewing_events(page)))
        };
        self.unsent.send(push).await
    }

    /// Notes when a play started, ending the plays its User left open on its title, and records
    /// it once it ends.
    async fn happened(&self, happened: &Happened) -> Result<(), PluginError> {
        if let Happened::PlaybackStarted(started) = happened {
            let open = Open {
                playback: Playback::clone(&started.playback),
                started: Started::now(),
            };
            self.open_lock().insert(started.playback.plan_id, open);
            self.end_left_open(&started.playback).await?;
        } else if let Happened::PlaybackEnded(ended) = happened {
            self.ended(&ended.playback).await?;
        }
        Ok(())
    }

    /// Ends now each earlier play `later`'s User left open on a title it plays. One left without
    /// stopping, as when its tab closed, ends only once its player goes 60 seconds without asking
    /// for it, by when `later`'s reports have replaced the Watch State it last reported (#2051).
    async fn end_left_open(&self, later: &Playback) -> Result<(), PluginError> {
        let earlier = Vec::from_iter(
            (self.open_lock().values())
                .map(|open| &open.playback)
                // Plan ids only grow, and two batches may reach an Observer in either order.
                .filter(|earlier| earlier.user_id == later.user_id)
                .filter(|earlier| earlier.plan_id < later.plan_id)
                .cloned(),
        );
        if earlier.is_empty() {
            return Ok(());
        }
        let later_titles = self.titles_of(later.version_id).await?;
        for playback in earlier {
            let titles = self.titles_of(playback.version_id).await?;
            if titles.iter().any(|title| later_titles.contains(title)) {
                self.ended(&playback).await?;
            }
        }
        Ok(())
    }

    /// The titles the Version `version_id` holds, in whichever Library, as Watch State keys them.
    async fn titles_of(&self, version_id: i64) -> Result<Vec<TitleRef>, PluginError> {
        let listed = (self.linker.kernel).ask::<ListVersionTitles>(VersionTitlesAsked {
            version_id,
            ..Default::default()
        });
        let titles = listed.await?.titles.into_iter().map(|title| TitleRef {
            library_id: 0,
            ..title
        });
        Ok(titles.collect())
    }

    fn open_lock(&self) -> std::sync::MutexGuard<'_, HashMap<u64, Open>> {
        self.open.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Observer<Events> for ViewingEvents {
    fn observe<'a>(&'a self, event: &'a ObservedEvents) -> BoxFuture<'a, Result<(), PluginError>> {
        Box::pin(async move {
            for happened in event.events.iter().filter_map(|event| event.event.as_ref()) {
                self.happened(happened).await?;
            }
            Ok(())
        })
    }
}

/// The Viewing Event of a play that `started` and changed `title`'s Watch State, on a Device of
/// `device_kind`, if it did. An Episode's names its Show, as `listed` pairs them.
fn viewed(
    title: &TitleWatchStates,
    listed: &VersionTitles,
    started: Started,
    device_kind: HubDeviceKind,
) -> Option<ViewingEvent> {
    let state = title.states.first()?;
    let title_ref = title.title_ref.as_option()?;
    let id = title_ref.id.as_option()?;
    let (stop_position_ms, duration_ms) = (
        i64::try_from(state.position_ms).ok()?,
        i64::try_from(state.duration_ms).ok()?,
    );
    // Watch State keeps whole seconds.
    if state.updated_at_ms / 1000 < started.at || duration_ms == 0 {
        return None;
    }
    let external_id = hub_external_id(id)?;
    let (episode, show_external_id) = match title_ref.kind {
        EnumValue::Known(TitleRefKind::TITLE_REF_KIND_MOVIE) => (false, String::new()),
        EnumValue::Known(TitleRefKind::TITLE_REF_KIND_EPISODE) => {
            (true, hub_external_id(show_id(listed, title_ref)?)?)
        }
        EnumValue::Known(TitleRefKind::TITLE_REF_KIND_UNSPECIFIED) | EnumValue::Unknown(_) => {
            return None;
        }
    };
    Some(ViewingEvent {
        episode,
        external_id,
        show_external_id,
        started_at: started.at,
        time_of_day_minutes: started.time_of_day_minutes,
        stop_position_ms,
        duration_ms,
        // As the Server marks a title played.
        finished: i128::from(stop_position_ms) * 10 > i128::from(duration_ms) * 9,
        device_kind: device_kind.into(),
        ..Default::default()
    })
}

/// `id` as the Hub takes an External ID, such as `tmdb-603`, unless it wouldn't take it.
fn hub_external_id(id: &TitleId) -> Option<String> {
    let external_id = format!("{}-{}", id.namespace, id.value);
    limits::takes_external_id(&external_id).then_some(external_id)
}

/// The id of the Show of the Episode `title_ref`, as `listed` pairs them, unless it has none.
fn show_id<'a>(listed: &'a VersionTitles, title_ref: &TitleRef) -> Option<&'a TitleId> {
    (listed.titles.iter().zip(&listed.show_ids))
        .find_map(|(title, show)| (title == title_ref).then_some(show))
        .filter(|show| !show.value.is_empty())
}

fn hub_device_kind(kind: EnumValue<DeviceKind>) -> HubDeviceKind {
    match kind {
        EnumValue::Known(DeviceKind::DEVICE_KIND_COMPUTER) => HubDeviceKind::DEVICE_KIND_COMPUTER,
        EnumValue::Known(DeviceKind::DEVICE_KIND_PHONE) => HubDeviceKind::DEVICE_KIND_PHONE,
        EnumValue::Known(DeviceKind::DEVICE_KIND_TABLET) => HubDeviceKind::DEVICE_KIND_TABLET,
        EnumValue::Known(DeviceKind::DEVICE_KIND_TV) => HubDeviceKind::DEVICE_KIND_TV,
        EnumValue::Known(DeviceKind::DEVICE_KIND_UNSPECIFIED) | EnumValue::Unknown(_) => {
            HubDeviceKind::DEVICE_KIND_UNSPECIFIED
        }
    }
}

/// A key naming a Viewing Event apart from every other, so the Hub records one sent twice once.
fn random_key() -> Result<String, PluginError> {
    let mut key = [0; 16];
    SystemRandom::new()
        .fill(&mut key)
        .map_err(|_unspecified| "could not generate a Viewing Event's key")?;
    Ok(URL_SAFE_NO_PAD.encode(key))
}

#[cfg(test)]
#[path = "viewing_tests.rs"]
mod tests;
