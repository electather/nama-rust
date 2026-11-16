use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering::Relaxed},
    },
    time::Duration,
};

use futures_util::StreamExt;
use nama_proto::nama::v1::{
    Playback, PlaybackChanged, PlaybackEnded, PlaybackMethod, PlaybackReason, PlaybackReasonKind,
    PlaybackStarted,
};

use super::{
    FallbackRun, Method, POLL_EVERY, Running, RunningPlans, Runs, Timeline, keyframe_number, lock,
    sweep_leftovers,
};
use crate::{
    db::Database,
    events::{EventBus, Filter, event},
    ffmpeg::{Ffmpeg, Keyframe, hls_init},
    library::contents::Contents,
    playback::{Probing, subtitles},
    plugins::ExtensionPoints,
    server_settings::{HardwareChoice, PlaybackChange, PlaybackSettings, ServerUrl, Settings},
};

fn event_bus(db: &Arc<Database>) -> EventBus {
    EventBus::new(
        Arc::clone(db),
        ExtensionPoints::default(),
        ServerUrl::default(),
    )
}

fn plans_in(db: &Arc<Database>, settings: PlaybackSettings) -> RunningPlans {
    let events = event_bus(db);
    plans_with(db, settings, &events)
}

fn plans_with(db: &Arc<Database>, settings: PlaybackSettings, events: &EventBus) -> RunningPlans {
    let ffmpeg = Arc::new(Ffmpeg::default());
    // Nothing here cleans subtitles, so their cache's folder is never touched.
    let subtitles = subtitles::Cache::new(&std::env::temp_dir());
    RunningPlans::new(
        settings,
        Arc::clone(&ffmpeg),
        Contents::new(Arc::clone(db)),
        Probing::new(Arc::clone(db), ffmpeg, subtitles),
        events.clone(),
    )
}

/// The playback of a Remux on the Device `device`.
fn playback(device: i64) -> Playback {
    Playback {
        device_id: device,
        method: PlaybackMethod::PLAYBACK_METHOD_REMUX.into(),
        ..Default::default()
    }
}

/// A playback's reasons when it has only one, of `kind`.
fn only_reason(kind: PlaybackReasonKind) -> Vec<PlaybackReason> {
    vec![PlaybackReason {
        kind: kind.into(),
        ..Default::default()
    }]
}

/// The HLS files of the plan `id`, as asked for by the Device `device`.
fn hls(plans: &RunningPlans, id: u64, device: i64) -> Arc<Running> {
    Arc::clone(plans.ask(id, device).unwrap().hls.as_ref().unwrap())
}

/// The Server settings in `db`, with segment files going to `default` while no folder is set.
fn settings(db: &Arc<Database>, default: PathBuf) -> Settings {
    Settings::new(Arc::clone(db), default)
}

async fn database(data: &Path) -> Arc<Database> {
    Arc::new(Database::open(data).await.unwrap())
}

async fn running_plans(data: &Path) -> RunningPlans {
    let db = database(data).await;
    plans_in(&db, settings(&db, data.join("transcodes")).playback())
}

/// A Version 1000 seconds long with a keyframe every 2, so each segment is 3 keyframes long.
fn every_two_seconds() -> Timeline {
    Timeline::new((0..500).map(|n| n * 2000).collect(), 1_000_000)
}

/// Starts a Remux for the Device 7 whose ffmpeg writes nothing, and returns its id with the
/// keyframes each run of ffmpeg started at.
async fn start_writing_nothing(plans: &RunningPlans) -> (u64, Arc<Mutex<Vec<usize>>>) {
    start_method_writing_nothing(plans, Method::Remux, every_two_seconds()).await
}

/// As [`start_writing_nothing`], for a plan by `method` cut as `timeline` says.
async fn start_method_writing_nothing(
    plans: &RunningPlans,
    method: Method,
    timeline: Timeline,
) -> (u64, Arc<Mutex<Vec<usize>>>) {
    let starts = Arc::new(Mutex::new(vec![]));
    let started = Arc::clone(&starts);
    let run = move |_: &Path, from: Keyframe| {
        lock(&started).push(from.n);
        tokio::process::Command::new("sleep")
            .arg("600")
            .kill_on_drop(true)
            .spawn()
    };
    let id = plans
        .start_hls(
            playback(7),
            method,
            timeline,
            Runs {
                run: Box::new(run),
                fallback: None,
            },
        )
        .await
        .unwrap();
    (id, starts)
}

// The fake ffmpeg is `sleep`.
#[cfg(unix)]
#[tokio::test]
async fn seeking_a_transcode_past_the_segment_ffmpeg_is_on_restarts_it_there() {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;
    let every_six_seconds = Timeline::new((0..167).map(|n| n * 6000).collect(), 1_000_000);
    let (id, starts) =
        start_method_writing_nothing(&plans, Method::Transcode, every_six_seconds).await;
    let plan = hls(&plans, id, 7);

    // A Remux would wait for this, 18 seconds on.
    let eighteen_seconds = tokio::spawn(async move { plan.segment(3).await });

    assert_eq!(until_started(&starts, 2).await, [0, 3]);
    write_keyframes(data.path(), id, 3..4);
    assert_eq!(answer(eighteen_seconds).await.unwrap().unwrap(), b"3");
}

/// Writes the files of `keyframes` for the plan `id` as ffmpeg would, each holding its number.
fn write_keyframes(data: &Path, id: u64, keyframes: std::ops::Range<usize>) {
    let dir = data.join("transcodes").join(format!("nama-plan-{id}"));
    for n in keyframes {
        fs::write(dir.join(format!("{n}.m4s")), n.to_string()).unwrap();
    }
}

fn delete_keyframes(data: &Path, id: u64, keyframes: std::ops::Range<usize>) {
    let dir = data.join("transcodes").join(format!("nama-plan-{id}"));
    for n in keyframes {
        fs::remove_file(dir.join(format!("{n}.m4s"))).unwrap();
    }
}

/// What `request` answers, failing the test if it doesn't within 10 seconds.
async fn answer<T>(request: tokio::task::JoinHandle<T>) -> T {
    let answered = tokio::time::timeout(Duration::from_secs(10), request).await;
    answered.unwrap().unwrap()
}

/// Waits until `starts` holds `count` runs.
async fn until_started<T: Clone>(starts: &Mutex<Vec<T>>, count: usize) -> Vec<T> {
    let started = async {
        while lock(starts).len() < count {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    tokio::time::timeout(Duration::from_secs(10), started)
        .await
        .unwrap();
    lock(starts).clone()
}

// The fake ffmpeg is `sleep`.
#[cfg(unix)]
#[tokio::test]
async fn seeking_past_what_ffmpeg_has_written_restarts_it_at_the_segment_asked_for() {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;
    let (id, starts) = start_writing_nothing(&plans).await;
    let plan = hls(&plans, id, 7);

    let eighty_percent = tokio::spawn(async move { plan.segment(133).await });

    assert_eq!(until_started(&starts, 2).await, [0, 399]);
    write_keyframes(data.path(), id, 399..402);
    let segment = answer(eighty_percent).await.unwrap().unwrap();
    assert_eq!(segment, b"399400401");
}

#[cfg(unix)]
#[tokio::test]
async fn a_request_left_waiting_by_a_later_seek_gives_up_rather_than_restart_ffmpeg() {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;
    let (id, starts) = start_writing_nothing(&plans).await;
    let plan = hls(&plans, id, 7);
    let first = Arc::clone(&plan);
    let eighty_percent = tokio::spawn(async move { first.segment(133).await });
    until_started(&starts, 2).await;

    let ten_percent = tokio::spawn(async move { plan.segment(10).await });

    assert_eq!(until_started(&starts, 3).await, [0, 399, 30]);
    assert_eq!(answer(eighty_percent).await.unwrap(), None);
    tokio::time::sleep(POLL_EVERY * 3).await;
    assert_eq!(*lock(&starts), [0, 399, 30]);
    write_keyframes(data.path(), id, 30..33);
    assert_eq!(answer(ten_percent).await.unwrap().unwrap(), b"303132");
}

#[cfg(unix)]
#[tokio::test]
async fn neighbouring_segments_asked_for_at_once_after_a_seek_are_both_served() {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;
    let (id, starts) = start_writing_nothing(&plans).await;
    let plan = hls(&plans, id, 7);
    let (first, second) = (Arc::clone(&plan), plan);

    let both = tokio::spawn(async move { tokio::join!(first.segment(133), second.segment(134)) });

    until_started(&starts, 2).await;
    tokio::time::sleep(POLL_EVERY * 3).await;
    // Whichever restarted ffmpeg first, it ends up at the earlier one.
    assert_eq!(lock(&starts).last(), Some(&399));
    write_keyframes(data.path(), id, 399..405);
    let (seeked_to, after) = answer(both).await;
    assert_eq!(seeked_to.unwrap().unwrap(), b"399400401");
    assert_eq!(after.unwrap().unwrap(), b"402403404");
}

#[cfg(unix)]
#[tokio::test]
async fn a_restart_that_fails_leaves_the_next_request_to_start_ffmpeg_again() {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;
    let runs = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&runs);
    let second_fails = move |_: &Path, _: Keyframe| match counted.fetch_add(1, Relaxed) {
        1 => Err(std::io::Error::other("too many open files")),
        _ => tokio::process::Command::new("sleep")
            .arg("600")
            .kill_on_drop(true)
            .spawn(),
    };
    let id = plans
        .start_hls(
            playback(7),
            Method::Remux,
            every_two_seconds(),
            Runs {
                run: Box::new(second_fails),
                fallback: None,
            },
        )
        .await
        .unwrap();
    let plan = hls(&plans, id, 7);
    plan.segment(133).await.unwrap_err();

    let near_the_start = tokio::spawn(async move { plan.segment(1).await });

    tokio::time::sleep(POLL_EVERY * 3).await;
    assert_eq!(runs.load(Relaxed), 3);
    write_keyframes(data.path(), id, 3..6);
    assert_eq!(answer(near_the_start).await.unwrap().unwrap(), b"345");
}

#[cfg(unix)]
#[tokio::test]
async fn a_segment_made_earlier_asked_for_after_a_seek_leaves_ffmpeg_running_there() {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;
    let (id, starts) = start_writing_nothing(&plans).await;
    let plan = hls(&plans, id, 7);
    write_keyframes(data.path(), id, 0..30);
    let first = Arc::clone(&plan);
    let eighty_percent = tokio::spawn(async move { first.segment(133).await });
    until_started(&starts, 2).await;

    plan.segment(2).await.unwrap().unwrap();

    tokio::time::sleep(POLL_EVERY * 3).await;
    write_keyframes(data.path(), id, 399..402);
    assert_eq!(answer(eighty_percent).await.unwrap().unwrap(), b"399400401");
    assert_eq!(*lock(&starts), [0, 399]);
}

#[cfg(unix)]
#[tokio::test]
async fn segments_already_made_are_served_without_restarting_ffmpeg() {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;
    let (id, starts) = start_writing_nothing(&plans).await;
    let plan = hls(&plans, id, 7);
    write_keyframes(data.path(), id, 0..30);
    write_keyframes(data.path(), id, 300..330);

    let back = plan.segment(2).await.unwrap().unwrap();
    let later = plan.segment(101).await.unwrap().unwrap();

    assert_eq!(back, b"678");
    assert_eq!(later, b"303304305");
    assert_eq!(*lock(&starts), [0]);
}

#[cfg(unix)]
#[tokio::test]
async fn ffmpeg_two_minutes_past_the_last_segment_asked_for_stops_and_playing_on_restarts_it() {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;
    let (id, starts) = start_writing_nothing(&plans).await;
    let plan = hls(&plans, id, 7);
    write_keyframes(data.path(), id, 0..60);
    plan.segment(0).await.unwrap().unwrap();
    // Long enough for ffmpeg's progress to be looked at.
    tokio::time::sleep(POLL_EVERY * 3).await;

    // Waited for rather than restarted at, were ffmpeg still running.
    let next = tokio::spawn(async move { plan.segment(20).await });

    assert_eq!(until_started(&starts, 2).await, [0, 60]);
    write_keyframes(data.path(), id, 60..63);
    assert_eq!(answer(next).await.unwrap().unwrap(), b"606162");
}

#[cfg(unix)]
#[tokio::test]
async fn ffmpeg_short_of_two_minutes_past_the_last_segment_asked_for_keeps_running() {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;
    let (id, starts) = start_writing_nothing(&plans).await;
    let plan = hls(&plans, id, 7);
    // To 118 seconds.
    write_keyframes(data.path(), id, 0..59);
    plan.segment(0).await.unwrap().unwrap();
    tokio::time::sleep(POLL_EVERY * 3).await;

    let next = tokio::spawn(async move { plan.segment(20).await });

    tokio::time::sleep(POLL_EVERY * 3).await;
    write_keyframes(data.path(), id, 59..63);
    assert_eq!(answer(next).await.unwrap().unwrap(), b"606162");
    assert_eq!(*lock(&starts), [0]);
}

#[cfg(unix)]
#[tokio::test]
async fn the_init_segment_is_served_once_ffmpeg_has_written_its_first_file() {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;
    let (id, _) = start_writing_nothing(&plans).await;
    let plan = hls(&plans, id, 7);
    let dir = data
        .path()
        .join("transcodes")
        .join(format!("nama-plan-{id}"));
    // ffmpeg makes it empty as it starts, and fills it in as the first file ends.
    fs::write(dir.join("init-0.mp4"), "").unwrap();

    let init = tokio::spawn(async move { plan.init().await });

    tokio::time::sleep(POLL_EVERY * 3).await;
    assert!(!init.is_finished());
    fs::write(dir.join("init-0.mp4"), "init").unwrap();
    write_keyframes(data.path(), id, 0..1);
    assert_eq!(init.await.unwrap().unwrap().unwrap(), b"init");
}

#[cfg(unix)]
#[tokio::test]
async fn the_init_segment_is_served_once_its_first_file_is_written_and_gone_since() {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;
    let (id, _) = start_writing_nothing(&plans).await;
    let plan = hls(&plans, id, 7);
    let dir = plan.dir.clone();
    fs::write(dir.join("init-0.mp4"), "init").unwrap();
    write_keyframes(data.path(), id, 0..1);
    tokio::time::sleep(POLL_EVERY * 3).await;
    delete_keyframes(data.path(), id, 0..1);

    let init = tokio::spawn(async move { plan.init().await });

    assert_eq!(answer(init).await.unwrap().unwrap(), b"init");
}

#[cfg(unix)]
#[tokio::test]
async fn a_segment_whose_files_are_gone_restarts_ffmpeg_there_and_is_served() {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;
    let (id, starts) = start_writing_nothing(&plans).await;
    let plan = hls(&plans, id, 7);
    write_keyframes(data.path(), id, 0..30);
    tokio::time::sleep(POLL_EVERY * 3).await;
    delete_keyframes(data.path(), id, 3..6);

    let again = tokio::spawn(async move { plan.segment(1).await });

    assert_eq!(until_started(&starts, 2).await, [0, 3]);
    write_keyframes(data.path(), id, 3..6);
    assert_eq!(answer(again).await.unwrap().unwrap(), b"345");
}

/// The keyframes whose files the plan `id` has, by number, in order.
fn keyframes_left(data: &Path, id: u64) -> Vec<usize> {
    let dir = data.join("transcodes").join(format!("nama-plan-{id}"));
    let mut left: Vec<usize> = (fs::read_dir(dir).unwrap())
        .filter_map(|entry| keyframe_number(&entry.unwrap().file_name().into_string().unwrap()))
        .collect();
    left.sort_unstable();
    left
}

#[cfg(unix)]
#[tokio::test]
async fn a_tick_deletes_files_ending_more_than_five_minutes_behind_the_last_segment_asked_for() {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;
    let (id, _) = start_writing_nothing(&plans).await;
    let plan = hls(&plans, id, 7);
    // To 400 seconds.
    write_keyframes(data.path(), id, 0..200);

    // At 360 seconds.
    plan.segment(60).await.unwrap().unwrap();
    plans.delete_behind_playback().await;

    // Keyframe 29 ends at 60 seconds, 5 minutes before.
    assert_eq!(
        keyframes_left(data.path(), id),
        (29..200).collect::<Vec<_>>()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_tick_after_keep_for_skipping_back_is_lowered_deletes_what_it_no_longer_keeps() {
    let data = tempfile::tempdir().unwrap();
    let db = database(data.path()).await;
    let settings = settings(&db, data.path().join("transcodes"));
    let plans = plans_in(&db, settings.playback());
    let (id, _) = start_writing_nothing(&plans).await;
    let plan = hls(&plans, id, 7);
    // To 400 seconds, played to 360.
    write_keyframes(data.path(), id, 0..200);
    plan.segment(60).await.unwrap().unwrap();
    plans.delete_behind_playback().await;

    settings
        .change_playback(
            PlaybackChange {
                segment_folder: "",
                max_transcodes: None,
                keep_behind_minutes: Some(1),
                hardware: HardwareChoice::Automatic,
                remote_limit: None,
            },
            &[],
        )
        .await
        .unwrap();
    plans.delete_behind_playback().await;

    // Keyframe 149 ends at 300 seconds, a minute before.
    assert_eq!(
        keyframes_left(data.path(), id),
        (149..200).collect::<Vec<_>>()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_tick_after_a_seek_back_keeps_the_files_ahead_of_it() {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;
    let (id, _) = start_writing_nothing(&plans).await;
    let plan = hls(&plans, id, 7);
    // To 600 seconds.
    write_keyframes(data.path(), id, 0..300);
    // At 540 seconds, then back to 360.
    plan.segment(90).await.unwrap().unwrap();

    plan.segment(60).await.unwrap().unwrap();
    plans.delete_behind_playback().await;

    assert_eq!(
        keyframes_left(data.path(), id),
        (29..300).collect::<Vec<_>>()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_tick_deletes_the_stretches_of_earlier_seeks_behind_playback_and_keeps_those_ahead() {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;
    let (id, starts) = start_writing_nothing(&plans).await;
    let plan = hls(&plans, id, 7);
    for stretch in [0..20, 100..150, 400..420] {
        write_keyframes(data.path(), id, stretch);
    }
    // The last seek, to 600 seconds.
    let seeked = Arc::clone(&plan);
    let seek = tokio::spawn(async move { seeked.segment(100).await });
    assert_eq!(until_started(&starts, 2).await, [0, 300]);
    write_keyframes(data.path(), id, 300..350);
    answer(seek).await.unwrap().unwrap();

    // At 660 seconds.
    plan.segment(110).await.unwrap().unwrap();
    plans.delete_behind_playback().await;

    let ahead: Vec<usize> = (300..350).chain(400..420).collect();
    assert_eq!(keyframes_left(data.path(), id), ahead);
}

#[cfg(unix)]
#[tokio::test]
async fn a_tick_deletes_the_init_files_of_earlier_runs_and_keeps_the_current_ones() {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;
    let (id, starts) = start_writing_nothing(&plans).await;
    let plan = hls(&plans, id, 7);
    let dir = plan.dir.clone();
    fs::write(dir.join(hls_init(0)), "init").unwrap();
    let seeked = tokio::spawn(async move { plan.segment(133).await });
    until_started(&starts, 2).await;
    fs::write(dir.join(hls_init(399)), "init").unwrap();

    plans.delete_behind_playback().await;

    assert!(!dir.join(hls_init(0)).exists());
    assert!(dir.join(hls_init(399)).exists());
    write_keyframes(data.path(), id, 399..402);
    answer(seeked).await.unwrap().unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn skipping_back_past_what_a_tick_kept_restarts_ffmpeg_there_and_is_served() {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;
    let (id, starts) = start_writing_nothing(&plans).await;
    let plan = hls(&plans, id, 7);
    write_keyframes(data.path(), id, 0..200);
    plan.segment(60).await.unwrap().unwrap();
    plans.delete_behind_playback().await;

    let back = tokio::spawn(async move { plan.segment(1).await });

    assert_eq!(until_started(&starts, 2).await, [0, 3]);
    write_keyframes(data.path(), id, 3..6);
    assert_eq!(answer(back).await.unwrap().unwrap(), b"345");
}

// No lock spans finding a file and reading it, so the test can't pause between the two: it
// deletes the file over and over instead, which failed reads within a few requests when they
// checked first.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn a_segment_whose_file_is_deleted_again_and_again_while_asked_for_is_served() {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;
    let (id, _) = start_writing_nothing(&plans).await;
    let plan = hls(&plans, id, 7);
    write_keyframes(data.path(), id, 3..6);
    let until = std::time::Instant::now() + Duration::from_millis(500);
    let dir = plan.dir.clone();
    let flickering = std::thread::spawn(move || delete_and_write_again(&dir, 3, until));

    let mut asking = tokio::task::JoinSet::new();
    for _ in 0..8 {
        asking.spawn(serves_segment_one_until(Arc::clone(&plan), until));
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
        // Gone only briefly, so the requests that find it gone and wait leave many more.
        fs::remove_file(&file).unwrap();
        fs::rename(&whole, &file).unwrap();
    }
}

/// Asks `plan` for segment 1 over and over until `until`, failing unless it's keyframes 3 to 5.
async fn serves_segment_one_until(plan: Arc<Running>, until: std::time::Instant) {
    while std::time::Instant::now() < until {
        assert_eq!(plan.segment(1).await.unwrap().unwrap(), b"345");
    }
}

// The fake ffmpegs are `sh`, failing once told to, and `sleep`.
#[cfg(unix)]
#[tokio::test]
async fn a_fallback_starts_where_the_failed_run_stopped_writing_though_its_start_is_gone() {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;
    let starts = Arc::new(Mutex::new(vec![]));
    let (on_hardware, in_software) = (Arc::clone(&starts), Arc::clone(&starts));
    let hardware = move |dir: &Path, from: Keyframe| {
        lock(&on_hardware).push(("hardware", from.n));
        tokio::process::Command::new("sh")
            .args(["-c", "until [ -e fail ]; do sleep 0.05; done; exit 1"])
            .current_dir(dir)
            .kill_on_drop(true)
            .spawn()
    };
    let software = move |_: &Path, from: Keyframe| {
        lock(&in_software).push(("software", from.n));
        tokio::process::Command::new("sleep")
            .arg("600")
            .kill_on_drop(true)
            .spawn()
    };
    let id = plans
        .start_hls(
            playback(7),
            Method::Transcode,
            every_two_seconds(),
            Runs {
                run: Box::new(hardware),
                fallback: Some(FallbackRun {
                    run: Box::new(software),
                    reasons: only_reason(
                        PlaybackReasonKind::PLAYBACK_REASON_KIND_HARDWARE_FAILED_PARTWAY,
                    ),
                }),
            },
        )
        .await
        .unwrap();
    let dir = hls(&plans, id, 7).dir.clone();
    write_keyframes(data.path(), id, 0..6);
    tokio::time::sleep(POLL_EVERY * 3).await;
    delete_keyframes(data.path(), id, 0..3);

    fs::write(dir.join("fail"), "").unwrap();

    assert_eq!(
        until_started(&starts, 2).await,
        [("hardware", 0), ("software", 6)]
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_running_plan_is_marked_with_its_method_and_found_only_by_its_device() {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;

    let (id, _) = start_writing_nothing(&plans).await;

    assert_eq!(hls(&plans, id, 7).method, Method::Remux);
    assert!(plans.ask(id, 8).is_none());
}

// The fake ffmpeg is `sleep`.
#[cfg(unix)]
#[tokio::test]
async fn another_devices_transcodes_fill_the_cap_until_one_stops_and_remuxes_never_do() {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;
    let transcode = || {
        let timeline = every_two_seconds();
        start_method_writing_nothing(&plans, Method::Transcode, timeline)
    };
    let (first, _) = transcode().await;
    start_writing_nothing(&plans).await;
    assert_eq!(plans.transcodes_full(8, 2), None, "a Remux doesn't count");
    transcode().await;

    let full = plans.transcodes_full(8, 2);
    let for_its_own_device = plans.transcodes_full(7, 2);
    plans.stop_for(first, 7).await;

    assert_eq!(full, Some(2), "out of the box");
    assert_eq!(
        for_its_own_device, None,
        "the one its new plan replaces doesn't count"
    );
    assert_eq!(plans.transcodes_full(8, 2), None);
}

// The fake ffmpeg is `sleep`.
#[cfg(unix)]
#[tokio::test]
async fn a_device_leaves_only_one_of_its_own_transcodes_out_of_the_count() {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;
    for _ in 0..3 {
        start_method_writing_nothing(&plans, Method::Transcode, every_two_seconds()).await;
    }

    assert_eq!(plans.transcodes_full(7, 2), Some(2));
}

// The fake ffmpegs are `false` and `sleep`.
#[cfg(unix)]
#[tokio::test]
async fn a_transcode_whose_hardware_fails_carries_on_in_software_and_says_so() {
    let data = tempfile::tempdir().unwrap();
    let db = database(data.path()).await;
    let events = event_bus(&db);
    let plans = plans_with(
        &db,
        settings(&db, data.path().join("transcodes")).playback(),
        &events,
    );
    let mut subscription = Box::pin(events.subscribe_as_admin(Filter::default()));
    let starts = Arc::new(Mutex::new(vec![]));
    let (on_hardware, in_software) = (Arc::clone(&starts), Arc::clone(&starts));
    let hardware = move |_: &Path, from: Keyframe| {
        lock(&on_hardware).push(("hardware", from.n));
        tokio::process::Command::new("false").spawn()
    };
    let software = move |_: &Path, from: Keyframe| {
        lock(&in_software).push(("software", from.n));
        tokio::process::Command::new("sleep")
            .arg("600")
            .kill_on_drop(true)
            .spawn()
    };
    let started = Playback {
        reasons: only_reason(PlaybackReasonKind::PLAYBACK_REASON_KIND_TRANSCODES_ON_HARDWARE),
        ..playback(7)
    };
    let software_reasons =
        only_reason(PlaybackReasonKind::PLAYBACK_REASON_KIND_HARDWARE_FAILED_PARTWAY);
    let id = plans
        .start_hls(
            started,
            Method::Transcode,
            every_two_seconds(),
            Runs {
                run: Box::new(hardware),
                fallback: Some(FallbackRun {
                    run: Box::new(software),
                    reasons: software_reasons.clone(),
                }),
            },
        )
        .await
        .unwrap();
    let plan = hls(&plans, id, 7);

    let later = tokio::spawn(async move { plan.segment(1).await });

    until_started(&starts, 2).await;
    tokio::time::sleep(POLL_EVERY * 3).await;
    assert_eq!(*lock(&starts), [("hardware", 0), ("software", 0)]);
    write_keyframes(data.path(), id, 0..6);
    assert_eq!(answer(later).await.unwrap().unwrap(), b"345");
    let changed = Playback {
        reasons: software_reasons,
        ..playback(7)
    };
    subscription.next().await.unwrap();
    let next = subscription.next().await.unwrap();
    assert_eq!(
        next.message,
        Some(
            event(PlaybackChanged {
                playback: changed.clone().into(),
                ..Default::default()
            })
            .into()
        )
    );
    plans.stop_for(id, 7).await;
    let ended = subscription.next().await.unwrap();
    assert_eq!(
        ended.message,
        Some(
            event(PlaybackEnded {
                playback: changed.into(),
                ..Default::default()
            })
            .into()
        ),
        "ends as it played"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn transcodes_are_full_once_as_many_as_the_cap_run() {
    let data = tempfile::tempdir().unwrap();
    let db = database(data.path()).await;
    let plans = plans_in(
        &db,
        settings(&db, data.path().join("transcodes")).playback(),
    );
    for _ in 0..2 {
        start_method_writing_nothing(&plans, Method::Transcode, every_two_seconds()).await;
    }

    assert_eq!(plans.transcodes_full(8, u32::MAX), None);
    assert_eq!(plans.transcodes_full(8, 3), None);
    assert_eq!(plans.transcodes_full(8, 2), Some(2));
}

#[test]
fn segments_are_cut_at_the_first_keyframe_past_each_six_seconds() {
    let every_two_and_a_half_seconds: Vec<i64> = (0..12).map(|n| n * 2500).collect();

    let timeline = Timeline::new(every_two_and_a_half_seconds, 30_020);

    let segment_ms: Vec<i64> = timeline.segments.iter().map(|segment| segment.ms).collect();
    assert_eq!(segment_ms, [7500, 5000, 7500, 5000, 5020]);
    assert_eq!(timeline.keyframes_of(1), 3..5);
    assert_eq!(timeline.keyframes_of(4), 10..12);
}

#[test]
fn a_keyframe_at_the_length_joins_the_last_segment() {
    let keyframes: Vec<i64> = (0..4).map(|n| n * 2000).collect();

    let timeline = Timeline::new(keyframes, 6000);

    let segment_ms: Vec<i64> = timeline.segments.iter().map(|segment| segment.ms).collect();
    assert_eq!(segment_ms, [6000]);
    assert_eq!(timeline.keyframes_of(0), 0..4);
    assert!(!timeline.playlist().contains("#EXTINF:0.000"));
}

#[test]
fn a_keyframe_past_the_length_joins_the_last_segment() {
    let timeline = Timeline::new(vec![0, 2500, 5000, 7500], 7400);

    let segment_ms: Vec<i64> = timeline.segments.iter().map(|segment| segment.ms).collect();
    assert_eq!(segment_ms, [7400]);
    assert!(!timeline.playlist().contains("#EXTINF:0.000"));
}

#[test]
fn a_transcode_lists_a_segment_for_each_six_seconds_of_its_length() {
    let playlist = Timeline::transcoded(13_999).playlist();

    let listed: Vec<&str> = (playlist.lines())
        .filter(|line| line.starts_with("#EXT-X-TARGETDURATION") || line.starts_with("#EXTINF"))
        .collect();
    assert_eq!(
        listed,
        [
            "#EXT-X-TARGETDURATION:6",
            "#EXTINF:6.000,",
            "#EXTINF:6.000,",
            "#EXTINF:1.999,",
        ]
    );
}

/// Starts a plan for the Device `device` whose ffmpeg is `sleep`, and returns its id.
async fn start_sleeping(plans: &RunningPlans, device: i64) -> u64 {
    plans
        .start_hls(
            playback(device),
            Method::Remux,
            every_two_seconds(),
            Runs {
                run: Box::new(|_: &Path, _: Keyframe| {
                    tokio::process::Command::new("sleep")
                        .arg("60")
                        .kill_on_drop(true)
                        .spawn()
                }),
                fallback: None,
            },
        )
        .await
        .unwrap()
}

// The stand-in ffmpeg is `sleep`.
#[cfg(unix)]
#[tokio::test]
async fn a_plan_unused_for_long_enough_is_stopped_and_its_files_deleted() {
    let data = tempfile::tempdir().unwrap();
    let db = database(data.path()).await;
    let plans = plans_in(
        &db,
        settings(&db, data.path().join("transcodes")).playback(),
    );
    let device = 7;
    let (used, unused) = (
        start_sleeping(&plans, device).await,
        start_sleeping(&plans, device).await,
    );
    let dir = |id| hls(&plans, id, device).dir.clone();
    let (used_dir, unused_dir) = (dir(used), dir(unused));
    let plan = hls(&plans, unused, device);
    tokio::time::sleep(Duration::from_millis(100)).await;
    plans.ask(used, device).unwrap();

    plans.stop_unused(Duration::from_millis(50)).await;

    assert!(plans.ask(unused, device).is_none());
    assert!(plan.closed.load(std::sync::atomic::Ordering::Acquire));
    assert!(!unused_dir.exists());
    assert!(plans.ask(used, device).is_some());
    assert!(used_dir.exists());
}

// The stand-in ffmpeg is `sleep`.
#[cfg(unix)]
#[tokio::test]
async fn a_plan_gone_unused_tells_its_playback_ended() {
    let data = tempfile::tempdir().unwrap();
    let db = database(data.path()).await;
    let events = event_bus(&db);
    let plans = plans_with(
        &db,
        settings(&db, data.path().join("transcodes")).playback(),
        &events,
    );
    let mut subscription = Box::pin(events.subscribe_as_admin(Filter::default()));
    start_sleeping(&plans, 7).await;

    plans.stop_unused(Duration::ZERO).await;

    let started = event(PlaybackStarted {
        playback: playback(7).into(),
        ..Default::default()
    });
    let ended = event(PlaybackEnded {
        playback: playback(7).into(),
        ..Default::default()
    });
    for told in [started, ended] {
        let next = subscription.next().await.unwrap();
        assert_eq!(next.message, Some(told.into()));
    }
}

// The stand-in ffmpeg is `sleep`.
#[cfg(unix)]
#[tokio::test]
async fn a_plan_its_device_stops_ends_at_once_and_another_device_cant_stop_it() {
    let data = tempfile::tempdir().unwrap();
    let db = database(data.path()).await;
    let events = event_bus(&db);
    let plans = plans_with(
        &db,
        settings(&db, data.path().join("transcodes")).playback(),
        &events,
    );
    let (device, another) = (7, 8);
    let id = start_sleeping(&plans, device).await;
    let dir = hls(&plans, id, device).dir.clone();
    let mut subscription = Box::pin(events.subscribe_as_admin(Filter::default()));

    plans.stop_for(id, another).await;
    assert!(plans.ask(id, device).is_some());
    plans.stop_for(id, device).await;

    assert!(plans.ask(id, device).is_none());
    assert!(!dir.exists());
    let ended = event(PlaybackEnded {
        playback: playback(device).into(),
        ..Default::default()
    });
    let next = subscription.next().await.unwrap();
    assert_eq!(next.message, Some(ended.into()));
}

#[tokio::test]
async fn a_direct_play_is_kept_while_its_device_asks_for_its_version_and_then_ends() {
    let data = tempfile::tempdir().unwrap();
    let db = database(data.path()).await;
    let plans = plans_in(
        &db,
        settings(&db, data.path().join("transcodes")).playback(),
    );
    let (version, device, another) = (4, 7, 8);
    let direct_play = |on| Playback {
        device_id: on,
        version_id: version,
        method: PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY.into(),
        ..Default::default()
    };
    let (asked, unasked) = (
        plans.direct_play(direct_play(device)),
        plans.direct_play(direct_play(another)),
    );
    tokio::time::sleep(Duration::from_millis(100)).await;

    plans.ask_direct_plays(version, device);
    plans.stop_unused(Duration::from_millis(50)).await;

    assert!(plans.ask(asked, device).is_some());
    assert!(plans.ask(unasked, another).is_none());
}

// The stand-in ffmpeg is `sleep`.
#[cfg(unix)]
#[tokio::test]
async fn shutting_down_deletes_every_plans_files() {
    let data = tempfile::tempdir().unwrap();
    let db = database(data.path()).await;
    let plans = plans_in(
        &db,
        settings(&db, data.path().join("transcodes")).playback(),
    );
    let id = start_sleeping(&plans, 7).await;
    let dir = hls(&plans, id, 7).dir.clone();

    plans.shutdown().await;

    assert!(plans.ask(id, 7).is_none());
    assert!(!dir.exists());
}

#[tokio::test]
async fn a_new_plan_writes_to_the_segment_folder_set_since_the_last() {
    let data = tempfile::tempdir().unwrap();
    let db = database(data.path()).await;
    let settings = settings(&db, data.path().join("transcodes"));
    let plans = plans_in(&db, settings.playback());
    let before = start_sleeping(&plans, 7).await;
    let tmpfs = data.path().join("tmpfs");
    settings
        .change_playback(
            PlaybackChange {
                segment_folder: tmpfs.to_str().unwrap(),
                max_transcodes: None,
                keep_behind_minutes: None,
                hardware: HardwareChoice::Automatic,
                remote_limit: None,
            },
            &[],
        )
        .await
        .unwrap();

    let after = start_sleeping(&plans, 7).await;

    assert!(
        hls(&plans, before, 7)
            .dir
            .starts_with(data.path().join("transcodes"))
    );
    assert!(hls(&plans, after, 7).dir.starts_with(&tmpfs));
}

#[tokio::test]
async fn the_sweep_deletes_plan_folders_in_the_set_and_default_folders_and_nothing_else() {
    let data = tempfile::tempdir().unwrap();
    let (default, set) = (data.path().join("transcodes"), data.path().join("tmpfs"));
    let leftovers = [default.join("nama-plan-0"), set.join("nama-plan-3")];
    for leftover in &leftovers {
        fs::create_dir_all(leftover).unwrap();
        fs::write(leftover.join("0.m4s"), "segment").unwrap();
    }
    let others = set.join("someone else's");
    fs::create_dir_all(&others).unwrap();
    // A file, which deleting as a folder fails, so the sweep must go on past it.
    fs::write(default.join("nama-plan-not-a-folder"), "").unwrap();
    let db = database(data.path()).await;
    let settings = settings(&db, default);
    settings
        .change_playback(
            PlaybackChange {
                segment_folder: set.to_str().unwrap(),
                max_transcodes: None,
                keep_behind_minutes: None,
                hardware: HardwareChoice::Automatic,
                remote_limit: None,
            },
            &[],
        )
        .await
        .unwrap();

    sweep_leftovers(&settings.playback()).await;

    for leftover in leftovers {
        assert!(!leftover.exists(), "{}", leftover.display());
    }
    assert!(others.exists());
}

/// A plan Direct Playing the Version `version`.
fn direct_play_of(version: i64) -> crate::playback::Plan {
    crate::playback::Plan {
        version,
        playing: Some(crate::playback::Playing::DirectPlay),
        tracks: vec![],
        reasons: vec![],
        subtitles: vec![],
        limit: None,
        audio: None,
        burn_in: None,
        starting_subtitle: None,
        offers: crate::playback::Offers::default(),
    }
}

fn ripley() -> crate::users::Caller {
    use crate::users::{Caller, Device, Permissions, User};

    Caller {
        user: User {
            id: 1,
            name: "Ripley".to_owned(),
            permissions: Permissions::NEW_USER,
            created_at: 0,
        },
        device: Device {
            id: 1,
            name: "Nostromo".to_owned(),
            created_at: 0,
            last_seen_at: 0,
            authenticated_at: 0,
        },
    }
}

#[tokio::test]
async fn a_version_being_deleted_starts_no_plan_until_it_is_gone() {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;
    let deleting = plans.end_for_deleting(&[4]).await;

    let while_deleting = plans.start(&ripley(), direct_play_of(4)).await.unwrap();
    drop(deleting);
    let after = plans.start(&ripley(), direct_play_of(4)).await.unwrap();

    assert!(while_deleting.is_none());
    assert!(after.is_some());
}
