use std::{
    fs,
    path::{Path, PathBuf},
};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
    middleware::from_fn_with_state,
    response::Response,
};
use tower::ServiceExt as _;

use super::{SHELL, asset, content_type, serve};

/// The answer to `request` of an app with no routes but `serve`, passing `/passed-on/` on.
async fn send(request: Request<Body>) -> Response {
    Router::new()
        .layer(from_fn_with_state("/passed-on/", serve))
        .oneshot(request)
        .await
        .unwrap()
}

/// The answer to a `GET` for `path` from a client accepting `encodings`.
async fn get(path: &str, encodings: &[&str]) -> Response {
    let mut request = Request::get(path);
    for encoding in encodings {
        request = request.header(header::ACCEPT_ENCODING, *encoding);
    }
    send(request.body(Body::empty()).unwrap()).await
}

/// The path of the file with `extension` under `/assets/` that the app's `index.html` links to.
fn linked_asset(extension: &str) -> &'static str {
    std::str::from_utf8(SHELL)
        .unwrap()
        .split('"')
        .find(|part| {
            part.starts_with("/assets/")
                && Path::new(part).extension().is_some_and(|e| e == extension)
        })
        .unwrap()
}

/// Where `vp build` writes the app.
fn dist() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/dist")
}

/// The path of a file with `extension` that `vp build` wrote under `/assets/`, linked or not.
fn built_asset(extension: &str) -> String {
    let name = fs::read_dir(dist().join("assets"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .find(|name| Path::new(name).extension().is_some_and(|e| e == extension))
        .unwrap();
    format!("/assets/{name}")
}

/// The app never answers for an RPC, the passed-on paths or a method but `GET`, so those reach
/// the routes behind it, here none.
#[tokio::test]
async fn rpcs_passed_on_paths_and_other_methods_go_on_to_next() {
    for request in [
        Request::get("/nama.v1.NoSuchService/Nothing"),
        Request::get("/passed-on/42"),
        Request::post("/sign-in"),
    ] {
        let response = send(request.body(Body::empty()).unwrap()).await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}

/// A deep link reloaded in the browser gets the app, which then routes to the page itself.
#[tokio::test]
async fn a_deep_link_serves_the_app() {
    let response = get("/libraries/3", &[]).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "text/html; charset=utf-8"
    );
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(body, SHELL);
}

/// A browser that accepts brotli or gzip gets the app's text compressed with it, marked so a cache
/// keeps each encoding apart.
#[tokio::test]
async fn the_app_s_text_is_compressed_for_a_browser_that_accepts_it() {
    let paths = [
        "/",
        "/sign-in",
        linked_asset("js"),
        linked_asset("css"),
        "/icon.svg",
    ];
    for encoding in ["br", "gzip"] {
        for path in paths {
            let response = get(path, &[encoding]).await;

            assert_eq!(
                response.headers()[header::CONTENT_ENCODING],
                encoding,
                "{path}"
            );
            assert_eq!(
                response.headers()[header::VARY],
                "accept-encoding",
                "{path}"
            );
        }
    }
}

/// A client that accepts no encoding gets the files exactly as `vp build` wrote them.
#[tokio::test]
async fn the_app_is_sent_as_built_to_a_client_that_accepts_no_encoding() {
    let script = linked_asset("js");
    let stylesheet = linked_asset("css");
    for (path, file) in [
        ("/", "index.html"),
        (script, script.trim_start_matches('/')),
        (stylesheet, stylesheet.trim_start_matches('/')),
        ("/icon.svg", "icon.svg"),
    ] {
        let response = get(path, &[]).await;

        assert!(
            !response.headers().contains_key(header::CONTENT_ENCODING),
            "{path}"
        );
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(body, fs::read(dist().join(file)).unwrap(), "{path}");
    }
}

/// A file stored compressed, such as a PNG or a woff2 font, isn't compressed again.
#[tokio::test]
async fn a_compressed_file_is_sent_as_it_is() {
    for path in ["/icon-192.png".to_owned(), built_asset("woff2")] {
        let response = get(&path, &["br, gzip"]).await;

        assert!(
            !response.headers().contains_key(header::CONTENT_ENCODING),
            "{path}"
        );
    }
}

/// A file under `/assets/` is named by its content's hash, so the browser keeps it for a year
/// without asking again, while the app's `index.html` isn't kept, so a new release still loads.
#[tokio::test]
async fn hashed_assets_are_cached_for_good_and_the_shell_is_not() {
    let asset_response = get(linked_asset("js"), &[]).await;
    let shell_response = get("/", &[]).await;

    assert_eq!(
        asset_response.headers()[header::CACHE_CONTROL],
        "public, max-age=31536000, immutable"
    );
    assert!(!shell_response.headers().contains_key(header::CACHE_CONTROL));
}

/// `WebAssembly.instantiateStreaming` refuses the ASS renderer's wasm under any other type, and
/// Chrome the manifest that installs the app.
#[tokio::test]
async fn the_app_s_webassembly_and_manifest_are_served_as_such() {
    for (path, content_type) in [
        (built_asset("wasm"), "application/wasm"),
        (
            "/manifest.webmanifest".to_owned(),
            "application/manifest+json",
        ),
    ] {
        let response = get(&path, &[]).await;

        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            content_type,
            "{path}"
        );
    }
}

/// Every file type `vp build` writes, so a browser takes each for what it is. A new one gets its
/// row here and in `content_type`, or is served as `application/octet-stream`.
#[test]
fn each_file_type_the_app_is_built_with_has_its_content_type() {
    for (path, expected) in [
        ("/index.html", "text/html; charset=utf-8"),
        ("/assets/index-1a2b.js", "text/javascript; charset=utf-8"),
        ("/assets/index-1a2b.css", "text/css; charset=utf-8"),
        ("/assets/inter-1a2b.woff2", "font/woff2"),
        ("/icon.svg", "image/svg+xml"),
        ("/icon-192.png", "image/png"),
        ("/favicon.ico", "image/x-icon"),
        ("/licenses/jassub.txt", "text/plain; charset=utf-8"),
        ("/manifest.webmanifest", "application/manifest+json"),
        ("/assets/jassub-1a2b.wasm", "application/wasm"),
        ("/assets/unknown-1a2b.xyz", "application/octet-stream"),
    ] {
        assert_eq!(content_type(path), expected, "{path}");
    }
}

/// The ASS renderer ships with the notices of what it bundles and of its fallback font, as their
/// licenses ask.
#[test]
fn the_app_ships_the_ass_renderer_s_notices() {
    let jassub_notices = std::str::from_utf8(asset("/licenses/jassub.txt").unwrap()).unwrap();
    let font_notice = std::str::from_utf8(asset("/licenses/liberation-sans.txt").unwrap()).unwrap();

    assert!(jassub_notices.contains("FreeType"));
    assert!(font_notice.contains("Reserved Font Name Liberation"));
    assert!(font_notice.contains("SIL OPEN FONT LICENSE Version 1.1"));
}
