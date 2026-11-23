use std::path::Path;

use axum::{Router, middleware};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

/// A file under `/assets/` is named by its content's hash, so the browser keeps it for a year
/// without asking again, while the app's `index.html` isn't kept, so a new release still loads.
#[tokio::test]
async fn hashed_assets_are_cached_for_good_and_the_shell_is_not() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = Router::new().layer(middleware::from_fn(super::serve));
    tokio::spawn(async move { axum::serve(listener, app).await });
    // The shell links its scripts in a `<script>` tag, which is no easier to find than a file in
    // the folder `vp -C hub build` wrote.
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/hub/dist/assets");
    let script = std::fs::read_dir(assets)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|file| file.extension().is_some_and(|extension| extension == "js"))
        .unwrap();
    let script = script.file_name().unwrap().to_str().unwrap();

    let asset_head = response_head(addr, &format!("/assets/{script}")).await;
    let shell_head = response_head(addr, "/").await;

    assert!(
        asset_head.contains("\r\ncache-control: public, max-age=31536000, immutable\r\n"),
        "{asset_head}"
    );
    assert!(!shell_head.contains("cache-control"), "{shell_head}");
}

/// Sends a plain HTTP/1.1 `GET` for `path` and returns the response's status line and headers.
async fn response_head(addr: std::net::SocketAddr, path: &str) -> String {
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let request = format!("GET {path} HTTP/1.1\r\nhost: hub\r\nconnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).await.unwrap();
    let response = String::from_utf8_lossy(&response);
    response.split("\r\n\r\n").next().unwrap().to_owned()
}
