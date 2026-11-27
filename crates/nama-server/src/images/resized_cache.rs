//! The resized cache: the local copies of stored images at the widths on
//! [`WIDTHS`](super::WIDTHS), the markers of those an image goes unresized at, each resized
//! image's thumbhash, the measured size copies made ahead stop at, and eviction of the copies sent
//! least recently (ADR 0039, ADR 0052). Only this module writes or deletes them (`AGENTS.md`).
//!
//! Lock order: a copy is made under the resize lock (`ResizedCache::measured`), then a whole-read
//! permit, then the image's write lock. A copy, marker or thumbhash is written only while that
//! write lock is held, with the original read under it. So an [`ImageStore::forget`] made under an
//! image's write lock, with the original deleted before the lock goes, is final: a resize after it
//! finds no original.

use std::{
    fs::{File, FileTimes},
    io,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime},
};

use tracing::Instrument as _;
use walkdir::WalkDir;

use super::{
    ImageStore, Resize, file_name,
    resized::{self, Copied, Format, ResizeThread},
    row_key, under, unescape, unless_gone,
};
use crate::{db, diagnostics};

/// The folder in the data directory resized copies are kept in (`AGENTS.md`).
pub(super) const RESIZED_DIR: &str = "resized";

/// How full, in percent of its limit, the resized cache may get from copies made ahead. The rest
/// is left to copies requests ask for, so a scan of a Library that outgrows the cache stops
/// rather than evicting what the last one made, to be made again by the next (#2253).
const AHEAD_UP_TO_PERCENT: u64 = 80;

/// The resized cache's state, which the image store's clones share.
#[derive(Debug, Clone)]
pub(super) struct ResizedCache {
    resizer: ResizeThread,
    /// The resize lock: held from fetching an original to resize until its copy is made, so one
    /// original at a time is held in memory for it, and a request that waited behind the one
    /// making its copy finds it rather than fetching the original again. It holds the bytes the
    /// copies take, as measured last, or `None` until a copy made ahead first needs it.
    measured: Arc<tokio::sync::Mutex<Option<u64>>>,
    /// Set by each forget, so the next copy made ahead measures the cache again rather than skip
    /// for bytes no longer there. Apart from `measured`, so a delete never waits on a resize.
    deleted_since: Arc<AtomicBool>,
}

impl ResizedCache {
    /// An empty state, starting the one resize thread (ADR 0039).
    pub(super) fn start() -> Self {
        Self {
            resizer: ResizeThread::start(),
            measured: Arc::default(),
            deleted_since: Arc::default(),
        }
    }
}

/// Who a copy is made for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MadeFor {
    Request,
    /// A scan, upload or startup, making it before it is asked for: none is made while the cache
    /// holds [`AHEAD_UP_TO_PERCENT`] of its limit.
    Ahead,
}

impl ImageStore {
    /// Makes each copy `copies` asks for of the image at `key` in each format a client may ask
    /// for, where it isn't there and the cache has room, so the first request for it is sent from
    /// the cache. Each waits its turn on the one resize thread, as a request's does (ADR 0039).
    pub(crate) async fn copy_ahead(&self, key: &[&str], copies: &[Resize]) {
        for &resize in copies {
            for format in [Format::Jpeg, Format::Jxl] {
                self.ensure_copy(key, resize, format, MadeFor::Ahead).await;
            }
        }
    }

    /// The file and format of the copy of the image at `key` to send for `resize`, made now if it
    /// wasn't there, as [`Self::ensure_copy`] says. `None` to send the image as stored.
    pub(super) async fn copy_to_send(
        &self,
        key: &[&str],
        resize: Resize,
        asked: Format,
    ) -> Option<(PathBuf, Format)> {
        let format = self
            .ensure_copy(key, resize, asked, MadeFor::Request)
            .await?;
        Some((self.copy_path(key, resize.width, format), format))
    }

    /// The format of the copy of the image at `key` to send for `resize`, made now if it wasn't
    /// there: `asked`, or JPEG where `make_copy` puts one in its place. `None` to send the image
    /// as stored, which an image that can't be resized or isn't stored also is, after a warning
    /// is logged for one that can't be resized, as is one made [`MadeFor::Ahead`] while the cache
    /// is nearly full. Where `make_copy` makes none, an empty marker beside the copies says so,
    /// so the original isn't read for it again; a failed read or resize leaves none, so the next
    /// ask tries again.
    async fn ensure_copy(
        &self,
        key: &[&str],
        resize: Resize,
        asked: Format,
        made_for: MadeFor,
    ) -> Option<Format> {
        let copy = self.copy_path(key, resize.width, asked);
        // Whether it is sent as stored depends on the format asked too (alpha in a JPEG, the
        // JPEG XL pixel limit), so the marker is named after the copy it stands in for.
        let unresized = copy.with_added_extension("same");
        if mark_sent(&copy).await {
            return Some(asked);
        }
        if mark_sent(&unresized).await {
            return None;
        }
        let mut cache_size = self.cache.measured.lock().await;
        if mark_sent(&copy).await {
            return Some(asked);
        }
        if mark_sent(&unresized).await {
            return None;
        }
        if made_for == MadeFor::Ahead
            && self
                .is_nearly_full(&mut cache_size, resize.cache_limit)
                .await
        {
            return None;
        }
        let _reading = self.whole_read().await.ok()?;
        let _writing = self.write_lock(&row_key(key)).lock().await;
        let original = self.original_to_resize(key).await?;
        let (folder, cache) = (self.copies_path(key), self.dir.join(RESIZED_DIR));
        let named = format!("{key:?}");
        let span = tracing::info_span!("resize image", image.width = resize.width);
        let resized = (self.cache.resizer)
            .run(move || {
                let copied = resized::on_disk(original, &cache)
                    .map_err(Into::into)
                    .and_then(|original| resized::make_copy(original, &folder, resize.width, asked))
                    .map_err(|err| format!("could not resize {named}: {err}"));
                let left = match &copied {
                    Ok(Copied::Made { format, .. }) => {
                        let made = folder.join(format.file_name(resize.width));
                        size_after_evicting(&cache, resize.cache_limit, &made)
                    }
                    _ => None,
                };
                (copied, left)
            })
            .instrument(span.clone())
            .await;
        // The thread having stopped, or a resize having panicked, is one more failed resize.
        let (copied, left) = resized.unwrap_or_else(|err| (Err(err.to_string()), None));
        *cache_size = left.or(*cache_size);
        match copied {
            Ok(Copied::Found(format)) => {
                mark_sent(&self.copy_path(key, resize.width, format)).await;
                Some(format)
            }
            Ok(Copied::Made { format, thumbhash }) => {
                // Without it the image only goes without a placeholder.
                if let Err(err) = self.keep_thumbhash(key, thumbhash).await {
                    tracing::error!("could not keep the thumbhash of {key:?}: {err}");
                }
                Some(format)
            }
            Ok(Copied::Unneeded) => {
                // Without it the original is only read again the next time it is asked for.
                if let Err(err) = mark_unresized(&unresized).await {
                    tracing::warn!("could not mark {} as unresized: {err}", unresized.display());
                }
                None
            }
            Err(err) => {
                diagnostics::failed(&span, "resize failed");
                tracing::warn!("{err}, so it is sent as it is");
                None
            }
        }
    }

    /// The bytes of the original at `key`, or `None` where it isn't stored, so it is sent as
    /// stored and answers 404, or can't be read, which is logged.
    async fn original_to_resize(&self, key: &[&str]) -> Option<Vec<u8>> {
        match self.blobs().get(&row_key(key)).await {
            Ok(original) => original,
            Err(err) => {
                tracing::warn!(
                    "could not read {key:?} to resize it, so it is sent as it is: {err}"
                );
                None
            }
        }
    }

    /// Whether the resized copies take [`AHEAD_UP_TO_PERCENT`] of `limit` or more, `cache_size`
    /// being their bytes as measured last, which it measures first where unknown or copies were
    /// deleted since.
    async fn is_nearly_full(&self, cache_size: &mut Option<u64>, limit: u64) -> bool {
        if self.cache.deleted_since.swap(false, Ordering::AcqRel) || cache_size.is_none() {
            let cache = self.dir.join(RESIZED_DIR);
            match self.cache.resizer.run(move || size(&cache)).await {
                Ok(Ok(size)) => *cache_size = Some(size),
                // The copy is made, and its eviction measures the cache.
                Ok(Err(err)) | Err(err) => {
                    tracing::warn!("could not measure the resized images: {err}");
                }
            }
        }
        cache_size.is_some_and(|size| size >= limit / 100 * AHEAD_UP_TO_PERCENT)
    }

    async fn keep_thumbhash(&self, key: &[&str], thumbhash: Vec<u8>) -> sqlx::Result<()> {
        let key = row_key(key);
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!(
                        "INSERT INTO image_thumbhashes (key, thumbhash) VALUES (?, ?)
                         ON CONFLICT (key) DO UPDATE SET thumbhash = excluded.thumbhash",
                        key,
                        thumbhash,
                    )
                    .execute(&mut **tx)
                    .await
                    .map(drop)
                })
            })
            .await
    }

    /// The key and thumbhash of every image stored under `dir` that has been resized, each key
    /// starting with `dir`.
    ///
    /// # Errors
    ///
    /// If the database can't be read.
    // ponytail: reads every thumbhash under `dir` for each response that shows images; query by
    // key if a store of tens of thousands of images makes that slow.
    pub(crate) async fn thumbhashes(&self, dir: &str) -> sqlx::Result<Vec<(Vec<String>, Vec<u8>)>> {
        let under = format!("{}/", file_name(dir));
        let rows = sqlx::query!(
            "SELECT key, thumbhash FROM image_thumbhashes WHERE instr(key, ?) = 1",
            under
        )
        .fetch_all(self.db.reader())
        .await?;
        Ok(rows
            .into_iter()
            .filter_map(|row| {
                let key = row.key.split('/').map(unescape).collect::<Option<_>>()?;
                Some((key, row.thumbhash))
            })
            .collect())
    }

    /// Forgets the thumbhash, copies and markers of the image at `key`, or of every image under
    /// it. Before the original goes, so nothing derived outlives it should the delete stop part
    /// way. Left, a marker would be found by an image stored at `key` again, which then is sent as
    /// stored where it could be resized. Final only as the module's lock order says.
    ///
    /// # Errors
    ///
    /// If the thumbhashes can't be deleted, or the copies are there but can't be.
    pub(super) async fn forget(&self, key: &[&str]) -> io::Result<()> {
        self.forget_thumbhashes(key).await?;
        let deleted = unless_gone(tokio::fs::remove_dir_all(self.copies_path(key)).await);
        self.cache.deleted_since.store(true, Ordering::Release);
        deleted
    }

    async fn forget_thumbhashes(&self, key: &[&str]) -> io::Result<()> {
        let (row, below) = (row_key(key), format!("{}/", row_key(key)));
        let forgotten = self
            .db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!(
                        "DELETE FROM image_thumbhashes WHERE key = ? OR instr(key, ?) = 1",
                        row,
                        below,
                    )
                    .execute(&mut **tx)
                    .await
                    .map(drop)
                })
            })
            .await;
        forgotten.map_err(io::Error::other)
    }

    /// Where the copy `width` pixels wide in `format` of the image at `key` is kept.
    fn copy_path(&self, key: &[&str], width: u32, format: Format) -> PathBuf {
        self.copies_path(key).join(format.file_name(width))
    }

    /// The folder the copies of the image at `key`, or of every image under it, are kept in.
    fn copies_path(&self, key: &[&str]) -> PathBuf {
        under(self.dir.join(RESIZED_DIR), key)
    }
}

/// The bytes the copies in `cache` take once [`evict`] has fit them in `limit`, keeping `made`,
/// or `None` where it failed, which is logged.
fn size_after_evicting(cache: &Path, limit: u64, made: &Path) -> Option<u64> {
    (evict(cache, limit, made))
        .inspect_err(|err| {
            tracing::error!("could not keep the resized images within their limit: {err}");
        })
        .ok()
}

/// Whether the copy, or unresized marker, at `copy` is there, marking it as just sent if it is.
async fn mark_sent(copy: &Path) -> bool {
    let sent = copy.to_owned();
    let touch = tokio::task::spawn_blocking(move || touch(&sent)).await;
    match touch.map_err(io::Error::from).and_then(|touched| touched) {
        Ok(()) => true,
        Err(err) if err.kind() == io::ErrorKind::NotFound => false,
        // It's there, and not marking it only lets eviction pick it sooner.
        Err(err) => {
            tracing::warn!("could not mark {} as sent: {err}", copy.display());
            true
        }
    }
}

/// Marks the copy at `copy` as just sent, so it is the last [`evict`] deletes.
///
/// # Errors
///
/// [`io::ErrorKind::NotFound`] if no copy is there.
fn touch(copy: &Path) -> io::Result<()> {
    // Set explicitly, as mounts with `noatime` or `relatime` don't update it on every read. Opened
    // for writing, as Windows refuses to set a file's times through a read-only handle.
    let now = SystemTime::UNIX_EPOCH
        + Duration::from_millis(u64::try_from(db::now_ms()).unwrap_or_default());
    File::options()
        .write(true)
        .open(copy)?
        .set_times(FileTimes::new().set_accessed(now))
}

/// Leaves an empty file at `marker`, which says the image goes unresized at the width and format
/// the marker is named after.
async fn mark_unresized(marker: &Path) -> io::Result<()> {
    if let Some(folder) = marker.parent() {
        tokio::fs::create_dir_all(folder).await?;
    }
    tokio::fs::write(marker, []).await
}

/// A copy in the cache, as when it was last sent, its size and its path.
type Listed = (SystemTime, u64, PathBuf);

/// The bytes the copies in `cache` take.
///
/// # Errors
///
/// If the cache can't be read.
fn size(cache: &Path) -> io::Result<u64> {
    Ok(copies(cache)?.iter().map(|(_, size, _)| size).sum())
}

/// Deletes the copies in `cache` sent least recently until they take at most `limit` bytes, never
/// `made`, the copy about to be sent. Returns the bytes those left take.
///
/// # Errors
///
/// If the cache can't be read or a copy deleted.
// ponytail: lists the whole cache after each new copy, which costs about a stat per copy; keep
// a running total if a cache of tens of thousands of copies makes resizes slow.
fn evict(cache: &Path, limit: u64, made: &Path) -> io::Result<u64> {
    evict_listed(copies(cache)?, limit, made)
}

/// [`evict`] from `copies`, the cache as listed. A copy already gone counts as deleted: a delete
/// of its image takes no resize lock, so it may remove one after the listing.
fn evict_listed(mut copies: Vec<Listed>, limit: u64, made: &Path) -> io::Result<u64> {
    let mut total: u64 = copies.iter().map(|(_, size, _)| size).sum();
    copies.sort_unstable();
    for (_, size, path) in copies {
        if total <= limit {
            break;
        }
        if path == made {
            continue;
        }
        unless_gone(std::fs::remove_file(path))?;
        total -= size;
    }
    Ok(total)
}

/// Each copy in `cache`, leaving out one a delete removes while it is listed.
fn copies(cache: &Path) -> io::Result<Vec<Listed>> {
    let gone =
        |err: &walkdir::Error| err.io_error().map(io::Error::kind) == Some(io::ErrorKind::NotFound);
    let mut copies = Vec::new();
    for entry in WalkDir::new(cache) {
        let entry = match entry {
            Ok(entry) if entry.file_type().is_file() => entry,
            Ok(_) => continue,
            Err(err) if gone(&err) => continue,
            Err(err) => return Err(err.into()),
        };
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(err) if gone(&err) => continue,
            Err(err) => return Err(err.into()),
        };
        copies.push((metadata.accessed()?, metadata.len(), entry.into_path()));
    }
    Ok(copies)
}

#[cfg(test)]
#[path = "resized_cache_tests.rs"]
mod tests;
