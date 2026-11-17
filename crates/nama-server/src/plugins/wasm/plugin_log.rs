//! An Installed Plugin's Plugin Log: the lines it writes through `nama.log` (ADR 0061).

use std::{
    mem,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use extism::{CurrentPlugin, UserData, Val};
use nama_plugin_api::PluginId;
use tracing::{Level, enabled, event, warn};

use super::without_secrets;
use crate::db;

/// The target every Installed Plugin's line is logged under.
const TARGET: &str = "nama::plugin";

/// The most of a line logged, in bytes, so a Plugin can't fill the log with one (ADR 0039).
const MAX_LINE_BYTES: usize = 2048;

/// How many lines a Plugin may log at once, and how many more each second.
const BURST: u64 = 50;
const LINES_PER_SECOND: u64 = 10;

/// How often the Server warns that a Plugin's lines were dropped over its rate.
const WARN_EVERY_MS: i64 = 60_000;

// Changing it changes what the Kernel gives a Plugin: see `crates/nama-plugin-proto/AGENTS.md`.
/// `nama.log`, taking a level, 0 `debug` to 3 `error`, and the offset of a block holding the line,
/// which the Plugin frees. Writes it to `log`, or drops it for no enablement. Unlike the Kernel's
/// functions it doesn't pause the call's clock, so a Plugin logging in a loop still meets its
/// limit. A level or block it can't read drops the line, never a trap.
#[expect(clippy::needless_pass_by_value, reason = "Extism's signature")]
pub(super) fn import(
    plugin: &mut CurrentPlugin,
    inputs: &[Val],
    _: &mut [Val],
    log: UserData<Option<Arc<PluginLog>>>,
) -> Result<(), extism::Error> {
    let Some(log) = log
        .get()?
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
    else {
        return Ok(());
    };
    let level = match inputs.first().and_then(Val::i32) {
        Some(0) => Level::DEBUG,
        Some(1) => Level::INFO,
        Some(2) => Level::WARN,
        Some(3) => Level::ERROR,
        _ => return Ok(()),
    };
    let Some(line) = (inputs.get(1).and_then(Val::i64))
        .and_then(|offset| plugin.memory_handle(offset.cast_unsigned()))
    else {
        return Ok(());
    };
    log.write(level, plugin.memory_bytes(line)?, db::now_ms());
    Ok(())
}

/// Where one enablement of an Installed Plugin writes its lines.
pub(super) struct PluginLog {
    plugin: PluginId,
    /// The values of its secret settings, blanked in each line.
    secrets: Arc<[String]>,
    bucket: Mutex<Bucket>,
}

impl PluginLog {
    pub(super) fn new(plugin: PluginId, secrets: Arc<[String]>) -> Self {
        Self {
            plugin,
            secrets,
            bucket: Mutex::new(Bucket {
                tokens: BURST * 1000,
                refilled_at_ms: 0,
                dropped: 0,
                dropping_since_ms: 0,
            }),
        }
    }

    /// Logs `text` at `level`, naming the Plugin, with its secrets blanked and cut at
    /// [`MAX_LINE_BYTES`], as written at `now_ms`. A line over the Plugin's rate is dropped and
    /// counted, and the count logged once a minute; one the level filter drops isn't counted.
    pub(super) fn write(&self, level: Level, text: &[u8], now_ms: i64) {
        self.warn_of_dropped(now_ms);
        // `tracing` fixes an event's level where it is written, so each level is written apart.
        macro_rules! write_at {
            ($level:expr) => {
                if enabled!(target: TARGET, $level) && self.lock().take(now_ms) {
                    event!(target: TARGET, $level, plugin = %self.plugin, "{}", self.line(text));
                }
            };
        }
        match level {
            Level::ERROR => write_at!(Level::ERROR),
            Level::WARN => write_at!(Level::WARN),
            Level::INFO => write_at!(Level::INFO),
            _ => write_at!(Level::DEBUG),
        }
    }

    /// `text` as the line logged: blanked, then cut.
    fn line(&self, text: &[u8]) -> String {
        // Only what can reach the cut is read, so a huge line costs no more than a short one,
        // with room for a secret that starts before the cut to be read whole.
        let longest_secret = self.secrets.iter().map(String::len).max().unwrap_or(0);
        let read = text.get(..MAX_LINE_BYTES + longest_secret).unwrap_or(text);
        let read_part = read.len() < text.len();
        // Blanked before cutting, so a cut can't leave the start of a secret showing.
        let mut line = without_secrets(String::from_utf8_lossy(read).into_owned(), &self.secrets);
        if read_part && longest_secret > 0 {
            // A secret the read ran through ends the text unblanked, and blanking may have pulled
            // it within the cut. Dropping a secret's length drops it, plus 3 bytes in case the
            // read also split a character, which reads as a 3-byte U+FFFD.
            let kept = line.len().saturating_sub(longest_secret + 3);
            line.truncate(line.floor_char_boundary(kept));
        }
        if read_part || line.len() > MAX_LINE_BYTES {
            line.truncate(line.floor_char_boundary(MAX_LINE_BYTES));
            line.push_str(" [cut]");
        }
        line
    }

    /// Warns how many lines were dropped, once [`WARN_EVERY_MS`] has passed since the first.
    // ponytail: warns only when the Plugin next writes or its enablement ends, so a flood's count
    // waits for either; warn from a timer if admins miss counts.
    fn warn_of_dropped(&self, now_ms: i64) {
        let dropped = {
            let mut bucket = self.lock();
            if bucket.dropped == 0
                || now_ms.saturating_sub(bucket.dropping_since_ms) < WARN_EVERY_MS
            {
                return;
            }
            mem::take(&mut bucket.dropped)
        };
        warn_dropped(&self.plugin, dropped);
    }

    fn lock(&self) -> MutexGuard<'_, Bucket> {
        self.bucket.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Drop for PluginLog {
    /// Warns of the lines dropped since the last warning, which no later line will.
    fn drop(&mut self) {
        let dropped = mem::take(&mut self.lock().dropped);
        if dropped > 0 {
            warn_dropped(&self.plugin, dropped);
        }
    }
}

fn warn_dropped(plugin: &PluginId, dropped: u64) {
    warn!(
        %plugin,
        dropped,
        "dropped {dropped} Plugin Log lines over its limit of {LINES_PER_SECOND} a second"
    );
}

/// A token bucket of lines, holding up to [`BURST`] and refilled by [`LINES_PER_SECOND`].
struct Bucket {
    /// In thousandths of a line, so the refill is a whole [`LINES_PER_SECOND`] each millisecond.
    tokens: u64,
    refilled_at_ms: i64,
    /// Lines dropped since the last warning.
    dropped: u64,
    /// When the first of [`Self::dropped`] was dropped.
    dropping_since_ms: i64,
}

impl Bucket {
    /// Takes a line's token at `now_ms`, or counts the line dropped when there is none.
    fn take(&mut self, now_ms: i64) -> bool {
        let elapsed_ms = u64::try_from(now_ms.saturating_sub(self.refilled_at_ms)).unwrap_or(0);
        self.tokens = (self.tokens)
            .saturating_add(elapsed_ms.saturating_mul(LINES_PER_SECOND))
            .min(BURST * 1000);
        self.refilled_at_ms = now_ms;
        if let Some(left) = self.tokens.checked_sub(1000) {
            self.tokens = left;
            return true;
        }
        if self.dropped == 0 {
            self.dropping_since_ms = now_ms;
        }
        self.dropped += 1;
        false
    }
}

#[cfg(test)]
#[path = "plugin_log_tests.rs"]
mod tests;
