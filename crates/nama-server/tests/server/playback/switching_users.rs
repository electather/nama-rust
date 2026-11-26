//! A Device switching between the Users signed in to it (ADR 0106): a plan belongs to the User it
//! was made for, so the Device gets it only while it acts as them.

use super::*;

#[tokio::test]
async fn a_remux_is_served_to_its_device_only_while_it_acts_as_the_user_it_was_planned_for() {
    let (fx, version) = Fixture::alien_mkv(system_ffmpeg(), "aac").await;
    let ripley = fx.server.user_with_default_permissions("Ripley").await;
    fx.server.create_user("Hicks").await;
    let original = picking(version, PlaybackLimit::PLAYBACK_LIMIT_ORIGINAL);
    let plan = ask_for_plan_as(ripley.client_config(), original)
        .await
        .unwrap();
    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_REMUX);
    let init = beside(&plan.url, "init.mp4");
    let status = || async { fetch(&fx.server, &init, Some(&ripley.token)).await.status };

    fx.server.sign_in_on(&ripley.token, "Hicks").await;
    let as_hicks = status().await;
    fx.server.switch_to(&ripley.token, ripley.id).await;

    assert_eq!(as_hicks, 404);
    assert_eq!(status().await, 200);
}
