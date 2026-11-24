//! The web app at the Hub calls a Linked Server straight from the browser, so the Server answers
//! CORS for the Hub its config names, for bearer requests only, never with its session cookie
//! (ADR 0015, ADR 0105).

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/fake_hub.rs"]
mod fake_hub;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/http.rs"]
mod http;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use fake_hub::FakeHub;
use http::{Reply, send};
use http_body_util::Full;
use hyper::{Request, body::Bytes};
use server::TestServer;

const WHO_AM_I: &str = "/nama.v1.SignInService/WhoAmI";

/// A Server Linked with a fake Hub, and that Hub.
async fn linked() -> (TestServer, FakeHub) {
    let hub = FakeHub::start().await;
    let server = TestServer::builder().config(hub.config()).start().await;
    hub.link(&server).await;
    (server, hub)
}

/// A browser's preflight from `origin` for a bearer RPC, presenting `cookie_token`, if any, in the
/// session cookie, which a browser never does.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn preflight(server: &TestServer, origin: &str, cookie_token: Option<&str>) -> Reply {
    let request = Request::options(WHO_AM_I)
        .header("origin", origin)
        .header("access-control-request-method", "POST")
        .header(
            "access-control-request-headers",
            "authorization,content-type",
        )
        .body(Full::<Bytes>::default())
        .unwrap();
    send(server, request, cookie_token).await
}

/// `WhoAmI` from the Hub's page, presenting the admin's token as a bearer or, with `cookie`, in the
/// session cookie.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn who_am_i_from_the_hub(server: &TestServer, hub: &FakeHub, cookie: bool) -> Reply {
    let request = Request::post(WHO_AM_I)
        .header("origin", hub.url())
        .header("content-type", "application/json");
    let token = server.admin_token.as_str();
    let (request, cookie_token) = if cookie {
        (request, Some(token))
    } else {
        (
            request.header("authorization", format!("Bearer {token}")),
            None,
        )
    };
    let request = request.body(Full::new(Bytes::from_static(b"{}"))).unwrap();
    send(server, request, cookie_token).await
}

fn approvals(reply: &Reply) -> Vec<&str> {
    (reply.headers.keys())
        .map(hyper::header::HeaderName::as_str)
        .filter(|name| name.starts_with("access-control-allow-"))
        .collect()
}

#[tokio::test]
async fn a_preflight_from_the_hub_for_a_bearer_request_is_allowed() {
    let (server, hub) = linked().await;

    let reply = preflight(&server, &hub.url(), None).await;

    assert!((200..300).contains(&reply.status), "{}", reply.status);
    assert_eq!(reply.header("access-control-allow-origin"), hub.url());
    let allowed = reply.header("access-control-allow-headers");
    assert!(allowed.contains("authorization"), "{allowed}");
}

#[tokio::test]
async fn a_bearer_request_from_the_hub_may_read_its_answer() {
    let (server, hub) = linked().await;

    let reply = who_am_i_from_the_hub(&server, &hub, false).await;

    assert_eq!(reply.status, 200);
    assert_eq!(reply.header("access-control-allow-origin"), hub.url());
}

#[tokio::test]
async fn a_request_from_the_hub_carrying_the_cookie_may_not_read_its_answer() {
    let (server, hub) = linked().await;

    let reply = who_am_i_from_the_hub(&server, &hub, true).await;

    assert!(approvals(&reply).is_empty(), "{:?}", reply.headers);
}

#[tokio::test]
async fn a_preflight_from_another_origin_is_refused() {
    let (server, _hub) = linked().await;

    for origin in ["https://elsewhere.example", "null"] {
        let reply = preflight(&server, origin, None).await;

        assert!(
            !(200..300).contains(&reply.status),
            "{origin}: {}",
            reply.status
        );
        assert!(
            approvals(&reply).is_empty(),
            "{origin}: {:?}",
            reply.headers
        );
    }
}

#[tokio::test]
async fn a_preflight_asking_for_credentials_is_refused() {
    let (server, hub) = linked().await;

    // A browser sends a preflight without credentials and refuses a credentialed request unless the
    // answer allows them.
    let bearer = preflight(&server, &hub.url(), None).await;
    assert_eq!(bearer.header("access-control-allow-credentials"), "");

    let cookie = preflight(&server, &hub.url(), Some(server.admin_token.as_str())).await;
    assert!(!(200..300).contains(&cookie.status), "{}", cookie.status);
    assert!(approvals(&cookie).is_empty(), "{:?}", cookie.headers);
}

#[tokio::test]
async fn an_unlinked_server_allows_no_origin() {
    let hub = FakeHub::start().await;
    let server = TestServer::builder().config(hub.config()).start().await;

    let reply = preflight(&server, &hub.url(), None).await;

    assert!(!(200..300).contains(&reply.status), "{}", reply.status);
    assert!(approvals(&reply).is_empty(), "{:?}", reply.headers);
}
