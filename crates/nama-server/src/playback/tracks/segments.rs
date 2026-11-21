//! Segments: where each Version's Intro, Recap and Credits are, kept per Version (ADR 0074). The
//! Kernel reads them from the Version's Chapter names, then asks its Library's Segment sources in
//! the Library's order for the kinds still missing (ADR 0075).
//!
//! Asking is split so a jobs system can take it over: [`Segments::due`] works out from what's
//! stored which Versions to ask about, and [`Segments::ask`] asks about one, needing only which
//! sources said to wait, and safe to run twice. The loop [`Segments::start`] runs joins them, in
//! the background after a probe pass and every so often, never while a Version plays.

use std::{
    collections::HashMap,
    sync::{Arc, LazyLock},
    time::Duration,
};

use buffa::EnumValue;
use nama_plugin_api::{
    PluginId,
    contract::{self, SegmentSearch, TitleId, TitleKind},
    segment_source::SegmentSource,
};
use regex::Regex;
use sqlx::SqliteConnection;
use tokio::sync::Notify;
use tokio_util::{sync::CancellationToken, task::TaskTracker};

use super::{Chapter, ignore_removed};
use crate::{
    db::{self, Database},
    library::{Contents, ItemKind, Libraries, LibraryError},
    plugins::{ExtensionPoints, Failure, Retryable},
};

/// How long a Version still missing a kind waits to be asked about again, as crowdsourced
/// services fill in over time (ADR 0075).
// ponytail: a source an Admin lists later, or a Version Matched later, waits out the rest of these
// 30 days too; record which sources each ask reached if Admins expect it sooner.
const ASK_AGAIN_AFTER: Duration = Duration::from_hours(30 * 24);

/// How long a Version a Segment source failed for waits to be asked about again, so a service
/// that's down or limiting its rate isn't asked about every Version on every pass.
const ASK_AGAIN_AFTER_FAILING: Duration = Duration::from_hours(1);

/// Until when, in [`db::now_ms`]'s milliseconds, each Segment source that said to try again is
/// asked about no Version.
type Paused = HashMap<PluginId, i64>;

/// What a Version's Segment sources came to, later ones outranking earlier ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SourcesSaid {
    /// Each asked answered, if only that it doesn't know.
    Answered,
    /// One failed.
    Failed,
    /// One said to try again, or was skipped while it waits, so the Version stays due.
    Waiting,
}

impl SourcesSaid {
    fn ask_again_after(self) -> Duration {
        match self {
            Self::Answered => ASK_AGAIN_AFTER,
            Self::Failed => ASK_AGAIN_AFTER_FAILING,
            Self::Waiting => Duration::ZERO,
        }
    }
}

/// What a Segment is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
pub(crate) enum SegmentKind {
    Intro,
    Recap,
    Credits,
}

impl SegmentKind {
    const ALL: [Self; 3] = [Self::Intro, Self::Recap, Self::Credits];
}

/// A stretch of a Version the player acts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Segment {
    pub(crate) kind: SegmentKind,
    /// From the file's start, as Chapters are.
    pub(crate) start_ms: i64,
    /// `None` runs to the end.
    pub(crate) end_ms: Option<i64>,
}

/// The Kernel's own Chapter names for each kind, matched as whole words ignoring case, the first
/// that matches naming the kind. Written for Nama: Jellyfin's and Intro Skipper's lists are GPL.
#[expect(
    clippy::expect_used,
    reason = "the patterns are constants, and the Segments tests compile them"
)]
static CHAPTER_NAMES: LazyLock<[(SegmentKind, Regex); 3]> = LazyLock::new(|| {
    [
        (
            SegmentKind::Intro,
            r"(?i)\b(intro|opening|main title)\b|^\s*op\d*\s*$",
        ),
        (
            SegmentKind::Recap,
            r"(?i)\b(recap|previously|last time|story so far)\b",
        ),
        (
            SegmentKind::Credits,
            r"(?i)\b(credits|ending|outro|closing)\b|^\s*ed\d*\s*$",
        ),
    ]
    .map(|(kind, names)| {
        (
            kind,
            Regex::new(names).expect("a constant pattern compiles"),
        )
    })
});

/// Finds and keeps where Versions' Segments are.
#[derive(Debug, Clone)]
pub(crate) struct Segments {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    db: Arc<Database>,
    contents: Contents,
    libraries: Libraries,
    points: ExtensionPoints,
    /// The loop, so shutting down can wait for it.
    tasks: TaskTracker,
    /// Cancelled when the Server shuts down.
    cancel: CancellationToken,
}

impl Segments {
    /// Keeps Segments in `db`, asking the Segment sources among `points`.
    pub(crate) fn new(db: Arc<Database>, points: ExtensionPoints) -> Self {
        Self {
            inner: Arc::new(Inner {
                contents: Contents::new(Arc::clone(&db)),
                libraries: Libraries::new(Arc::clone(&db)),
                db,
                points,
                tasks: TaskTracker::new(),
                cancel: CancellationToken::new(),
            }),
        }
    }

    /// Asks about each Version due, one at a time, now, then whenever `probed` is notified and at
    /// least every `every`, until [`Self::shutdown`].
    pub(crate) fn start(&self, probed: Arc<Notify>, every: Duration) {
        let segments = self.clone();
        self.inner.tasks.spawn(async move {
            let cancel = segments.inner.cancel.clone();
            cancel
                .run_until_cancelled(segments.run(probed, every))
                .await;
        });
    }

    async fn run(&self, probed: Arc<Notify>, every: Duration) {
        loop {
            self.ask_due().await;
            tokio::select! {
                () = probed.notified() => {}
                () = tokio::time::sleep(every) => {}
            }
        }
    }

    /// Stops asking, waiting for the Version being asked about.
    pub(crate) async fn shutdown(&self) {
        self.inner.cancel.cancel();
        self.inner.tasks.close();
        self.inner.tasks.wait().await;
    }

    /// Asks about each Version due, then, while a source's wait left any due, waits for the first
    /// wait to end and asks about those again.
    async fn ask_due(&self) {
        let mut paused = Paused::new();
        loop {
            let due = match self.due().await {
                Ok(due) => due,
                Err(err) => {
                    tracing::error!("could not read which Versions' Segments to look for: {err}");
                    return;
                }
            };
            let mut waiting = false;
            for version in due {
                match self.ask(version, &mut paused).await {
                    Ok(left_due) => waiting |= left_due,
                    Err(err) => tracing::error!(
                        version,
                        "could not keep where a Version's Segments are: {err}"
                    ),
                }
            }
            if !waiting {
                return;
            }
            let now = db::now_ms();
            let first_ends = (paused.values().copied())
                .filter(|&until| until > now)
                .min()
                .unwrap_or(now);
            let wait_ms = u64::try_from(first_ends - now).unwrap_or_default();
            tokio::time::sleep(Duration::from_millis(wait_ms)).await;
        }
    }

    /// The probed Versions of a known length that lack a kind of Segment and are due to be asked
    /// about: never since they were probed, or not for [`ASK_AGAIN_AFTER`], or
    /// [`ASK_AGAIN_AFTER_FAILING`] if a source failed.
    async fn due(&self) -> sqlx::Result<Vec<i64>> {
        let kinds = i64::try_from(SegmentKind::ALL.len()).unwrap_or(i64::MAX);
        let now = db::now_ms();
        sqlx::query_scalar!(
            r#"SELECT version_id AS "version_id!" FROM version_probes
               WHERE duration_ms IS NOT NULL
                 AND (segments_due_ms IS NULL OR segments_due_ms <= ?)
                 AND (SELECT COUNT(*) FROM segments
                      WHERE segments.version_id = version_probes.version_id) < ?
               ORDER BY version_id"#,
            now,
            kinds,
        )
        .fetch_all(self.inner.db.reader())
        .await
    }

    /// Finds the Segments of the kinds the Version `version` lacks, from its Chapter names, then
    /// from the Segment sources its Library lists, in order, skipping those `paused`, keeping the
    /// first found of each kind, and records when to ask about it again. Whether a source's wait
    /// left it due. Does nothing for a Version gone or not probed.
    async fn ask(&self, version: i64, paused: &mut Paused) -> sqlx::Result<bool> {
        let Some(Probed {
            duration_ms,
            chapters,
            stamp,
        }) = self.probed(version).await?
        else {
            return Ok(false);
        };
        let stored = self.kinds_stored(version).await?;
        let mut found = Found {
            duration_ms,
            missing: SegmentKind::ALL
                .into_iter()
                .filter(|kind| !stored.contains(kind))
                .collect(),
            segments: Vec::new(),
        };
        for segment in named_by(&chapters) {
            found.offer(segment, None);
        }
        let said = self.ask_sources(version, &mut found, paused).await?;
        let due_ms = db::now_ms().saturating_add(millis(said.ask_again_after()));
        let Found { segments, .. } = found;
        let kept = (self.inner.db)
            .write(move |tx| Box::pin(keep(tx, version, stamp, segments, due_ms)))
            .await;
        ignore_removed(kept)?;
        Ok(said == SourcesSaid::Waiting)
    }

    /// The Version `version`'s probe, if it's probed and its length is known.
    async fn probed(&self, version: i64) -> sqlx::Result<Option<Probed>> {
        let reader = self.inner.db.reader();
        let row = sqlx::query!(
            "SELECT duration_ms, probe_version, size, modified_ns FROM version_probes
             WHERE version_id = ?",
            version
        )
        .fetch_optional(reader)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let Some(duration_ms) = row.duration_ms else {
            return Ok(None);
        };
        let stamp = ProbeStamp {
            probe_version: row.probe_version,
            size: row.size,
            modified_ns: row.modified_ns,
        };
        let chapters = sqlx::query_as!(
            Chapter,
            "SELECT start_ms, title FROM chapters WHERE version_id = ? ORDER BY position",
            version
        )
        .fetch_all(reader)
        .await?;
        Ok(Some(Probed {
            duration_ms,
            chapters,
            stamp,
        }))
    }

    async fn kinds_stored(&self, version: i64) -> sqlx::Result<Vec<SegmentKind>> {
        sqlx::query_scalar!(
            r#"SELECT kind AS "kind: SegmentKind" FROM segments WHERE version_id = ?"#,
            version
        )
        .fetch_all(self.inner.db.reader())
        .await
    }

    /// Asks the Segment sources the Version `version`'s Library lists, in its order, for the kinds
    /// `found` still lacks, until none is, skipping those `paused` and pausing those that say to
    /// try again. Asks none about a Version whose title has no ids, as Unmatched, since a source
    /// finds Segments by them.
    // ponytail: a Version left waiting asks the Library's other sources again on the next pass;
    // record which sources each ask reached if that costs a service too much.
    async fn ask_sources(
        &self,
        version: i64,
        found: &mut Found,
        paused: &mut Paused,
    ) -> sqlx::Result<SourcesSaid> {
        let mut said = SourcesSaid::Answered;
        if found.missing.is_empty() {
            return Ok(said);
        }
        let Some(item) = self.inner.contents.first_item(version).await? else {
            return Ok(said);
        };
        if item.ids.is_empty() {
            return Ok(said);
        }
        let sources = match self.inner.libraries.get(item.library).await {
            Ok(library) => library.segment_sources,
            Err(LibraryError::Database(err)) => return Err(err),
            // Gone since, and its Versions with it.
            Err(_) => return Ok(said),
        };
        let (season, episode) = item.episode.unzip();
        let search = SegmentSearch {
            kind: match item.kind {
                ItemKind::Movie => TitleKind::TITLE_KIND_MOVIE,
                ItemKind::Episode => TitleKind::TITLE_KIND_SHOW,
            }
            .into(),
            ids: (item.ids.iter())
                .map(|id| TitleId {
                    namespace: id.namespace().to_owned(),
                    value: id.value().to_owned(),
                    ..TitleId::default()
                })
                .collect(),
            season: season.and_then(|number| u32::try_from(number).ok()),
            episode: episode.and_then(|number| u32::try_from(number).ok()),
            duration_ms: u64::try_from(found.duration_ms).unwrap_or_default(),
            ..SegmentSearch::default()
        };
        for plugin in sources.into_iter().map(PluginId::from) {
            if found.missing.is_empty() {
                break;
            }
            if paused
                .get(&plugin)
                .is_some_and(|&until| until > db::now_ms())
            {
                said = SourcesSaid::Waiting;
                continue;
            }
            let search = &search;
            let answer = (self.inner.points)
                .ask(&plugin, move |source: Arc<dyn SegmentSource>| async move {
                    source.search(search).await.map_err(Retryable)
                })
                .await;
            match answer.map(|answer| answer.result) {
                Some(Ok(results)) => (results.segments.iter().filter_map(from_result))
                    .for_each(|segment| found.offer(segment, Some(&plugin))),
                Some(Err(Failure {
                    kind: Some(wait), ..
                })) => {
                    paused.insert(plugin, db::now_ms().saturating_add(millis(wait)));
                    said = SourcesSaid::Waiting;
                }
                // The Plugins page shows the failure; logged on every Version, it would flood.
                Some(Err(Failure {
                    kind: None,
                    message,
                })) => {
                    tracing::debug!(%plugin, version, "a Segment source failed: {message}");
                    said = said.max(SourcesSaid::Failed);
                }
                // Not enabled, or it panicked and is disabled now.
                None => {}
            }
        }
        Ok(said)
    }
}

fn millis(wait: Duration) -> i64 {
    i64::try_from(wait.as_millis()).unwrap_or(i64::MAX)
}

/// The Segments the Version `version` has, earliest first.
pub(crate) async fn segments_of(db: &Database, version: i64) -> sqlx::Result<Vec<Segment>> {
    sqlx::query_as!(
        Segment,
        r#"SELECT kind AS "kind: SegmentKind", start_ms, end_ms FROM segments
           WHERE version_id = ? ORDER BY start_ms"#,
        version
    )
    .fetch_all(db.reader())
    .await
}

/// What [`Segments::ask`] reads of a Version's probe.
struct Probed {
    duration_ms: i64,
    chapters: Vec<Chapter>,
    stamp: ProbeStamp,
}

/// Which probe of a Version Segments were found after. The same file probed by the same code reads
/// the same, so Segments found after one probe fit another with the same stamp.
#[derive(Debug, Clone, Copy)]
struct ProbeStamp {
    probe_version: i64,
    size: i64,
    modified_ns: i64,
}

/// The Segments found for one Version, of the kinds it lacked.
struct Found {
    duration_ms: i64,
    /// The kinds still to find.
    missing: Vec<SegmentKind>,
    /// Each with the Plugin whose source gave it, `None` for the Kernel.
    segments: Vec<(Segment, Option<PluginId>)>,
}

impl Found {
    /// Keeps `segment`, from `source`, if its kind is still missing and it fits the Version.
    fn offer(&mut self, segment: Segment, source: Option<&PluginId>) {
        let Some(place) = self.missing.iter().position(|kind| *kind == segment.kind) else {
            return;
        };
        if let Some(segment) = fitted(segment, self.duration_ms) {
            self.missing.swap_remove(place);
            self.segments.push((segment, source.cloned()));
        }
    }
}

/// `segment` with an end past `duration_ms` cut back to it, or `None` if it ends before it starts
/// or starts at or past the end.
fn fitted(segment: Segment, duration_ms: i64) -> Option<Segment> {
    let ends_first = segment.end_ms.is_some_and(|end| end <= segment.start_ms);
    if ends_first || segment.start_ms >= duration_ms {
        return None;
    }
    Some(Segment {
        end_ms: segment.end_ms.map(|end| end.min(duration_ms)),
        ..segment
    })
}

/// The Segments `chapters`' names say, in their order: an Intro or a Recap runs to the next
/// Chapter's start, and Credits to the end.
fn named_by(chapters: &[Chapter]) -> impl Iterator<Item = Segment> + '_ {
    chapters.iter().enumerate().filter_map(|(place, chapter)| {
        let (kind, _) = (CHAPTER_NAMES.iter()).find(|(_, names)| names.is_match(&chapter.title))?;
        let end_ms = match kind {
            SegmentKind::Credits => None,
            SegmentKind::Intro | SegmentKind::Recap => {
                chapters.get(place + 1).map(|next| next.start_ms)
            }
        };
        Some(Segment {
            kind: *kind,
            start_ms: chapter.start_ms,
            end_ms,
        })
    })
}

/// What a Segment source answered, or `None` for a kind the Kernel doesn't know.
fn from_result(result: &contract::SegmentResult) -> Option<Segment> {
    use contract::SegmentKind as Theirs;
    let kind = match result.kind {
        EnumValue::Known(Theirs::SEGMENT_KIND_INTRO) => SegmentKind::Intro,
        EnumValue::Known(Theirs::SEGMENT_KIND_RECAP) => SegmentKind::Recap,
        EnumValue::Known(Theirs::SEGMENT_KIND_CREDITS) => SegmentKind::Credits,
        EnumValue::Known(Theirs::SEGMENT_KIND_UNSPECIFIED) | EnumValue::Unknown(_) => return None,
    };
    // Past `i64::MAX` is past any Version's end, which `fitted` drops or cuts back.
    let ms = |ms: u64| i64::try_from(ms).unwrap_or(i64::MAX);
    Some(Segment {
        kind,
        start_ms: ms(result.start_ms),
        end_ms: result.end_ms.map(ms),
    })
}

/// Keeps `segments` for the Version `version`, leaving any of a kind it has, and has it asked
/// about again at `due_ms`, unless it's been probed again since its probe `stamp`.
async fn keep(
    conn: &mut SqliteConnection,
    version: i64,
    stamp: ProbeStamp,
    segments: Vec<(Segment, Option<PluginId>)>,
    due_ms: i64,
) -> sqlx::Result<()> {
    // Its file may have changed while the sources were asked; the new probe is due by itself.
    let same_probe = sqlx::query!(
        "UPDATE version_probes SET segments_due_ms = ?
         WHERE version_id = ? AND probe_version = ? AND size = ? AND modified_ns = ?",
        due_ms,
        version,
        stamp.probe_version,
        stamp.size,
        stamp.modified_ns,
    )
    .execute(&mut *conn)
    .await?;
    if same_probe.rows_affected() == 0 {
        return Ok(());
    }
    for (segment, source) in segments {
        let source = source.as_ref().map(PluginId::as_str);
        sqlx::query!(
            "INSERT OR IGNORE INTO segments (version_id, kind, start_ms, end_ms, source)
             VALUES (?, ?, ?, ?, ?)",
            version,
            segment.kind,
            segment.start_ms,
            segment.end_ms,
            source,
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "segments_tests.rs"]
mod tests;
