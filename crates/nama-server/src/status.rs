//! The Server's setup and load, which no request's trace shows: a `server status` span every
//! [`EVERY`] while Diagnostics are recorded, for the admin's collector and for what they share
//! (ADR 0054). Counts are rounded down to a power of ten, so they say how big a Server is without
//! telling one household from another.

use std::{path::PathBuf, sync::Arc, time::Duration};

use tokio::task::JoinHandle;
use tracing::{Instrument as _, field::Empty};

use crate::{
    db::Database,
    diagnostics,
    ffmpeg::{Abilities, Ffmpeg},
    library::Contents,
    playback::RunningPlans,
};

/// How often the status is recorded.
const EVERY: Duration = Duration::from_mins(15);

/// What the status reads from.
pub(crate) struct Sources {
    pub(crate) db: Arc<Database>,
    pub(crate) contents: Contents,
    pub(crate) ffmpeg: Arc<Ffmpeg>,
    pub(crate) abilities: Abilities,
    pub(crate) running: RunningPlans,
    pub(crate) data_dir: PathBuf,
}

/// Records the status now and every [`EVERY`] after, until the returned task is aborted.
pub(crate) fn keep_recording(sources: Sources) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut every = tokio::time::interval(EVERY);
        loop {
            every.tick().await;
            record(&sources).await;
        }
    })
}

/// Records the status, unless nothing records spans.
async fn record(sources: &Sources) {
    let span = tracing::info_span!(
        parent: None,
        "server status",
        ffmpeg.version = Empty,
        hardware = Empty,
        sqlite.version = Empty,
        libraries = Empty,
        library_items = Empty,
        users = Empty,
        writes.queued = Empty,
        transcodes = Empty,
        memory.rss = Empty,
        disk.free = Empty,
        shares.refused = Empty,
    );
    // Its counts query the database and the hardware test may run ffmpeg: not for nothing.
    if span.is_disabled() {
        return;
    }
    let (counts, items, can_do) = async {
        tokio::join!(
            sqlx::query!(
                r#"SELECT
                       (SELECT COUNT(*) FROM libraries) AS "libraries!: i64",
                       (SELECT COUNT(*) FROM users) AS "users!: i64",
                       sqlite_version() AS "sqlite_version!: String""#
            )
            .fetch_one(sources.db.reader()),
            sources.contents.item_count(),
            sources.abilities.can_do(),
        )
    }
    .instrument(span.clone())
    .await;
    if let Ok(ffmpeg) = &sources.ffmpeg.ffmpeg {
        span.record("ffmpeg.version", ffmpeg.version.as_str());
    }
    span.record(
        "hardware",
        can_do
            .automatic()
            .map_or("none", |hardware| hardware.accel.name()),
    );
    match counts.and_then(|counts| Ok((counts, items?))) {
        Ok((counts, items)) => {
            span.record("sqlite.version", counts.sqlite_version.as_str());
            span.record("libraries", rounded(counts.libraries));
            span.record("library_items", rounded(items));
            span.record("users", rounded(counts.users));
        }
        Err(err) => tracing::warn!("could not count for the Server's status: {err}"),
    }
    span.record("writes.queued", sources.db.queued_writes());
    span.record("transcodes", sources.running.running_transcodes().all);
    if let Some(rss) = rss() {
        span.record("memory.rss", rss);
    }
    if let Some(free) = disk_free(&sources.data_dir) {
        span.record("disk.free", free);
    }
    span.record("shares.refused", diagnostics::take_shares_refused());
}

/// `n` rounded down to a power of ten, such as 1000 for 1234, or 0.
fn rounded(n: i64) -> i64 {
    u64::try_from(n)
        .ok()
        .and_then(u64::checked_ilog10)
        .map_or(0, |digits| 10_i64.pow(digits))
}

/// The Server's resident memory in bytes, where the OS says (Linux).
fn rss() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with("VmRSS:"))?;
    let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    kib.checked_mul(1024)
}

/// The bytes free for the Server on the disk `data_dir` is on.
#[cfg(unix)]
fn disk_free(data_dir: &std::path::Path) -> Option<u64> {
    let stats = rustix::fs::statvfs(data_dir).ok()?;
    stats.f_bavail.checked_mul(stats.f_frsize)
}

/// The bytes free for the Server on the disk `data_dir` is on: unknown off Unix.
#[cfg(not(unix))]
fn disk_free(_data_dir: &std::path::Path) -> Option<u64> {
    None
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
