//! Calls the Server's API over a real socket, the way clients will.

#[path = "support/refused.rs"]
mod refused;
#[path = "support/server.rs"]
#[expect(dead_code, reason = "the harness's `data_dir` has no use here")]
mod server;

use std::net::SocketAddr;

use connectrpc::{ErrorCode, client::HttpClient};
use nama_proto::nama::v1::{
    GetServerInfoRequest, GetServerInfoResponse, GetServerSettingsRequest, RefusalReason,
    ServerServiceClient, ServerSettings, UpdateServerSettingsRequest,
};
use nama_server::{config::Config, plugins::Registry};
use refused::assert_refused;
use server::TestServer;

fn config_named(name: &str) -> Config {
    Config {
        name: name.to_owned(),
        ..Config::default()
    }
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn get_server_info(client: &ServerServiceClient<HttpClient>) -> GetServerInfoResponse {
    client
        .get_server_info(GetServerInfoRequest::default())
        .await
        .unwrap()
        .into_owned()
}

/// Every transport the Server promises: HTTP/1.1 and cleartext HTTP/2, each with JSON and binary
/// protobuf bodies.
#[tokio::test]
async fn get_server_info_returns_the_server_name_and_version_over_every_transport() {
    let server = TestServer::start(config_named("Living Room"), Registry::new()).await;
    let transports = [
        (
            "HTTP/1.1 JSON",
            HttpClient::plaintext(),
            server.client_config().json(),
        ),
        (
            "HTTP/1.1 binary",
            HttpClient::plaintext(),
            server.client_config().proto(),
        ),
        (
            "HTTP/2 JSON",
            HttpClient::plaintext_http2_only(),
            server.client_config().json(),
        ),
        (
            "HTTP/2 binary",
            HttpClient::plaintext_http2_only(),
            server.client_config().proto(),
        ),
    ];

    for (transport, http, config) in transports {
        let info = get_server_info(&ServerServiceClient::new(http, config)).await;

        assert_eq!(info.name, "Living Room", "{transport}");
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"), "{transport}");
    }
}

/// What `curl -H 'Content-Type: application/json' -d '{}' .../GetServerInfo` sends.
#[tokio::test]
async fn a_plain_http1_json_post_gets_a_json_answer() {
    let server = TestServer::start(config_named("Attic"), Registry::new()).await;

    let (status, content_type, body) =
        http1_post_json(server.addr, "/nama.v1.ServerService/GetServerInfo", "{}").await;

    assert_eq!(status, 200);
    assert_eq!(content_type, "application/json");
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        body,
        serde_json::json!({
            "name": "Attic",
            "version": env!("CARGO_PKG_VERSION"),
        })
    );
}

/// Sends a bare HTTP/1.1 request with no Connect client involved, so only the wire format is
/// shared with the Server. Returns the status, content type and body.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn http1_post_json(addr: SocketAddr, path: &str, body: &str) -> (u16, String, Vec<u8>) {
    use http_body_util::{BodyExt, Full};
    use hyper::{Request, body::Bytes, client::conn::http1};
    use hyper_util::rt::TokioIo;

    let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let (mut sender, connection) = http1::handshake(TokioIo::new(stream)).await.unwrap();
    tokio::spawn(connection);

    let request = Request::post(path)
        .header("host", addr.to_string())
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(body.to_owned())))
        .unwrap();
    let response = sender.send_request(request).await.unwrap();

    let status = response.status().as_u16();
    let content_type = response.headers()["content-type"]
        .to_str()
        .unwrap()
        .to_owned();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, content_type, bytes.to_vec())
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn server_url(client: &ServerServiceClient<HttpClient>) -> String {
    client
        .get_server_settings(GetServerSettingsRequest::default())
        .await
        .unwrap()
        .into_owned()
        .settings
        .url
        .clone()
}

fn set_server_url(url: &str) -> UpdateServerSettingsRequest {
    UpdateServerSettingsRequest {
        settings: ServerSettings {
            url: url.to_owned(),
            ..Default::default()
        }
        .into(),
        ..Default::default()
    }
}

#[tokio::test]
async fn the_server_url_is_empty_until_an_admin_sets_it() {
    let server = TestServer::start(Config::default(), Registry::new()).await;
    let client = ServerServiceClient::new(HttpClient::plaintext(), server.client_config());
    let unset = server_url(&client).await;

    client
        .update_server_settings(set_server_url(" https://nama.example.com "))
        .await
        .unwrap();

    assert_eq!(unset, "");
    assert_eq!(server_url(&client).await, "https://nama.example.com");
}

#[tokio::test]
async fn a_server_url_that_isnt_an_http_url_is_refused_naming_it_and_nothing_changes() {
    let server = TestServer::start(Config::default(), Registry::new()).await;
    let client = ServerServiceClient::new(HttpClient::plaintext(), server.client_config());
    client
        .update_server_settings(set_server_url("https://nama.example.com"))
        .await
        .unwrap();

    for not_a_url in [
        "nama.example.com",
        "ftp://nama.example.com",
        "https://nama.example.com/?a=b",
    ] {
        let updated = client
            .update_server_settings(set_server_url(not_a_url))
            .await;

        let refusal = assert_refused(
            updated,
            ErrorCode::InvalidArgument,
            RefusalReason::REFUSAL_REASON_NOT_A_URL,
        );
        assert_eq!(refusal.field, "settings.url", "{not_a_url}");
    }
    assert_eq!(server_url(&client).await, "https://nama.example.com");
}
