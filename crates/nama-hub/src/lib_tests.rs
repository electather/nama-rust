use std::{future, net::SocketAddr};

use sqlx::postgres::PgPoolOptions;
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::{TcpListener, TcpStream},
};

use super::serve;
use crate::signing::Keys;

/// A signing key, made with `openssl genpkey -algorithm ed25519`.
const SIGNING_KEY: &str = "-----BEGIN PRIVATE KEY-----
MC4CAQAwBQYDK2VwBCIEIJzJDsQn3SpQXPoFGl74lUI4L+huggYWuNcSt9vaCwTS
-----END PRIVATE KEY-----
";

#[tokio::test]
async fn the_key_set_is_passed_on_past_the_web_app_to_the_api() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    // Never connects: neither path asks the database.
    let db = PgPoolOptions::new()
        .connect_lazy("postgres://localhost:1/nowhere")
        .unwrap();
    let keys = Keys::from_pems(SIGNING_KEY).unwrap();
    let hub = serve(
        listener,
        db,
        keys,
        &[],
        None,
        None,
        None,
        None,
        future::pending(),
    );
    tokio::spawn(hub);

    let key_set = http1_get(addr, "/.well-known/jwks.json").await;
    let page = http1_get(addr, "/sign-in").await;

    assert!(
        key_set.contains("content-type: application/json"),
        "{key_set}"
    );
    assert!(page.contains("content-type: text/html"), "{page}");
}

/// The whole response, head and body, to a plain HTTP/1.1 `GET` for `path`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn http1_get(addr: SocketAddr, path: &str) -> String {
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let request = format!("GET {path} HTTP/1.1\r\nhost: hub\r\nconnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    response
}
