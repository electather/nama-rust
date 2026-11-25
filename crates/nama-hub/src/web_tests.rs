use std::{fs, path::Path};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, header},
    middleware::from_fn,
    response::Response,
};
use tower::ServiceExt as _;

use super::{SHELL, asset, serve};

/// The Hub's answer to a `GET` for `path` from a client accepting `encodings`.
async fn get(path: &str, encodings: &[&str]) -> Response {
    let mut request = Request::get(path);
    for encoding in encodings {
        request = request.header(header::ACCEPT_ENCODING, *encoding);
    }
    Router::new()
        .layer(from_fn(serve))
        .oneshot(request.body(Body::empty()).unwrap())
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

#[tokio::test]
async fn the_app_s_text_is_compressed_for_a_browser_that_accepts_it() {
    let paths = ["/", "/sign-in", linked_asset("js"), linked_asset("css")];
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
    for (path, built) in [
        ("/", SHELL),
        (script, asset(script).unwrap()),
        (stylesheet, asset(stylesheet).unwrap()),
    ] {
        let response = get(path, &[]).await;

        assert!(
            !response.headers().contains_key(header::CONTENT_ENCODING),
            "{path}"
        );
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(body, built, "{path}");
    }
}

#[tokio::test]
async fn a_font_is_sent_as_it_is() {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/dist/assets");
    let font = fs::read_dir(assets)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .find(|name| Path::new(name).extension().is_some_and(|e| e == "woff2"))
        .unwrap();

    let response = get(&format!("/assets/{font}"), &["br, gzip"]).await;

    assert!(!response.headers().contains_key(header::CONTENT_ENCODING));
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
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/dist/assets");
    let wasm = fs::read_dir(assets)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .find(|name| Path::new(name).extension().is_some_and(|e| e == "wasm"))
        .unwrap();

    for (path, content_type) in [
        (format!("/assets/{wasm}"), "application/wasm"),
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
