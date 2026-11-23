use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, header},
    middleware::from_fn,
    response::Response,
};
use tower::ServiceExt as _;

use super::{SHELL, serve};

/// A deep link's page, `index.html`, as the Hub answers it to a request accepting `encodings`.
async fn deep_link(encodings: &[&str]) -> Response {
    let mut request = Request::get("/accounts/sign-in");
    for encoding in encodings {
        request = request.header(header::ACCEPT_ENCODING, *encoding);
    }
    Router::new()
        .layer(from_fn(serve))
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

#[tokio::test]
async fn the_pages_are_compressed_for_a_browser_that_accepts_it() {
    for encoding in ["br", "gzip"] {
        let response = deep_link(&[encoding]).await;

        assert_eq!(response.headers()[header::CONTENT_ENCODING], encoding);
        assert_eq!(response.headers()[header::VARY], "accept-encoding");
    }
}

#[tokio::test]
async fn the_pages_are_sent_as_built_to_a_client_that_accepts_no_encoding() {
    let response = deep_link(&[]).await;

    assert!(!response.headers().contains_key(header::CONTENT_ENCODING));
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(body, SHELL);
}
