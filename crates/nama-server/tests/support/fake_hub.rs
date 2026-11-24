//! An in-process stand-in for the Hub a Server's config names, answering the calls the Kernel
//! makes as the Server's Link (ADR 0105): its device flow, Unlinking, its Users' Links, Users
//! created from Accounts, one sign-in with an Account at a time, the Server Addresses, whose public
//! port it reaches only at [`OPEN_PORT`], the Relay token, and the key set it signs tokens with. It
//! speaks the Connect protocol's unary calls with protobuf bodies, as the Kernel's client does, so
//! it needs none of the Hub API's other calls.

use std::{
    collections::HashMap,
    future,
    net::SocketAddr,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::{
    Router,
    body::Bytes,
    http::{HeaderMap, header::CONTENT_TYPE},
    response::{IntoResponse as _, Response},
    routing::{MethodRouter, get, post},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use buffa::Message;
use connectrpc::{ConnectError, client::HttpClient};
use nama_hub_proto::nama::hub::v1::{
    GetRelayTokenRequest, GetRelayTokenResponse, GetUserLinkRequest, GetUserLinkResponse,
    InviteUserRequest, InviteUserResponse, PollServerLinkRequest, PollServerLinkResponse,
    PollServerSignInRequest, PollServerSignInResponse, SetServerAddressesRequest,
    SetServerAddressesResponse, StartServerLinkRequest, StartServerLinkResponse,
    StartServerSignInRequest, StartServerSignInResponse, StartUserLinkRequest,
    StartUserLinkResponse, UnlinkServerRequest, UnlinkServerResponse, UnlinkUserRequest,
    UnlinkUserResponse,
};
use nama_proto::nama::v1::{
    GetServerLinkRequest, ServerServiceClient, StartServerLinkRequest as Start,
};
use nama_server::config::Config;
use ring::signature::{Ed25519KeyPair, KeyPair as _};
use serde_json::{Value, json};
use tokio::{net::TcpListener, task::JoinHandle};

use crate::server::{TestServer, WAIT};

/// The code the fake shows a Server to Link.
pub(crate) const CODE: &str = "BDFH-JKLM";

/// The email of the Account the code is entered by.
pub(crate) const EMAIL: &str = "ada@example.com";

/// The name the fake gives every Server that reports its addresses.
pub(crate) const OWN: &str = "abc.namadirect.test";

/// The code the fake gives a User to Link.
pub(crate) const USER_CODE: &str = "CDFG-HJKL";

/// The id of the one sign-in with an Account the fake starts.
pub(crate) const SIGN_IN_ID: &str = "sign-in-id";

/// The one-time code approving that sign-in sends the person back with.
pub(crate) const SIGN_IN_CODE: &str = "code";

/// The Server's id at the fake, which the token a newly Linked Server gets names.
const SERVER_ID: &str = "server";

const SIGN_IN_DEVICE_CODE: &str = "sign-in-device-code";

/// The IP address the fake sees the Server's calls come from.
pub(crate) const PUBLIC_IP: &str = "203.0.113.7";

/// The one public port the fake reaches the Server at.
pub(crate) const OPEN_PORT: u16 = 7980;

const DEVICE_CODE: &str = "device-code";

const KID: &str = "hub";

/// What the fake knows.
#[derive(Default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each is a switch a test flips on its own, not states of one thing"
)]
struct State {
    /// The name the Server started Linking as, while its code waits.
    starting: Option<String>,
    entered: bool,
    /// Whether the Server holds a credential the fake knows.
    linked: bool,
    /// How many credentials the fake handed out; only the last works.
    credentials: usize,
    /// Each set of Server Addresses the Server reported, in order.
    told: Vec<SetServerAddressesRequest>,
    /// Whether every call waits forever, as a Hub slow to time out.
    hanging: bool,
    /// The name each User waiting on a code started Linking as, by id.
    user_codes: HashMap<i64, String>,
    /// The email of the Account each Linked User is Linked to, by id.
    user_accounts: HashMap<i64, String>,
    /// The Users the Server Unlinked, by id, in order.
    unlinked_users: Vec<i64>,
    /// Whether Unlinking a User fails.
    unlinks_fail: bool,
    /// The User the Server last invited, by id, name and email.
    invited: Option<(i64, String, String)>,
    /// Where the sign-in with an Account the Server last started returns to.
    returns_to: Option<String>,
    /// The User an Account approved the sign-in as, until the Server collects it.
    approved: Option<i64>,
    /// Whom the fake signs sign-in tokens for, the Server's id unless a test says otherwise.
    audience: Option<String>,
    /// How often the Server asked for a sign-in's token.
    sign_in_polls: usize,
}

/// The Hub, serving until [`FakeHub::stop`] or its drop.
pub(crate) struct FakeHub {
    addr: SocketAddr,
    state: Arc<Mutex<State>>,
    serving: JoinHandle<()>,
}

impl FakeHub {
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    pub(crate) async fn start() -> Self {
        let state: Arc<Mutex<State>> = Arc::default();
        let key = Arc::new(Ed25519KeyPair::from_seed_unchecked(&[7; 32]).unwrap());
        let key_set = json!({ "keys": [{
            "kty": "OKP",
            "crv": "Ed25519",
            "kid": KID,
            "x": URL_SAFE_NO_PAD.encode(key.public_key()),
        }] })
        .to_string();
        let linking = "/nama.hub.v1.ServerLinkService";
        let linked = "/nama.hub.v1.LinkedServerService";
        let app = Router::new()
            .route("/.well-known/jwks.json", get(async move || key_set))
            .route(
                &format!("{linking}/StartServerLink"),
                unary(&state, &key, start),
            )
            .route(
                &format!("{linking}/PollServerLink"),
                unary(&state, &key, poll),
            )
            .route(
                &format!("{linked}/UnlinkServer"),
                unary(&state, &key, unlink),
            )
            .route(
                &format!("{linked}/SetServerAddresses"),
                unary(&state, &key, tell),
            )
            .route(
                &format!("{linked}/GetRelayToken"),
                unary(&state, &key, relay_token),
            )
            .route(
                &format!("{linked}/StartUserLink"),
                unary(&state, &key, start_user),
            )
            .route(
                &format!("{linked}/GetUserLink"),
                unary(&state, &key, user_link),
            )
            .route(
                &format!("{linked}/UnlinkUser"),
                unary(&state, &key, unlink_user),
            )
            .route(&format!("{linked}/InviteUser"), unary(&state, &key, invite))
            .route(
                &format!("{linked}/StartServerSignIn"),
                unary(&state, &key, start_sign_in),
            )
            .route(
                &format!("{linked}/PollServerSignIn"),
                unary(&state, &key, poll_sign_in),
            );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let serving = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self {
            addr,
            state,
            serving,
        }
    }

    pub(crate) fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// The config of a Server that Links with the fake.
    pub(crate) fn config(&self) -> Config {
        Config {
            hub_url: Some(self.url()),
            ..Config::default()
        }
    }

    /// Someone enters the code on the Hub.
    pub(crate) fn enter_code(&self) {
        lock(&self.state).entered = true;
    }

    /// The name the waiting Server started Linking as.
    pub(crate) fn started_as(&self) -> Option<String> {
        lock(&self.state).starting.clone()
    }

    /// Whether the Server holds a credential the fake still knows.
    pub(crate) fn linked(&self) -> bool {
        lock(&self.state).linked
    }

    /// Forgets the Server's credential and its Users' Links, as deleting its Account does.
    pub(crate) fn forget_server(&self) {
        let mut state = lock(&self.state);
        state.linked = false;
        state.user_codes.clear();
        state.user_accounts.clear();
    }

    /// The User `user_id` enters their code on the Hub, signed in to [`EMAIL`]'s Account.
    pub(crate) fn enter_user_code(&self, user_id: i64) {
        let mut state = lock(&self.state);
        if state.user_codes.remove(&user_id).is_some() {
            state.user_accounts.insert(user_id, EMAIL.to_owned());
        }
    }

    /// An Account Links the User `user_id` on the Hub, as by accepting their invite.
    pub(crate) fn link_user(&self, user_id: i64) {
        let mut state = lock(&self.state);
        state.user_codes.remove(&user_id);
        state.user_accounts.insert(user_id, EMAIL.to_owned());
    }

    /// The Account Unlinks the User `user_id` on the Hub, as deleting it does.
    pub(crate) fn unlink_user_on_hub(&self, user_id: i64) {
        lock(&self.state).user_accounts.remove(&user_id);
    }

    /// The name the User `user_id` started Linking as, while their code waits.
    pub(crate) fn user_starting(&self, user_id: i64) -> Option<String> {
        lock(&self.state).user_codes.get(&user_id).cloned()
    }

    /// Has Unlinking a User fail from now on.
    pub(crate) fn fail_unlinks(&self) {
        lock(&self.state).unlinks_fail = true;
    }

    /// The Users the Server Unlinked, by id, in order.
    pub(crate) fn unlinked_users(&self) -> Vec<i64> {
        lock(&self.state).unlinked_users.clone()
    }

    /// Waits until the Server Unlinked exactly `expected`, since a delete doesn't wait for it.
    pub(crate) async fn until_unlinked(&self, expected: &[i64]) {
        let waited = tokio::time::timeout(WAIT, async {
            while self.unlinked_users() != expected {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await;
        assert!(
            waited.is_ok(),
            "Unlinked {:?}, not {expected:?}",
            self.unlinked_users()
        );
    }

    /// The User the Server last invited, by id, name and email.
    pub(crate) fn invited(&self) -> Option<(i64, String, String)> {
        lock(&self.state).invited.clone()
    }

    /// Where the sign-in with an Account the Server last started returns to.
    pub(crate) fn returns_to(&self) -> Option<String> {
        lock(&self.state).returns_to.clone()
    }

    /// An Account approves the sign-in as the User `user_id`.
    pub(crate) fn approve_sign_in(&self, user_id: i64) {
        lock(&self.state).approved = Some(user_id);
    }

    /// Signs sign-in tokens for the Server `server_id` from now on.
    pub(crate) fn sign_in_for(&self, server_id: &str) {
        lock(&self.state).audience = Some(server_id.to_owned());
    }

    /// How often the Server asked for a sign-in's token.
    pub(crate) fn sign_in_polls(&self) -> usize {
        lock(&self.state).sign_in_polls
    }

    /// The Server Addresses the Server reported last.
    pub(crate) fn last_told(&self) -> Option<SetServerAddressesRequest> {
        lock(&self.state).told.last().cloned()
    }

    /// Has every call from now on wait forever.
    pub(crate) fn hang(&self) {
        lock(&self.state).hanging = true;
    }

    /// Goes down, as if unreachable.
    pub(crate) fn stop(&self) {
        self.serving.abort();
    }

    /// A Server Linked to [`EMAIL`]'s Account on a fake started for it, with no Plugin enabled,
    /// and the fake.
    pub(crate) async fn linked_server() -> (TestServer, Self) {
        let hub = Self::start().await;
        let server = TestServer::builder().config(hub.config()).start().await;
        hub.link(&server).await;
        (server, hub)
    }

    /// Links `server` to [`EMAIL`]'s Account, as its Admin does in Settings.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    pub(crate) async fn link(&self, server: &TestServer) {
        let settings = ServerServiceClient::new(HttpClient::plaintext(), server.client_config());
        settings.start_server_link(Start::default()).await.unwrap();
        self.enter_code();
        let linked = settings
            .get_server_link(GetServerLinkRequest::default())
            .await
            .unwrap()
            .into_owned()
            .link
            .into_option()
            .unwrap();
        assert_eq!(linked.account, EMAIL);
    }
}

impl Drop for FakeHub {
    fn drop(&mut self) {
        self.serving.abort();
    }
}

/// How the fake answers one call: what the request asks, signed with the key where it signs.
type Answer<Req, Res> =
    fn(&mut State, &HeaderMap, &Req, &Ed25519KeyPair) -> Result<Res, ConnectError>;

/// A Connect unary call `answer` answers.
fn unary<Req, Res>(
    state: &Arc<Mutex<State>>,
    key: &Arc<Ed25519KeyPair>,
    answer: Answer<Req, Res>,
) -> MethodRouter
where
    Req: Message + Send + 'static,
    Res: Message + 'static,
{
    let (state, key) = (Arc::clone(state), Arc::clone(key));
    post(async move |headers: HeaderMap, body: Bytes| -> Response {
        let hanging = lock(&state).hanging;
        if hanging {
            future::pending::<()>().await;
        }
        let answered = Req::decode_from_slice(&body)
            .map_err(|_unreadable| ConnectError::invalid_argument("not a request"))
            .and_then(|request| answer(&mut lock(&state), &headers, &request, &key));
        match answered {
            Ok(response) => (
                [(CONTENT_TYPE, "application/proto")],
                response.encode_to_vec(),
            )
                .into_response(),
            Err(err) => (
                err.http_status(),
                [(CONTENT_TYPE, "application/json")],
                err.to_json(),
            )
                .into_response(),
        }
    })
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "every call's answer has the one type `unary` takes"
)]
fn start(
    state: &mut State,
    _: &HeaderMap,
    request: &StartServerLinkRequest,
    _: &Ed25519KeyPair,
) -> Result<StartServerLinkResponse, ConnectError> {
    state.starting = Some(request.name.clone());
    Ok(StartServerLinkResponse {
        device_code: DEVICE_CODE.to_owned(),
        user_code: CODE.to_owned(),
        expires_in_seconds: 600,
        // So a test's every look asks.
        interval_seconds: 0,
        ..Default::default()
    })
}

fn poll(
    state: &mut State,
    _: &HeaderMap,
    request: &PollServerLinkRequest,
    key: &Ed25519KeyPair,
) -> Result<PollServerLinkResponse, ConnectError> {
    if request.device_code != DEVICE_CODE || state.starting.is_none() {
        return Err(ConnectError::not_found("the code expired"));
    }
    if !state.entered {
        return Ok(PollServerLinkResponse::default());
    }
    state.starting = None;
    state.entered = false;
    state.linked = true;
    state.credentials += 1;
    Ok(PollServerLinkResponse {
        linked: true,
        credential: credential(state.credentials),
        token: sign(
            key,
            &json!({ "sub": SERVER_ID, "email": EMAIL, "exp": in_5_minutes() }),
        ),
        ..Default::default()
    })
}

fn unlink(
    state: &mut State,
    headers: &HeaderMap,
    _: &UnlinkServerRequest,
    _: &Ed25519KeyPair,
) -> Result<UnlinkServerResponse, ConnectError> {
    caller(state, headers)?;
    state.linked = false;
    state.user_codes.clear();
    state.user_accounts.clear();
    Ok(UnlinkServerResponse::default())
}

fn tell(
    state: &mut State,
    headers: &HeaderMap,
    request: &SetServerAddressesRequest,
    _: &Ed25519KeyPair,
) -> Result<SetServerAddressesResponse, ConnectError> {
    caller(state, headers)?;
    let mut names = vec![OWN.to_owned()];
    if !request.relay.is_empty() {
        names.push(format!("relay.{OWN}"));
    }
    let port = request.public_port;
    state.told.push(request.clone());
    if port == 0 {
        return Ok(SetServerAddressesResponse {
            names,
            ..Default::default()
        });
    }
    Ok(SetServerAddressesResponse {
        public_address: format!("http://{PUBLIC_IP}:{port}"),
        reachable: port == u32::from(OPEN_PORT),
        names,
        ..Default::default()
    })
}

fn relay_token(
    state: &mut State,
    headers: &HeaderMap,
    _: &GetRelayTokenRequest,
    key: &Ed25519KeyPair,
) -> Result<GetRelayTokenResponse, ConnectError> {
    caller(state, headers)?;
    Ok(GetRelayTokenResponse {
        token: sign(
            key,
            &json!({ "sub": OWN, "aud": "relay", "exp": in_5_minutes() }),
        ),
        ..Default::default()
    })
}

fn start_user(
    state: &mut State,
    headers: &HeaderMap,
    request: &StartUserLinkRequest,
    _: &Ed25519KeyPair,
) -> Result<StartUserLinkResponse, ConnectError> {
    caller(state, headers)?;
    if state.user_accounts.contains_key(&request.user_id) {
        return Err(ConnectError::already_exists("the User is Linked"));
    }
    state
        .user_codes
        .insert(request.user_id, request.name.clone());
    Ok(StartUserLinkResponse {
        user_code: USER_CODE.to_owned(),
        expires_in_seconds: 600,
        ..Default::default()
    })
}

fn user_link(
    state: &mut State,
    headers: &HeaderMap,
    request: &GetUserLinkRequest,
    _: &Ed25519KeyPair,
) -> Result<GetUserLinkResponse, ConnectError> {
    caller(state, headers)?;
    Ok(GetUserLinkResponse {
        account: (state.user_accounts.get(&request.user_id).cloned()).unwrap_or_default(),
        ..Default::default()
    })
}

fn unlink_user(
    state: &mut State,
    headers: &HeaderMap,
    request: &UnlinkUserRequest,
    _: &Ed25519KeyPair,
) -> Result<UnlinkUserResponse, ConnectError> {
    caller(state, headers)?;
    state.unlinked_users.push(request.user_id);
    if state.unlinks_fail {
        return Err(ConnectError::internal("the Hub failed"));
    }
    state.user_codes.remove(&request.user_id);
    state.user_accounts.remove(&request.user_id);
    Ok(UnlinkUserResponse::default())
}

fn invite(
    state: &mut State,
    headers: &HeaderMap,
    request: &InviteUserRequest,
    _: &Ed25519KeyPair,
) -> Result<InviteUserResponse, ConnectError> {
    caller(state, headers)?;
    state.invited = Some((request.user_id, request.name.clone(), request.email.clone()));
    Ok(InviteUserResponse::default())
}

fn start_sign_in(
    state: &mut State,
    headers: &HeaderMap,
    request: &StartServerSignInRequest,
    _: &Ed25519KeyPair,
) -> Result<StartServerSignInResponse, ConnectError> {
    caller(state, headers)?;
    state.returns_to = Some(request.return_url.clone());
    Ok(StartServerSignInResponse {
        id: SIGN_IN_ID.to_owned(),
        device_code: SIGN_IN_DEVICE_CODE.to_owned(),
        ..Default::default()
    })
}

/// The token naming the approved User, collected once with the right code; anything else ends
/// the sign-in, as the Hub does.
fn poll_sign_in(
    state: &mut State,
    headers: &HeaderMap,
    request: &PollServerSignInRequest,
    key: &Ed25519KeyPair,
) -> Result<PollServerSignInResponse, ConnectError> {
    caller(state, headers)?;
    state.sign_in_polls += 1;
    let approved = (state.approved.take())
        .filter(|_| request.device_code == SIGN_IN_DEVICE_CODE && request.code == SIGN_IN_CODE)
        .ok_or_else(|| ConnectError::not_found("no such sign-in"))?;
    let audience = state.audience.as_deref().unwrap_or(SERVER_ID);
    Ok(PollServerSignInResponse {
        token: sign(
            key,
            &json!({ "sub": approved.to_string(), "aud": audience, "exp": in_5_minutes() }),
        ),
        ..Default::default()
    })
}

/// Refuses a call without the credential of the Server the fake has Linked, as the Hub does.
fn caller(state: &State, headers: &HeaderMap) -> Result<(), ConnectError> {
    let bearer = format!("Bearer {}", credential(state.credentials));
    let sent = headers
        .get("authorization")
        .and_then(|sent| sent.to_str().ok());
    if state.linked && sent == Some(bearer.as_str()) {
        Ok(())
    } else {
        Err(ConnectError::unauthenticated("no Linked Server has it"))
    }
}

/// The `nth` credential the fake hands out.
fn credential(nth: usize) -> String {
    format!("credential-{nth}")
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn in_5_minutes() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 300
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

fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    state.lock().unwrap_or_else(PoisonError::into_inner)
}
