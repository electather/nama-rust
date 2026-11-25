//! Subtitle sources over the API (ADR 0067): the test Plugin built as a subtitle source.

#[path = "support/browsing.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod browsing;
#[path = "support/installed_test_plugin.rs"]
mod installed_test_plugin;
#[path = "support/server.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod server;
#[path = "support/test_plugin.rs"]
mod test_plugin;

use browsing::Fixture;
use buffa::EnumValue;
use connectrpc::client::HttpClient;
use nama_proto::nama::v1::{
    DisablePluginRequest, ExtensionPoint, ListPluginsRequest, PlanPlaybackRequest, PlaybackMethod,
    PlaybackPlan, PlaybackServiceClient, PluginServiceClient,
};
use server::{TestServer, tmdb::ALIEN};

#[tokio::test]
async fn a_plugin_serving_subtitles_is_listed_under_the_subtitle_source_point() {
    let server = TestServer::builder().start().await;
    server.install_test_plugin("subtitle-source").await;

    let listed = PluginServiceClient::new(HttpClient::plaintext(), server.client_config())
        .list_plugins(ListPluginsRequest::default())
        .await
        .unwrap()
        .into_owned();

    let plugin = (listed.summaries.iter())
        .find(|plugin| plugin.id == "test-plugin")
        .unwrap();
    assert_eq!(plugin.refused_reason, "");
    assert!(
        plugin
            .extension_points
            .contains(&ExtensionPoint::EXTENSION_POINT_SUBTITLE_SOURCE.into())
    );
}

#[tokio::test]
async fn a_playback_plan_says_whether_a_subtitle_source_is_enabled() {
    let fx = Fixture::alien().await;
    let version = fx.version_ids(ALIEN).await[0];

    let none_enabled = plan(&fx, version, &[]).await;
    fx.server.enable_test_plugin("subtitle-source").await;
    let enabled = plan(&fx, version, &[]).await;
    disable_test_plugin(&fx.server).await;
    let disabled = plan(&fx, version, &[]).await;

    assert!(!none_enabled.can_search_subtitles);
    assert!(enabled.can_search_subtitles);
    assert!(!disabled.can_search_subtitles);
}

#[tokio::test]
async fn an_unplayable_plan_also_says_whether_a_subtitle_source_is_enabled() {
    let fx = Fixture::alien().await;
    let version = fx.version_ids(ALIEN).await[0];
    let every_method = [
        PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY,
        PlaybackMethod::PLAYBACK_METHOD_REMUX,
        PlaybackMethod::PLAYBACK_METHOD_TRANSCODE,
    ];

    let none_enabled = plan(&fx, version, &every_method).await;
    fx.server.enable_test_plugin("subtitle-source").await;
    let enabled = plan(&fx, version, &every_method).await;

    assert_eq!(
        none_enabled.method,
        PlaybackMethod::PLAYBACK_METHOD_UNSPECIFIED
    );
    assert_eq!(enabled.method, PlaybackMethod::PLAYBACK_METHOD_UNSPECIFIED);
    assert!(!none_enabled.can_search_subtitles);
    assert!(enabled.can_search_subtitles);
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn disable_test_plugin(server: &TestServer) {
    (server.plugin_service())
        .disable_plugin(DisablePluginRequest {
            id: "test-plugin".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();
}

/// The Admin's Playback Plan for `version`, on a Device where the methods `failed` just failed.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn plan(fx: &Fixture, version: i64, failed: &[PlaybackMethod]) -> PlaybackPlan {
    PlaybackServiceClient::new(HttpClient::plaintext(), fx.server.client_config())
        .plan_playback(PlanPlaybackRequest {
            version_id: version,
            failed_methods: failed.iter().copied().map(EnumValue::from).collect(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .plan
        .into_option()
        .unwrap()
}
