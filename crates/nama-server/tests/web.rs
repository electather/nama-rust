//! The web app the Server serves, with the `web` feature on, around its own routes; `nama-web`
//! tests how it serves the app. Needs `vp build` in `web/` first.
#![cfg(feature = "web")]

#[path = "support/server.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod server;

use std::net::SocketAddr;

use hyper::HeaderMap;
use server::TestServer;

/// A deep link reloaded in the browser gets the app, which then routes to the page itself.
#[tokio::test]
async fn a_deep_link_serves_the_app() {
    let server = TestServer::builder().start().await;

    let (status, headers, body) = http1_get(server.addr, "/libraries/3").await;

    assert_eq!(status, 200);
    assert_eq!(headers["content-type"], "text/html; charset=utf-8");
    assert!(
        String::from_utf8(body)
            .unwrap()
            .starts_with("<!doctype html>")
    );
}

/// The app never answers for the API or media, so a signed-in client that gets a path wrong sees
/// a 404. Without a session, media would see a 401 first; an RPC path no `ACCESS` entry lists is
/// a 404 either way.
#[tokio::test]
async fn unknown_rpc_and_media_paths_are_not_found() {
    let server = TestServer::builder().start().await;

    for path in ["/nama.v1.NoSuchService/Nothing", "/media/42"] {
        let (status, _, _) = http1_get_as(server.addr, path, Some(&server.admin_token)).await;

        assert_eq!(status, 404, "{path}");
    }
}

/// Sends a plain HTTP/1.1 `GET` for `path` with no session and returns the status, headers and
/// body.
async fn http1_get(addr: SocketAddr, path: &str) -> (u16, HeaderMap, Vec<u8>) {
    http1_get_as(addr, path, None).await
}

/// Like [`http1_get`], presenting `token`, if any, as a bearer token.
async fn http1_get_as(
    addr: SocketAddr,
    path: &str,
    token: Option<&str>,
) -> (u16, HeaderMap, Vec<u8>) {
    let authorization = token.map(|token| format!("Bearer {token}"));
    let header = authorization
        .as_deref()
        .map(|value| ("authorization", value));
    http1_get_with(addr, path, header.as_slice()).await
}

/// Sends a plain HTTP/1.1 `GET` for `path` with `request_headers` and returns the status, the
/// response's headers and its body as sent.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn http1_get_with(
    addr: SocketAddr,
    path: &str,
    request_headers: &[(&str, &str)],
) -> (u16, HeaderMap, Vec<u8>) {
    use http_body_util::{BodyExt, Empty};
    use hyper::{Request, body::Bytes, client::conn::http1};
    use hyper_util::rt::TokioIo;

    let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let (mut sender, connection) = http1::handshake(TokioIo::new(stream)).await.unwrap();
    tokio::spawn(connection);

    let mut request = Request::get(path).header("host", addr.to_string());
    for (name, value) in request_headers {
        request = request.header(*name, *value);
    }
    let request = request.body(Empty::<Bytes>::new()).unwrap();
    let response = sender.send_request(request).await.unwrap();

    let status = response.status().as_u16();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, headers, bytes.to_vec())
}
