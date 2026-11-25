//! Installing the test Plugin on a `TestServer` as an Admin would: dropped into `plugins/`, then
//! listed and enabled through the API.

use nama_proto::nama::v1::{EnablePluginRequest, ListPluginsRequest};

use super::{server::TestServer, test_plugin};

impl TestServer {
    /// Installs the test Plugin built with `features`, not yet enabled.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    pub(crate) async fn install_test_plugin(&self, features: &str) {
        let dir = self.data_dir().join("plugins");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("test.wasm"), test_plugin::build(features)).unwrap();
        // Listing is what reads `plugins/`.
        (self.plugin_service())
            .list_plugins(ListPluginsRequest::default())
            .await
            .unwrap();
    }

    /// Installs the test Plugin built with `features` and enables it, granting it nothing.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    pub(crate) async fn enable_test_plugin(&self, features: &str) {
        self.install_test_plugin(features).await;
        (self.plugin_service())
            .enable_plugin(EnablePluginRequest {
                id: "test-plugin".to_owned(),
                ..Default::default()
            })
            .await
            .unwrap();
    }
}
