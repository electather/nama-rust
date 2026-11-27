use axum::{Router, body::Body, middleware};
use tower::ServiceExt as _;

use super::*;

const GRANT: PlaybackGrant = PlaybackGrant {
    viewer: Viewer { user: 1, device: 7 },
    version: 3,
    plan: 5,
};

/// What a route behind [`admit`] sees of a `GET` for `url`, as [`shown`] writes it.
async fn seen(signer: &UrlSigner, url: &str) -> String {
    let router = Router::new()
        .fallback(|request: Request| async move {
            let grant = request.extensions().get::<SignedGrant>();
            shown(request.uri().path(), grant.map(|signed| signed.expires))
        })
        .layer(middleware::from_fn_with_state(signer.clone(), admit));
    let request = Request::get(url).body(Body::empty()).unwrap();
    let body = router.oneshot(request).await.unwrap().into_body();
    let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();
    String::from_utf8(body.to_vec()).unwrap()
}

/// A request's path and the expiry of the grant admitted with it, if any.
fn shown(path: &str, expires: Option<i64>) -> String {
    format!("{path} {expires:?}")
}

/// [`GRANT`]'s URL for `rest`, signed as expiring at `expires`.
fn signed_until(signer: &UrlSigner, expires: i64, rest: &str) -> String {
    let token = GRANT.token(expires);
    let signature = signer.signature(&playback_message(&token));
    format!("{SIGNED}{token}/{signature}/{rest}")
}

#[tokio::test]
async fn a_playback_url_is_admitted_with_its_expiry_unjudged_even_past_it() {
    let signer = UrlSigner::new();
    let expired = now() - 1;

    let admitted = seen(
        &signer,
        &signed_until(&signer, expired, "plans/5/index.m3u8"),
    )
    .await;

    assert_eq!(admitted, shown("/media/plans/5/index.m3u8", Some(expired)));
}

#[tokio::test]
async fn a_playback_url_moved_off_its_version_and_plan_goes_on_unsigned() {
    let signer = UrlSigner::new();
    let fresh = signer.sign_playback("/media/versions/3", GRANT);

    for rest in [
        "versions/4",
        "plans/6/index.m3u8",
        "downloads/3",
        "versions/3/../4",
    ] {
        let moved = format!("{}/{rest}", fresh.rsplit_once("/versions/").unwrap().0);

        assert_eq!(seen(&signer, &moved).await, shown(&moved, None), "{rest}");
    }
}

#[tokio::test]
async fn an_image_url_is_admitted_until_it_expires() {
    let signer = UrlSigner::new();
    let image = signer.sign("/media/images/poster.jpg");
    let expired = now() - 1;
    let signature = signer.signature(&image_message(&expired.to_string(), "/media/images/a.jpg"));
    let expired = format!("{SIGNED}{expired}/{signature}/images/a.jpg");

    assert_eq!(
        seen(&signer, &image).await,
        shown("/media/images/poster.jpg", None)
    );
    assert_eq!(seen(&signer, &expired).await, shown(&expired, None));
}
