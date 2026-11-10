//! A Built-in Plugin that logs the Kernel's events. It is the smallest Plugin there is, and shows
//! that the wiring from the Kernel to a Plugin works.

use nama_plugin_api::{
    BoxFuture, Manifest, Observer, Plugin, PluginId, PointKey, Registrar,
    points::{Events, ServerStarted, ServerStartedEvent},
    settings::SettingsSchema,
};
use nama_proto::nama::v1::{Event, Playback, event::Event as Happened};
use tracing::info;

/// The Plugin.
#[derive(Debug, Default)]
pub struct LogEvents;

impl LogEvents {
    /// The Plugin's id.
    pub const ID: PluginId = PluginId::from_static("log-events");
}

impl Plugin for LogEvents {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: Self::ID,
            name: "Event log".to_owned(),
            description:
                "Writes when the Server starts, and when each playback starts and ends, to its log."
                    .to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            settings: SettingsSchema::default(),
            extension_points: vec![
                PointKey::observe::<ServerStarted>(),
                PointKey::observe::<Events>(),
            ],
            logo_svg: Some(include_str!("../logo.svg").to_owned()),
        }
    }

    fn register(&self, registrar: &mut Registrar) {
        registrar.observe::<ServerStarted>(Log);
        registrar.observe::<Events>(Log);
    }
}

struct Log;

impl Observer<ServerStarted> for Log {
    fn observe<'a>(&'a self, event: &'a ServerStartedEvent) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            info!(name = %event.name, version = %event.version, "observed server started");
        })
    }
}

// ponytail: only playback is logged; a scan's batches of up to thousands of Items would flood the
// log, so log them once someone needs them there.
impl Observer<Events> for Log {
    fn observe<'a>(&'a self, event: &'a Vec<Event>) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            for (what, playback) in event.iter().filter_map(playback) {
                log_playback(what, playback);
            }
        })
    }
}

/// What happened to a playback in `event`, such as `started`, and the playback, if it's about
/// one.
fn playback(event: &Event) -> Option<(&'static str, &Playback)> {
    if let Some(Happened::PlaybackStarted(started)) = &event.event {
        Some(("started", &started.playback))
    } else if let Some(Happened::PlaybackEnded(ended)) = &event.event {
        Some(("ended", &ended.playback))
    } else if let Some(Happened::PlaybackChanged(changed)) = &event.event {
        Some(("changed", &changed.playback))
    } else {
        None
    }
}

/// Logs that `playback` `what` happened, such as `started`.
fn log_playback(what: &str, playback: &Playback) {
    info!(
        user = playback.user_id,
        device = playback.device_id,
        version = playback.version_id,
        method = %playback.method,
        "observed playback {what}"
    );
}

#[cfg(test)]
mod tests {
    use std::{
        io,
        sync::{Arc, Mutex, PoisonError},
    };

    use nama_plugin_api::{Observer, points::Events};
    use nama_proto::nama::v1::{
        Event, Playback, PlaybackEnded, PlaybackMethod, PlaybackStarted, event,
    };

    use super::Log;

    /// Where a test's log lines are written.
    #[derive(Clone, Default)]
    struct Captured(Arc<Mutex<Vec<u8>>>);

    impl io::Write for Captured {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let mut written = self.0.lock().unwrap_or_else(PoisonError::into_inner);
            written.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn event(happened: impl Into<event::Event>) -> Event {
        Event {
            event: Some(happened.into()),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn a_remuxed_playback_is_logged_as_it_starts_and_ends() {
        let captured = Captured::default();
        let writer = captured.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(move || writer.clone())
            .with_ansi(false)
            .finish();
        let _logging = tracing::subscriber::set_default(subscriber);
        let playback = Playback {
            user_id: 2,
            device_id: 3,
            version_id: 4,
            method: PlaybackMethod::PLAYBACK_METHOD_REMUX.into(),
            ..Default::default()
        };
        let started = PlaybackStarted {
            playback: playback.clone().into(),
            ..Default::default()
        };
        let ended = PlaybackEnded {
            playback: playback.into(),
            ..Default::default()
        };

        Observer::<Events>::observe(&Log, &vec![event(started), event(ended)]).await;

        let logged = String::from_utf8(captured.0.lock().unwrap().clone()).unwrap();
        let lines: Vec<&str> = logged.lines().collect();
        assert_eq!(lines.len(), 2, "{logged}");
        for (line, what) in lines.iter().zip(["started", "ended"]) {
            assert!(
                line.contains(&format!("observed playback {what}")),
                "{line}"
            );
            assert!(
                line.contains("user=2 device=3 version=4 method=PLAYBACK_METHOD_REMUX"),
                "{line}"
            );
        }
    }
}
