use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
    middleware::from_fn,
};
use tower::ServiceExt as _;

use super::serve;

/// A path under `/.well-known/`, such as the key set's, goes on to the Hub's routes, never the app.
#[tokio::test]
async fn the_key_set_s_paths_are_passed_on() {
    let request = Request::get("/.well-known/jwks.json").body(Body::empty());

    let response = Router::new()
        .layer(from_fn(serve))
        .oneshot(request.unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
