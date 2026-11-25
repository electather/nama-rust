//! HTTPS on the Server's names on the Hub (ADR 0057): the certificate the Server keeps for them,
//! served on the port it listens on beside plain HTTP, which the LAN keeps using, and through the
//! Relay it dials out to, and its public name as its Server URL while the Admin sets none.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/fake_hub.rs"]
mod fake_hub;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::{io, net::SocketAddr, num::NonZeroU16, sync::Arc, time::Duration};

use connectrpc::client::HttpClient;
use fake_hub::{FakeHub, OPEN_PORT, OWN};
use nama_proto::nama::v1::{CreateInviteRequest, UserServiceClient};
use nama_server::config::Config;
use rustls::{
    ClientConfig, RootCertStore,
    pki_types::{CertificateDer, ServerName},
};
use server::{TestServer, WAIT};
use tokio::{
    io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _},
    net::{TcpListener, TcpStream},
};
use tokio_rustls::TlsConnector;

/// A certificate for [`OWN`] and the wildcard under it, as an ACME CA gives one: its PEM, key
/// first, as the Server keeps it, and the certificate for a client to trust.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn certificate() -> (String, CertificateDer<'static>) {
    let names = [OWN.to_owned(), format!("*.{OWN}")];
    let key = rcgen::KeyPair::generate().unwrap();
    let certificate = (rcgen::CertificateParams::new(names).unwrap())
        .self_signed(&key)
        .unwrap();
    let pem = format!("{}{}", key.serialize_pem(), certificate.pem());
    (pem, certificate.der().clone())
}

/// `server` started again with `config` and `pem` kept as the certificate for its names.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn keeping(server: TestServer, pem: &str, config: Config) -> TestServer {
    std::fs::write(server.data_dir().join("certificate.pem"), pem).unwrap();
    TestServer::builder().config(config).restart(server).await
}

/// A Connect call of `method` with an empty JSON body, presenting `cookie`, if any.
fn call(method: &str, cookie: Option<&str>) -> String {
    let cookie = cookie.map_or_else(String::new, |cookie| format!("Cookie: {cookie}\r\n"));
    format!(
        "POST /nama.v1.{method} HTTP/1.1\r\nHost: {OWN}\r\nContent-Type: application/json\r\n\
         Content-Length: 2\r\nConnection: close\r\n{cookie}\r\n{{}}"
    )
}

/// The status `request` gets on `connection`, such as `200`.
async fn status(
    mut connection: impl AsyncRead + AsyncWrite + Unpin,
    request: &str,
) -> io::Result<String> {
    connection.write_all(request.as_bytes()).await?;
    let mut status_line = [0; 12];
    connection.read_exact(&mut status_line).await?;
    Ok(String::from_utf8_lossy(&status_line[9..]).into_owned())
}

/// A TLS connection to `addr` as `name`, trusting only `trusted`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn over_tls(
    addr: SocketAddr,
    name: &str,
    trusted: CertificateDer<'static>,
) -> io::Result<impl AsyncRead + AsyncWrite + Unpin> {
    let mut roots = RootCertStore::empty();
    roots.add(trusted).unwrap();
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = (ClientConfig::builder_with_provider(provider))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let tcp = TcpStream::connect(addr).await?;
    let name = ServerName::try_from(name.to_owned()).unwrap();
    TlsConnector::from(Arc::new(config))
        .connect(name, tcp)
        .await
}

#[tokio::test]
async fn the_server_serves_https_on_its_names_and_plain_http_on_the_same_port() {
    let (pem, trusted) = certificate();
    let server = keeping(TestServer::builder().start().await, &pem, Config::default()).await;
    let info = call("ServerService/GetServerInfo", None);

    for name in [OWN, &format!("192-168-1-10.{OWN}")] {
        let tls = over_tls(server.addr, name, trusted.clone()).await.unwrap();
        assert_eq!(status(tls, &info).await.unwrap(), "200", "{name}");
    }
    let plain = TcpStream::connect(server.addr).await.unwrap();
    assert_eq!(status(plain, &info).await.unwrap(), "200");
}

#[tokio::test]
async fn a_request_over_the_servers_own_tls_counts_as_https() {
    let (pem, trusted) = certificate();
    let server = keeping(TestServer::builder().start().await, &pem, Config::default()).await;
    let cookie = format!("__Host-nama_session={}", server.admin_token);
    let who = call("SignInService/WhoAmI", Some(&cookie));

    let tls = over_tls(server.addr, OWN, trusted).await.unwrap();
    let plain = TcpStream::connect(server.addr).await.unwrap();

    // Only over HTTPS does the Server take the `__Host-` cookie.
    assert_eq!(status(tls, &who).await.unwrap(), "200");
    assert_ne!(status(plain, &who).await.unwrap(), "200");
}

#[tokio::test]
async fn without_a_certificate_the_server_serves_plain_http_alone() {
    let server = TestServer::builder().start().await;
    let (_, trusted) = certificate();

    let refused = over_tls(server.addr, OWN, trusted).await;
    let plain = TcpStream::connect(server.addr).await.unwrap();

    assert!(refused.is_err());
    let info = call("ServerService/GetServerInfo", None);
    assert_eq!(status(plain, &info).await.unwrap(), "200");
}

/// The link of an Invite `server` creates, once it has a Server URL to link it to.
async fn invite_link(server: &TestServer) -> String {
    let invites = UserServiceClient::new(HttpClient::plaintext(), server.client_config());
    loop {
        match invites.create_invite(CreateInviteRequest::default()).await {
            Ok(created) => return created.into_owned().url,
            Err(_no_url_yet) => tokio::time::sleep(Duration::from_millis(50)).await,
        }
    }
}

#[tokio::test]
async fn a_linked_server_reached_from_outside_links_invites_to_its_public_name() {
    let hub = FakeHub::start().await;
    let (pem, _) = certificate();
    let server = TestServer::builder().config(hub.config()).start().await;
    hub.link(&server).await;
    let reached_at_its_open_port = Config {
        forwarded_port: NonZeroU16::new(OPEN_PORT),
        ..hub.config()
    };

    let server = keeping(server, &pem, reached_at_its_open_port).await;

    let created = tokio::time::timeout(WAIT, invite_link(&server))
        .await
        .expect("an Invite was never linked");

    assert!(
        created.starts_with("https://abc.namadirect.test:7980/invite/"),
        "{created}"
    );
}

#[tokio::test]
async fn a_client_reaches_the_server_over_its_own_tls_through_the_relay_it_dials() {
    let hub = FakeHub::start().await;
    let relay = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let relay_addr = relay.local_addr().unwrap();
    let hub_url = hub.url();
    tokio::spawn(async move { nama_server::relay::serve(relay, &hub_url).await });
    let (pem, trusted) = certificate();
    let server = TestServer::builder().config(hub.config()).start().await;
    hub.link(&server).await;
    let dialing = Config {
        relay: Some(relay_addr),
        ..hub.config()
    };

    let server = keeping(server, &pem, dialing).await;

    let relay_name = format!("relay.{OWN}");
    let tls = over_tls(relay_addr, &relay_name, trusted).await.unwrap();
    let info = call("ServerService/GetServerInfo", None);
    assert_eq!(status(tls, &info).await.unwrap(), "200");
    let told_relay = async {
        while hub.last_told().map(|told| told.relay) != Some(relay_addr.to_string()) {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    };
    tokio::time::timeout(WAIT, told_relay)
        .await
        .expect("the Hub was never told the Relay");
    drop(server);
}
