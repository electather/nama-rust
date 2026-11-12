//! Resized copies of stored images, as JPEG: made on one thread, one at a time, and kept in a
//! cache that drops the least recently sent once it passes its limit (ADR 0039).

use std::{
    error::Error,
    fs::{File, FileTimes},
    io,
    panic::{AssertUnwindSafe, catch_unwind},
    path::{Path, PathBuf},
    sync::mpsc,
    time::SystemTime,
};

use fast_image_resize::Resizer;
use image::{
    DynamicImage, ImageDecoder as _, ImageReader, RgbImage, codecs::jpeg::JpegEncoder,
    metadata::Orientation,
};
use walkdir::WalkDir;

use super::write_now;

/// The most pixels an image is resized from. A larger one is sent as it is, because one resize
/// must stay within 128 MiB above the Server's idle memory (`docs/research/image-memory-use.md`).
const MAX_PIXELS: u64 = 12_000_000;

/// The largest decoded bitmap an image is resized from, which keeps a 16-bit PNG under
/// [`MAX_PIXELS`] within the same 128 MiB (`docs/research/image-memory-use.md`, rule 1).
const MAX_BITMAP_BYTES: u64 = 64 * 1024 * 1024;

/// The JPEG quality of every copy.
const QUALITY: u8 = 80;

/// The most pixels a side of the image a thumbhash is taken from may have, `thumbhash`'s limit.
const THUMBHASH_SIDE: u32 = 100;

type Job = Box<dyn FnOnce() + Send>;

/// The one thread images are resized on, so at most one resize runs at a time and glibc keeps
/// one thread's freed memory rather than one share per thread (ADR 0039). Clones share it, and it
/// ends once the last clone is dropped.
#[derive(Debug, Clone)]
pub(super) struct ResizeThread(mpsc::Sender<Job>);

impl ResizeThread {
    pub(super) fn start() -> Self {
        let (jobs, queue) = mpsc::channel::<Job>();
        let work = move || {
            for job in queue {
                // A panicking job only drops its answer, so the next one still runs.
                drop(catch_unwind(AssertUnwindSafe(job)));
            }
        };
        let thread = std::thread::Builder::new().name("image-resize".to_owned());
        if let Err(err) = thread.spawn(work) {
            tracing::error!(
                "could not start the image thread, so every image is sent as stored: {err}"
            );
        }
        Self(jobs)
    }

    /// Runs `work` on the thread once every job sent before it has run.
    ///
    /// # Errors
    ///
    /// If the thread isn't running, or `work` panicked.
    pub(super) async fn run<T: Send + 'static>(
        &self,
        work: impl FnOnce() -> T + Send + 'static,
    ) -> io::Result<T> {
        let (answer, answered) = tokio::sync::oneshot::channel();
        let job: Job = Box::new(move || drop(answer.send(work())));
        let stopped = || io::Error::other("the image thread stopped or a resize panicked");
        self.0.send(job).map_err(|_unsent| stopped())?;
        answered.await.map_err(|_unanswered| stopped())
    }
}

/// What [`make_copy`] did.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Copied {
    /// It was there already.
    Found,
    /// It was made now, and the original's thumbhash taken on the way.
    Made { thumbhash: Vec<u8> },
    /// None is made: the original is to be sent as it is.
    Unneeded,
}

/// Writes `original` resized to `width`, its shape kept, as a JPEG at `copy`, unless a copy is
/// there already. Makes none where the original is to be sent as it is: it isn't a format the
/// Server reads (such as SVG), has transparency, is over [`MAX_PIXELS`] or [`MAX_BITMAP_BYTES`],
/// or is no wider than `width`.
///
/// # Errors
///
/// If the original can't be read or decoded, the copy or its thumbhash can't be made, or the copy
/// can't be written.
pub(super) fn make_copy(
    original: &Path,
    copy: &Path,
    width: u32,
) -> Result<Copied, Box<dyn Error + Send + Sync>> {
    // A request that waited behind the one that made it finds it here.
    if copy.try_exists()? {
        return Ok(Copied::Found);
    }
    // Read from the file, not a copy in memory: the JPEG decoder buffers the whole input itself.
    let reader = ImageReader::open(original)?.with_guessed_format()?;
    if reader.format().is_none() {
        return Ok(Copied::Unneeded);
    }
    let mut decoder = reader.into_decoder()?;
    let (stored_width, stored_height) = decoder.dimensions();
    let orientation = decoder.orientation()?;
    let (shown_width, shown_height) = if turns_sideways(orientation) {
        (stored_height, stored_width)
    } else {
        (stored_width, stored_height)
    };
    // Checked from the header before decoding: `image`'s `Limits` don't bound a decoder's own
    // buffers.
    let pixels = u64::from(stored_width) * u64::from(stored_height);
    let color = decoder.color_type();
    let bitmap_bytes = pixels * u64::from(color.bytes_per_pixel());
    if pixels > MAX_PIXELS
        || bitmap_bytes > MAX_BITMAP_BYTES
        || color.has_alpha()
        || width >= shown_width
    {
        return Ok(Copied::Unneeded);
    }
    let mut source = DynamicImage::from_decoder(decoder)?;
    source.apply_orientation(orientation);
    let source = source.into_rgb8();
    let height = u64::from(shown_height) * u64::from(width) / u64::from(shown_width);
    let mut resized = RgbImage::new(width, u32::try_from(height)?.max(1));
    Resizer::new().resize(&source, &mut resized, None)?;
    drop(source);
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, QUALITY).encode_image(&resized)?;
    // Before the copy is written: once it is, the image is never resized again to take one.
    let thumbhash = thumbhash_of(&resized)?;
    write_now(copy, &jpeg)?;
    Ok(Copied::Made { thumbhash })
}

/// The thumbhash of `image`, shrunk first to the size `thumbhash` takes, so the original is never
/// decoded for it alone.
fn thumbhash_of(image: &RgbImage) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
    let (width, height) = image.dimensions();
    let longest = width.max(height);
    let fit = |side: u32| (side * THUMBHASH_SIDE / longest).max(1);
    let mut small = RgbImage::new(fit(width), fit(height));
    Resizer::new().resize(image, &mut small, None)?;
    let small = DynamicImage::from(small).into_rgba8();
    Ok(thumbhash::rgba_to_thumb_hash(
        usize::try_from(small.width())?,
        usize::try_from(small.height())?,
        small.as_raw(),
    ))
}

/// Whether `orientation` shows a stored image turned a quarter, so its width is its height.
const fn turns_sideways(orientation: Orientation) -> bool {
    matches!(
        orientation,
        Orientation::Rotate90
            | Orientation::Rotate270
            | Orientation::Rotate90FlipH
            | Orientation::Rotate270FlipH
    )
}

/// Marks the copy at `copy` as just sent, so it is the last [`evict`] deletes.
///
/// # Errors
///
/// [`io::ErrorKind::NotFound`] if no copy is there.
pub(super) fn touch(copy: &Path) -> io::Result<()> {
    // Set explicitly, as mounts with `noatime` or `relatime` don't update it on every read.
    File::open(copy)?.set_times(FileTimes::new().set_accessed(SystemTime::now()))
}

/// Deletes the copies in `cache` sent least recently until they take at most `limit` bytes, never
/// `made`, the copy about to be sent.
///
/// # Errors
///
/// If the cache can't be read or a copy deleted.
// ponytail: lists the whole cache after each new copy, which costs about a stat per copy; keep
// a running total if a cache of tens of thousands of copies makes resizes slow.
pub(super) fn evict(cache: &Path, limit: u64, made: &Path) -> io::Result<()> {
    let mut copies: Vec<(SystemTime, u64, PathBuf)> = Vec::new();
    for entry in WalkDir::new(cache) {
        let entry = match entry {
            Ok(entry) if entry.file_type().is_file() => entry,
            Ok(_) => continue,
            Err(err) if err.io_error().map(io::Error::kind) == Some(io::ErrorKind::NotFound) => {
                continue;
            }
            Err(err) => return Err(err.into()),
        };
        let metadata = entry.metadata()?;
        copies.push((metadata.accessed()?, metadata.len(), entry.into_path()));
    }
    let mut total: u64 = copies.iter().map(|(_, size, _)| size).sum();
    copies.sort_unstable();
    for (_, size, path) in copies {
        if total <= limit {
            break;
        }
        if path == made {
            continue;
        }
        std::fs::remove_file(path)?;
        total -= size;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use super::ResizeThread;

    /// A job that counts itself as running while it sleeps, keeping in `most` the most that ran
    /// at once.
    fn counted(
        running: &Arc<AtomicUsize>,
        most: &Arc<AtomicUsize>,
    ) -> impl FnOnce() + Send + use<> {
        let (running, most) = (Arc::clone(running), Arc::clone(most));
        move || {
            let now = running.fetch_add(1, Ordering::SeqCst) + 1;
            most.fetch_max(now, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(5));
            running.fetch_sub(1, Ordering::SeqCst);
        }
    }

    #[tokio::test]
    async fn jobs_sent_together_run_one_at_a_time() {
        let thread = ResizeThread::start();
        let running = Arc::new(AtomicUsize::new(0));
        let most = Arc::new(AtomicUsize::new(0));

        let jobs = (0..8).map(|_| thread.run(counted(&running, &most)));
        for done in futures_util::future::join_all(jobs).await {
            done.unwrap();
        }

        assert_eq!(most.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_job_that_panics_fails_alone() {
        let thread = ResizeThread::start();

        let panicked = thread.run(|| -> u8 { panic!("a bad image") }).await;
        let next = thread.run(|| 7).await;

        panicked.unwrap_err();
        assert_eq!(next.unwrap(), 7);
    }
}
