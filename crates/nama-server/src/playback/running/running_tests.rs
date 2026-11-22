use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use futures_util::StreamExt;
use nama_proto::nama::v1::{
    Playback, PlaybackChanged, PlaybackEnded, PlaybackMethod, PlaybackReason, PlaybackReasonKind,
    PlaybackStarted,
};
use tokio::task::JoinHandle;

use super::{
    FallbackRun, Method, RunningPlans, RunningTranscodes, Runs, StartError, sweep_leftovers,
    testing::{FakeRuns, every_two_seconds},
};
use crate::{
    db::Database,
    events::{EventBus, Filter, event},
    ffmpeg::{Accel, Ffmpeg, Hardware},
    library::Contents,
    playback::{ProbePasses, Probing, WholeFileReads, subtitles},
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

fn plans_in(db: &Arc<Database>, settings: PlaybackSettings) -> RunningPlans {
    let events = event_bus(db);
    plans_with(db, settings, &events)
}

pub(super) fn plans_with(
    db: &Arc<Database>,
    settings: PlaybackSettings,
    events: &EventBus,
) -> RunningPlans {
    let ffmpeg = Arc::new(Ffmpeg::default());
    // Nothing here burns a subtitle in, so the cache's folder is never touched.
    let subtitles = subtitles::Cache::new(&std::env::temp_dir());
    let probing = Probing::new(
        Arc::clone(db),
        Arc::clone(&ffmpeg),
        ProbePasses::default(),
        WholeFileReads::default(),
    );
    RunningPlans::new(
        settings,
        ffmpeg,
        Contents::new(Arc::clone(db)),
        probing,
        subtitles,
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
fn only_reason(kind: PlaybackReasonKind) -> Vec<PlaybackReason> {
    vec![PlaybackReason {
        kind: kind.into(),
        ..Default::default()
    }]
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

/// The folder of the plan `id`, in the default segment folder in `data`.
fn plan_dir(data: &Path, id: u64) -> PathBuf {
    data.join("transcodes").join(format!("nama-plan-{id}"))
}

/// `starting`'s outcome, on paused time, so the wait for a first file that fake runs never write
/// ends at once, as it would for a slow but healthy ffmpeg.
async fn past_the_wait<T>(starting: impl Future<Output = T>) -> T {
    tokio::time::pause();
    let started = starting.await;
    tokio::time::resume();
    started
}

/// Starts a plan by `method` for the Device `device`, whose runs the test stands in for and which
/// write nothing, and returns its id with those runs.
pub(super) async fn start_faked(
    plans: &RunningPlans,
    method: Method,
    device: i64,
) -> (u64, FakeRuns) {
    let runs = FakeRuns::default();
    let started = Runs {
        method,
        start: runs.start(),
        cpu_bound: false,
        fallback: None,
    };
    let starting = plans.start_hls(playback(device), every_two_seconds(), 0, started, None);
    (past_the_wait(starting).await.unwrap(), runs)
}

/// Starts a Remux for the Device `device` whose runs the test stands in for, and returns its id.
pub(super) async fn start_remux(plans: &RunningPlans, device: i64) -> u64 {
    start_faked(plans, Method::Remux, device).await.0
}

/// Starts a Transcode on hardware for the Device 7, as `started`, falling back to software, and
/// returns its id with its runs on hardware and in software, which write nothing.
pub(super) async fn start_on_hardware(
    plans: &RunningPlans,
    started: Playback,
    software_reasons: Vec<PlaybackReason>,
) -> (u64, FakeRuns, FakeRuns) {
    let (starting, hardware, software) = starting_on_hardware(plans, started, software_reasons);
    (
        past_the_wait(starting).await.unwrap().unwrap(),
        hardware,
        software,
    )
}

/// Starts a Transcode on hardware as [`start_on_hardware`] does, in a task of its own, and returns
/// the task with the runs.
fn starting_on_hardware(
    plans: &RunningPlans,
    started: Playback,
    software_reasons: Vec<PlaybackReason>,
) -> (JoinHandle<Result<u64, StartError>>, FakeRuns, FakeRuns) {
    let (hardware, software) = (FakeRuns::default(), FakeRuns::default());
    let runs = Runs {
        method: Method::Transcode,
        start: hardware.start(),
        cpu_bound: false,
        fallback: None,
    };
    let fallback = FallbackRun {
        start: software.start(),
        reasons: software_reasons,
        hardware: vaapi(),
    };
    let plans = plans.clone();
    let starting = tokio::spawn(async move {
        (plans.start_hls(started, every_two_seconds(), 0, runs, Some(fallback))).await
    });
    (starting, hardware, software)
}

/// The hardware [`start_on_hardware`] Transcodes on.
pub(super) fn vaapi() -> Hardware {
    Hardware {
        accel: Accel::Vaapi,
        gpu: None,
        tone_maps: false,
    }
}

#[tokio::test]
async fn a_running_plan_is_marked_with_its_method_and_found_only_by_its_device() {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;

    let id = start_remux(&plans, 7).await;

    let plan = plans.ask(id, 7).unwrap();
    assert_eq!(plan.hls.as_ref().unwrap().method(), Method::Remux);
    assert!(plans.ask(id, 8).is_none());
}

#[tokio::test]
async fn every_devices_transcodes_count_until_one_stops_and_remuxes_never_do() {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;
    let (first, _) = start_faked(&plans, Method::Transcode, 7).await;
    start_remux(&plans, 7).await;
    assert_eq!(plans.running_transcodes().all, 1, "a Remux doesn't count");
    start_faked(&plans, Method::Transcode, 8).await;

    let full = plans.running_transcodes().all;
    plans.stop_for(first, 7).await;

    assert_eq!(full, 2);
    assert_eq!(plans.running_transcodes().all, 1);
}

#[tokio::test]
async fn a_transcode_on_hardware_counts_as_cpu_bound_once_it_fell_back() {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;
    let (_, hardware, software) = start_on_hardware(&plans, playback(7), vec![]).await;
    start_remux(&plans, 7).await;
    let before = plans.running_transcodes();

    hardware.fail(0, "Failed to upload the frame to the GPU");
    software.until_started(1).await;

    assert_eq!(before.cpu_bound, 0);
    assert_eq!(
        plans.running_transcodes(),
        RunningTranscodes {
            all: 1,
            cpu_bound: 1
        }
    );
}

#[tokio::test]
async fn a_transcode_whose_hardware_fails_says_once_it_plays_in_software_and_ends_so() {
    let data = tempfile::tempdir().unwrap();
    let db = database(data.path()).await;
    let events = event_bus(&db);
    let plans = plans_with(
        &db,
        settings(&db, data.path().join("transcodes")).playback(),
        &events,
    );
    let mut subscription = Box::pin(events.subscribe_as_admin(Filter::default()));
    let started = Playback {
        reasons: only_reason(PlaybackReasonKind::PLAYBACK_REASON_KIND_TRANSCODES_ON_HARDWARE),
        ..playback(7)
    };
    let software_reasons =
        only_reason(PlaybackReasonKind::PLAYBACK_REASON_KIND_HARDWARE_FAILED_PARTWAY);
    let (id, hardware, software) =
        start_on_hardware(&plans, started, software_reasons.clone()).await;

    hardware.fail(0, "Failed to upload the frame to the GPU");
    software.until_started(1).await;
    software.fail(0, "Conversion failed!");
    plans.stop_for(id, 7).await;

    let changed = Playback {
        reasons: software_reasons,
        plan_id: id,
        ..playback(7)
    };
    subscription.next().await.unwrap();
    let told_in_order = [
        event(PlaybackChanged {
            playback: changed.clone().into(),
            ..Default::default()
        }),
        event(PlaybackEnded {
            playback: changed.into(),
            ..Default::default()
        }),
    ];
    for told in told_in_order {
        let next = subscription.next().await.unwrap();
        assert_eq!(
            next.message,
            Some(told.into()),
            "once, and ends as it played"
        );
    }
}

#[tokio::test]
async fn a_plan_whose_ffmpeg_is_still_going_after_the_wait_for_its_first_file_starts() {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;

    let (id, runs) = start_faked(&plans, Method::Transcode, 7).await;

    assert!(plans.ask(id, 7).is_some());
    assert!(!runs.dropped(0));
}

#[tokio::test]
async fn a_transcode_whose_hardware_fails_while_it_starts_is_told_once_as_started_in_software() {
    let data = tempfile::tempdir().unwrap();
    let db = database(data.path()).await;
    let events = event_bus(&db);
    let plans = plans_with(
        &db,
        settings(&db, data.path().join("transcodes")).playback(),
        &events,
    );
    let mut subscription = Box::pin(events.subscribe_as_admin(Filter::default()));
    let software_reasons =
        only_reason(PlaybackReasonKind::PLAYBACK_REASON_KIND_HARDWARE_FAILED_PARTWAY);
    // Once the database is open, whose pool would time out on paused time.
    tokio::time::pause();
    let (starting, hardware, software) =
        starting_on_hardware(&plans, playback(7), software_reasons.clone());

    hardware.until_started(1).await;
    hardware.fail(0, "Failed to upload the frame to the GPU");
    software.until_started(1).await;
    let id = starting.await.unwrap().unwrap();
    plans.stop_for(id, 7).await;

    let started = Playback {
        reasons: software_reasons,
        plan_id: id,
        ..playback(7)
    };
    let told_in_order = [
        event(PlaybackStarted {
            playback: started.clone().into(),
            ..Default::default()
        }),
        event(PlaybackEnded {
            playback: started.into(),
            ..Default::default()
        }),
    ];
    for told in told_in_order {
        let next = subscription.next().await.unwrap();
        assert_eq!(next.message, Some(told.into()), "once, and nothing else");
    }
}

#[tokio::test]
async fn a_plan_unused_for_long_enough_is_stopped_and_its_files_deleted() {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;
    let device = 7;
    let (used, _) = start_faked(&plans, Method::Remux, device).await;
    let (unused, unused_runs) = start_faked(&plans, Method::Remux, device).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    plans.ask(used, device).unwrap();

    plans.stop_unused(Duration::from_millis(50)).await;

    assert!(plans.ask(unused, device).is_none());
    assert!(unused_runs.dropped(0));
    assert!(!plan_dir(data.path(), unused).exists());
    assert!(plans.ask(used, device).is_some());
    assert!(plan_dir(data.path(), used).exists());
}

#[tokio::test]
async fn a_plan_whose_ffmpeg_cant_be_spawned_deletes_its_folder_and_isnt_kept() {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;
    let runs = FakeRuns::default();
    runs.fail_next_start();
    let started = Runs {
        method: Method::Remux,
        start: runs.start(),
        cpu_bound: false,
        fallback: None,
    };
    let (first_id, device) = (1, 7);

    let failed = plans.start_hls(playback(device), every_two_seconds(), 0, started, None);

    assert!(matches!(failed.await, Err(StartError::Run(_))));
    assert!(!plan_dir(data.path(), first_id).exists());
    assert!(plans.ask(first_id, device).is_none());
}

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
    let id = start_remux(&plans, 7).await;

    plans.stop_unused(Duration::ZERO).await;

    let playback = Playback {
        plan_id: id,
        ..playback(7)
    };
    let started = event(PlaybackStarted {
        playback: playback.clone().into(),
        ..Default::default()
    });
    let ended = event(PlaybackEnded {
        playback: playback.into(),
        ..Default::default()
    });
    for told in [started, ended] {
        let next = subscription.next().await.unwrap();
        assert_eq!(next.message, Some(told.into()));
    }
}

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
    let (id, runs) = start_faked(&plans, Method::Remux, device).await;
    let mut subscription = Box::pin(events.subscribe_as_admin(Filter::default()));

    plans.stop_for(id, another).await;
    assert!(plans.ask(id, device).is_some());
    plans.stop_for(id, device).await;

    assert!(plans.ask(id, device).is_none());
    assert!(runs.dropped(0));
    assert!(!plan_dir(data.path(), id).exists());
    let ended = event(PlaybackEnded {
        playback: Playback {
            plan_id: id,
            ..playback(device)
        }
        .into(),
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

#[tokio::test]
async fn a_direct_play_is_kept_while_its_file_is_sent_and_then_ends() {
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
    let (sent_to, not_sent_to) = (
        plans.direct_play(direct_play(device)),
        plans.direct_play(direct_play(another)),
    );
    let late_part = futures_util::stream::once(async {
        tokio::time::sleep(Duration::from_millis(100)).await;
    });
    let mut file =
        std::pin::pin!(plans.ask_direct_plays_while_sending(version, Some(device), late_part));

    file.next().await;
    plans.stop_unused(Duration::from_millis(50)).await;

    assert!(plans.ask(sent_to, device).is_some());
    assert!(plans.ask(not_sent_to, another).is_none());

    assert!(file.next().await.is_none());
    tokio::time::sleep(Duration::from_millis(100)).await;
    plans.stop_unused(Duration::from_millis(50)).await;

    assert!(plans.ask(sent_to, device).is_none());
}

#[tokio::test]
async fn a_tick_after_keep_for_skipping_back_is_lowered_deletes_what_it_no_longer_keeps() {
    let data = tempfile::tempdir().unwrap();
    let db = database(data.path()).await;
    let settings = settings(&db, data.path().join("transcodes"));
    let plans = plans_in(&db, settings.playback());
    let id = start_remux(&plans, 7).await;
    let dir = plan_dir(data.path(), id);
    // To 400 seconds, played to 360.
    for n in 0..200 {
        fs::write(dir.join(format!("{n}.m4s")), "").unwrap();
    }
    let plan = plans.ask(id, 7).unwrap();
    plan.hls
        .as_ref()
        .unwrap()
        .segment(60)
        .await
        .unwrap()
        .unwrap();
    plans.delete_unkept_files().await;

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
    plans.delete_unkept_files().await;

    // Keyframe 149 ends at 300 seconds, a minute before.
    let mut left: Vec<String> = (fs::read_dir(&dir).unwrap())
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    left.sort_by_key(|name| name.trim_end_matches(".m4s").parse::<usize>().unwrap());
    assert_eq!(left.first().map(String::as_str), Some("149.m4s"));
    assert_eq!(left.len(), 51);
}

#[tokio::test]
async fn shutting_down_deletes_every_plans_files() {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;
    let (id, runs) = start_faked(&plans, Method::Remux, 7).await;

    plans.shutdown().await;

    assert!(plans.ask(id, 7).is_none());
    assert!(runs.dropped(0));
    assert!(!plan_dir(data.path(), id).exists());
}

#[tokio::test]
async fn a_new_plan_writes_to_the_segment_folder_set_since_the_last() {
    let data = tempfile::tempdir().unwrap();
    let db = database(data.path()).await;
    let settings = settings(&db, data.path().join("transcodes"));
    let plans = plans_in(&db, settings.playback());
    let before = start_remux(&plans, 7).await;
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

    let after = start_remux(&plans, 7).await;

    assert!(plan_dir(data.path(), before).exists());
    assert!(tmpfs.join(format!("nama-plan-{after}")).exists());
    assert!(!plan_dir(data.path(), after).exists());
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
pub(super) fn direct_play_of(version: i64) -> crate::playback::Plan {
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
        chapters: vec![],
        segments: vec![],
        preview_frames: None,
        subtitle_offsets: crate::playback::SubtitleOffsets::default(),
    }
}

pub(super) fn ripley() -> crate::users::Caller {
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
