//! A Device switching between the Users signed in to it (ADR 0106): a plan belongs to the User it
//! was made for, so the Device gets it only while it acts as them.

use super::*;

#[tokio::test]
async fn a_remux_is_served_to_its_device_only_while_it_acts_as_the_user_it_was_planned_for() {
    let (fx, version) = Fixture::alien_mkv(system_ffmpeg(), "aac").await;
    let ripley = fx.server.user_with_default_permissions("Ripley").await;
    let hicks = fx.server.create_user("Hicks").await;
    fx.server.sign_in_on(&ripley.token, "Hicks").await;
    fx.server.switch_to(&ripley.token, ripley.id).await;
    let original = picking(version, PlaybackLimit::PLAYBACK_LIMIT_ORIGINAL);
    let plan = ask_for_plan_as(ripley.client_config(), original)
        .await
        .unwrap();
    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_REMUX);
    let init = beside(&plan.url, "init.mp4");
    let status = || async { fetch(&fx.server, &init, Some(&ripley.token)).await.status };

    fx.server.switch_to(&ripley.token, hicks).await;
    let as_hicks = status().await;
    fx.server.switch_to(&ripley.token, ripley.id).await;

    assert_eq!(as_hicks, 404);
    assert_eq!(status().await, 200);
}

// A bearer client, such as a shared TV, plays through signed URLs, which act as the User they were
// signed for (ADR 0105), so they must stop once the Device acts as someone else.
#[tokio::test]
async fn a_bearer_devices_signed_playback_urls_load_only_while_it_acts_as_the_user_they_were_for() {
    let (fx, version) = Fixture::alien_mkv(system_ffmpeg(), "aac").await;
    let ripley = fx.server.user_with_default_permissions("Ripley").await;
    let hicks = fx.server.create_user("Hicks").await;
    let as_bearer = || {
        fx.server
            .anonymous_client_config()
            .with_default_header("authorization", format!("Bearer {}", ripley.token))
    };
    let remux = picking(version, PlaybackLimit::PLAYBACK_LIMIT_ORIGINAL);
    let hls = ask_for_plan_as(as_bearer(), remux.clone()).await.unwrap();
    let in_matroska = PlanPlaybackRequest {
        version_id: version,
        device_decoding: DeviceDecoding {
            containers: vec![Container::CONTAINER_MATROSKA.into()],
            ..chrome()
        }
        .into(),
        ..Default::default()
    };
    let direct = ask_for_plan_as(as_bearer(), in_matroska).await.unwrap();
    assert_eq!(hls.method, PlaybackMethod::PLAYBACK_METHOD_REMUX);
    assert_eq!(direct.method, PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY);
    let urls = [beside(&hls.url, "init.mp4"), direct.url];
    let statuses = || async {
        let mut statuses = vec![];
        for url in &urls {
            statuses.push(fetch(&fx.server, url, None).await.status);
        }
        statuses
    };
    let before = statuses().await;

    fx.server.sign_in_on(&ripley.token, "Hicks").await;
    let as_hicks = statuses().await;
    fx.server.switch_to(&ripley.token, ripley.id).await;
    let back_as_ripley = statuses().await;
    fx.server.switch_to(&ripley.token, hicks).await;
    let switched_to_hicks = statuses().await;
    let hicks_plan = ask_for_plan_as(as_bearer(), remux).await.unwrap();
    let hicks_hls = fetch(&fx.server, &beside(&hicks_plan.url, "init.mp4"), None)
        .await
        .status;

    assert_eq!(before, [200, 200]);
    assert_eq!(as_hicks, [401, 401]);
    assert_eq!(back_as_ripley, [200, 200]);
    assert_eq!(switched_to_hicks, [401, 401]);
    assert_eq!(hicks_hls, 200);
}
