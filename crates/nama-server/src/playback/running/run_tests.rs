use std::{
    fs,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering::Relaxed},
    },
    time::Duration,
};

use futures_util::StreamExt;
use nama_proto::nama::v1::{Playback, PlaybackChanged, PlaybackEnded, PlaybackReasonKind};

use super::super::{
    lock,
    tests::{
        database, event_bus, every_two_seconds, hls, only_reason, plans_in, plans_with, playback,
        running_plans, settings, start_method_writing_nothing, start_writing_nothing,
    },
};
use super::{FallbackRun, Method, POLL_EVERY, Running, Runs, Timeline, keyframe_number};
use crate::{
    events::{Filter, event},
    ffmpeg::{Keyframe, hls_init},
    server_settings::{HardwareChoice, PlaybackChange},
};

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
