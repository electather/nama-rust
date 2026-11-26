use axum::{Router, body::Body, middleware};
use tower::ServiceExt as _;

use super::*;

const ADA_ON_THE_TV: Viewer = Viewer { user: 1, device: 7 };

const GRANT: PlaybackGrant = PlaybackGrant {
    viewer: ADA_ON_THE_TV,
    version: 3,
    plan: 5,
};

/// What a route behind [`admit`] sees of a `GET` for `url`: its path and Viewer, its plan running
/// all along and its Device acting as `acting` says.
async fn seen(signer: &UrlSigner, url: &str, acting: bool) -> (String, Option<Viewer>) {
    let plan_runs: PlanRuns = Arc::new(|_, _| true);
    let still_acting: StillActing = Arc::new(move |_| Box::pin(async move { acting }));
    let router = Router::new()
        .fallback(|request: Request| async move {
            let viewer = request.extensions().get::<Viewer>();
            format!("{} {viewer:?}", request.uri().path())
        })
        .layer(middleware::from_fn_with_state(
            (signer.clone(), plan_runs, still_acting),
            admit,
        ));
    let request = Request::get(url).body(Body::empty()).unwrap();
    let body = router.oneshot(request).await.unwrap().into_body();
    let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();
    let (path, viewer) = std::str::from_utf8(&body).unwrap().split_once(' ').unwrap();
    let viewer = (viewer != "None").then_some(ADA_ON_THE_TV);
    (path.to_owned(), viewer)
}

/// [`GRANT`]'s URL for its Version, signed as having expired a second ago.
fn expired(signer: &UrlSigner) -> String {
    let token = GRANT.token(now() - 1);
    let signature = signer.signature(&playback_message(&token));
    format!("{SIGNED}{token}/{signature}/versions/3")
}

#[tokio::test]
async fn a_playback_url_acts_as_its_viewer_while_its_device_acts_as_them_even_past_its_expiry() {
    let signer = UrlSigner::new();
    let fresh = signer.sign_playback("/media/versions/3", GRANT);

    for url in [fresh, expired(&signer)] {
        let admitted = seen(&signer, &url, true).await;

        assert_eq!(
            admitted,
            ("/media/versions/3".to_owned(), Some(ADA_ON_THE_TV)),
            "{url}"
        );
    }
}

#[tokio::test]
async fn a_playback_url_goes_on_unsigned_once_its_device_acts_as_another_user_though_its_plan_runs()
{
    let signer = UrlSigner::new();
    let fresh = signer.sign_playback("/media/versions/3", GRANT);

    for url in [fresh, expired(&signer)] {
        let refused = seen(&signer, &url, false).await;

        assert_eq!(refused, (url.clone(), None));
    }
}

#[tokio::test]
async fn an_image_url_is_admitted_whoever_any_device_acts_as() {
    let signer = UrlSigner::new();
    let image = signer.sign("/media/images/poster.jpg");

    let admitted = seen(&signer, &image, false).await;

    assert_eq!(admitted, ("/media/images/poster.jpg".to_owned(), None));
}
