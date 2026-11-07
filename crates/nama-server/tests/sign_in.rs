//! Creating the first Admin with the Setup Code, signing Devices in and out, and finding out who
//! is signed in, over a real socket and against a real SQLite database.

use std::{
    io,
    net::SocketAddr,
    path::Path,
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};

use connectrpc::client::{CallOptions, ClientConfig, HttpClient, ServerStream};
use hyper::HeaderMap;
use nama_plugin_local_auth::LocalAuth;
use nama_proto::nama::v1::{
    __buffa::view::SubscribeResponseView, CreateLibraryRequest, EventServiceClient, LibraryKind,
    LibraryServiceClient, ScanLibraryRequest, SubscribeRequest, SubscribeResponse,
};
use nama_server::{config::Config, db::Database, plugins::Registry, users::SetupCode};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use tokio::{net::TcpListener, sync::oneshot};

const PASSWORD: &str = "correct horse battery staple";

/// A fresh Server whose Authenticator is `local-auth`, serving on a free port until dropped.
struct TestServer {
    addr: SocketAddr,
    /// `None` for a Server that started with Users.
    setup_code: Option<SetupCode>,
    data_dir: TempDir,
    _stop: oneshot::Sender<()>,
}

/// Starts a fresh [`TestServer`], which takes a Setup Code.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn start() -> TestServer {
    start_with(Some(SetupCode::generate().unwrap()), &Config::default()).await
}

/// Starts a [`TestServer`] that takes a Setup Code and trusts the forwarded headers of the test's
/// own address, as the reverse proxy in front of it, and of `10.0.0.0/8`, as proxies in front of
/// that one.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn start_behind_proxy() -> TestServer {
    let config = Config {
        trusted_proxies: vec!["127.0.0.1".parse().unwrap(), "10.0.0.0/8".parse().unwrap()],
        ..Config::default()
    };
    start_with(Some(SetupCode::generate().unwrap()), &config).await
}

/// Starts a [`TestServer`] serving `config`, that takes `setup_code`, or none, as one that started
/// with Users.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn start_with(setup_code: Option<SetupCode>, config: &Config) -> TestServer {
    let plugins = Registry::new();
    plugins.enable(Arc::new(LocalAuth)).unwrap();
    let data_dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (stop, stopped) = oneshot::channel::<()>();
    tokio::spawn(nama_server::serve(
        listener,
        config,
        db,
        plugins,
        setup_code.clone(),
        async {
            let _: Result<(), oneshot::error::RecvError> = stopped.await;
        },
    ));
    TestServer {
        addr,
        setup_code,
        data_dir,
        _stop: stop,
    }
}

/// A `Setup` request with the Server's Setup Code, asking for `delivery`.
fn setup_request(server: &TestServer, delivery: &str) -> Value {
    json!({
        "setupCode": server.setup_code.as_ref().map(ToString::to_string).unwrap_or_default(),
        "name": "Ada",
        "password": PASSWORD,
        "deviceName": "Laptop",
        "delivery": delivery,
    })
}

/// What the Server answered.
struct Reply {
    status: u16,
    headers: HeaderMap,
    body: Value,
}

/// Calls `method` of `SignInService` with a JSON `body` and the extra `headers`.
async fn call(server: &TestServer, method: &str, body: &Value, headers: &[(&str, &str)]) -> Reply {
    post_json(
        server.addr,
        &format!("/nama.v1.SignInService/{method}"),
        body,
        headers,
    )
    .await
}

/// What `GetServerInfo` answers.
async fn server_info(server: &TestServer) -> Reply {
    post_json(
        server.addr,
        "/nama.v1.ServerService/GetServerInfo",
        &json!({}),
        &[],
    )
    .await
}

/// Sends a bare HTTP/1.1 JSON `POST`, so the test sees every header the Server sets.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn post_json(
    addr: SocketAddr,
    path: &str,
    body: &Value,
    request_headers: &[(&str, &str)],
) -> Reply {
    use http_body_util::{BodyExt, Full};
    use hyper::{Request, body::Bytes, client::conn::http1};
    use hyper_util::rt::TokioIo;

    let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let (mut sender, connection) = http1::handshake(TokioIo::new(stream)).await.unwrap();
    tokio::spawn(connection);

    let mut request = Request::post(path)
        .header("host", addr.to_string())
        .header("content-type", "application/json");
    for (name, value) in request_headers {
        request = request.header(*name, *value);
    }
    let request = request
        .body(Full::new(Bytes::from(body.to_string())))
        .unwrap();
    let response = sender.send_request(request).await.unwrap();

    let status = response.status().as_u16();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    Reply {
        status,
        headers,
        body: serde_json::from_slice(&bytes).unwrap(),
    }
}

/// The token a `TOKEN` setup or sign-in handed out.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn token(reply: &Reply) -> String {
    assert_eq!(reply.status, 200, "{}", reply.body);
    reply
        .body
        .get("token")
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned()
}

#[tokio::test]
async fn setup_with_the_right_code_creates_an_admin_signed_in_on_the_device() {
    let server = start().await;

    let setup = call(
        &server,
        "Setup",
        &setup_request(&server, "SESSION_DELIVERY_TOKEN"),
        &[],
    )
    .await;
    let bearer = format!("Bearer {}", token(&setup));
    let me = call(&server, "WhoAmI", &json!({}), &[("authorization", &bearer)]).await;

    assert_eq!(me.status, 200, "{}", me.body);
    assert_eq!(me.body["user"]["name"], "Ada");
    assert_eq!(me.body["user"]["role"], "ROLE_ADMIN");
    assert_eq!(me.body["device"]["name"], "Laptop");
}

#[tokio::test]
async fn setup_with_a_wrong_code_is_refused() {
    let server = start().await;
    let mut request = setup_request(&server, "SESSION_DELIVERY_TOKEN");
    request["setupCode"] = json!("AAAA-AAAA-AAAA");

    let setup = call(&server, "Setup", &request, &[]).await;

    assert_eq!(setup.body["code"], "permission_denied", "{}", setup.body);
    let info = server_info(&server).await;
    assert_eq!(info.body["setupRequired"], true, "a User was created");
}

#[tokio::test]
async fn setup_once_a_user_exists_is_refused() {
    let server = start().await;
    let request = setup_request(&server, "SESSION_DELIVERY_TOKEN");
    call(&server, "Setup", &request, &[]).await;

    let again = call(&server, "Setup", &request, &[]).await;

    assert_eq!(again.body["code"], "failed_precondition", "{}", again.body);
}

#[tokio::test]
async fn setup_once_a_user_exists_is_refused_as_already_set_up_whatever_the_code() {
    let server = start().await;
    call(
        &server,
        "Setup",
        &setup_request(&server, "SESSION_DELIVERY_TOKEN"),
        &[],
    )
    .await;
    let mut request = setup_request(&server, "SESSION_DELIVERY_TOKEN");
    request["setupCode"] = json!("AAAA-AAAA-AAAA");

    let again = call(&server, "Setup", &request, &[]).await;

    assert_eq!(again.body["code"], "failed_precondition", "{}", again.body);
}

#[tokio::test]
async fn setup_on_a_server_that_started_with_users_is_refused_as_already_set_up() {
    let server = start_with(None, &Config::default()).await;

    let setup = call(
        &server,
        "Setup",
        &setup_request(&server, "SESSION_DELIVERY_TOKEN"),
        &[],
    )
    .await;

    assert_eq!(setup.body["code"], "failed_precondition", "{}", setup.body);
}

#[tokio::test]
async fn setup_is_required_until_the_first_admin_is_created() {
    let server = start().await;
    let before = server_info(&server).await;
    call(
        &server,
        "Setup",
        &setup_request(&server, "SESSION_DELIVERY_TOKEN"),
        &[],
    )
    .await;
    let after = server_info(&server).await;

    assert_eq!(before.body["setupRequired"], true);
    assert_eq!(
        after.body["setupRequired"],
        Value::Null,
        "false is left out of JSON"
    );
}

/// The token in a `COOKIE` setup's cookie, and the cookie's attributes in the order sent.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn session_cookie(reply: &Reply) -> (String, Vec<String>) {
    assert_eq!(reply.status, 200, "{}", reply.body);
    let cookie = reply.headers.get("set-cookie").unwrap().to_str().unwrap();
    let mut parts = cookie.split("; ");
    let token = parts.next().unwrap().strip_prefix("nama_session=").unwrap();
    (token.to_owned(), parts.map(str::to_owned).collect())
}

#[tokio::test]
async fn a_cookie_setup_sets_an_http_only_strict_cookie_and_leaves_the_token_out_of_the_body() {
    let server = start().await;

    let setup = call(
        &server,
        "Setup",
        &setup_request(&server, "SESSION_DELIVERY_COOKIE"),
        &[],
    )
    .await;

    let (token, attributes) = session_cookie(&setup);
    assert!(token.starts_with("nama_"), "{token}");
    // 90 days, as long as an unused Device lasts.
    assert_eq!(
        attributes,
        ["HttpOnly", "SameSite=Strict", "Path=/", "Max-Age=7776000"]
    );
    assert_eq!(setup.body.get("token"), None, "{}", setup.body);
}

#[tokio::test]
async fn a_cookie_setup_through_a_trusted_https_proxy_sets_a_secure_cookie() {
    let server = start_behind_proxy().await;

    let setup = call(
        &server,
        "Setup",
        &setup_request(&server, "SESSION_DELIVERY_COOKIE"),
        &[("x-forwarded-proto", "https")],
    )
    .await;

    let (_, attributes) = session_cookie(&setup);
    assert_eq!(
        attributes,
        [
            "HttpOnly",
            "SameSite=Strict",
            "Path=/",
            "Max-Age=7776000",
            "Secure"
        ]
    );
}

#[tokio::test]
async fn the_same_session_works_as_a_cookie_and_as_a_bearer_token() {
    let server = start().await;
    let setup = call(
        &server,
        "Setup",
        &setup_request(&server, "SESSION_DELIVERY_COOKIE"),
        &[],
    )
    .await;
    let (token, _) = session_cookie(&setup);

    let cookie = format!("theme=dark; nama_session={token}");
    let by_cookie = call(&server, "WhoAmI", &json!({}), &[("cookie", &cookie)]).await;
    // The scheme's name is case-insensitive (RFC 7235).
    let bearer = format!("bearer {token}");
    let by_bearer = call(&server, "WhoAmI", &json!({}), &[("authorization", &bearer)]).await;

    assert_eq!(by_cookie.status, 200, "{}", by_cookie.body);
    assert_eq!(by_cookie.body, by_bearer.body);
    assert_eq!(by_cookie.body["device"], setup.body["device"]);
}

#[tokio::test]
async fn who_am_i_without_a_valid_session_is_unauthenticated() {
    let server = start().await;
    call(
        &server,
        "Setup",
        &setup_request(&server, "SESSION_DELIVERY_TOKEN"),
        &[],
    )
    .await;

    let anonymous = call(&server, "WhoAmI", &json!({}), &[]).await;
    let made_up = call(
        &server,
        "WhoAmI",
        &json!({}),
        &[("authorization", "Bearer nama_not-a-real-token")],
    )
    .await;

    assert_eq!(
        anonymous.body["code"], "unauthenticated",
        "{}",
        anonymous.body
    );
    assert_eq!(made_up.body["code"], "unauthenticated", "{}", made_up.body);
}

#[tokio::test]
async fn setup_refuses_blank_names_and_names_that_are_too_long() {
    let server = start().await;
    let too_long = "é".repeat(65);
    let cases = [
        ("name", "   "),
        ("name", too_long.as_str()),
        ("deviceName", too_long.as_str()),
    ];

    for (field, value) in cases {
        let mut request = setup_request(&server, "SESSION_DELIVERY_TOKEN");
        request[field] = json!(value);

        let setup = call(&server, "Setup", &request, &[]).await;

        assert_eq!(
            setup.body["code"], "invalid_argument",
            "{field} {value:?}: {}",
            setup.body
        );
    }
}

#[tokio::test]
async fn setup_trims_the_name_and_accepts_64_characters() {
    let server = start().await;
    let name = "é".repeat(64);
    let mut request = setup_request(&server, "SESSION_DELIVERY_TOKEN");
    request["name"] = json!(format!("  {name}\t"));
    request["deviceName"] = json!(name);

    let setup = call(&server, "Setup", &request, &[]).await;

    assert_eq!(setup.body["user"]["name"], name.as_str(), "{}", setup.body);
    assert_eq!(setup.body["device"]["name"], name.as_str());
}

/// Every byte of the database's files, including any write-ahead log not yet checkpointed.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn database_bytes(data_dir: &Path) -> Vec<u8> {
    let mut bytes = Vec::new();
    for entry in std::fs::read_dir(data_dir).unwrap() {
        bytes.extend(std::fs::read(entry.unwrap().path()).unwrap());
    }
    bytes
}

/// Whether `needle` occurs in `haystack`.
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

#[tokio::test]
async fn the_database_keeps_the_token_hash_and_the_credential_but_not_the_token_or_password() {
    let server = start().await;
    let setup = call(
        &server,
        "Setup",
        &setup_request(&server, "SESSION_DELIVERY_TOKEN"),
        &[],
    )
    .await;
    let token = token(&setup);

    let stored = database_bytes(server.data_dir.path());

    assert!(
        !contains(&stored, PASSWORD.as_bytes()),
        "the password is stored"
    );
    assert!(!contains(&stored, token.as_bytes()), "the token is stored");
    assert!(
        contains(&stored, &Sha256::digest(&token)),
        "the token's hash is missing"
    );
    assert!(
        contains(&stored, b"$argon2id$"),
        "the credential is missing"
    );
}

/// Everything logged through a subscriber that writes here.
#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<u8>>>);

impl Log {
    fn contents(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap_or_else(PoisonError::into_inner)).into_owned()
    }
}

impl io::Write for Log {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// The subscriber is global, because a thread-local one races with other tests over which
/// callsites are enabled. So it also sees what the other tests here log, which use the same password.
#[tokio::test]
async fn no_password_token_or_setup_code_is_logged_even_at_trace() {
    let log = Log::default();
    let writer = log.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(move || writer.clone())
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();
    let server = start().await;
    let wrong_code = "WRONG-CODE-0000";
    let mut wrong_setup = setup_request(&server, "SESSION_DELIVERY_TOKEN");
    wrong_setup["setupCode"] = json!(wrong_code);
    call(&server, "Setup", &wrong_setup, &[]).await;

    let setup = call(
        &server,
        "Setup",
        &setup_request(&server, "SESSION_DELIVERY_TOKEN"),
        &[],
    )
    .await;
    let token = token(&setup);
    let bearer = format!("Bearer {token}");
    call(&server, "WhoAmI", &json!({}), &[("authorization", &bearer)]).await;
    let wrong = "a wrong password";
    let refused = sign_in_request("Ada", wrong, "SESSION_DELIVERY_TOKEN");
    call(&server, "SignIn", &refused, &[]).await;
    let signed_in = sign_in(&server, "SESSION_DELIVERY_COOKIE").await;
    let (cookie_token, _) = session_cookie(&signed_in);
    let cookie = format!("nama_session={cookie_token}");
    call(&server, "SignOut", &json!({}), &[("cookie", &cookie)]).await;
    let wrong_sign_in_code = "WRNG-C0DE";
    with_code(&server, "LookUpSignInCode", wrong_sign_in_code, &token).await;
    let (code, handle) = start_sign_in_code(&server, "SESSION_DELIVERY_TOKEN").await;
    with_code(&server, "ApproveSignInCode", &code, &token).await;
    let code_token = self::token(&poll(&server, &handle).await);
    call(
        &server,
        "SignOut",
        &json!({}),
        &[("authorization", &bearer)],
    )
    .await;

    let logged = log.contents();
    assert!(
        logged.contains("enabled plugin"),
        "nothing was captured: {logged}"
    );
    assert!(!logged.contains(PASSWORD), "the password was logged");
    assert!(!logged.contains(wrong), "a wrong password was logged");
    assert!(
        !logged.contains(wrong_code),
        "a wrong Setup Code was logged"
    );
    let setup_code = server.setup_code.as_ref().map(ToString::to_string);
    assert!(
        !logged.contains(&setup_code.unwrap_or_default()),
        "the Setup Code was logged"
    );
    assert!(!logged.contains(&token), "the token was logged");
    assert!(
        !logged.contains(&cookie_token),
        "the cookie's token was logged"
    );
    assert!(
        !logged.contains(&code_token),
        "a Sign-in Code's token was logged"
    );
    // A Sign-in Code matches without its dash, so it mustn't be logged in either form.
    let logs_sign_in_code =
        |shown: &str| logged.contains(shown) || logged.contains(&shown.replace('-', ""));
    assert!(!logs_sign_in_code(&code), "a Sign-in Code was logged");
    assert!(
        !logs_sign_in_code(wrong_sign_in_code),
        "a wrong Sign-in Code was logged"
    );
    assert!(!logged.contains(&handle), "a poll handle was logged");
}

/// Creates the first Admin, Ada, signed in on the Device "Laptop" with a token.
async fn set_up(server: &TestServer) -> Reply {
    call(
        server,
        "Setup",
        &setup_request(server, "SESSION_DELIVERY_TOKEN"),
        &[],
    )
    .await
}

/// Signs Ada in on the Device "Phone" with her password, asking for `delivery`.
async fn sign_in(server: &TestServer, delivery: &str) -> Reply {
    call(
        server,
        "SignIn",
        &sign_in_request("Ada", PASSWORD, delivery),
        &[],
    )
    .await
}

/// A `SignIn` request as `name` with `password`, asking for `delivery`.
fn sign_in_request(name: &str, password: &str, delivery: &str) -> Value {
    json!({
        "name": name,
        "password": password,
        "deviceName": "Phone",
        "delivery": delivery,
    })
}

#[tokio::test]
async fn sign_in_with_the_right_password_works_with_the_name_in_any_case() {
    let server = start().await;
    set_up(&server).await;

    let signed_in = call(
        &server,
        "SignIn",
        &sign_in_request("aDA", PASSWORD, "SESSION_DELIVERY_TOKEN"),
        &[],
    )
    .await;
    let bearer = format!("Bearer {}", token(&signed_in));
    let me = call(&server, "WhoAmI", &json!({}), &[("authorization", &bearer)]).await;

    assert_eq!(me.body["user"]["name"], "Ada", "{}", me.body);
    assert_eq!(me.body["device"]["name"], "Phone");
}

#[tokio::test]
async fn a_device_that_gives_no_name_is_named_after_its_browser_and_os() {
    const MAC_CHROME: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
        (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
    let server = start().await;
    let mut setup = setup_request(&server, "SESSION_DELIVERY_TOKEN");
    setup["deviceName"] = json!("");
    let mut sign_in = sign_in_request("Ada", PASSWORD, "SESSION_DELIVERY_TOKEN");
    sign_in["deviceName"] = json!("  ");

    let set_up = call(&server, "Setup", &setup, &[("user-agent", MAC_CHROME)]).await;
    let signed_in = call(&server, "SignIn", &sign_in, &[("user-agent", MAC_CHROME)]).await;

    assert_eq!(
        set_up.body["device"]["name"], "Chrome on macOS",
        "{}",
        set_up.body
    );
    assert_eq!(
        signed_in.body["device"]["name"], "Chrome on macOS",
        "{}",
        signed_in.body
    );
}

#[tokio::test]
async fn sign_in_with_a_wrong_password_is_refused() {
    let server = start().await;
    set_up(&server).await;

    let refused = call(
        &server,
        "SignIn",
        &sign_in_request("Ada", "not the password", "SESSION_DELIVERY_TOKEN"),
        &[],
    )
    .await;

    assert_eq!(refused.body["code"], "unauthenticated", "{}", refused.body);
}

#[tokio::test]
async fn sign_in_with_an_unknown_name_is_refused_the_same_way_as_a_wrong_password() {
    let server = start().await;
    set_up(&server).await;

    let wrong_password = call(
        &server,
        "SignIn",
        &sign_in_request("Ada", "not the password", "SESSION_DELIVERY_TOKEN"),
        &[],
    )
    .await;
    let unknown_name = call(
        &server,
        "SignIn",
        &sign_in_request("Grace", PASSWORD, "SESSION_DELIVERY_TOKEN"),
        &[],
    )
    .await;

    assert_eq!(
        (unknown_name.status, unknown_name.body),
        (wrong_password.status, wrong_password.body)
    );
}

#[tokio::test]
async fn a_cookie_sign_in_sets_the_cookie_and_leaves_the_token_out_of_the_body() {
    let server = start().await;
    set_up(&server).await;

    let signed_in = sign_in(&server, "SESSION_DELIVERY_COOKIE").await;

    let (token, _) = session_cookie(&signed_in);
    assert!(token.starts_with("nama_"), "{token}");
    assert_eq!(signed_in.body.get("token"), None, "{}", signed_in.body);
}

#[tokio::test]
async fn signing_out_makes_the_token_stop_working_at_once() {
    let server = start().await;
    let bearer = format!("Bearer {}", token(&set_up(&server).await));
    let auth = [("authorization", bearer.as_str())];

    let signed_out = call(&server, "SignOut", &json!({}), &auth).await;
    let me = call(&server, "WhoAmI", &json!({}), &auth).await;

    assert_eq!(signed_out.status, 200, "{}", signed_out.body);
    assert_eq!(me.body["code"], "unauthenticated", "{}", me.body);
}

#[tokio::test]
async fn signing_out_a_cookie_session_clears_the_cookie() {
    let server = start().await;
    set_up(&server).await;
    let signed_in = sign_in(&server, "SESSION_DELIVERY_COOKIE").await;
    let (token, _) = session_cookie(&signed_in);
    let cookie = format!("nama_session={token}");

    let signed_out = call(&server, "SignOut", &json!({}), &[("cookie", &cookie)]).await;

    let (cleared, attributes) = session_cookie(&signed_out);
    assert_eq!(cleared, "");
    assert!(
        attributes.contains(&"Max-Age=0".to_owned()),
        "{attributes:?}"
    );
}

#[tokio::test]
async fn signing_out_a_bearer_session_sets_no_cookie() {
    let server = start().await;
    let bearer = format!("Bearer {}", token(&set_up(&server).await));

    let signed_out = call(
        &server,
        "SignOut",
        &json!({}),
        &[("authorization", &bearer)],
    )
    .await;

    assert_eq!(signed_out.headers.get("set-cookie"), None);
}

#[tokio::test]
async fn signing_out_without_a_session_is_unauthenticated() {
    let server = start().await;

    let signed_out = call(&server, "SignOut", &json!({}), &[]).await;

    assert_eq!(
        signed_out.body["code"], "unauthenticated",
        "{}",
        signed_out.body
    );
}

/// How long a test waits for a stream before failing.
const WAIT: Duration = Duration::from_secs(10);

/// A subscription to the Server's events.
type Subscription = ServerStream<hyper::body::Incoming, SubscribeResponseView<'static>>;

/// Client settings that point at `server`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn client_config(server: &TestServer) -> ClientConfig {
    ClientConfig::new(format!("http://{}", server.addr).parse().unwrap())
}

/// Call options that send `token` as a bearer token.
fn bearer(token: &str) -> CallOptions {
    CallOptions::default().with_header("authorization", format!("Bearer {token}"))
}

/// Subscribes to every event with the session `token`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn subscribe(server: &TestServer, token: &str) -> Subscription {
    EventServiceClient::new(HttpClient::plaintext(), client_config(server))
        .subscribe_with_options(SubscribeRequest::default(), bearer(token))
        .await
        .unwrap()
}

/// The next message on `subscription`, or `None` if it ended.
#[expect(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn next_message(subscription: &mut Subscription) -> Option<SubscribeResponse> {
    let message = tokio::time::timeout(WAIT, subscription.message::<SubscribeResponse>())
        .await
        .expect("timed out waiting for the stream")
        .unwrap()?;
    Some(message.to_owned_message())
}

/// Creates a Library over an empty folder and scans it, so every subscriber gets a
/// `ScanStarted`. Returns the folder.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn scan_a_library(server: &TestServer, token: &str) -> TempDir {
    let folder = tempfile::tempdir().unwrap();
    let libraries = LibraryServiceClient::new(HttpClient::plaintext(), client_config(server));
    let create = CreateLibraryRequest {
        name: "Movies".to_owned(),
        kind: LibraryKind::LIBRARY_KIND_MOVIES.into(),
        folders: vec![folder.path().to_str().unwrap().to_owned()],
        ..Default::default()
    };
    let library = libraries
        .create_library_with_options(create, bearer(token))
        .await
        .unwrap()
        .into_owned()
        .library
        .into_option()
        .unwrap();
    let scan = ScanLibraryRequest {
        id: library.id,
        ..Default::default()
    };
    libraries
        .scan_library_with_options(scan, bearer(token))
        .await
        .unwrap();
    folder
}

#[tokio::test]
async fn signing_out_ends_that_devices_event_streams_and_leaves_other_devices_open() {
    let server = start().await;
    let laptop = token(&set_up(&server).await);
    let phone = token(&sign_in(&server, "SESSION_DELIVERY_TOKEN").await);
    let mut laptop_events = subscribe(&server, &laptop).await;
    let mut phone_events = subscribe(&server, &phone).await;

    let bearer = format!("Bearer {laptop}");
    call(
        &server,
        "SignOut",
        &json!({}),
        &[("authorization", &bearer)],
    )
    .await;
    let laptop_next = next_message(&mut laptop_events).await;
    let _folder = scan_a_library(&server, &phone).await;
    let phone_next = next_message(&mut phone_events).await;

    assert_eq!(laptop_next, None, "the signed-out Device's stream is open");
    assert!(phone_next.is_some(), "the other Device's stream ended");
}

/// Fails to sign in as `name` with a wrong password `times` times.
#[expect(
    clippy::indexing_slicing,
    reason = "a test helper fails its test by panicking"
)]
async fn fail_sign_in(server: &TestServer, name: &str, times: usize) {
    let request = sign_in_request(name, "not the password", "SESSION_DELIVERY_TOKEN");
    for _ in 0..times {
        let refused = call(server, "SignIn", &request, &[]).await;
        assert_eq!(refused.body["code"], "unauthenticated", "{}", refused.body);
    }
}

/// Asserts `reply` was refused as rate limited, with a retry delay of at most 15 minutes.
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a test helper fails its test by panicking"
)]
fn assert_blocked(reply: &Reply) {
    assert_eq!(reply.body["code"], "resource_exhausted", "{}", reply.body);
    let retry_after: u64 = reply
        .headers
        .get("retry-after")
        .unwrap()
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!((1..=15 * 60).contains(&retry_after), "{retry_after}");
}

#[tokio::test]
async fn the_11th_failed_sign_in_for_a_name_is_blocked_even_with_the_right_password() {
    let server = start().await;
    set_up(&server).await;
    fail_sign_in(&server, "Ada", 10).await;

    let blocked = call(
        &server,
        "SignIn",
        &sign_in_request("aDA", PASSWORD, "SESSION_DELIVERY_TOKEN"),
        &[],
    )
    .await;

    assert_blocked(&blocked);
}

#[tokio::test]
async fn failures_for_one_name_dont_block_another_name() {
    let server = start().await;
    set_up(&server).await;
    fail_sign_in(&server, "Grace", 10).await;

    let signed_in = sign_in(&server, "SESSION_DELIVERY_TOKEN").await;

    assert_eq!(signed_in.status, 200, "{}", signed_in.body);
}

#[tokio::test]
async fn x_forwarded_proto_from_an_untrusted_peer_doesnt_set_a_secure_cookie() {
    let server = start().await;

    let setup = call(
        &server,
        "Setup",
        &setup_request(&server, "SESSION_DELIVERY_COOKIE"),
        &[("x-forwarded-proto", "https")],
    )
    .await;

    let (_, attributes) = session_cookie(&setup);
    assert_eq!(
        attributes,
        ["HttpOnly", "SameSite=Strict", "Path=/", "Max-Age=7776000"]
    );
}

#[tokio::test]
async fn a_successful_sign_in_resets_that_names_failures() {
    let server = start().await;
    set_up(&server).await;
    fail_sign_in(&server, "Ada", 9).await;
    sign_in(&server, "SESSION_DELIVERY_TOKEN").await;
    fail_sign_in(&server, "Ada", 9).await;

    let signed_in = sign_in(&server, "SESSION_DELIVERY_TOKEN").await;

    assert_eq!(signed_in.status, 200, "{}", signed_in.body);
}

/// Fails to set up with a wrong Setup Code `times` times, the `i`th time sending
/// `X-Forwarded-For: forwarded_for(i)`.
#[expect(
    clippy::indexing_slicing,
    reason = "a test helper fails its test by panicking"
)]
async fn fail_setup(server: &TestServer, times: usize, forwarded_for: impl Fn(usize) -> String) {
    let mut request = setup_request(server, "SESSION_DELIVERY_TOKEN");
    request["setupCode"] = json!("AAAA-AAAA-AAAA");
    for i in 0..times {
        let forwarded_for = forwarded_for(i);
        let headers = [("x-forwarded-for", forwarded_for.as_str())];
        let refused = call(server, "Setup", &request, &headers).await;
        assert_eq!(
            refused.body["code"], "permission_denied",
            "{}",
            refused.body
        );
    }
}

#[tokio::test]
async fn the_51st_failure_from_one_ip_mixing_sign_in_and_setup_is_blocked() {
    let server = start().await;
    fail_setup(&server, 40, |i| format!("203.0.113.{i}")).await;
    fail_sign_in(&server, "Grace", 10).await;

    let blocked = set_up(&server).await;

    assert_blocked(&blocked);
}

#[tokio::test]
async fn x_forwarded_for_from_an_untrusted_peer_doesnt_change_which_ip_is_counted() {
    let server = start().await;
    fail_setup(&server, 50, |i| format!("203.0.113.{i}")).await;

    let blocked = call(
        &server,
        "Setup",
        &setup_request(&server, "SESSION_DELIVERY_TOKEN"),
        &[("x-forwarded-for", "198.51.100.1")],
    )
    .await;

    assert_blocked(&blocked);
}

#[tokio::test]
async fn through_a_trusted_proxy_failures_count_against_the_forwarded_ip() {
    let server = start_behind_proxy().await;
    fail_setup(&server, 50, |_| "203.0.113.7".to_owned()).await;

    let blocked = call(
        &server,
        "Setup",
        &setup_request(&server, "SESSION_DELIVERY_TOKEN"),
        &[("x-forwarded-for", "203.0.113.7")],
    )
    .await;
    let set_up = call(
        &server,
        "Setup",
        &setup_request(&server, "SESSION_DELIVERY_TOKEN"),
        &[("x-forwarded-for", "203.0.113.8")],
    )
    .await;

    assert_blocked(&blocked);
    assert_eq!(set_up.status, 200, "{}", set_up.body);
}

#[tokio::test]
async fn a_forged_x_forwarded_for_chain_counts_against_the_ip_the_trusted_proxies_saw() {
    let server = start_behind_proxy().await;
    // The client claims an address of its choosing; the proxies append the one they saw.
    fail_setup(&server, 50, |i| {
        format!("203.0.113.{i}, 198.51.100.1, 10.0.0.5")
    })
    .await;

    let blocked = call(
        &server,
        "Setup",
        &setup_request(&server, "SESSION_DELIVERY_TOKEN"),
        &[("x-forwarded-for", "10.0.0.9, 198.51.100.1, 10.0.0.5")],
    )
    .await;

    assert_blocked(&blocked);
}

/// Starts a Sign-in Code sign-in for the Device "Living room TV", asking for `delivery`, and
/// returns the code and the poll handle.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn start_sign_in_code(server: &TestServer, delivery: &str) -> (String, String) {
    let request = json!({ "deviceName": "Living room TV", "delivery": delivery });
    let started = call(server, "StartSignInCode", &request, &[]).await;
    assert_eq!(started.status, 200, "{}", started.body);
    let field = |name: &str| started.body.get(name).unwrap().as_str().unwrap().to_owned();
    (field("code"), field("handle"))
}

/// Calls `method`, `LookUpSignInCode` or `ApproveSignInCode`, with `code` as the Device whose
/// session is `token`.
async fn with_code(server: &TestServer, method: &str, code: &str, token: &str) -> Reply {
    let bearer = format!("Bearer {token}");
    call(
        server,
        method,
        &json!({ "code": code }),
        &[("authorization", &bearer)],
    )
    .await
}

/// Polls with `handle`.
async fn poll(server: &TestServer, handle: &str) -> Reply {
    call(server, "PollSignInCode", &json!({ "handle": handle }), &[]).await
}

#[tokio::test]
async fn polling_an_approved_code_signs_the_device_in_as_the_approving_user() {
    let server = start().await;
    let ada = token(&set_up(&server).await);
    let (code, handle) = start_sign_in_code(&server, "SESSION_DELIVERY_TOKEN").await;

    let approved = with_code(&server, "ApproveSignInCode", &code, &ada).await;
    let polled = poll(&server, &handle).await;
    let bearer = format!("Bearer {}", token(&polled));
    let me = call(&server, "WhoAmI", &json!({}), &[("authorization", &bearer)]).await;

    assert_eq!(approved.status, 200, "{}", approved.body);
    assert_eq!(polled.body["status"], "SIGN_IN_CODE_STATUS_SIGNED_IN");
    assert_eq!(me.body["user"]["name"], "Ada", "{}", me.body);
    assert_eq!(me.body["device"]["name"], "Living room TV");
    assert_eq!(me.body["device"], polled.body["device"]);
}

#[tokio::test]
async fn polling_again_after_signing_in_finds_nothing() {
    let server = start().await;
    let ada = token(&set_up(&server).await);
    let (code, handle) = start_sign_in_code(&server, "SESSION_DELIVERY_TOKEN").await;
    with_code(&server, "ApproveSignInCode", &code, &ada).await;
    poll(&server, &handle).await;

    let again = poll(&server, &handle).await;

    assert_eq!(again.body["code"], "not_found", "{}", again.body);
}

#[tokio::test]
async fn looking_up_a_code_shows_the_device_name_without_approving_it() {
    let server = start().await;
    let ada = token(&set_up(&server).await);
    let (code, handle) = start_sign_in_code(&server, "SESSION_DELIVERY_TOKEN").await;

    let looked_up = with_code(&server, "LookUpSignInCode", &code, &ada).await;
    let polled = poll(&server, &handle).await;

    assert_eq!(
        looked_up.body["deviceName"], "Living room TV",
        "{}",
        looked_up.body
    );
    assert_eq!(
        polled.body["status"], "SIGN_IN_CODE_STATUS_WAITING",
        "{}",
        polled.body
    );
}

#[tokio::test]
async fn a_code_matches_typed_in_lower_case_and_without_its_dash() {
    let server = start().await;
    let ada = token(&set_up(&server).await);
    let (code, handle) = start_sign_in_code(&server, "SESSION_DELIVERY_TOKEN").await;
    let typed = code.replace('-', "").to_lowercase();

    let approved = with_code(&server, "ApproveSignInCode", &typed, &ada).await;
    let polled = poll(&server, &handle).await;

    assert_eq!(approved.status, 200, "{}", approved.body);
    assert_eq!(polled.body["status"], "SIGN_IN_CODE_STATUS_SIGNED_IN");
}

#[tokio::test]
async fn an_unknown_code_cant_be_looked_up_or_approved() {
    let server = start().await;
    let ada = token(&set_up(&server).await);

    let looked_up = with_code(&server, "LookUpSignInCode", "AAAA-AAAA", &ada).await;
    let approved = with_code(&server, "ApproveSignInCode", "AAAA-AAAA", &ada).await;

    assert_eq!(looked_up.body["code"], "not_found", "{}", looked_up.body);
    assert_eq!(approved.body["code"], "not_found", "{}", approved.body);
}

#[tokio::test]
async fn a_cookie_poll_sets_the_cookie_and_leaves_the_token_out_of_the_body() {
    let server = start().await;
    let ada = token(&set_up(&server).await);
    let (code, handle) = start_sign_in_code(&server, "SESSION_DELIVERY_COOKIE").await;
    with_code(&server, "ApproveSignInCode", &code, &ada).await;

    let polled = poll(&server, &handle).await;

    let (token, _) = session_cookie(&polled);
    assert!(token.starts_with("nama_"), "{token}");
    assert_eq!(polled.body.get("token"), None, "{}", polled.body);
}

#[tokio::test]
async fn looking_up_or_approving_a_code_without_a_session_is_unauthenticated() {
    let server = start().await;
    set_up(&server).await;
    let (code, handle) = start_sign_in_code(&server, "SESSION_DELIVERY_TOKEN").await;
    let request = json!({ "code": code });

    let looked_up = call(&server, "LookUpSignInCode", &request, &[]).await;
    let approved = call(&server, "ApproveSignInCode", &request, &[]).await;
    let polled = poll(&server, &handle).await;

    assert_eq!(
        looked_up.body["code"], "unauthenticated",
        "{}",
        looked_up.body
    );
    assert_eq!(
        approved.body["code"], "unauthenticated",
        "{}",
        approved.body
    );
    assert_eq!(polled.body["status"], "SIGN_IN_CODE_STATUS_WAITING");
}

#[tokio::test]
async fn the_database_keeps_only_the_hash_of_a_token_a_sign_in_code_handed_out() {
    let server = start().await;
    let ada = token(&set_up(&server).await);
    let (code, handle) = start_sign_in_code(&server, "SESSION_DELIVERY_TOKEN").await;
    with_code(&server, "ApproveSignInCode", &code, &ada).await;
    let token = token(&poll(&server, &handle).await);

    let stored = database_bytes(server.data_dir.path());

    assert!(!contains(&stored, token.as_bytes()), "the token is stored");
    assert!(
        contains(&stored, &Sha256::digest(&token)),
        "the token's hash is missing"
    );
}

/// Fails `times` times to call `method`, `LookUpSignInCode` or `ApproveSignInCode`, with an
/// unknown code, as the Device whose session is `token`.
#[expect(
    clippy::indexing_slicing,
    reason = "a test helper fails its test by panicking"
)]
async fn fail_with_code(server: &TestServer, method: &str, token: &str, times: usize) {
    for _ in 0..times {
        let refused = with_code(server, method, "AAAA-AAAA", token).await;
        assert_eq!(refused.body["code"], "not_found", "{}", refused.body);
    }
}

#[tokio::test]
async fn the_11th_failed_code_lookup_from_a_device_is_blocked_even_for_a_valid_code() {
    let server = start().await;
    let ada = token(&set_up(&server).await);
    let (code, _) = start_sign_in_code(&server, "SESSION_DELIVERY_TOKEN").await;
    fail_with_code(&server, "LookUpSignInCode", &ada, 10).await;

    let blocked = with_code(&server, "LookUpSignInCode", &code, &ada).await;

    assert_blocked(&blocked);
}

#[tokio::test]
async fn failed_code_lookups_and_approvals_count_towards_the_same_limit() {
    let server = start().await;
    let ada = token(&set_up(&server).await);
    let (code, _) = start_sign_in_code(&server, "SESSION_DELIVERY_TOKEN").await;
    fail_with_code(&server, "LookUpSignInCode", &ada, 5).await;
    fail_with_code(&server, "ApproveSignInCode", &ada, 5).await;

    let blocked = with_code(&server, "ApproveSignInCode", &code, &ada).await;

    assert_blocked(&blocked);
}

#[tokio::test]
async fn failed_code_lookups_from_one_device_dont_block_another_device() {
    let server = start().await;
    let laptop = token(&set_up(&server).await);
    let phone = token(&sign_in(&server, "SESSION_DELIVERY_TOKEN").await);
    let (code, _) = start_sign_in_code(&server, "SESSION_DELIVERY_TOKEN").await;
    fail_with_code(&server, "LookUpSignInCode", &laptop, 10).await;

    let approved = with_code(&server, "ApproveSignInCode", &code, &phone).await;

    assert_eq!(approved.status, 200, "{}", approved.body);
}

#[tokio::test]
async fn looking_up_an_approved_code_again_isnt_counted_as_a_failure() {
    let server = start().await;
    let ada = token(&set_up(&server).await);
    let (code, _) = start_sign_in_code(&server, "SESSION_DELIVERY_TOKEN").await;
    with_code(&server, "ApproveSignInCode", &code, &ada).await;
    for _ in 0..10 {
        with_code(&server, "LookUpSignInCode", &code, &ada).await;
    }

    let again = with_code(&server, "LookUpSignInCode", &code, &ada).await;

    assert_eq!(again.body["code"], "failed_precondition", "{}", again.body);
}
