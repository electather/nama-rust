use std::{fs, path::Path};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
    middleware::from_fn,
    response::Response,
};
use tower::ServiceExt as _;

use super::{SHELL, serve};

/// The answer to a `GET` for `path` from a client accepting `encodings`, passing `/media/` on.
async fn get(path: &str, encodings: &[&str]) -> Response {
    let mut request = Request::get(path);
    for encoding in encodings {
        request = request.header(header::ACCEPT_ENCODING, *encoding);
    }
    send(request.body(Body::empty()).unwrap()).await
}

/// The answer to `request` from `serve` passing `/media/` on, in front of no routes at all.
async fn send(request: Request<Body>) -> Response {
    Router::new()
        .layer(from_fn(|incoming, next| {
            serve(incoming, next, &["/media/"])
        }))
        .oneshot(request)
        .await
        .unwrap()
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

/// The path of a file with `extension` that `vp build` wrote under `/assets/`, linked or not.
fn built_asset(extension: &str) -> String {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/dist/assets");
    let name = fs::read_dir(assets)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .find(|name| Path::new(name).extension().is_some_and(|e| e == extension))
        .unwrap();
    format!("/assets/{name}")
}

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

#[tokio::test]
async fn the_app_is_sent_as_built_to_a_client_that_accepts_no_encoding() {
    let script = linked_asset("js");
    let stylesheet = linked_asset("css");
    let dist = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/dist");
    for (path, file) in [
        ("/", "index.html"),
        (script, script.trim_start_matches('/')),
        (stylesheet, stylesheet.trim_start_matches('/')),
        ("/icon.svg", "icon.svg"),
    ] {
        let response = get(path, &[]).await;

        assert_eq!(response.status(), StatusCode::OK, "{path}");
        assert!(
            !response.headers().contains_key(header::CONTENT_ENCODING),
            "{path}"
        );
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(body, fs::read(dist.join(file)).unwrap(), "{path}");
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

/// A browser applies the app's stylesheet only as CSS, `WebAssembly.instantiateStreaming` refuses
/// the ASS renderer's wasm under any other type, and Chrome the manifest that installs the app.
#[tokio::test]
async fn the_app_s_stylesheet_webassembly_and_manifest_are_served_as_such() {
    for (path, content_type) in [
        (linked_asset("css").to_owned(), "text/css; charset=utf-8"),
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

/// The ASS renderer ships with the notices of what it bundles and of its fallback font, as their
/// licenses ask.
#[tokio::test]
async fn the_app_ships_the_ass_renderer_s_notices() {
    let jassub = get("/licenses/jassub.txt", &[]).await;
    let font = get("/licenses/liberation-sans.txt", &[]).await;

    // An unknown path answers `index.html`, so the text is what shows the file shipped.
    let jassub = to_bytes(jassub.into_body(), usize::MAX).await.unwrap();
    let font = to_bytes(font.into_body(), usize::MAX).await.unwrap();
    let jassub = std::str::from_utf8(&jassub).unwrap();
    let font = std::str::from_utf8(&font).unwrap();
    assert!(jassub.contains("FreeType"));
    assert!(font.contains("Reserved Font Name Liberation"));
    assert!(font.contains("SIL OPEN FONT LICENSE Version 1.1"));
}

/// An RPC's path, a passed-on path and anything but a `GET` go on to the routes behind, which here
/// have none, so they get a 404 and never the app.
#[tokio::test]
async fn rpcs_passed_on_paths_and_other_methods_go_on() {
    for (method, path) in [
        (Method::GET, "/nama.v1.NoSuchService/Nothing"),
        (Method::GET, "/media/42"),
        (Method::POST, "/"),
    ] {
        let request = Request::builder().method(&method).uri(path);
        let response = send(request.body(Body::empty()).unwrap()).await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{method} {path}");
    }
}
