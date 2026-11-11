//! The web app the Server serves, with the `web` feature on. Needs `vp build` in `web/` first.
#![cfg(feature = "web")]

#[path = "support/server.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod server;

use std::{net::SocketAddr, path::Path};

use server::TestServer;

/// A deep link reloaded in the browser gets the app, which then routes to the page itself.
#[tokio::test]
async fn a_deep_link_serves_the_app() {
    let server = TestServer::builder().start().await;

    let (status, content_type, body) = http1_get(server.addr, "/libraries/3").await;

    assert_eq!(status, 200);
    assert_eq!(content_type, "text/html; charset=utf-8");
    assert!(
        String::from_utf8(body)
            .unwrap()
            .starts_with("<!DOCTYPE html>")
    );
}

/// The stylesheet the app links to is served as CSS, not as the app's HTML.
#[tokio::test]
async fn the_app_s_stylesheet_is_served() {
    let server = TestServer::builder().start().await;
    let (_, _, shell) = http1_get(server.addr, "/").await;
    let shell = String::from_utf8(shell).unwrap();
    let stylesheet = shell
        .split('"')
        .find(|part| {
            part.starts_with("/assets/") && Path::new(part).extension().is_some_and(|e| e == "css")
        })
        .unwrap();

    let (status, content_type, _) = http1_get(server.addr, stylesheet).await;

    assert_eq!(status, 200);
    assert_eq!(content_type, "text/css; charset=utf-8");
}

/// A WebAssembly module the app loads, such as the PGS renderer's, is served as one, which
/// `WebAssembly.instantiateStreaming` needs.
#[tokio::test]
async fn the_app_s_webassembly_is_served_as_such() {
    let server = TestServer::builder().start().await;
    // Only a chunk loaded on demand names it, not the shell, so it's found where `vp build` wrote it.
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/dist/client/assets");
    let wasm = std::fs::read_dir(assets)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .find(|name| name.ends_with(".wasm"))
        .unwrap();

    let (status, content_type, _) = http1_get(server.addr, &format!("/assets/{wasm}")).await;

    assert_eq!(status, 200);
    assert_eq!(content_type, "application/wasm");
}

/// The ASS renderer ships with the notices of what it bundles, and without its asm.js worker for
/// browsers without WebAssembly, which the app never lists ASS on.
#[tokio::test]
async fn the_app_ships_the_ass_renderer_s_notices_and_not_its_asm_js_worker() {
    let server = TestServer::builder().start().await;
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/dist/client/assets");
    let legacy = std::fs::read_dir(assets)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .find(|name| name.contains("worker-legacy"));

    let (status, _, notices) = http1_get(server.addr, "/licenses/libass-wasm.txt").await;

    assert_eq!(legacy, None);
    assert_eq!(status, 200);
    assert!(String::from_utf8(notices).unwrap().contains("FreeType"));
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

/// Sends a plain HTTP/1.1 `GET` for `path` with no session and returns the status,
/// `Content-Type` (empty if none) and body.
async fn http1_get(addr: SocketAddr, path: &str) -> (u16, String, Vec<u8>) {
    http1_get_as(addr, path, None).await
}

/// Like [`http1_get`], presenting `token`, if any, as a bearer token.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn http1_get_as(addr: SocketAddr, path: &str, token: Option<&str>) -> (u16, String, Vec<u8>) {
    use http_body_util::{BodyExt, Empty};
    use hyper::{Request, body::Bytes, client::conn::http1};
    use hyper_util::rt::TokioIo;

    let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let (mut sender, connection) = http1::handshake(TokioIo::new(stream)).await.unwrap();
    tokio::spawn(connection);

    let mut request = Request::get(path).header("host", addr.to_string());
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    let request = request.body(Empty::<Bytes>::new()).unwrap();
    let response = sender.send_request(request).await.unwrap();

    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get("content-type")
        .map(|value| value.to_str().unwrap().to_owned())
        .unwrap_or_default();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, content_type, bytes.to_vec())
}
