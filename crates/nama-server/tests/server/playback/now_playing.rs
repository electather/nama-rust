//! An Admin lists the plans playing now and stops any of them (`ListPlays`, `StopPlay`).

use nama_proto::nama::v1::{
    AdminStop, ListPlaysRequest, Play, ReportProgressRequest, StopPlayRequest, TranscodeHardware,
    WatchStateServiceClient,
};

use super::*;

/// The plans playing now, as `who` lists them.
async fn plays_as(who: ClientConfig) -> Result<Vec<Play>, ConnectError> {
    let listed = ServerServiceClient::new(HttpClient::plaintext(), who)
        .list_plays(ListPlaysRequest::default())
        .await?;
    Ok(listed.into_owned().plays)
}

/// Stops the plan `plan_id` as `who`, telling its viewer `message`.
async fn stop_play_as(who: ClientConfig, plan_id: u64, message: &str) -> Result<(), ConnectError> {
    ServerServiceClient::new(HttpClient::plaintext(), who)
        .stop_play(StopPlayRequest {
            plan_id,
            message: message.to_owned(),
            ..Default::default()
        })
        .await?;
    Ok(())
}

#[tokio::test]
async fn an_admin_lists_every_play_with_its_title_and_latest_progress_and_no_one_else_may() {
    let fx = Fixture::alien().await;
    let version = fx.version_ids(ALIEN).await[0];
    let bishop = fx.server.user_with_default_permissions("Bishop").await;
    let plan = plan_as(&bishop, version).await.unwrap();
    for position_ms in [5_000, 15_000] {
        WatchStateServiceClient::new(HttpClient::plaintext(), bishop.client_config())
            .report_progress(ReportProgressRequest {
                version_id: version,
                position_ms,
                duration_ms: 60_000,
                ..Default::default()
            })
            .await
            .unwrap();
    }

    let listed = plays_as(fx.server.client_config()).await.unwrap();
    let as_bishop = plays_as(bishop.client_config()).await;

    let [bishops] = listed.try_into().unwrap();
    let playback = bishops.playback.as_option().unwrap();
    assert_eq!(
        (playback.plan_id, playback.user_id, playback.method),
        (plan.id, bishop.id, plan.method)
    );
    assert_eq!(bishops.title, "Alien");
    assert_eq!(
        (bishops.position_ms, bishops.duration_ms),
        (Some(15_000), 60_000)
    );
    assert!(bishops.transcodes_on.is_unset(), "a Direct Play");
    assert_refused(
        as_bishop,
        ErrorCode::PermissionDenied,
        RefusalReason::REFUSAL_REASON_PERMISSION_REQUIRED,
    );
}

#[tokio::test]
async fn an_admins_stop_ends_another_users_play_and_tells_them_with_the_message() {
    let fx = Fixture::alien().await;
    let version = fx.version_ids(ALIEN).await[0];
    let bishop = fx.server.user_with_default_permissions("Bishop").await;
    let ended = [EventType::EVENT_TYPE_PLAYBACK_ENDED];
    let mut bishops_events = subscribe(&bishop.client_config(), &ended).await;
    let plan = plan_as(&bishop, version).await.unwrap();
    // 200 characters, each more than one byte.
    let message = "é".repeat(200);

    stop_play_as(fx.server.client_config(), plan.id, &message)
        .await
        .unwrap();

    let event::Event::PlaybackEnded(told) = bishops_events.next().await else {
        panic!("not a PlaybackEnded");
    };
    let playback = told.playback.as_option().unwrap();
    assert_eq!((playback.plan_id, playback.user_id), (plan.id, bishop.id));
    let admin_stop = AdminStop {
        message,
        ..Default::default()
    };
    assert_eq!(told.admin_stop.as_option(), Some(&admin_stop));
    assert_eq!(plays_as(fx.server.client_config()).await.unwrap(), []);
}

#[tokio::test]
async fn a_stop_message_over_200_characters_is_refused_and_stops_nothing() {
    let fx = Fixture::alien().await;
    let version = fx.version_ids(ALIEN).await[0];
    let plan = fx.plan(version).await.unwrap();

    let stopped = stop_play_as(fx.server.client_config(), plan.id, &"a".repeat(201)).await;

    let refusal = assert_refused(
        stopped,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_MESSAGE_TOO_LONG,
    );
    assert_eq!(refusal.field, "message");
    assert_eq!(
        refusal.params.get("maxLength").map(String::as_str),
        Some("200")
    );
    assert_eq!(plays_as(fx.server.client_config()).await.unwrap().len(), 1);
}

// The fake ffmpeg and ffprobe are shell scripts.
#[cfg(unix)]
#[tokio::test]
async fn a_transcode_is_listed_on_its_gpu_and_on_the_cpu_once_its_hardware_failed() {
    let bin = tempfile::tempdir().unwrap();
    let (sys, dri) = (bin.path().join("sys"), bin.path().join("dri"));
    fake::two_amd_gpus_at(bin.path(), ["renderD129", "renderD128"]);
    let fail = bin.path().join("fail");
    // Passes every test encode; a Transcode of Alien on VAAPI fails once `fail` exists.
    let ffmpeg = format!(
        "#!/bin/sh\ncase \"$*\" in *-encoders*) printf ' V....D %s  H.264\\n' libx264 \
         h264_vaapi ;; *-filters*) ;; *vaapi*Alien*) while [ ! -e '{}' ]; do sleep 0.1; done; \
         echo 'Failed to upload the frame' >&2; exit 1 ;; *Alien*) exec sleep 600 ;; esac\n",
        fail.display()
    );
    let ffmpeg = Ffmpeg {
        ffmpeg: Ok(script(bin.path(), "ffmpeg-failing-on-gpu", &ffmpeg)),
        gpus: ffmpeg::find_gpus(&dri, &sys.join("class/drm")),
        ..ffmpeg_writing_nothing(bin.path(), "libx264")
    };
    let fx = Fixture::start_with_ffmpeg(LibraryKind::LIBRARY_KIND_MOVIES, ffmpeg).await;
    fx.write(ALIEN_2160P, b"video");
    fx.scan_and_match().await;
    ServerServiceClient::new(HttpClient::plaintext(), fx.server.client_config())
        .update_playback_settings(UpdatePlaybackSettingsRequest {
            settings: PlaybackSettings {
                hardware_acceleration: HardwareAcceleration::HARDWARE_ACCELERATION_VAAPI.into(),
                ..Default::default()
            }
            .into(),
            ..Default::default()
        })
        .await
        .unwrap();
    let [hevc] = fx.version_ids(ALIEN).await.try_into().unwrap();
    let changed = [EventType::EVENT_TYPE_PLAYBACK_CHANGED];
    let mut subscription = subscribe(&fx.server.client_config(), &changed).await;
    fx.transcode(hevc).await;

    let while_on_gpu = plays_as(fx.server.client_config()).await.unwrap();
    fs::write(&fail, "").unwrap();
    subscription.next().await;
    let once_failed = plays_as(fx.server.client_config()).await.unwrap();

    let gpu = Gpu {
        id: "0000:03:00.0".to_owned(),
        name: "AMD GPU at 0000:03:00.0".to_owned(),
        ..Default::default()
    };
    let transcodes_on = |plays: &[Play]| plays[0].transcodes_on.as_option().cloned();
    assert_eq!(
        transcodes_on(&while_on_gpu),
        Some(TranscodeHardware {
            api: HardwareAcceleration::HARDWARE_ACCELERATION_VAAPI.into(),
            gpu: gpu.into(),
            ..Default::default()
        })
    );
    assert_eq!(
        transcodes_on(&once_failed),
        Some(TranscodeHardware {
            api: HardwareAcceleration::HARDWARE_ACCELERATION_OFF.into(),
            ..Default::default()
        })
    );
}
