use futures_util::StreamExt;
use nama_proto::nama::v1::{AdminStop, Playback, PlaybackEnded};

use super::{
    super::{
        Method,
        tests::{
            database, direct_play_of, event_bus, plans_with, playback, ripley, running_plans,
            settings, start_faked, start_on_hardware, start_remux, vaapi,
        },
    },
    ReportedProgress,
};
use crate::events::{Filter, event};

#[tokio::test]
async fn a_play_runs_on_its_hardware_until_it_falls_back_to_software() {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;
    let (_, hardware, software) = start_on_hardware(&plans, playback(7), vec![]).await;
    let before = plans.plays().await.unwrap();

    hardware.fail(0, "Failed to upload the frame to the GPU");
    software.until_started(1).await;

    assert_eq!(before[0].hardware, Some(vaapi()));
    assert_eq!(plans.plays().await.unwrap()[0].hardware, None);
}

#[tokio::test]
async fn plays_are_listed_oldest_first_with_the_progress_their_device_last_reported() {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;
    let (version, device) = (0, 7);
    let first = start_remux(&plans, device).await;
    let second = start_remux(&plans, 8).await;
    let at = |position_ms| ReportedProgress {
        position_ms,
        duration_ms: 60_000,
    };

    plans.report_progress(version, device, at(1_000));
    plans.report_progress(version, device, at(2_000));
    plans.report_progress(5, device, at(3_000));

    let listed = plans.plays().await.unwrap();
    let progress: Vec<_> = (listed.iter())
        .map(|play| (play.playback.plan_id, play.progress))
        .collect();
    assert_eq!(progress, [(first, Some(at(2_000))), (second, None)]);
}

#[tokio::test]
async fn an_admin_stops_a_plan_on_any_device_and_its_end_carries_their_message() {
    let data = tempfile::tempdir().unwrap();
    let db = database(data.path()).await;
    let events = event_bus(&db);
    let plans = plans_with(
        &db,
        settings(&db, data.path().join("transcodes")).playback(),
        &events,
    );
    let (id, runs) = start_faked(&plans, Method::Remux, 7).await;
    let mut subscription = Box::pin(events.subscribe_as_admin(Filter::default()));
    let admin_stop = AdminStop {
        message: "Bedtime".to_owned(),
        ..Default::default()
    };

    plans.stop_by_admin(id, "Bedtime".to_owned()).await.unwrap();

    assert!(plans.ask(id, 7).is_none());
    assert!(runs.dropped(0));
    let ended = event(PlaybackEnded {
        playback: Playback {
            plan_id: id,
            ..playback(7)
        }
        .into(),
        admin_stop: admin_stop.into(),
        ..Default::default()
    });
    let next = subscription.next().await.unwrap();
    assert_eq!(next.message, Some(ended.into()));
}

#[tokio::test]
async fn an_admin_stopping_a_direct_play_cuts_off_its_devices_file_until_it_plays_again() {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;
    let on_ripley = Some(ripley().device.id);
    let while_playing = plans.stopped(4, on_ripley);
    let to_another = plans.stopped(4, Some(2));
    let started = plans.start(&ripley(), direct_play_of(4), 0).await.unwrap();

    plans
        .stop_by_admin(started.unwrap().id, String::new())
        .await
        .unwrap();
    let asked_again = plans.stopped(4, on_ripley);
    plans.start(&ripley(), direct_play_of(4), 0).await.unwrap();

    assert!(while_playing.is_cancelled());
    assert!(
        asked_again.is_cancelled(),
        "a stop isn't undone by asking again"
    );
    assert!(!to_another.is_cancelled());
    assert!(
        !plans.stopped(4, on_ripley).is_cancelled(),
        "playing again lifts it"
    );
}

#[tokio::test]
async fn an_admin_stopping_one_of_a_devices_plans_of_a_version_cuts_off_nothing_while_another_plays()
 {
    let data = tempfile::tempdir().unwrap();
    let plans = running_plans(data.path()).await;
    let stale = plans.start(&ripley(), direct_play_of(4), 0).await.unwrap();
    plans.start(&ripley(), direct_play_of(4), 0).await.unwrap();
    let to_ripley = plans.stopped(4, Some(ripley().device.id));

    plans
        .stop_by_admin(stale.unwrap().id, String::new())
        .await
        .unwrap();

    assert!(!to_ripley.is_cancelled());
}
