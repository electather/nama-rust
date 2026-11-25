//! A Plugin's setup text, links and licence: what its Manifest gives, less what the Server drops,
//! reaches `GetPlugin`, over a real socket.

#[path = "support/server.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod server;

use std::sync::Arc;

use connectrpc::client::HttpClient;
use nama_plugin_api::{Manifest, Plugin, PluginError, PluginId, Registrar};
use nama_proto::nama::v1::{GetPluginRequest, PluginServiceClient};
use nama_server::plugins::Registry;
use server::TestServer;

/// A Built-in Plugin that registers nothing and gives setup text, a homepage and a licence, and a
/// source URL the Server drops.
struct Presented;

impl Plugin for Presented {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            setup_markdown: "Get an API key at <https://example.com/keys>.".to_owned(),
            homepage_url: "https://example.com/docs".to_owned(),
            source_url: "javascript:alert(1)".to_owned(),
            license: "MIT OR Apache-2.0".to_owned(),
            ..Manifest::new(PluginId::from_static("presented"))
        }
    }

    fn register(&self, _registrar: &mut Registrar) -> Result<(), PluginError> {
        Ok(())
    }
}

#[tokio::test]
async fn get_plugin_sends_the_setup_text_links_and_licence_the_server_kept() {
    let registry = Registry::new();
    registry.describe_builtin(|| Arc::new(Presented));
    let server = TestServer::builder().plugins(registry).start().await;

    let plugin = PluginServiceClient::new(HttpClient::plaintext(), server.client_config())
        .get_plugin(GetPluginRequest {
            id: "presented".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .plugin
        .unwrap();

    assert_eq!(
        [
            plugin.setup_markdown.as_str(),
            &plugin.homepage_url,
            &plugin.source_url,
            &plugin.license,
        ],
        [
            "Get an API key at <https://example.com/keys>.",
            "https://example.com/docs",
            "",
            "MIT OR Apache-2.0",
        ]
    );
}
