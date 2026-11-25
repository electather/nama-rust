//! Reading what the Server logs.

use std::{
    io,
    sync::{Arc, Mutex, OnceLock, PoisonError},
    time::{Duration, Instant},
};

use tracing_subscriber::layer::SubscriberExt as _;

use super::server::WAIT;

/// Everything the Server logs, to the test process's one global subscriber: a thread-local one
/// wouldn't see what Observers log on the runtime's other threads.
#[derive(Clone, Default)]
pub(crate) struct Log(Arc<Mutex<Vec<u8>>>);

impl Log {
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    pub(crate) fn global() -> &'static Self {
        // Kept with one test per process: a test calls this before its Server logs and again to
        // read the log, and installing a second global subscriber fails.
        static LOG: OnceLock<Log> = OnceLock::new();
        LOG.get_or_init(|| {
            let log = Self::default();
            let writer = log.clone();
            let subscriber = tracing_subscriber::fmt()
                .with_max_level(tracing::Level::INFO)
                .with_writer(move || writer.clone())
                .finish()
                .with(nama_server::diagnostics::plugin_lines_layer());
            tracing::subscriber::set_global_default(subscriber).unwrap();
            log
        })
    }

    pub(crate) fn contents(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap_or_else(PoisonError::into_inner)).into_owned()
    }

    /// How many logged lines contain every one of `needles`.
    pub(crate) fn count(&self, needles: &[&str]) -> usize {
        self.contents()
            .lines()
            .filter(|line| needles.iter().all(|needle| line.contains(needle)))
            .count()
    }

    /// Waits until `times` lines containing every one of `needles` are logged.
    pub(crate) async fn wait_for(&self, needles: &[&str], times: usize) {
        let started = Instant::now();
        while self.count(needles) < times {
            assert!(
                started.elapsed() < WAIT,
                "nothing logged {needles:?}: {}",
                self.contents()
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}

impl io::Write for Log {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
