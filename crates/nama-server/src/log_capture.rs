//! What a unit test's thread logs, captured as "Tests" in `crates/nama-server/AGENTS.md` requires.

use std::{
    fs,
    sync::{Arc, Once},
};

use tempfile::NamedTempFile;
use tracing::{
    Event, Level, Metadata, span,
    subscriber::{DefaultGuard, Interest, Subscriber},
};

/// What the current thread logs between [`CapturedLog::start`] and [`CapturedLog::finish`].
pub(crate) struct CapturedLog {
    file: NamedTempFile,
    logging: DefaultGuard,
}

impl CapturedLog {
    /// Starts capturing what the current thread logs, including the tasks of a current-thread
    /// runtime it drives.
    ///
    /// tracing caches each callsite's interest process-wide. While one dispatcher is live, such as
    /// this capture's thread-local one, the first thread to reach a callsite decides it from its
    /// own default, so another test's thread with no subscriber caches `never` and this capture
    /// misses the line (#945). A global [`Silent`] subscriber keeps a second dispatcher live, so
    /// every callsite comes out `sometimes` and each event asks its own thread's subscriber.
    pub(crate) fn start() -> Self {
        Self::start_at(Level::INFO)
    }

    /// Like [`CapturedLog::start`], capturing lines down to `level`; below INFO, a test also sees
    /// what sqlx and wasmtime log, so it looks for its own line instead of the whole log.
    pub(crate) fn start_at(level: Level) -> Self {
        static SILENT: Once = Once::new();
        SILENT.call_once(|| {
            tracing::subscriber::set_global_default(Silent)
                .expect("no other unit test sets a global subscriber");
        });
        let file = NamedTempFile::new().unwrap();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(Arc::new(file.reopen().unwrap()))
            .with_ansi(false)
            .with_max_level(level)
            .finish();
        let logging = tracing::subscriber::set_default(subscriber);
        Self { file, logging }
    }

    /// Stops capturing and returns what was logged.
    pub(crate) fn finish(self) -> String {
        drop(self.logging);
        fs::read_to_string(self.file.path()).unwrap()
    }
}

/// A subscriber that enables nothing yet leaves every callsite's interest `sometimes`.
struct Silent;

impl Subscriber for Silent {
    fn register_callsite(&self, _: &'static Metadata<'static>) -> Interest {
        Interest::sometimes()
    }

    fn enabled(&self, _: &Metadata<'_>) -> bool {
        false
    }

    fn new_span(&self, _: &span::Attributes<'_>) -> span::Id {
        span::Id::from_u64(1)
    }

    fn record(&self, _: &span::Id, _: &span::Record<'_>) {}

    fn record_follows_from(&self, _: &span::Id, _: &span::Id) {}

    fn event(&self, _: &Event<'_>) {}

    fn enter(&self, _: &span::Id) {}

    fn exit(&self, _: &span::Id) {}
}

#[cfg(test)]
#[path = "log_capture_tests.rs"]
mod tests;
