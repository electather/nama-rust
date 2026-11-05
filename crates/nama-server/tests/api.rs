//! Calls the Server's API over a real socket, the way clients will.

use std::net::SocketAddr;

use connectrpc::client::{ClientConfig, HttpClient};
use nama_proto::nama::v1::{GetServerInfoRequest, ServerServiceClient};
use nama_server::config::Config;
use tokio::{net::TcpListener, sync::oneshot};

/// A Server serving on a random local port until dropped.
struct TestServer {
    addr: SocketAddr,
    _stop: oneshot::Sender<()>,
}

impl TestServer {
    /// Starts serving `config` on a free port.
    async fn start(config: Config) -> TestServer {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (stop, stopped) = oneshot::channel::<()>();
        tokio::spawn(nama_server::serve(listener, &config, async {
            let _ = stopped.await;
        }));
        TestServer { addr, _stop: stop }
    }

    /// Client settings that point at this Server.
    fn client_config(&self) -> ClientConfig {
        ClientConfig::new(format!("http://{}", self.addr).parse().unwrap())
    }
}

/// The default config with the Server named `name`.
fn named(name: &str) -> Config {
    Config {
        name: name.to_owned(),
        ..Config::default()
    }
}

#[tokio::test]
async fn get_server_info_returns_the_server_name_and_version_over_http1_json() {
    let server = TestServer::start(named("Living Room")).await;
    let client = ServerServiceClient::new(HttpClient::plaintext(), server.client_config().json());

    let info = client
        .get_server_info(GetServerInfoRequest::default())
        .await
        .unwrap()
        .into_owned();

    assert_eq!(info.name, "Living Room");
    assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
}

#[tokio::test]
async fn get_server_info_works_over_http2_with_binary_protobuf() {
    let server = TestServer::start(named("Basement")).await;
    let client = ServerServiceClient::new(
        HttpClient::plaintext_http2_only(),
        server.client_config().proto(),
    );

    let info = client
        .get_server_info(GetServerInfoRequest::default())
        .await
        .unwrap()
        .into_owned();

    assert_eq!(info.name, "Basement");
    assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
}

/// What `curl -H 'Content-Type: application/json' -d '{}' .../GetServerInfo` sends.
#[tokio::test]
async fn a_plain_http1_json_post_gets_a_json_answer() {
    let server = TestServer::start(named("Attic")).await;

    let (status, content_type, body) =
        http1_post_json(server.addr, "/nama.v1.ServerService/GetServerInfo", "{}").await;

    assert_eq!(status, 200);
    assert_eq!(content_type, "application/json");
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        body,
        serde_json::json!({ "name": "Attic", "version": env!("CARGO_PKG_VERSION") })
    );
}

/// Sends a bare HTTP/1.1 request with no Connect client involved, so only the wire format is
/// shared with the Server. Returns the status, content type and body.
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
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, content_type, body.to_vec())
}
