//! The web app the Server serves, with the `web` feature on. Needs `vp build` in `web/` first.
#![cfg(feature = "web")]

#[path = "support/server.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod server;

use std::{net::SocketAddr, path::Path};

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

/// The stylesheet the app links to is served as CSS, not as the app's HTML.
#[tokio::test]
async fn the_app_s_stylesheet_is_served() {
    let server = TestServer::builder().start().await;
    let stylesheet = linked_asset(server.addr, "css").await;

    let (status, headers, _) = http1_get(server.addr, &stylesheet).await;

    assert_eq!(status, 200);
    assert_eq!(headers["content-type"], "text/css; charset=utf-8");
}

/// A file under `/assets/` is named by its content's hash, so the browser keeps it for a year
/// without asking again, while the app's `index.html` isn't kept, so a new release still loads.
#[tokio::test]
async fn hashed_assets_are_cached_for_good_and_the_shell_is_not() {
    let server = TestServer::builder().start().await;
    let stylesheet = linked_asset(server.addr, "css").await;

    let (_, asset_headers, _) = http1_get(server.addr, &stylesheet).await;
    let (_, shell_headers, _) = http1_get(server.addr, "/").await;

    assert_eq!(
        asset_headers["cache-control"],
        "public, max-age=31536000, immutable"
    );
    assert!(!shell_headers.contains_key("cache-control"));
}

/// A browser that accepts brotli or gzip gets the app's text compressed with it, marked so a cache
/// keeps each encoding apart.
#[tokio::test]
async fn the_app_s_text_is_compressed_for_a_browser_that_accepts_it() {
    let server = TestServer::builder().start().await;
    let script = linked_asset(server.addr, "js").await;
    let stylesheet = linked_asset(server.addr, "css").await;

    for encoding in ["br", "gzip"] {
        for path in ["/", "/libraries/3", &script, &stylesheet, "/icon.svg"] {
            let (status, headers, _) =
                http1_get_with(server.addr, path, &[("accept-encoding", encoding)]).await;

            assert_eq!(status, 200, "{path}");
            assert_eq!(headers["content-encoding"], encoding, "{path}");
            assert_eq!(headers["vary"], "accept-encoding", "{path}");
        }
    }
}

/// A client that accepts no encoding gets the files exactly as `vp build` wrote them.
#[tokio::test]
async fn the_app_is_sent_as_built_to_a_client_that_accepts_no_encoding() {
    let server = TestServer::builder().start().await;
    let script = linked_asset(server.addr, "js").await;
    let stylesheet = linked_asset(server.addr, "css").await;
    let dist = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/dist");

    for (path, file) in [
        ("/", "index.html"),
        (&script, script.trim_start_matches('/')),
        (&stylesheet, stylesheet.trim_start_matches('/')),
        ("/icon.svg", "icon.svg"),
    ] {
        let (status, headers, body) = http1_get_with(server.addr, path, &[]).await;

        assert_eq!(status, 200, "{path}");
        assert!(!headers.contains_key("content-encoding"), "{path}");
        assert_eq!(body, std::fs::read(dist.join(file)).unwrap(), "{path}");
    }
}

/// A file stored compressed, such as a PNG or a woff2 font, isn't compressed again.
#[tokio::test]
async fn a_compressed_file_is_sent_as_it_is() {
    let server = TestServer::builder().start().await;
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/dist/assets");
    let font = std::fs::read_dir(assets)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .find(|name| Path::new(name).extension().is_some_and(|e| e == "woff2"))
        .unwrap();

    for path in ["/icon-192.png", &format!("/assets/{font}")] {
        let (status, headers, _) =
            http1_get_with(server.addr, path, &[("accept-encoding", "br, gzip")]).await;

        assert_eq!(status, 200, "{path}");
        assert!(!headers.contains_key("content-encoding"), "{path}");
    }
}

/// A WebAssembly module the app loads, such as the PGS renderer's, is served as one, which
/// `WebAssembly.instantiateStreaming` needs.
#[tokio::test]
async fn the_app_s_webassembly_is_served_as_such() {
    let server = TestServer::builder().start().await;
    // Only a chunk loaded on demand names it, not the shell, so it's found where `vp build` wrote it.
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/dist/assets");
    let wasm = std::fs::read_dir(assets)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .find(|name| name.ends_with(".wasm"))
        .unwrap();

    let (status, headers, _) = http1_get(server.addr, &format!("/assets/{wasm}")).await;

    assert_eq!(status, 200);
    assert_eq!(headers["content-type"], "application/wasm");
}

/// The manifest the app links to is served as one, so a browser can install the app.
#[tokio::test]
async fn the_app_s_manifest_is_served_as_such() {
    let server = TestServer::builder().start().await;

    let (status, headers, _) = http1_get(server.addr, "/manifest.webmanifest").await;

    assert_eq!(status, 200);
    assert_eq!(headers["content-type"], "application/manifest+json");
}

/// The ASS renderer ships with the notices of what it bundles and of its fallback font, as their
/// licenses ask.
#[tokio::test]
async fn the_app_ships_the_ass_renderer_s_notices() {
    let server = TestServer::builder().start().await;

    let (jassub_status, _, jassub_notices) = http1_get(server.addr, "/licenses/jassub.txt").await;
    let (font_status, _, font_notice) =
        http1_get(server.addr, "/licenses/liberation-sans.txt").await;
    let jassub_notices = String::from_utf8(jassub_notices).unwrap();
    let font_notice = String::from_utf8(font_notice).unwrap();

    // An unknown path answers `index.html` with 200, so the text is what shows the file shipped.
    assert_eq!(jassub_status, 200);
    assert!(jassub_notices.contains("FreeType"));
    assert_eq!(font_status, 200);
    assert!(font_notice.contains("Reserved Font Name Liberation"));
    assert!(font_notice.contains("SIL OPEN FONT LICENSE Version 1.1"));
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

/// The path of the file with `extension` under `/assets/` that the app's `index.html` links to.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn linked_asset(addr: SocketAddr, extension: &str) -> String {
    let (_, _, shell) = http1_get(addr, "/").await;
    String::from_utf8(shell)
        .unwrap()
        .split('"')
        .find(|part| {
            part.starts_with("/assets/")
                && Path::new(part).extension().is_some_and(|e| e == extension)
        })
        .unwrap()
        .to_owned()
}

/// Sends a plain HTTP/1.1 `GET` for `path` with no session and returns the status, headers and
/// body.
async fn http1_get(addr: SocketAddr, path: &str) -> (u16, HeaderMap, Vec<u8>) {
    http1_get_as(addr, path, None).await
}

/// Like [`http1_get`], presenting `token`, if any, as a bearer token.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
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
