use axum::{Router, routing::get};
use ring::{
    rand::SystemRandom,
    signature::{Ed25519KeyPair, KeyPair as _},
};
use rustls::{ClientConfig, ClientConnection, RootCertStore, pki_types::ServerName};
use serde_json::{Value, json};

use super::*;

const OWN: &str = "abc.namadirect.test";

/// How long a test waits for the Relay before it fails.
const WAIT: Duration = Duration::from_secs(10);

fn key() -> Ed25519KeyPair {
    let pkcs8 = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).unwrap();
    Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap()
}

/// Serves `key`'s public half as the Hub's key set, and answers the Hub's address.
async fn hub_publishing(key: &Ed25519KeyPair) -> String {
    let key_set = json!({ "keys": [{
        "kty": "OKP",
        "crv": "Ed25519",
        "kid": "hub",
        "x": URL_SAFE_NO_PAD.encode(key.public_key()),
    }] })
    .to_string();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let app = Router::new().route("/.well-known/jwks.json", get(async move || key_set));
    tokio::spawn(async move { axum::serve(listener, app).await });
    url
}

/// `claims` as a JWT `key` signed, as the Hub signs them.
fn signed(key: &Ed25519KeyPair, claims: &Value) -> String {
    let header = json!({ "alg": "EdDSA", "typ": "JWT", "kid": "hub" });
    let signed = format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(header.to_string()),
        URL_SAFE_NO_PAD.encode(claims.to_string())
    );
    format!(
        "{signed}.{}",
        URL_SAFE_NO_PAD.encode(key.sign(signed.as_bytes()))
    )
}

fn relay_claims(own: &str) -> Value {
    json!({ "sub": own, "aud": "relay", "exp": db::now() + 300 })
}

/// A Relay checking tokens against the Hub at `hub_url`, and where it listens.
async fn relay(hub_url: String) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { serve(listener, &hub_url).await });
    addr
}

/// A Server's connection to `relay`, dialed with `token`.
async fn dialed(relay: SocketAddr, token: &str) -> TcpStream {
    let mut server = TcpStream::connect(relay).await.unwrap();
    server.write_all(&dial(token).unwrap()).await.unwrap();
    server
}

/// The `ClientHello` a TLS client sends first for `server_name`.
fn client_hello(server_name: &str) -> Vec<u8> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = (ClientConfig::builder_with_provider(provider))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(RootCertStore::empty())
        .with_no_client_auth();
    let server_name = ServerName::try_from(server_name.to_owned()).unwrap();
    let mut connection = ClientConnection::new(Arc::new(config), server_name).unwrap();
    let mut hello = Vec::new();
    connection.write_tls(&mut hello).unwrap();
    hello
}

async fn read(stream: &mut TcpStream, length: usize) -> Vec<u8> {
    let mut read = vec![0; length];
    timeout(WAIT, stream.read_exact(&mut read))
        .await
        .unwrap()
        .unwrap();
    read
}

#[tokio::test]
async fn a_client_reaches_the_server_waiting_under_its_sni_with_its_bytes_untouched() {
    let key = key();
    let relay = relay(hub_publishing(&key).await).await;
    let mut server = dialed(relay, &signed(&key, &relay_claims(OWN))).await;
    let hello = client_hello(&format!("relay.{OWN}"));

    let mut client = TcpStream::connect(relay).await.unwrap();
    client.write_all(&hello).await.unwrap();

    assert_eq!(read(&mut server, hello.len()).await, hello);
    server.write_all(b"answer").await.unwrap();
    assert_eq!(read(&mut client, 6).await, b"answer");
}

#[tokio::test]
async fn a_dial_whose_token_doesnt_check_out_is_closed() {
    let (key, other) = (key(), key());
    let relay = relay(hub_publishing(&key).await).await;
    let expired = json!({ "sub": OWN, "aud": "relay", "exp": db::now() - 3600 });
    let for_a_server = json!({ "sub": OWN, "aud": "server", "exp": db::now() + 300 });

    for token in [
        signed(&other, &relay_claims(OWN)),
        signed(&key, &expired),
        signed(&key, &for_a_server),
        "not.a.token".to_owned(),
    ] {
        let mut server = dialed(relay, &token).await;
        let closed = timeout(WAIT, server.read(&mut [0; 1])).await.unwrap();
        assert_eq!(closed.unwrap(), 0, "{token}");
    }
}

#[test]
fn a_name_routes_to_itself_and_each_name_it_is_under() {
    assert_eq!(
        names_under("relay.abc.zone").collect::<Vec<_>>(),
        ["relay.abc.zone", "abc.zone", "zone"]
    );
    assert!(!names_under("xabc.zone").any(|name| name == "abc.zone"));
}
