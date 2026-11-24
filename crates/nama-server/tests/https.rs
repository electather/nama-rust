//! HTTPS on the Server's names on the Hub (ADR 0057): the certificate the Server keeps for them,
//! served on the port it listens on beside plain HTTP, which the LAN keeps using, and its public
//! name as its Server URL while the Admin sets none.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/scripted_linker.rs"]
mod scripted_linker;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::{io, sync::Arc, time::Duration};

use nama_plugin_api::server_link::{Link, Reachability};
use nama_proto::nama::v1::CreateInviteRequest;
use rustls::{
    ClientConfig, RootCertStore,
    pki_types::{CertificateDer, ServerName},
};
use scripted_linker::{Scripted, enabling, users, with_linker};
use server::{TestServer, WAIT};
use tokio::{
    io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _},
    net::TcpStream,
};
use tokio_rustls::TlsConnector;

/// The Server's own name on the Hub.
const OWN: &str = "abc.namadirect.test";

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

/// `server` started again with `pem` kept as the certificate for its names, and `linker` enabled.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn keeping(server: TestServer, pem: &str, linker: &Scripted) -> TestServer {
    std::fs::write(server.data_dir().join("certificate.pem"), pem).unwrap();
    (TestServer::builder().plugins(enabling(linker)))
        .restart(server)
        .await
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

/// A TLS connection to `server` as `name`, trusting only `trusted`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn over_tls(
    server: &TestServer,
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
    let tcp = TcpStream::connect(server.addr).await?;
    let name = ServerName::try_from(name.to_owned()).unwrap();
    TlsConnector::from(Arc::new(config))
        .connect(name, tcp)
        .await
}

#[tokio::test]
async fn the_server_serves_https_on_its_names_and_plain_http_on_the_same_port() {
    let linker = Scripted::default();
    let (pem, trusted) = certificate();
    let server = keeping(with_linker(&linker).await, &pem, &linker).await;
    let info = call("ServerService/GetServerInfo", None);

    for name in [OWN, &format!("192-168-1-10.{OWN}")] {
        let tls = over_tls(&server, name, trusted.clone()).await.unwrap();
        assert_eq!(status(tls, &info).await.unwrap(), "200", "{name}");
    }
    let plain = TcpStream::connect(server.addr).await.unwrap();
    assert_eq!(status(plain, &info).await.unwrap(), "200");
}

#[tokio::test]
async fn a_request_over_the_servers_own_tls_counts_as_https() {
    let linker = Scripted::default();
    let (pem, trusted) = certificate();
    let server = keeping(with_linker(&linker).await, &pem, &linker).await;
    let cookie = format!("__Host-nama_session={}", server.admin_token);
    let who = call("SignInService/WhoAmI", Some(&cookie));

    let tls = over_tls(&server, OWN, trusted).await.unwrap();
    let plain = TcpStream::connect(server.addr).await.unwrap();

    // Only over HTTPS does the Server take the `__Host-` cookie.
    assert_eq!(status(tls, &who).await.unwrap(), "200");
    assert_ne!(status(plain, &who).await.unwrap(), "200");
}

#[tokio::test]
async fn without_a_certificate_the_server_serves_plain_http_alone() {
    let server = TestServer::builder().start().await;
    let (_, trusted) = certificate();

    let refused = over_tls(&server, OWN, trusted).await;
    let plain = TcpStream::connect(server.addr).await.unwrap();

    assert!(refused.is_err());
    let info = call("ServerService/GetServerInfo", None);
    assert_eq!(status(plain, &info).await.unwrap(), "200");
}

/// The link of an Invite `server` creates, once it has a Server URL to link it to.
async fn invite_link(server: &TestServer) -> String {
    let invites = users(server.client_config());
    loop {
        match invites.create_invite(CreateInviteRequest::default()).await {
            Ok(created) => return created.into_owned().url,
            Err(_no_url_yet) => tokio::time::sleep(Duration::from_millis(50)).await,
        }
    }
}

#[tokio::test]
async fn a_linked_server_reached_from_outside_links_invites_to_its_public_name() {
    let linker = Scripted::default();
    let (pem, _) = certificate();
    let server = with_linker(&linker).await;
    linker.script().link = Some(Link::Linked {
        account: "ada@example.com".to_owned(),
        reachability: Some(Reachability {
            address: "http://203.0.113.7:7980".to_owned(),
            reachable: true,
            names: vec![OWN.to_owned(), format!("192-168-1-10.{OWN}")],
        }),
    });

    let server = keeping(server, &pem, &linker).await;

    let created = tokio::time::timeout(WAIT, invite_link(&server))
        .await
        .expect("an Invite was never linked");

    assert!(
        created.starts_with("https://abc.namadirect.test:7980/invite/"),
        "{created}"
    );
}
