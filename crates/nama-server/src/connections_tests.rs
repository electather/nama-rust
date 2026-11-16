use std::sync::atomic::{AtomicUsize, Ordering};

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PointKey, Priority, Registrar,
    connector::ConnectionStarted,
};

use super::*;
use crate::plugins::Registry;

/// A Plugin whose Connector counts its polls, and nobody ever finishes with its links.
struct NeverConnected(Arc<AtomicUsize>);

impl Plugin for NeverConnected {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "1.0.0".to_owned(),
            extension_points: vec![PointKey::provide::<dyn Connector>()],
            ..Manifest::new(PluginId::from_static("chat"))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        let connector: Arc<dyn Connector> = Arc::new(Self(Arc::clone(&self.0)));
        registrar.provide(Priority::DEFAULT, connector);
        Ok(())
    }
}

impl Connector for NeverConnected {
    fn start_connection<'a>(
        &'a self,
        code: &'a str,
    ) -> BoxFuture<'a, Result<ConnectionStarted, PluginError>> {
        Box::pin(async move {
            Ok(ConnectionStarted {
                url: format!("https://chat.example.com/{code}"),
                pending: code.to_owned(),
                ..ConnectionStarted::default()
            })
        })
    }

    fn poll_connection<'a>(
        &'a self,
        _pending: &'a str,
    ) -> BoxFuture<'a, Result<Option<String>, PluginError>> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Box::pin(async { Ok(None) })
    }
}

#[tokio::test]
async fn a_link_nobody_finishes_within_ten_minutes_expires_and_is_polled_no_more() {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
    let polls = Arc::new(AtomicUsize::new(0));
    let plugins = Registry::new();
    plugins
        .enable(Arc::new(NeverConnected(Arc::clone(&polls))))
        .unwrap();
    let connections = Connections::new(db, plugins.extension_points());
    let chat = PluginId::from_static("chat");
    // After opening the database, whose pool times out by the clock.
    tokio::time::pause();

    connections.start(1, &chat).await.unwrap();
    tokio::time::sleep(Duration::from_mins(9)).await;
    let polled_while_waiting = polls.load(Ordering::Relaxed);
    tokio::time::sleep(Duration::from_secs(61)).await;
    let polled_by_expiry = polls.load(Ordering::Relaxed);
    tokio::time::sleep(Duration::from_secs(60)).await;
    let polled_after = polls.load(Ordering::Relaxed);
    tokio::time::resume();

    assert!(polled_while_waiting > 0, "the link was never polled");
    assert_eq!(polled_after, polled_by_expiry, "polled after it expired");
    assert_eq!(
        connections.list(1).await.unwrap(),
        [(chat, State::NotConnected)]
    );
}

#[tokio::test]
async fn disabling_the_connector_ends_its_link_poll_at_once() {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
    let polls = Arc::new(AtomicUsize::new(0));
    let plugins = Registry::new();
    plugins
        .enable(Arc::new(NeverConnected(Arc::clone(&polls))))
        .unwrap();
    let connections = Connections::new(db, plugins.extension_points());
    let chat = PluginId::from_static("chat");
    // After opening the database, whose pool times out by the clock.
    tokio::time::pause();

    connections.start(1, &chat).await.unwrap();
    tokio::time::sleep(POLL_EVERY * 3).await;
    let disabled_at = Instant::now();
    assert!(plugins.disable(&chat).await);
    let disabled_after = disabled_at.elapsed();
    let polled_by_disable = polls.load(Ordering::Relaxed);
    let waiting_after = connections.lock().len();
    tokio::time::sleep(Duration::from_mins(1)).await;
    let polled_after = polls.load(Ordering::Relaxed);
    tokio::time::resume();

    assert!(polled_by_disable > 0, "the link was never polled");
    assert_eq!(
        disabled_after,
        Duration::ZERO,
        "the poll outlived a disable"
    );
    assert_eq!(waiting_after, 0, "the link still waits");
    assert_eq!(polled_after, polled_by_disable, "polled after disable");
}
