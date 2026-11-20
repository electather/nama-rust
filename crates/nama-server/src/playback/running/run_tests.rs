use std::{
    fs,
    ops::Range,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering::Relaxed},
    },
    time::Duration,
};

use super::{
    super::testing::{FakeRuns, every_two_seconds},
    DISK_FULL, Fallback, Method, POLL_EVERY, Running, Runs, Timeline, keyframe_number,
};
use crate::{ffmpeg::hls_init, log_capture::CapturedLog};

/// How long behind playback the Playback settings keep files by default, five minutes.
const KEEP_BEHIND_MS: i64 = 300_000;

/// A plan running in a temp folder of its own, and the runs a test stands in for ffmpeg with.
struct Test {
    /// The segment folder, which holds the plan's folder.
    data: tempfile::TempDir,
    dir: PathBuf,
    runs: FakeRuns,
    /// What a Transcode on hardware falls back to, if it does.
    software: FakeRuns,
    /// How many times the plan told it fell back.
    told: Arc<AtomicUsize>,
    plan: Arc<Running>,
}

/// A Remux cut as [`every_two_seconds`], played from the start.
async fn remux() -> Test {
    start(Method::Remux, every_two_seconds(), 0, false).await
}

/// A Remux like [`remux`]'s, played from 800.5 seconds: in segment 133, which keyframes 399 to
/// 401 make, past keyframe 400.
async fn resumed() -> Test {
    start(Method::Remux, every_two_seconds(), 800_500, false).await
}

/// A Transcode on hardware cut as [`every_two_seconds`], played from the start, which falls back
/// to software.
async fn on_hardware() -> Test {
    start(Method::Transcode, every_two_seconds(), 0, true).await
}

/// A plan by `method` cut as `timeline` says, played from `start_ms`, which falls back to
/// software if `falls_back`.
async fn start(method: Method, timeline: Timeline, start_ms: i64, falls_back: bool) -> Test {
    let data = tempfile::tempdir().unwrap();
    let dir = data.path().join("nama-plan-1");
    fs::create_dir(&dir).unwrap();
    let (runs, software) = (FakeRuns::default(), FakeRuns::default());
    let told = Arc::new(AtomicUsize::new(0));
    let fallback = falls_back.then(|| {
        let told = Arc::clone(&told);
        Fallback {
            start: software.start(),
            on_fall_back: Box::new(move || {
                told.fetch_add(1, Relaxed);
            }),
        }
    });
    let started = Runs {
        method,
        start: runs.start(),
        cpu_bound: false,
        fallback,
    };
    let plan = Running::start(1, timeline, dir.clone(), start_ms, started, KEEP_BEHIND_MS);
    let plan = plan.await.unwrap();
    Test {
        data,
        dir,
        runs,
        software,
        told,
        plan,
    }
}

impl Test {
    /// Writes the files of `keyframes` as ffmpeg would, each holding its number.
    fn write(&self, keyframes: Range<usize>) {
        for n in keyframes {
            fs::write(self.dir.join(format!("{n}.m4s")), n.to_string()).unwrap();
        }
    }

    fn delete(&self, keyframes: Range<usize>) {
        for n in keyframes {
            fs::remove_file(self.dir.join(format!("{n}.m4s"))).unwrap();
        }
    }

    /// The keyframes whose files the plan has, by number, in order.
    fn left(&self) -> Vec<usize> {
        let mut left: Vec<usize> = (fs::read_dir(&self.dir).unwrap())
            .filter_map(|entry| keyframe_number(&entry.unwrap().file_name().into_string().unwrap()))
            .collect();
        left.sort_unstable();
        left
    }

    /// Asks for segment `n` in the background.
    fn ask(&self, n: usize) -> tokio::task::JoinHandle<std::io::Result<Option<Vec<u8>>>> {
        let plan = Arc::clone(&self.plan);
        tokio::spawn(async move { plan.segment(n).await })
    }

    /// Asks for segment `n` and returns it, failing unless the plan has it.
    async fn served(&self, n: usize) -> Vec<u8> {
        self.plan.segment(n).await.unwrap().unwrap()
    }
}

/// What `request` answers, failing the test if it doesn't within 10 seconds.
async fn answer<T>(request: tokio::task::JoinHandle<T>) -> T {
    let answered = tokio::time::timeout(Duration::from_secs(10), request).await;
    answered.unwrap().unwrap()
}

/// Long enough for the plan to look at how far its run has written.
async fn a_look_later() {
    tokio::time::sleep(POLL_EVERY * 3).await;
}

#[tokio::test]
async fn seeking_a_transcode_past_the_segment_ffmpeg_is_on_restarts_it_there() {
    let every_six_seconds = Timeline::new((0..167).map(|n| n * 6000).collect(), 1_000_000);
    let test = start(Method::Transcode, every_six_seconds, 0, false).await;

    // A Remux would wait for this, 18 seconds on.
    let eighteen_seconds = test.ask(3);

    assert_eq!(test.runs.until_started(2).await, [0, 3]);
    test.runs.until_dropped(0).await;
    test.write(3..4);
    assert_eq!(answer(eighteen_seconds).await.unwrap().unwrap(), b"3");
}

#[tokio::test]
async fn seeking_past_what_ffmpeg_has_written_restarts_it_at_the_segment_asked_for() {
    let test = remux().await;

    let eighty_percent = test.ask(133);

    assert_eq!(test.runs.until_started(2).await, [0, 399]);
    test.write(399..402);
    assert_eq!(answer(eighty_percent).await.unwrap().unwrap(), b"399400401");
}

#[tokio::test]
async fn a_resumed_plan_runs_ffmpeg_once_from_the_first_keyframe_of_the_segment_resumed_in() {
    let test = resumed().await;

    let (resumed_in, after) = (test.ask(133), test.ask(134));

    a_look_later().await;
    test.write(399..405);
    assert_eq!(answer(resumed_in).await.unwrap().unwrap(), b"399400401");
    assert_eq!(answer(after).await.unwrap().unwrap(), b"402403404");
    assert_eq!(test.runs.started(), [399]);
}

#[tokio::test]
async fn a_resumed_transcode_runs_ffmpeg_from_the_segment_resumed_in() {
    let six_second_segments = Timeline::transcoded(1_000_000);

    let test = start(Method::Transcode, six_second_segments, 800_500, false).await;

    assert_eq!(test.runs.started(), [133]);
}

#[tokio::test]
async fn the_init_segment_of_a_resumed_plan_is_served_once_ffmpeg_has_written_its_first_file() {
    let test = resumed().await;
    let plan = Arc::clone(&test.plan);

    let init = tokio::spawn(async move { plan.init().await });

    a_look_later().await;
    assert!(!init.is_finished());
    fs::write(test.dir.join(hls_init(399)), "init").unwrap();
    test.write(399..400);
    assert_eq!(answer(init).await.unwrap().unwrap(), b"init");
    assert_eq!(test.runs.started(), [399]);
}

#[tokio::test]
async fn seeking_back_before_where_a_plan_resumed_restarts_ffmpeg_there_and_is_served() {
    let test = resumed().await;

    let back = test.ask(10);

    assert_eq!(test.runs.until_started(2).await, [399, 30]);
    test.write(30..33);
    assert_eq!(answer(back).await.unwrap().unwrap(), b"303132");
}

#[tokio::test]
async fn a_request_left_waiting_by_a_later_seek_gives_up_rather_than_restart_ffmpeg() {
    let test = remux().await;
    let eighty_percent = test.ask(133);
    test.runs.until_started(2).await;

    let ten_percent = test.ask(10);

    assert_eq!(test.runs.until_started(3).await, [0, 399, 30]);
    assert_eq!(answer(eighty_percent).await.unwrap(), None);
    a_look_later().await;
    assert_eq!(test.runs.started(), [0, 399, 30]);
    test.write(30..33);
    assert_eq!(answer(ten_percent).await.unwrap().unwrap(), b"303132");
}

#[tokio::test]
async fn neighbouring_segments_asked_for_at_once_after_a_seek_are_both_served() {
    let test = remux().await;

    let (seeked_to, after) = (test.ask(133), test.ask(134));

    test.runs.until_started(2).await;
    a_look_later().await;
    // Whichever restarted ffmpeg first, it ends up at the earlier one.
    assert_eq!(test.runs.started().last(), Some(&399));
    test.write(399..405);
    assert_eq!(answer(seeked_to).await.unwrap().unwrap(), b"399400401");
    assert_eq!(answer(after).await.unwrap().unwrap(), b"402403404");
}

#[tokio::test]
async fn a_restart_that_fails_leaves_the_next_request_to_start_ffmpeg_again() {
    let test = remux().await;
    test.runs.fail_next_start();
    test.plan.segment(133).await.unwrap_err();

    let near_the_start = test.ask(1);

    assert_eq!(test.runs.until_started(3).await, [0, 399, 3]);
    test.write(3..6);
    assert_eq!(answer(near_the_start).await.unwrap().unwrap(), b"345");
}

#[tokio::test]
async fn a_segment_made_earlier_asked_for_after_a_seek_leaves_ffmpeg_running_there() {
    let test = remux().await;
    test.write(0..30);
    let eighty_percent = test.ask(133);
    test.runs.until_started(2).await;

    test.served(2).await;

    a_look_later().await;
    test.write(399..402);
    assert_eq!(answer(eighty_percent).await.unwrap().unwrap(), b"399400401");
    assert_eq!(test.runs.started(), [0, 399]);
    assert!(!test.runs.dropped(1));
}

#[tokio::test]
async fn segments_already_made_are_served_without_restarting_ffmpeg() {
    let test = remux().await;
    test.write(0..30);
    test.write(300..330);

    let back = test.served(2).await;
    let later = test.served(101).await;

    assert_eq!(back, b"678");
    assert_eq!(later, b"303304305");
    assert_eq!(test.runs.started(), [0]);
}

#[tokio::test]
async fn ffmpeg_two_minutes_past_the_last_segment_asked_for_stops_and_playing_on_restarts_it() {
    let test = remux().await;
    test.write(0..60);
    test.served(0).await;
    test.runs.until_dropped(0).await;

    // Waited for rather than restarted at, were ffmpeg still running.
    let next = test.ask(20);

    assert_eq!(test.runs.until_started(2).await, [0, 60]);
    test.write(60..63);
    assert_eq!(answer(next).await.unwrap().unwrap(), b"606162");
}

#[tokio::test]
async fn ffmpeg_short_of_two_minutes_past_the_last_segment_asked_for_keeps_running() {
    let test = remux().await;
    // To 118 seconds.
    test.write(0..59);
    test.served(0).await;
    a_look_later().await;

    let next = test.ask(20);

    a_look_later().await;
    test.write(59..63);
    assert_eq!(answer(next).await.unwrap().unwrap(), b"606162");
    assert_eq!(test.runs.started(), [0]);
    assert!(!test.runs.dropped(0));
}

#[tokio::test]
async fn the_init_segment_is_served_once_ffmpeg_has_written_its_first_file() {
    let test = remux().await;
    // ffmpeg makes it empty as it starts, and fills it in as the first file ends.
    fs::write(test.dir.join(hls_init(0)), "").unwrap();
    let plan = Arc::clone(&test.plan);

    let init = tokio::spawn(async move { plan.init().await });

    a_look_later().await;
    assert!(!init.is_finished());
    fs::write(test.dir.join(hls_init(0)), "init").unwrap();
    test.write(0..1);
    assert_eq!(answer(init).await.unwrap().unwrap(), b"init");
}

#[tokio::test]
async fn the_init_segment_is_served_once_its_first_file_is_written_and_gone_since() {
    let test = remux().await;
    fs::write(test.dir.join(hls_init(0)), "init").unwrap();
    test.write(0..1);
    a_look_later().await;
    test.delete(0..1);

    let init = test.plan.init().await;

    assert_eq!(init.unwrap().unwrap(), b"init");
}

#[tokio::test]
async fn the_init_segment_of_a_run_that_stopped_before_writing_a_file_is_not_served() {
    let test = remux().await;

    test.runs
        .fail(0, "Invalid data found when processing input");

    assert_eq!(test.plan.init().await.unwrap(), None);
}

#[tokio::test]
async fn a_segment_whose_files_are_gone_restarts_ffmpeg_there_and_is_served() {
    let test = remux().await;
    test.write(0..30);
    a_look_later().await;
    test.delete(3..6);

    let again = test.ask(1);

    assert_eq!(test.runs.until_started(2).await, [0, 3]);
    test.write(3..6);
    assert_eq!(answer(again).await.unwrap().unwrap(), b"345");
}

#[tokio::test]
async fn a_tick_deletes_files_ending_more_than_five_minutes_behind_the_last_segment_asked_for() {
    let test = remux().await;
    // To 400 seconds.
    test.write(0..200);

    // At 360 seconds.
    test.served(60).await;
    test.plan.delete_unkept(KEEP_BEHIND_MS).await.unwrap();

    // Keyframe 29 ends at 60 seconds, 5 minutes before.
    assert_eq!(test.left(), Vec::from_iter(29..200));
}

#[tokio::test]
async fn a_tick_told_to_keep_less_behind_deletes_what_it_no_longer_keeps() {
    let test = remux().await;
    // To 400 seconds, played to 360.
    test.write(0..200);
    test.served(60).await;
    test.plan.delete_unkept(KEEP_BEHIND_MS).await.unwrap();

    test.plan.delete_unkept(60_000).await.unwrap();

    // Keyframe 149 ends at 300 seconds, a minute before.
    assert_eq!(test.left(), Vec::from_iter(149..200));
}

#[tokio::test]
async fn a_tick_after_a_seek_back_deletes_the_files_more_than_two_minutes_ahead_of_it() {
    let test = remux().await;
    // To 600 seconds.
    test.write(0..300);
    // At 540 seconds, then back to 360.
    test.served(90).await;

    test.served(60).await;
    test.plan.delete_unkept(KEEP_BEHIND_MS).await.unwrap();

    // Keyframe 240 starts at 480 seconds, 2 minutes ahead.
    assert_eq!(test.left(), Vec::from_iter(29..240));
}

#[tokio::test]
async fn a_tick_deletes_the_stretches_of_earlier_seeks_behind_and_far_ahead_of_playback() {
    let test = remux().await;
    for stretch in [0..20, 100..150, 400..420] {
        test.write(stretch);
    }
    // The last seek, to 600 seconds.
    let seek = test.ask(100);
    assert_eq!(test.runs.until_started(2).await, [0, 300]);
    test.write(300..350);
    answer(seek).await.unwrap().unwrap();

    // At 660 seconds, while ffmpeg runs on to 780.
    test.served(110).await;
    test.plan.delete_unkept(KEEP_BEHIND_MS).await.unwrap();

    assert_eq!(test.left(), Vec::from_iter(300..350));
}

#[tokio::test]
async fn a_tick_keeps_what_ffmpeg_writes_for_a_seek_ahead_though_playback_skipped_back() {
    let test = remux().await;
    test.write(0..3);
    let seek = test.ask(100);
    assert_eq!(test.runs.until_started(2).await, [0, 300]);
    test.write(300..320);
    answer(seek).await.unwrap().unwrap();

    test.served(0).await;
    test.plan.delete_unkept(KEEP_BEHIND_MS).await.unwrap();

    assert_eq!(test.left(), Vec::from_iter((0..3).chain(300..320)));
}

#[tokio::test]
async fn a_tick_deletes_the_init_files_of_earlier_runs_and_keeps_the_current_ones() {
    let test = remux().await;
    fs::write(test.dir.join(hls_init(0)), "init").unwrap();
    let seeked = test.ask(133);
    test.runs.until_started(2).await;
    fs::write(test.dir.join(hls_init(399)), "init").unwrap();

    test.plan.delete_unkept(KEEP_BEHIND_MS).await.unwrap();

    assert!(!test.dir.join(hls_init(0)).exists());
    assert!(test.dir.join(hls_init(399)).exists());
    test.write(399..402);
    answer(seeked).await.unwrap().unwrap();
}

#[tokio::test]
async fn skipping_back_past_what_a_tick_kept_restarts_ffmpeg_there_and_is_served() {
    let test = remux().await;
    test.write(0..200);
    test.served(60).await;
    test.plan.delete_unkept(KEEP_BEHIND_MS).await.unwrap();

    let back = test.ask(1);

    assert_eq!(test.runs.until_started(2).await, [0, 3]);
    test.write(3..6);
    assert_eq!(answer(back).await.unwrap().unwrap(), b"345");
}

// No lock spans finding a file and reading it, so the test can't pause between the two: it
// deletes the file over and over instead, which failed reads within a few requests when they
// checked first.
#[tokio::test(flavor = "multi_thread")]
async fn a_segment_whose_file_is_deleted_again_and_again_while_asked_for_is_served() {
    let test = remux().await;
    test.write(3..6);
    let until = std::time::Instant::now() + Duration::from_millis(500);
    let dir = test.dir.clone();
    let flickering = std::thread::spawn(move || delete_and_write_again(&dir, 3, until));

    let mut asking = tokio::task::JoinSet::new();
    for _ in 0..8 {
        asking.spawn(serves_segment_one_until(Arc::clone(&test.plan), until));
    }
    while let Some(asked) = asking.join_next().await {
        asked.unwrap();
    }
    flickering.join().unwrap();
}

/// Deletes keyframe `n`'s file in `dir` and writes it again as ffmpeg would, over and over
/// until `until`.
fn delete_and_write_again(dir: &Path, n: usize, until: std::time::Instant) {
    let (file, whole) = (
        dir.join(format!("{n}.m4s")),
        dir.join(format!("{n}.m4s.tmp")),
    );
    while std::time::Instant::now() < until {
        fs::write(&whole, n.to_string()).unwrap();
        // Gone only briefly, so the requests that find it gone and wait leave many more. Windows
        // refuses to delete it while a request reads it, so it's tried again, and keeps its name
        // until the last reader closes it, so the rename is tried again too.
        if fs::remove_file(&file).is_ok() {
            while fs::rename(&whole, &file).is_err() {}
        }
    }
}

/// Asks `plan` for segment 1 over and over until `until`, failing unless it's keyframes 3 to 5.
async fn serves_segment_one_until(plan: Arc<Running>, until: std::time::Instant) {
    while std::time::Instant::now() < until {
        assert_eq!(plan.segment(1).await.unwrap().unwrap(), b"345");
    }
}

#[tokio::test]
async fn a_transcode_whose_hardware_fails_carries_on_in_software_and_says_so_once() {
    let test = on_hardware().await;
    let later = test.ask(1);

    test.runs.fail(0, "Failed to upload the frame to the GPU");

    assert_eq!(test.software.until_started(1).await, [0]);
    assert!(test.plan.fell_back() && test.plan.cpu_bound());
    test.write(0..6);
    assert_eq!(answer(later).await.unwrap().unwrap(), b"345");
    test.software.fail(0, "Conversion failed!");
    a_look_later().await;
    assert_eq!(test.told.load(Relaxed), 1);
    assert_eq!(test.runs.started(), [0], "never back on hardware");
}

#[tokio::test]
async fn a_fallback_starts_where_the_failed_run_stopped_writing_though_its_start_is_gone() {
    let test = on_hardware().await;
    test.write(0..6);
    a_look_later().await;
    test.delete(0..3);

    test.runs.fail(0, "Failed to upload the frame to the GPU");

    assert_eq!(test.software.until_started(1).await, [6]);
}

#[tokio::test]
async fn a_transcode_whose_hardware_fills_the_segment_folder_stays_on_hardware() {
    let log = CapturedLog::start();
    let test = on_hardware().await;

    test.runs
        .fail(0, &format!("av_interleaved_write_frame(): {DISK_FULL}"));

    a_look_later().await;
    assert_eq!(test.software.started(), [0_usize; 0]);
    assert!(!test.plan.fell_back() && !test.plan.cpu_bound());
    assert_eq!(test.told.load(Relaxed), 0);
    assert!(log.finish().contains("is full"), "still warns");
    let restarted = test.ask(1);
    assert_eq!(test.runs.until_started(2).await, [0, 3]);
    test.runs.fail(1, DISK_FULL);
    assert_eq!(
        answer(restarted).await.unwrap(),
        None,
        "the restart fails too"
    );
}

#[tokio::test]
async fn ffmpeg_failing_on_a_full_disk_warns_naming_the_segment_folder_and_time_kept_behind() {
    // This test's runtime runs on this thread alone, so the task watching ffmpeg logs here too.
    let log = CapturedLog::start();
    let test = remux().await;
    let asked = test.ask(0);

    test.runs.fail(0, &format!("[mp4] 0.m4s: {DISK_FULL}"));

    // Given up on once ffmpeg is marked stopped, after it logged why it failed.
    assert_eq!(answer(asked).await.unwrap(), None);
    let logged = log.finish();
    let warning = logged
        .lines()
        .find(|line| line.contains("is full"))
        .unwrap();
    assert!(warning.contains("WARN"), "{warning}");
    assert!(
        warning.contains(test.data.path().to_str().unwrap()),
        "{warning}"
    );
    assert!(
        warning.contains("free while plans keep 5 minutes"),
        "{warning}"
    );
    if cfg!(unix) {
        assert!(warning.contains("MiB free"), "{warning}");
    }
}

#[tokio::test]
async fn stopping_a_plan_stops_its_run_and_deletes_its_files() {
    let test = remux().await;
    test.write(0..3);
    let waiting = test.ask(10);

    test.plan.stop().await;

    assert!(test.runs.dropped(0));
    assert!(!test.dir.exists());
    assert_eq!(answer(waiting).await.unwrap(), None);
    assert_eq!(test.runs.started(), [0], "nor started again");
}
