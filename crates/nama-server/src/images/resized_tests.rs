use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use image::{ImageFormat, RgbImage, RgbaImage};

use super::{Copied, Format, ResizeThread, make_copy, on_disk};

/// A job that counts itself as running while it sleeps, keeping in `most` the most that ran
/// at once.
fn counted(running: &Arc<AtomicUsize>, most: &Arc<AtomicUsize>) -> impl FnOnce() + Send + use<> {
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

/// `image` encoded as `format`, in a file `make_copy` reads.
fn original(
    image: impl Into<image::DynamicImage>,
    format: ImageFormat,
    dir: &std::path::Path,
) -> std::fs::File {
    let mut bytes = std::io::Cursor::new(Vec::new());
    image.into().write_to(&mut bytes, format).unwrap();
    on_disk(bytes.into_inner(), dir).unwrap()
}

#[test]
fn a_jpeg_xl_over_7_megapixels_is_made_as_a_jpeg_or_not_at_all_with_transparency() {
    let dir = tempfile::tempdir().unwrap();
    // 2190 wide of 2200x3300 is 2190x3285, 7.2 megapixels.
    let (width, height, asked) = (2200, 3300, 2190);
    let opaque = original(RgbImage::new(width, height), ImageFormat::Jpeg, dir.path());
    let transparent = original(RgbaImage::new(width, height), ImageFormat::Png, dir.path());

    let as_jpeg = make_copy(opaque, &dir.path().join("opaque"), asked, Format::Jxl).unwrap();
    let as_stored = make_copy(
        transparent,
        &dir.path().join("transparent"),
        asked,
        Format::Jxl,
    )
    .unwrap();

    assert!(
        matches!(
            as_jpeg,
            Copied::Made {
                format: Format::Jpeg,
                ..
            }
        ),
        "{as_jpeg:?}"
    );
    assert!(
        dir.path()
            .join("opaque")
            .join(Format::Jpeg.file_name(asked))
            .is_file()
    );
    assert_eq!(as_stored, Copied::Unneeded);
}
