//! Who may call what: a session for everything but the public RPCs, the Admin Role for managing
//! Libraries, Matches and Users, and Recent Authentication for changing who can sign in as whom,
//! over a real socket and against a real SQLite database.

#[path = "support/refused.rs"]
mod refused;
#[path = "support/server.rs"]
mod server;

use std::{collections::BTreeSet, net::Ipv4Addr, time::Duration};

use connectrpc::{
    ConnectError, ErrorCode,
    client::{ClientConfig, HttpClient, ServerStream},
};
use hyper::{HeaderMap, Method};
use nama_proto::nama::v1::{
    __buffa::view::SubscribeResponseView, ApproveSignInCodeRequest, CreateLibraryRequest,
    CreateUserRequest, EventServiceClient, LibraryKind, LibraryServiceClient, ListLibrariesRequest,
    PollSignInCodeRequest, RefusalReason, Role, ScanLibraryRequest, SessionDelivery,
    SignInServiceClient, StartSignInCodeRequest, SubscribeRequest, SubscribeResponse,
    UserServiceClient,
};
use nama_server::{
    config::Config,
    db::Database,
    plugins::Registry,
    users::{NewDevice, Users},
};
use refused::{assert_refused, refusal_of};
use serde_json::{Value, json};
use server::{PASSWORD, TestServer};

/// Every RPC that manages Libraries, Matches or Users, which only an Admin may call.
const ADMIN_ONLY: [&str; 14] = [
    "/nama.v1.LibraryService/CreateLibrary",
    "/nama.v1.LibraryService/GetLibrary",
    "/nama.v1.LibraryService/UpdateLibrary",
    "/nama.v1.LibraryService/DeleteLibrary",
    "/nama.v1.LibraryService/ScanLibrary",
    "/nama.v1.MatchService/SearchMetadata",
    "/nama.v1.MatchService/SetMatch",
    "/nama.v1.MatchService/ClearMatch",
    "/nama.v1.MatchService/ListUnmatchedItems",
    "/nama.v1.UserService/CreateUser",
    "/nama.v1.UserService/ListUsers",
    "/nama.v1.UserService/SetRole",
    "/nama.v1.UserService/DeleteUser",
    "/nama.v1.UserService/ResetPassword",
];

/// The RPCs for browsing the Libraries, what they hold and the caller's Continue Watching, which
/// any signed-in User may call.
const BROWSE: [&str; 7] = [
    "/nama.v1.LibraryService/ListLibraries",
    "/nama.v1.BrowseService/ListMovies",
    "/nama.v1.BrowseService/GetMovie",
    "/nama.v1.BrowseService/ListShows",
    "/nama.v1.BrowseService/GetShow",
    "/nama.v1.BrowseService/ListEpisodes",
    "/nama.v1.BrowseService/ListContinueWatching",
];

/// Every other unary RPC that needs a session.
const SIGNED_IN: [&str; 10] = [
    SIGN_OUT,
    "/nama.v1.SignInService/WhoAmI",
    "/nama.v1.SignInService/Reauthenticate",
    "/nama.v1.SignInService/LookUpSignInCode",
    "/nama.v1.SignInService/ApproveSignInCode",
    "/nama.v1.UserService/ListDevices",
    "/nama.v1.UserService/RevokeDevice",
    "/nama.v1.WatchStateService/ReportProgress",
    "/nama.v1.WatchStateService/SetPlayed",
    "/nama.v1.UserService/ChangePassword",
];

const SIGN_OUT: &str = "/nama.v1.SignInService/SignOut";

/// The RPCs that change who can sign in as whom, which need Recent Authentication (ADR 0014).
const SENSITIVE: [&str; 5] = [
    "/nama.v1.UserService/ChangePassword",
    "/nama.v1.UserService/ResetPassword",
    "/nama.v1.UserService/SetRole",
    "/nama.v1.UserService/DeleteUser",
    "/nama.v1.SignInService/ApproveSignInCode",
];

const WAIT: Duration = Duration::from_secs(10);

type Subscription = ServerStream<hyper::body::Incoming, SubscribeResponseView<'static>>;

struct Reply {
    status: u16,
    headers: HeaderMap,
    body: Vec<u8>,
}

impl Reply {
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

    /// The error the body holds, as a client decodes it.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    fn refused(&self) -> Result<(), ConnectError> {
        Err(serde_json::from_slice(&self.body).unwrap())
    }
}

/// Sends a bare HTTP/1.1 request, so the test controls every header.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn send(
    server: &TestServer,
    method: Method,
    path: &str,
    headers: &[(&str, &str)],
    body: String,
) -> Reply {
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
    let request = request.body(Full::new(Bytes::from(body))).unwrap();
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

async fn post_json(server: &TestServer, path: &str, bearer: Option<&str>) -> Reply {
    let authorization = bearer.map(|token| format!("Bearer {token}"));
    let mut headers = vec![("content-type", "application/json")];
    if let Some(authorization) = &authorization {
        headers.push(("authorization", authorization));
    }
    send(server, Method::POST, path, &headers, "{}".to_owned()).await
}

/// Starts a Server with an Admin, and a Member named Grace signed in. Returns her session token.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn start_with_member() -> (TestServer, String) {
    let plugins = Registry::new();
    let (server, db) = TestServer::start_with_db(Config::default(), plugins.clone()).await;
    UserServiceClient::new(HttpClient::plaintext(), server.client_config())
        .create_user(CreateUserRequest {
            name: "Grace".to_owned(),
            password: PASSWORD.to_owned(),
            role: Role::ROLE_MEMBER.into(),
            ..Default::default()
        })
        .await
        .unwrap();
    let users = Users::new(db, plugins);
    let phone = NewDevice {
        name: "Phone".to_owned(),
        ip_address: Ipv4Addr::LOCALHOST.into(),
        user_agent: None,
    };
    let signed_in = users.sign_in("Grace", PASSWORD, phone).await.unwrap();
    (server, signed_in.token().to_owned())
}

/// Signs a Device in as the Admin by a Sign-in Code the Admin approves, and returns its session
/// token.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn signed_in_by_code(server: &TestServer) -> String {
    let anonymous =
        SignInServiceClient::new(HttpClient::plaintext(), server.anonymous_client_config());
    let started = anonymous
        .start_sign_in_code(StartSignInCodeRequest {
            device_name: "Living room TV".to_owned(),
            delivery: SessionDelivery::SESSION_DELIVERY_TOKEN.into(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned();
    SignInServiceClient::new(HttpClient::plaintext(), server.client_config())
        .approve_sign_in_code(ApproveSignInCodeRequest {
            code: started.code,
            ..Default::default()
        })
        .await
        .unwrap();
    let polled = anonymous
        .poll_sign_in_code(PollSignInCodeRequest {
            handle: started.handle,
            ..Default::default()
        })
        .await
        .unwrap();
    polled.into_owned().token
}

/// Moves every Device's last authentication back past the 10 minutes Recent Authentication lasts,
/// as if they had all signed in 11 minutes ago.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn authenticated_long_ago(db: &Database) {
    db.write(|tx| {
        Box::pin(async move {
            sqlx::query("UPDATE devices SET authenticated_at = authenticated_at - 11 * 60")
                .execute(&mut **tx)
                .await
        })
    })
    .await
    .unwrap();
}

/// The RPCs refused for want of Recent Authentication when called as `token`, out of all but
/// `SignOut`, which would end the session. None may be refused for want of a session or the Admin
/// Role, so `token` must be an Admin's.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn refused_for_recent_authentication(
    server: &TestServer,
    token: &str,
) -> BTreeSet<&'static str> {
    let mut refused = BTreeSet::new();
    for path in ADMIN_ONLY.iter().chain(&BROWSE).chain(&SIGNED_IN) {
        if *path == SIGN_OUT {
            continue;
        }
        let reply = post_json(server, path, Some(token)).await;
        let code = reply.code();
        assert!(
            !["unauthenticated", "permission_denied"].contains(&code.as_str()),
            "{path}: {code}"
        );
        if code == "failed_precondition"
            && refusal_of(&reply.refused().unwrap_err()).reason
                == RefusalReason::REFUSAL_REASON_RECENT_AUTHENTICATION_REQUIRED
        {
            refused.insert(*path);
        }
    }
    refused
}

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

async fn subscribe(config: ClientConfig) -> Result<Subscription, ConnectError> {
    EventServiceClient::new(HttpClient::plaintext(), config)
        .subscribe(SubscribeRequest::default())
        .await
}

#[tokio::test]
async fn an_unauthenticated_call_to_anything_but_the_public_rpcs_is_unauthenticated() {
    let server = TestServer::start(Config::default(), Registry::new()).await;

    for path in ADMIN_ONLY.iter().chain(&BROWSE).chain(&SIGNED_IN) {
        let reply = post_json(&server, path, None).await;

        assert_refused(
            reply.refused(),
            ErrorCode::Unauthenticated,
            RefusalReason::REFUSAL_REASON_SIGN_IN_REQUIRED,
        );
    }
    let subscribed = match subscribe(server.anonymous_client_config()).await {
        Ok(mut subscription) => next_event(&mut subscription).await.map(|_| ()),
        Err(err) => Err(err),
    };
    assert_refused(
        subscribed,
        ErrorCode::Unauthenticated,
        RefusalReason::REFUSAL_REASON_SIGN_IN_REQUIRED,
    );
}

#[tokio::test]
async fn a_member_managing_libraries_matches_or_users_is_permission_denied() {
    let (server, member_token) = start_with_member().await;

    for path in ADMIN_ONLY {
        let reply = post_json(&server, path, Some(&member_token)).await;

        assert_refused(
            reply.refused(),
            ErrorCode::PermissionDenied,
            RefusalReason::REFUSAL_REASON_ADMIN_REQUIRED,
        );
    }
}

#[tokio::test]
async fn an_unlisted_rpc_is_the_routers_not_found_with_or_without_a_session() {
    let server = TestServer::start(Config::default(), Registry::new()).await;

    for bearer in [None, Some(server.admin_token.as_str())] {
        let reply = post_json(&server, "/nama.v1.LibraryService/Unlisted", bearer).await;

        assert_eq!(reply.status, 404, "{bearer:?}");
        let body: Value = serde_json::from_slice(&reply.body).unwrap();
        assert_eq!(
            body,
            json!({
                "code": "unimplemented",
                "message": "method not found: nama.v1.LibraryService/Unlisted",
            }),
            "{bearer:?}"
        );
    }
}

#[tokio::test]
async fn an_admin_may_manage_libraries_matches_and_users() {
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
async fn a_member_may_browse() {
    let (server, member_token) = start_with_member().await;

    for path in BROWSE {
        let reply = post_json(&server, path, Some(&member_token)).await;

        // An empty request is wrong for them, but not for want of access.
        let code = reply.code();
        assert!(
            !["unauthenticated", "permission_denied"].contains(&code.as_str()),
            "{path}: {code}"
        );
    }
}

#[tokio::test]
async fn a_member_lists_libraries_without_their_folders() {
    let (server, member_token) = start_with_member().await;
    let folder = tempfile::tempdir().unwrap();
    LibraryServiceClient::new(HttpClient::plaintext(), server.client_config())
        .create_library(CreateLibraryRequest {
            name: "Movies".to_owned(),
            kind: LibraryKind::LIBRARY_KIND_MOVIES.into(),
            folders: vec![folder.path().to_str().unwrap().to_owned()],
            ..Default::default()
        })
        .await
        .unwrap();

    let listed = LibraryServiceClient::new(
        HttpClient::plaintext(),
        server.client_config_as(&member_token),
    )
    .list_libraries(ListLibrariesRequest::default())
    .await
    .unwrap()
    .into_owned();

    let [library] = listed.libraries.as_slice() else {
        panic!("expected one Library, got {:?}", listed.libraries);
    };
    assert_eq!(library.name, "Movies");
    assert!(library.folders.is_empty(), "{:?}", library.folders);
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
        "{}".to_owned(),
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
        let reply = send(&server, Method::OPTIONS, path, headers, String::new()).await;

        assert!(!(200..300).contains(&reply.status), "{}", reply.status);
        let approvals: Vec<_> = reply
            .headers
            .keys()
            .filter(|name| name.as_str().starts_with("access-control-allow-"))
            .collect();
        assert!(approvals.is_empty(), "{approvals:?}");
    }
}

#[tokio::test]
async fn a_device_that_just_set_up_the_server_needs_no_reauthentication() {
    let server = TestServer::start(Config::default(), Registry::new()).await;

    let refused = refused_for_recent_authentication(&server, &server.admin_token).await;

    assert!(refused.is_empty(), "{refused:?}");
}

#[tokio::test]
async fn ten_minutes_after_signing_in_only_the_sensitive_rpcs_need_recent_authentication() {
    let (server, db) = TestServer::start_with_db(Config::default(), Registry::new()).await;
    authenticated_long_ago(&db).await;

    for path in SENSITIVE {
        let reply = post_json(&server, path, Some(&server.admin_token)).await;
        let refusal = assert_refused(
            reply.refused(),
            ErrorCode::FailedPrecondition,
            RefusalReason::REFUSAL_REASON_RECENT_AUTHENTICATION_REQUIRED,
        );
        assert_eq!(refusal.field, "", "{path}");
    }
    let refused = refused_for_recent_authentication(&server, &server.admin_token).await;
    assert_eq!(refused, BTreeSet::from(SENSITIVE));
}

#[tokio::test]
async fn a_device_signed_in_by_a_sign_in_code_needs_recent_authentication_for_only_the_sensitive_rpcs()
 {
    let server = TestServer::start(Config::default(), Registry::new()).await;
    let tv = signed_in_by_code(&server).await;

    let refused = refused_for_recent_authentication(&server, &tv).await;

    assert_eq!(refused, BTreeSet::from(SENSITIVE));
}

#[tokio::test]
async fn reauthenticating_lets_the_same_session_make_sensitive_changes_without_a_new_cookie() {
    let server = TestServer::start(Config::default(), Registry::new()).await;
    let tv = signed_in_by_code(&server).await;
    let cookie = format!("nama_session={tv}");

    let reauthenticated = send(
        &server,
        Method::POST,
        "/nama.v1.SignInService/Reauthenticate",
        &[("content-type", "application/json"), ("cookie", &cookie)],
        json!({ "password": PASSWORD }).to_string(),
    )
    .await;

    assert_eq!(
        reauthenticated.status,
        200,
        "{}",
        String::from_utf8_lossy(&reauthenticated.body)
    );
    assert_eq!(reauthenticated.headers.get("set-cookie"), None);
    let refused = refused_for_recent_authentication(&server, &tv).await;
    assert!(refused.is_empty(), "{refused:?}");
}
