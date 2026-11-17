use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, atomic::Ordering},
    time::Duration,
};

use futures_util::StreamExt;
use nama_proto::nama::v1::{
    Playback, PlaybackEnded, PlaybackMethod, PlaybackReason, PlaybackReasonKind, PlaybackStarted,
};

use super::{
    Method, Running, RunningPlans, RunningTranscodes, Runs, Timeline, lock, sweep_leftovers,
};
use crate::{
    db::Database,
    events::{EventBus, Filter, event},
    ffmpeg::{Ffmpeg, Keyframe},
    library::Contents,
    playback::{Probing, subtitles},
    plugins::ExtensionPoints,
    server_settings::{HardwareChoice, PlaybackChange, PlaybackSettings, ServerUrl, Settings},
};

pub(super) fn event_bus(db: &Arc<Database>) -> EventBus {
    EventBus::new(
        Arc::clone(db),
        ExtensionPoints::default(),
        ServerUrl::default(),
    )
}

pub(super) fn plans_in(db: &Arc<Database>, settings: PlaybackSettings) -> RunningPlans {
    let events = event_bus(db);
    plans_with(db, settings, &events)
}

pub(super) fn plans_with(
    db: &Arc<Database>,
    settings: PlaybackSettings,
    events: &EventBus,
) -> RunningPlans {
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
pub(super) fn playback(device: i64) -> Playback {
    Playback {
        device_id: device,
        method: PlaybackMethod::PLAYBACK_METHOD_REMUX.into(),
        ..Default::default()
    }
}

/// A playback's reasons when it has only one, of `kind`.
pub(super) fn only_reason(kind: PlaybackReasonKind) -> Vec<PlaybackReason> {
    vec![PlaybackReason {
        kind: kind.into(),
        ..Default::default()
    }]
}

/// The HLS files of the plan `id`, as asked for by the Device `device`.
pub(super) fn hls(plans: &RunningPlans, id: u64, device: i64) -> Arc<Running> {
    Arc::clone(plans.ask(id, device).unwrap().hls.as_ref().unwrap())
}

/// The Server settings in `db`, with segment files going to `default` while no folder is set.
pub(super) fn settings(db: &Arc<Database>, default: PathBuf) -> Settings {
    Settings::new(Arc::clone(db), default)
}

pub(super) async fn database(data: &Path) -> Arc<Database> {
    Arc::new(Database::open(data).await.unwrap())
}

pub(super) async fn running_plans(data: &Path) -> RunningPlans {
    let db = database(data).await;
    plans_in(&db, settings(&db, data.join("transcodes")).playback())
}

/// A Version 1000 seconds long with a keyframe every 2, so each segment is 3 keyframes long.
pub(super) fn every_two_seconds() -> Timeline {
    Timeline::new((0..500).map(|n| n * 2000).collect(), 1_000_000)
}

/// Starts a Remux for the Device 7 whose ffmpeg writes nothing, and returns its id with the
/// keyframes each run of ffmpeg started at.
pub(super) async fn start_writing_nothing(plans: &RunningPlans) -> (u64, Arc<Mutex<Vec<usize>>>) {
    start_method_writing_nothing(plans, Method::Remux, every_two_seconds(), 0).await
}

/// As [`start_writing_nothing`], for a plan by `method` cut as `timeline` says, played from
/// `start_ms`.
pub(super) async fn start_method_writing_nothing(
    plans: &RunningPlans,
    method: Method,
    timeline: Timeline,
    start_ms: i64,
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
            start_ms,
            Runs {
                run: Box::new(run),
                fallback: None,
                cpu_bound: false,
            },
        )
        .await
        .unwrap();
    (id, starts)
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
async fn every_devices_transcodes_count_until_one_stops_and_remuxes_never_do() {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;
    let transcode = || {
        let timeline = every_two_seconds();
        start_method_writing_nothing(&plans, Method::Transcode, timeline, 0)
    };
    let (first, _) = transcode().await;
    start_writing_nothing(&plans).await;
    assert_eq!(plans.running_transcodes().all, 1, "a Remux doesn't count");
    transcode().await;

    let full = plans.running_transcodes().all;
    plans.stop_for(first, 7).await;

    assert_eq!(full, 2);
    assert_eq!(plans.running_transcodes().all, 1);
}

// The fake ffmpeg is `sleep`.
#[cfg(unix)]
#[tokio::test]
async fn a_transcode_on_hardware_counts_as_cpu_bound_once_it_fell_back_and_a_remux_never_does() {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;
    let (on_hardware, _) =
        start_method_writing_nothing(&plans, Method::Transcode, every_two_seconds(), 0).await;
    let (remux, _) = start_writing_nothing(&plans).await;
    let before = plans.running_transcodes();

    hls(&plans, on_hardware, 7)
        .fell_back
        .store(true, Ordering::Release);
    hls(&plans, remux, 7)
        .fell_back
        .store(true, Ordering::Release);

    assert_eq!(before.cpu_bound, 0);
    assert_eq!(
        plans.running_transcodes(),
        RunningTranscodes {
            all: 1,
            cpu_bound: 1
        }
    );
}

/// Starts a plan for the Device `device` whose ffmpeg is `sleep`, and returns its id.
async fn start_sleeping(plans: &RunningPlans, device: i64) -> u64 {
    plans
        .start_hls(
            playback(device),
            Method::Remux,
            every_two_seconds(),
            0,
            Runs {
                run: Box::new(|_: &Path, _: Keyframe| {
                    tokio::process::Command::new("sleep")
                        .arg("60")
                        .kill_on_drop(true)
                        .spawn()
                }),
                fallback: None,
                cpu_bound: false,
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
    assert!(plan.closed.load(Ordering::Acquire));
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

    let while_deleting = plans.start(&ripley(), direct_play_of(4), 0).await.unwrap();
    drop(deleting);
    let after = plans.start(&ripley(), direct_play_of(4), 0).await.unwrap();

    assert!(while_deleting.is_none());
    assert!(after.is_some());
}
