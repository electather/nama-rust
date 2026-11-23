//! Viewing Events (ADR 0093): each play of a Matched title by a Linked User, sent to the Hub under
//! the External ID its Watch State is kept by. The Server records none of its own: it keeps only
//! those the Hub hasn't had yet, in the Plugin's Store, and [`SEND_JOB`] sends them once the Hub
//! is back.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, PoisonError},
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use buffa::{EnumValue, Message as _};
use nama_hub_proto::nama::hub::v1::{
    DeviceKind as HubDeviceKind, RecordViewingEventsRequest, UserViewingEvent, ViewingEvent,
};
use nama_plugin_api::{
    BoxFuture, Observer, PluginError,
    automation::functions::{
        ListDevices, ListVersionTitles, ListWatchStates, StoreDelete, StorePut,
    },
    contract::{
        DeviceKind, DevicesAsked, Playback, StoreEntry, TitleId, TitleRef, TitleRefKind,
        TitleWatchStates, VersionTitles, VersionTitlesAsked, WatchStatesAsked,
        event::Event as Happened,
    },
    job::Job,
    points::{Events, ObservedEvents},
};
use ring::rand::{SecureRandom as _, SystemRandom};

use super::{Linker, store_key};

/// The id of the Job that sends the Viewing Events the Hub hasn't had yet.
pub(super) const SEND_JOB: &str = "send-viewing-events";

/// The Store key of the Viewing Events the Hub hasn't had yet, as the request that sends them.
const UNSENT_KEY: &str = "viewing-events";

/// The most Viewing Events kept unsent, as many as the Hub takes in one call.
const MAX_UNSENT: usize = 1000;

/// The most bytes the Store keeps under one key.
const MAX_STORED_BYTES: usize = 64 * 1024;

/// The most characters the Hub takes in an External ID (`ViewingEvent.external_id`).
const MAX_EXTERNAL_ID_CHARS: usize = 100;

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
        let now = jiff::Zoned::now();
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
    /// When each plan playing now started, by its id.
    started: Arc<Mutex<HashMap<u64, Started>>>,
    /// Held while the unsent Viewing Events are read and written, so one added while they are
    /// sent isn't deleted with them.
    unsent: Arc<tokio::sync::Mutex<()>>,
}

impl ViewingEvents {
    pub(crate) fn new(linker: Arc<Linker>) -> Self {
        Self {
            linker,
            started: Arc::default(),
            unsent: Arc::default(),
        }
    }

    /// Keeps a Viewing Event for each title the play `playback`, which just ended, reached, if its
    /// User is Linked, and tries sending them.
    async fn ended(&self, playback: &Playback) -> Result<(), PluginError> {
        let started = self.started_lock().remove(&playback.plan_id);
        // One that started before the Plugin was enabled has no start to record.
        let Some(started) = started else {
            return Ok(());
        };
        let Some(kept) = self.linker.kept().await? else {
            return Ok(());
        };
        let user = playback.user_id;
        if self.linker.kept_account(user, &kept).await?.is_none() {
            return Ok(());
        }
        let events = self.viewing_events(playback, started).await?;
        if events.is_empty() {
            return Ok(());
        }
        self.keep(events).await?;
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

    /// Adds `events` to those the Hub hasn't had yet, dropping the oldest past what the Store
    /// holds.
    async fn keep(&self, events: Vec<UserViewingEvent>) -> Result<(), PluginError> {
        let _held = self.unsent.lock().await;
        let mut unsent = self.unsent_events().await?;
        unsent.events.extend(events);
        // ponytail: about 600 plays fit; past that, while the Hub stays down, the oldest go.
        // Keep them under a key each once Store keys can be listed.
        while unsent.events.len() > MAX_UNSENT
            || usize::try_from(unsent.encoded_len()).unwrap_or(usize::MAX) > MAX_STORED_BYTES
        {
            unsent.events.remove(0);
        }
        (self.linker.kernel)
            .ask::<StorePut>(StoreEntry {
                key: UNSENT_KEY.to_owned(),
                value: unsent.encode_to_vec(),
                ..Default::default()
            })
            .await?;
        Ok(())
    }

    /// Sends the Hub the Viewing Events it hasn't had yet, and forgets them once it has, or once
    /// the Server isn't Linked, when nothing can take them.
    pub(super) async fn send(&self) -> Result<(), PluginError> {
        let _held = self.unsent.lock().await;
        let unsent = self.unsent_events().await?;
        if unsent.events.is_empty() {
            return Ok(());
        }
        if let Some(kept) = self.linker.kept().await? {
            (self.linker.hub)
                .record_viewing_events(&kept.credential, unsent)
                .await?;
        }
        (self.linker.kernel)
            .ask::<StoreDelete>(store_key(UNSENT_KEY))
            .await?;
        Ok(())
    }

    async fn unsent_events(&self) -> Result<RecordViewingEventsRequest, PluginError> {
        Ok(match self.linker.stored(UNSENT_KEY).await? {
            Some(stored) => RecordViewingEventsRequest::decode_from_slice(&stored)?,
            None => RecordViewingEventsRequest::default(),
        })
    }

    /// Notes when a play started, and records it once it ends.
    async fn happened(&self, happened: &Happened) -> Result<(), PluginError> {
        if let Happened::PlaybackStarted(started) = happened {
            let plan = started.playback.plan_id;
            self.started_lock().insert(plan, Started::now());
        } else if let Happened::PlaybackEnded(ended) = happened {
            self.ended(&ended.playback).await?;
        }
        Ok(())
    }

    fn started_lock(&self) -> std::sync::MutexGuard<'_, HashMap<u64, Started>> {
        self.started.lock().unwrap_or_else(PoisonError::into_inner)
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

/// `id` as the Hub takes an External ID, such as `tmdb-603`, unless it is longer than that.
fn hub_external_id(id: &TitleId) -> Option<String> {
    let external_id = format!("{}-{}", id.namespace, id.value);
    // The Hub refuses every event sent with a longer one, and `send` then forgets them all.
    (external_id.chars().count() <= MAX_EXTERNAL_ID_CHARS).then_some(external_id)
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
