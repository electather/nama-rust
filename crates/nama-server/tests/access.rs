//! Who may call what: a session for everything but the public RPCs, and the Admin Role for
//! managing Libraries and Matches, over a real socket and against a real SQLite database.

#[path = "support/server.rs"]
mod server;

use std::time::Duration;

use connectrpc::{
    ConnectError, ErrorCode,
    client::{ClientConfig, HttpClient, ServerStream},
};
use hyper::{HeaderMap, Method};
use nama_proto::nama::v1::{
    __buffa::view::SubscribeResponseView, CreateLibraryRequest, EventServiceClient, LibraryKind,
    LibraryServiceClient, ScanLibraryRequest, SubscribeRequest, SubscribeResponse,
};
use nama_server::{config::Config, plugins::Registry, users::Users};
use serde_json::Value;
use server::{PASSWORD, TestServer};

/// Every RPC that manages Libraries or Matches, which only an Admin may call.
const ADMIN_ONLY: [&str; 10] = [
    "/nama.v1.LibraryService/CreateLibrary",
    "/nama.v1.LibraryService/ListLibraries",
    "/nama.v1.LibraryService/GetLibrary",
    "/nama.v1.LibraryService/UpdateLibrary",
    "/nama.v1.LibraryService/DeleteLibrary",
    "/nama.v1.LibraryService/ScanLibrary",
    "/nama.v1.MatchService/SearchMetadata",
    "/nama.v1.MatchService/SetMatch",
    "/nama.v1.MatchService/ClearMatch",
    "/nama.v1.MatchService/ListUnmatchedItems",
];

/// Every other unary RPC that needs a session.
const SIGNED_IN: [&str; 2] = [
    "/nama.v1.SignInService/SignOut",
    "/nama.v1.SignInService/WhoAmI",
];

/// How long a test waits for a stream before failing.
const WAIT: Duration = Duration::from_secs(10);

/// A subscription to the Server's events.
type Subscription = ServerStream<hyper::body::Incoming, SubscribeResponseView<'static>>;

/// What the Server answered a bare HTTP/1.1 request.
struct Reply {
    status: u16,
    headers: HeaderMap,
    body: Vec<u8>,
}

impl Reply {
    /// The Connect error code in a JSON error body.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    fn code(&self) -> String {
        let body: Value = serde_json::from_slice(&self.body).unwrap();
        body.get("code")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    }
}

/// Sends a bare HTTP/1.1 request, so the test controls every header.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn send(server: &TestServer, method: Method, path: &str, headers: &[(&str, &str)]) -> Reply {
    use http_body_util::{BodyExt, Full};
    use hyper::{Request, body::Bytes, client::conn::http1};
    use hyper_util::rt::TokioIo;

    let stream = tokio::net::TcpStream::connect(server.addr).await.unwrap();
    let (mut sender, connection) = http1::handshake(TokioIo::new(stream)).await.unwrap();
    tokio::spawn(connection);
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("host", server.addr.to_string());
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let request = request.body(Full::new(Bytes::from_static(b"{}"))).unwrap();
    let response = sender.send_request(request).await.unwrap();
    Reply {
        status: response.status().as_u16(),
        headers: response.headers().clone(),
        body: response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    }
}

/// Posts an empty JSON request to the RPC at `path`, with `bearer` as the session if given.
async fn post_json(server: &TestServer, path: &str, bearer: Option<&str>) -> Reply {
    let authorization = bearer.map(|token| format!("Bearer {token}"));
    let mut headers = vec![("content-type", "application/json")];
    if let Some(authorization) = &authorization {
        headers.push(("authorization", authorization));
    }
    send(server, Method::POST, path, &headers).await
}

/// Starts a Server with an Admin, and a Member named Grace signed in. Returns her session token.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn start_with_member() -> (TestServer, String) {
    let plugins = Registry::new();
    let (server, db) = TestServer::start_with_db(Config::default(), plugins.clone()).await;
    let users = Users::new(db, plugins);
    users.create_member("Grace", PASSWORD).await.unwrap();
    let signed_in = users.sign_in("Grace", PASSWORD, "Phone").await.unwrap();
    (server, signed_in.token().to_owned())
}

/// The next message on `subscription`, `None` if it ended, or why it failed.
#[expect(
    clippy::expect_used,
    reason = "a test helper fails its test by panicking"
)]
async fn next_event(
    subscription: &mut Subscription,
) -> Result<Option<SubscribeResponse>, ConnectError> {
    let message = tokio::time::timeout(WAIT, subscription.message::<SubscribeResponse>())
        .await
        .expect("timed out waiting for the stream")?;
    Ok(message.map(|message| message.to_owned_message()))
}

/// Subscribes to every event with `config`.
async fn subscribe(config: ClientConfig) -> Result<Subscription, ConnectError> {
    EventServiceClient::new(HttpClient::plaintext(), config)
        .subscribe(SubscribeRequest::default())
        .await
}

#[tokio::test]
async fn an_unauthenticated_call_to_anything_but_the_public_rpcs_is_unauthenticated() {
    let server = TestServer::start(Config::default(), Registry::new()).await;

    for path in ADMIN_ONLY.iter().chain(&SIGNED_IN) {
        let reply = post_json(&server, path, None).await;

        assert_eq!(reply.code(), "unauthenticated", "{path}");
    }
    let subscribed = match subscribe(server.anonymous_client_config()).await {
        Ok(mut subscription) => next_event(&mut subscription).await.map(|_| ()),
        Err(err) => Err(err),
    };
    assert_eq!(
        subscribed.map_err(|err| err.code),
        Err(ErrorCode::Unauthenticated)
    );
}

#[tokio::test]
async fn a_member_managing_libraries_or_matches_is_permission_denied() {
    let (server, member_token) = start_with_member().await;

    for path in ADMIN_ONLY {
        let reply = post_json(&server, path, Some(&member_token)).await;

        assert_eq!(reply.code(), "permission_denied", "{path}");
    }
}

#[tokio::test]
async fn an_admin_may_manage_libraries_and_matches() {
    let server = TestServer::start(Config::default(), Registry::new()).await;

    for path in ADMIN_ONLY {
        let reply = post_json(&server, path, Some(&server.admin_token)).await;

        // An empty request is wrong for most of them, but not for want of access.
        let code = reply.code();
        assert!(
            !["unauthenticated", "permission_denied"].contains(&code.as_str()),
            "{path}: {code}"
        );
    }
}

#[tokio::test]
async fn a_member_receives_events() {
    let (server, member_token) = start_with_member().await;
    let mut events = subscribe(server.client_config_as(&member_token))
        .await
        .unwrap();

    let folder = tempfile::tempdir().unwrap();
    let libraries = LibraryServiceClient::new(HttpClient::plaintext(), server.client_config());
    let library = libraries
        .create_library(CreateLibraryRequest {
            name: "Movies".to_owned(),
            kind: LibraryKind::LIBRARY_KIND_MOVIES.into(),
            folders: vec![folder.path().to_str().unwrap().to_owned()],
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .library
        .into_option()
        .unwrap();
    libraries
        .scan_library(ScanLibraryRequest {
            id: library.id,
            ..Default::default()
        })
        .await
        .unwrap();

    let first = next_event(&mut events).await;
    assert!(matches!(first, Ok(Some(_))), "{first:?}");
}

/// A cross-site form can post `text/plain` without a CORS preflight, and would carry the cookie if
/// `SameSite` failed, so the Server must not run it as an RPC (ADR 0015).
#[tokio::test]
async fn a_text_plain_post_with_a_valid_session_cookie_is_refused() {
    let server = TestServer::start(Config::default(), Registry::new()).await;
    let cookie = format!("nama_session={}", server.admin_token);

    let reply = send(
        &server,
        Method::POST,
        "/nama.v1.LibraryService/CreateLibrary",
        &[("content-type", "text/plain"), ("cookie", &cookie)],
    )
    .await;

    assert_eq!(
        reply.status,
        415,
        "{}",
        String::from_utf8_lossy(&reply.body)
    );
}

/// Browsers send a preflight without the cookie; one that carried it anyway gets past the session
/// check, so both are tried.
#[tokio::test]
async fn a_cors_preflight_for_a_credentialed_request_is_not_approved() {
    let server = TestServer::start(Config::default(), Registry::new()).await;
    let preflight = [
        ("origin", "https://elsewhere.example"),
        ("access-control-request-method", "POST"),
        ("access-control-request-headers", "content-type"),
    ];
    let cookie = format!("nama_session={}", server.admin_token);
    let with_cookie = [preflight.as_slice(), &[("cookie", &cookie)]].concat();

    for headers in [preflight.as_slice(), &with_cookie] {
        let path = "/nama.v1.LibraryService/CreateLibrary";
        let reply = send(&server, Method::OPTIONS, path, headers).await;

        assert!(!(200..300).contains(&reply.status), "{}", reply.status);
        let approvals: Vec<_> = reply
            .headers
            .keys()
            .filter(|name| name.as_str().starts_with("access-control-allow-"))
            .collect();
        assert!(approvals.is_empty(), "{approvals:?}");
    }
}
