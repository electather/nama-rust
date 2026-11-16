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
