//! An in-process stand-in for the Hub, so the tests need no network and no PostgreSQL. It serves
//! one Server's device flow, Unlinking, and a key set of one Ed25519 key it signs with, as the
//! Hub's API (`proto/nama/hub/v1/link.proto`) says the Hub does.

use std::{
    net::SocketAddr,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::{SystemTime, UNIX_EPOCH},
};

use axum::http::StatusCode;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use nama_hub_proto::nama::hub::v1::{
    LinkedServerService, PollServerLinkRequest, PollServerLinkResponse, ServerLinkService,
    StartServerLinkRequest, StartServerLinkResponse, UnlinkServerRequest, UnlinkServerResponse,
};
use ring::signature::{Ed25519KeyPair, KeyPair as _};
use serde_json::{Value, json};
use tokio::{net::TcpListener, task::JoinHandle};

/// The one device code the fake hands out.
const DEVICE_CODE: &str = "device-code";

/// The one code the fake shows.
pub(super) const USER_CODE: &str = "BDFH-JKLM";

/// The credential the fake hands out once the code is entered.
pub(super) const CREDENTIAL: &str = "the-credential";

/// The email of the Account the code is entered by.
pub(super) const EMAIL: &str = "ada@example.com";

/// What the fake Hub knows.
#[derive(Default)]
struct State {
    /// The name the Server started with, while its code waits.
    starting: Option<String>,
    entered: bool,
    /// Whether the Server holds a credential the fake knows.
    linked: bool,
    key_set_fetches: usize,
    /// What the key set answers in place of the keys, if anything.
    key_set_failure: Option<StatusCode>,
}

/// The Hub, serving until [`FakeHub::stop`] or its drop.
pub(super) struct FakeHub {
    addr: SocketAddr,
    state: Arc<Mutex<State>>,
    key: Arc<Ed25519KeyPair>,
    serving: JoinHandle<()>,
}

impl FakeHub {
    pub(super) async fn start() -> Self {
        let state = Arc::default();
        let key = Arc::new(Ed25519KeyPair::from_seed_unchecked(&[7; 32]).unwrap());
        let shared = Arc::new(Shared {
            state: Arc::clone(&state),
            key: Arc::clone(&key),
        });
        let rpcs = connectrpc::Router::new()
            .add_service(Arc::new(Starting(Arc::clone(&shared))))
            .add_service(Arc::new(Linked(shared)))
            .into_axum_service();
        let key_set = key_set(&key);
        let key_set_state = Arc::clone(&state);
        let app = axum::Router::new()
            .route(
                "/.well-known/jwks.json",
                axum::routing::get(async move || {
                    let mut fetched = lock(&key_set_state);
                    fetched.key_set_fetches += 1;
                    match fetched.key_set_failure {
                        Some(failure) => (failure, String::new()),
                        None => (StatusCode::OK, key_set.to_string()),
                    }
                }),
            )
            .fallback_service(rpcs);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let serving = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self {
            addr,
            state,
            key,
            serving,
        }
    }

    pub(super) fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Someone enters the code on the Hub.
    pub(super) fn enter_code(&self) {
        lock(&self.state).entered = true;
    }

    /// Has the key set fail, as if the Hub went down right after handing out a credential.
    pub(super) fn break_key_set(&self) {
        lock(&self.state).key_set_failure = Some(StatusCode::INTERNAL_SERVER_ERROR);
    }

    /// The name the waiting Server started with.
    pub(super) fn starting(&self) -> Option<String> {
        lock(&self.state).starting.clone()
    }

    /// Whether the Server holds a credential the Hub still knows.
    pub(super) fn linked(&self) -> bool {
        lock(&self.state).linked
    }

    /// How often the key set was fetched.
    pub(super) fn key_set_fetches(&self) -> usize {
        lock(&self.state).key_set_fetches
    }

    /// `claims` signed as the Hub signs them.
    pub(super) fn sign(&self, claims: &Value) -> String {
        sign(&self.key, claims)
    }

    /// Goes down, as if unreachable.
    pub(super) fn stop(&self) {
        self.serving.abort();
    }
}

impl Drop for FakeHub {
    fn drop(&mut self) {
        self.serving.abort();
    }
}

struct Shared {
    state: Arc<Mutex<State>>,
    key: Arc<Ed25519KeyPair>,
}

/// `ServerLinkService`.
struct Starting(Arc<Shared>);

/// `LinkedServerService`.
struct Linked(Arc<Shared>);

#[expect(
    refining_impl_trait,
    clippy::unused_async_trait_impl,
    reason = "plain `async fn` returning the owned response is the form the generated trait \
              documents, and a fake answers at once"
)]
impl ServerLinkService for Starting {
    async fn start_server_link(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, StartServerLinkRequest>,
    ) -> ServiceResult<StartServerLinkResponse> {
        lock(&self.0.state).starting = Some(request.name.to_owned());
        Response::ok(StartServerLinkResponse {
            device_code: DEVICE_CODE.to_owned(),
            user_code: USER_CODE.to_owned(),
            expires_in_seconds: 600,
            // So a test's every poll asks.
            interval_seconds: 0,
            ..Default::default()
        })
    }

    async fn poll_server_link(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, PollServerLinkRequest>,
    ) -> ServiceResult<PollServerLinkResponse> {
        let mut state = lock(&self.0.state);
        if request.device_code != DEVICE_CODE || state.starting.is_none() {
            return Err(ConnectError::not_found("the code expired"));
        }
        if !state.entered {
            return Response::ok(PollServerLinkResponse::default());
        }
        state.starting = None;
        state.linked = true;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let token = sign(
            &self.0.key,
            &json!({ "sub": "server", "email": EMAIL, "iat": now, "exp": now + 300 }),
        );
        Response::ok(PollServerLinkResponse {
            linked: true,
            credential: CREDENTIAL.to_owned(),
            token,
            ..Default::default()
        })
    }
}

#[expect(
    refining_impl_trait,
    clippy::unused_async_trait_impl,
    reason = "plain `async fn` returning the owned response is the form the generated trait \
              documents, and a fake answers at once"
)]
impl LinkedServerService for Linked {
    async fn unlink_server(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, UnlinkServerRequest>,
    ) -> ServiceResult<UnlinkServerResponse> {
        let mut state = lock(&self.0.state);
        let bearer = format!("Bearer {CREDENTIAL}");
        if !state.linked
            || ctx
                .header("authorization")
                .is_none_or(|sent| sent != bearer.as_str())
        {
            return Err(ConnectError::unauthenticated("no Linked Server has it"));
        }
        state.linked = false;
        Response::ok(UnlinkServerResponse::default())
    }
}

fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    state.lock().unwrap_or_else(PoisonError::into_inner)
}

const KID: &str = "fake-key";

fn key_set(key: &Ed25519KeyPair) -> Value {
    json!({ "keys": [{
        "kty": "OKP",
        "crv": "Ed25519",
        "x": URL_SAFE_NO_PAD.encode(key.public_key()),
        "kid": KID,
        "alg": "EdDSA",
        "use": "sig",
    }]})
}

fn sign(key: &Ed25519KeyPair, claims: &Value) -> String {
    let header = json!({ "alg": "EdDSA", "typ": "JWT", "kid": KID });
    let signed = format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(header.to_string()),
        URL_SAFE_NO_PAD.encode(claims.to_string()),
    );
    format!(
        "{signed}.{}",
        URL_SAFE_NO_PAD.encode(key.sign(signed.as_bytes()))
    )
}
