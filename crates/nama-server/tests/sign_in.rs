//! Creating the first Admin with the Setup Code, and finding out who is signed in, over a real
//! socket and against a real SQLite database.

use std::{
    io,
    net::SocketAddr,
    path::Path,
    sync::{Arc, Mutex, PoisonError},
};

use hyper::HeaderMap;
use nama_plugin_local_auth::LocalAuth;
use nama_server::{config::Config, db::Database, plugins::Registry, users::SetupCode};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use tokio::{net::TcpListener, sync::oneshot};

const PASSWORD: &str = "correct horse battery staple";

/// A fresh Server whose Authenticator is `local-auth`, serving on a free port until dropped.
struct TestServer {
    addr: SocketAddr,
    setup_code: SetupCode,
    data_dir: TempDir,
    _stop: oneshot::Sender<()>,
}

/// Starts a [`TestServer`].
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn start() -> TestServer {
    let plugins = Registry::new();
    plugins.enable(Arc::new(LocalAuth)).unwrap();
    let data_dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let setup_code = SetupCode::generate().unwrap();
    let (stop, stopped) = oneshot::channel::<()>();
    tokio::spawn(nama_server::serve(
        listener,
        &Config::default(),
        db,
        plugins,
        Some(setup_code.clone()),
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
        "setupCode": server.setup_code.to_string(),
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

/// The token a `TOKEN` setup handed out.
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
    assert_eq!(attributes, ["HttpOnly", "SameSite=Strict", "Path=/"]);
    assert_eq!(setup.body.get("token"), None, "{}", setup.body);
}

#[tokio::test]
async fn a_cookie_setup_behind_an_https_proxy_sets_a_secure_cookie() {
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
        ["HttpOnly", "SameSite=Strict", "Path=/", "Secure"]
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
    let bearer = format!("Bearer {token}");
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
async fn setup_refuses_names_and_device_names_that_are_blank_or_too_long() {
    let server = start().await;
    let too_long = "é".repeat(65);
    let cases = [
        ("name", "   "),
        ("name", too_long.as_str()),
        ("deviceName", ""),
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
async fn neither_the_password_nor_the_token_is_logged_even_at_trace() {
    let log = Log::default();
    let writer = log.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(move || writer.clone())
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();
    let server = start().await;

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

    let logged = log.contents();
    assert!(
        logged.contains("enabled plugin"),
        "nothing was captured: {logged}"
    );
    assert!(!logged.contains(PASSWORD), "the password was logged");
    assert!(!logged.contains(&token), "the token was logged");
}
