//! Subtitle sources over the API (ADR 0067): the test Plugin built as a subtitle source.

#[path = "support/server.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod server;
#[path = "support/test_plugin.rs"]
mod test_plugin;

use connectrpc::client::HttpClient;
use nama_proto::nama::v1::{ExtensionPoint, ListPluginsRequest, PluginServiceClient};
use server::TestServer;

#[tokio::test]
async fn a_plugin_serving_subtitles_is_listed_under_the_subtitle_source_point() {
    let server = TestServer::builder().start().await;
    let dir = server.data_dir().join("plugins");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("test.wasm"), test_plugin::build("subtitle-source")).unwrap();

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
