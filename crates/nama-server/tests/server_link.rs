//! Linking the Server with the Hub its config names, which the Kernel does with no Plugin (ADR
//! 0105), here an in-process fake, and a Server whose config names none, which has nothing to Link
//! (ADR 0002). The Hub is told the Server's Server Addresses, and the Server shows whether it reached
//! it from outside. Its Users Link through the Kernel too (ADR 0011, ADR 0105), as do Users created
//! from Accounts, signing in with an Account, and the Admin who can always sign in without the Hub
//! (ADR 0092), all with no Plugin enabled.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/fake_hub.rs"]
mod fake_hub;
#[path = "support/refused.rs"]
mod refused;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::{num::NonZeroU16, time::Duration};

use connectrpc::{
    ConnectError, ErrorCode,
    client::{ClientConfig, HttpClient},
};
use fake_hub::{
    CODE, EMAIL, FakeHub, OPEN_PORT, OWN, PUBLIC_IP, SIGN_IN_CODE, SIGN_IN_ID, USER_CODE,
};
use nama_proto::nama::v1::{
    self as proto, CreateUserFromAccountRequest, DeleteUserRequest, GetServerInfoRequest,
    GetServerLinkRequest, GetUserLinkRequest, ListUsersRequest, Permission, ReauthenticateRequest,
    RefusalReason, ServerServiceClient, ServerSettings, SessionDelivery, SetPermissionsRequest,
    SignInServiceClient, SignInWithAccountRequest, StartAccountSignInRequest,
    StartServerLinkRequest, StartUserLinkRequest, UnlinkServerRequest, UnlinkUserRequest,
    UpdateServerSettingsRequest, UserServiceClient, WhoAmIRequest, reauthenticate_request::Proof,
};
use nama_server::config::Config;
use refused::assert_refused;
use server::TestServer;

fn users(config: ClientConfig) -> UserServiceClient<HttpClient> {
    UserServiceClient::new(HttpClient::plaintext(), config)
}

fn settings(server: &TestServer) -> ServerServiceClient<HttpClient> {
    ServerServiceClient::new(HttpClient::plaintext(), server.client_config())
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn set_server_url(server: &TestServer, url: &str) {
    settings(server)
        .update_server_settings(UpdateServerSettingsRequest {
            settings: ServerSettings {
                url: url.to_owned(),
                ..Default::default()
            }
            .into(),
            ..Default::default()
        })
        .await
        .unwrap();
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn link(server: &TestServer) -> Option<proto::ServerLink> {
    let got = settings(server)
        .get_server_link(GetServerLinkRequest::default())
        .await
        .unwrap();
    got.into_owned().link.into_option()
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn user_link(config: ClientConfig) -> Option<proto::UserLink> {
    let got = users(config)
        .get_user_link(GetUserLinkRequest::default())
        .await
        .unwrap();
    got.into_owned().link.into_option()
}

#[tokio::test]
async fn a_server_whose_config_names_no_hub_has_nothing_to_link() {
    let server = TestServer::builder().start().await;
    set_server_url(&server, "https://nama.example").await;

    assert_eq!(link(&server).await, None);
    assert_refused(
        settings(&server)
            .start_server_link(StartServerLinkRequest::default())
            .await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_NO_SERVER_LINKER,
    );
    assert_refused(
        settings(&server)
            .unlink_server(UnlinkServerRequest::default())
            .await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_NO_SERVER_LINKER,
    );
    assert_eq!(user_link(server.client_config()).await, None);
    assert!(!offers_account_sign_in(&server).await);
    assert_refused(
        start_account_sign_in(&server, ON_SERVER).await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_NO_SERVER_LINKER,
    );
    assert_refused(
        create_from_account(&server, "Grace").await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_NO_SERVER_LINKER,
    );
    assert_refused(
        users(server.client_config())
            .start_user_link(StartUserLinkRequest::default())
            .await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_NO_SERVER_LINKER,
    );
    assert_refused(
        users(server.client_config())
            .unlink_user(UnlinkUserRequest::default())
            .await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_NO_SERVER_LINKER,
    );
}

#[tokio::test]
async fn a_users_link_is_unset_until_the_server_is_linked() {
    let hub = FakeHub::start().await;
    let server = TestServer::builder().config(hub.config()).start().await;

    assert_eq!(user_link(server.client_config()).await, None);
}

#[tokio::test]
async fn a_user_shows_the_code_the_hub_gives_them_and_then_whose_they_are() {
    let (server, hub) = linked_server().await;
    let grace = server.user_with_default_permissions("Grace").await;
    assert_eq!(
        user_link(grace.client_config()).await,
        Some(proto::UserLink::default())
    );

    let started = users(grace.client_config())
        .start_user_link(StartUserLinkRequest::default())
        .await
        .unwrap()
        .into_owned()
        .link
        .into_option();

    let waiting = proto::UserLink {
        code: USER_CODE.to_owned(),
        url: hub.url(),
        ..Default::default()
    };
    assert_eq!(started, Some(waiting.clone()));
    assert_eq!(hub.user_starting(grace.id).as_deref(), Some("Grace"));
    assert_eq!(user_link(grace.client_config()).await, Some(waiting));
    hub.enter_user_code(grace.id);
    let shown = user_link(grace.client_config()).await.unwrap();
    assert_eq!(shown.account, EMAIL);

    users(grace.client_config())
        .unlink_user(UnlinkUserRequest::default())
        .await
        .unwrap();

    assert_eq!(
        user_link(grace.client_config()).await,
        Some(proto::UserLink::default())
    );
    assert_eq!(hub.unlinked_users(), [grace.id]);
}

#[tokio::test]
async fn a_server_linked_again_doesnt_show_its_users_old_links_while_the_hub_is_unreachable() {
    let (server, hub) = linked_server().await;
    let grace = server.create_user("Grace").await;
    // Before she signs in, as signing in asks the Hub and keeps its answer a minute.
    hub.link_user(grace);
    let grace = server.sign_in(grace, "Grace").await;
    assert_eq!(
        user_link(grace.client_config()).await.unwrap().account,
        EMAIL
    );
    settings(&server)
        .unlink_server(UnlinkServerRequest::default())
        .await
        .unwrap();
    hub.link(&server).await;
    hub.stop();

    let shown = user_link(grace.client_config()).await;

    assert_eq!(shown, Some(proto::UserLink::default()));
}

#[tokio::test]
async fn the_server_links_and_unlinks_with_the_hub_with_no_plugin() {
    let hub = FakeHub::start().await;
    let server = TestServer::builder().config(hub.config()).start().await;
    assert_eq!(link(&server).await, Some(proto::ServerLink::default()));

    let started = settings(&server)
        .start_server_link(StartServerLinkRequest::default())
        .await
        .unwrap()
        .into_owned()
        .link
        .into_option();

    let waiting = proto::ServerLink {
        code: CODE.to_owned(),
        url: hub.url(),
        ..Default::default()
    };
    assert_eq!(started, Some(waiting.clone()));
    assert_eq!(hub.started_as().as_deref(), Some("Nama"));
    assert_eq!(link(&server).await, Some(waiting));
    hub.enter_code();
    let shown = link(&server).await.unwrap();
    assert_eq!(shown.account, EMAIL);
    assert_eq!(shown.code, "");
    assert!(hub.linked());
    assert!(offers_account_sign_in(&server).await);

    settings(&server)
        .unlink_server(UnlinkServerRequest::default())
        .await
        .unwrap();

    assert!(!hub.linked());
    assert_eq!(link(&server).await, Some(proto::ServerLink::default()));
}

#[tokio::test]
async fn a_hub_that_cant_be_reached_leaves_the_server_as_it_was() {
    let (hub, down) = (FakeHub::start().await, FakeHub::start().await);
    let linked = TestServer::builder().config(hub.config()).start().await;
    hub.link(&linked).await;
    hub.stop();
    down.stop();
    let not_linked = TestServer::builder().config(down.config()).start().await;

    let started = settings(&not_linked)
        .start_server_link(StartServerLinkRequest::default())
        .await;
    let unlinked = settings(&linked)
        .unlink_server(UnlinkServerRequest::default())
        .await;

    assert_eq!(started.unwrap_err().code, ErrorCode::Unavailable);
    assert_eq!(unlinked.unwrap_err().code, ErrorCode::Unavailable);
    assert_eq!(link(&linked).await.unwrap().account, EMAIL);
}

#[tokio::test]
async fn a_server_the_hub_forgot_is_unlinked_by_its_next_report() {
    let hub = FakeHub::start().await;
    let server = TestServer::builder().config(hub.config()).start().await;
    hub.link(&server).await;
    hub.forget_server();

    set_server_url(&server, "https://nama.example.com").await;

    let unlinked = async {
        while link(&server).await != Some(proto::ServerLink::default()) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    };
    tokio::time::timeout(server::WAIT, unlinked).await.unwrap();
}

/// A `return_url` on the Server URL [`linked_server`] sets.
const ON_SERVER: &str = "https://nama.example/sign-in";

/// A Server Linked to an Account on the fake Hub, at the Server URL `https://nama.example`, with no
/// Plugin enabled, and the Hub.
async fn linked_server() -> (TestServer, FakeHub) {
    let (server, hub) = FakeHub::linked_server().await;
    set_server_url(&server, "https://nama.example").await;
    (server, hub)
}

async fn create_from_account(server: &TestServer, name: &str) -> Result<i64, ConnectError> {
    Ok(users(server.client_config())
        .create_user_from_account(CreateUserFromAccountRequest {
            name: name.to_owned(),
            email: "grace@example.com".to_owned(),
            ..Default::default()
        })
        .await?
        .into_owned()
        .id)
}

fn sign_in(server: &TestServer) -> SignInServiceClient<HttpClient> {
    SignInServiceClient::new(HttpClient::plaintext(), server.anonymous_client_config())
}

async fn start_account_sign_in(
    server: &TestServer,
    return_url: &str,
) -> Result<proto::StartAccountSignInResponse, ConnectError> {
    Ok(sign_in(server)
        .start_account_sign_in(StartAccountSignInRequest {
            return_url: return_url.to_owned(),
            ..Default::default()
        })
        .await?
        .into_owned())
}

/// Finishes the sign-in `handle` stands for with the one-time code the service sent the person
/// back with, and returns the session token.
async fn sign_in_with_account(
    server: &TestServer,
    handle: &str,
    code: &str,
) -> Result<proto::SignInWithAccountResponse, ConnectError> {
    Ok(sign_in(server)
        .sign_in_with_account(SignInWithAccountRequest {
            handle: handle.to_owned(),
            code: code.to_owned(),
            delivery: SessionDelivery::SESSION_DELIVERY_TOKEN.into(),
            ..Default::default()
        })
        .await?
        .into_owned())
}

/// Reauthenticates the signed-in `device` with the sign-in with an Account `handle` stands for,
/// finished with the service's one-time `code`.
async fn reauthenticate_with_account(
    device: ClientConfig,
    handle: &str,
    code: &str,
) -> Result<(), ConnectError> {
    SignInServiceClient::new(HttpClient::plaintext(), device)
        .reauthenticate(ReauthenticateRequest {
            proof: Some(Proof::AccountSignInHandle(handle.to_owned())),
            account_sign_in_code: code.to_owned(),
            ..Default::default()
        })
        .await
        .map(drop)
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn offers_account_sign_in(server: &TestServer) -> bool {
    ServerServiceClient::new(HttpClient::plaintext(), server.anonymous_client_config())
        .get_server_info(GetServerInfoRequest::default())
        .await
        .unwrap()
        .into_owned()
        .account_sign_in
}

#[tokio::test]
async fn a_user_created_from_an_account_signs_in_and_reauthenticates_once_the_account_approves() {
    let (server, hub) = linked_server().await;

    let grace = create_from_account(&server, " Grace ").await.unwrap();

    assert_eq!(
        hub.invited(),
        Some((grace, "Grace".to_owned(), "grace@example.com".to_owned()))
    );
    assert!(offers_account_sign_in(&server).await);
    let started = start_account_sign_in(&server, ON_SERVER).await.unwrap();
    assert_eq!(
        started.url,
        format!("{}/server-sign-in/{SIGN_IN_ID}", hub.url())
    );
    assert_eq!(hub.returns_to().as_deref(), Some(ON_SERVER));
    assert_refused(
        start_account_sign_in(&server, "javascript:alert(1)").await,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_NOT_A_URL,
    );
    // Its Server URL is the one Server Address a Server listening on loopback reports.
    let off_server = assert_refused(
        start_account_sign_in(&server, "http://localhost:7980/sign-in").await,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_NOT_A_SERVER_ADDRESS,
    );
    assert_eq!(off_server.params["serverUrl"], "https://nama.example");
    assert_eq!(hub.returns_to().as_deref(), Some(ON_SERVER));
    assert_refused(
        sign_in_with_account(&server, &started.handle, SIGN_IN_CODE).await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_ACCOUNT_SIGN_IN_NOT_FOUND,
    );
    hub.approve_sign_in(grace);
    let signed_in = sign_in_with_account(&server, &started.handle, SIGN_IN_CODE)
        .await
        .unwrap();
    assert_eq!(signed_in.user.into_option().unwrap().name, "Grace");
    let who = SignInServiceClient::new(
        HttpClient::plaintext(),
        server.client_config_as(&signed_in.token),
    )
    .who_am_i(WhoAmIRequest::default())
    .await
    .unwrap()
    .into_owned();
    assert_eq!(who.user.into_option().unwrap().id, grace);
    let device = server.client_config_as(&signed_in.token);
    let start_user_link = async || {
        users(device.clone())
            .start_user_link(StartUserLinkRequest::default())
            .await
    };
    // Whoever approved on the Hub may not hold this Device, as with a Sign-in Code (ADR 0014).
    assert_refused(
        start_user_link().await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_RECENT_AUTHENTICATION_REQUIRED,
    );
    let hedy = create_from_account(&server, "Hedy").await.unwrap();
    let by_hedy = start_account_sign_in(&server, ON_SERVER).await.unwrap();
    hub.approve_sign_in(hedy);
    assert_refused(
        reauthenticate_with_account(device.clone(), &by_hedy.handle, SIGN_IN_CODE).await,
        ErrorCode::Unauthenticated,
        RefusalReason::REFUSAL_REASON_WRONG_ACCOUNT,
    );
    let by_grace = start_account_sign_in(&server, ON_SERVER).await.unwrap();
    hub.approve_sign_in(grace);
    reauthenticate_with_account(device.clone(), &by_grace.handle, SIGN_IN_CODE)
        .await
        .unwrap();
    start_user_link().await.unwrap();
}

#[tokio::test]
async fn with_the_hub_down_devices_stay_signed_in_but_accounts_cant_sign_in_or_reauthenticate() {
    let (server, hub) = linked_server().await;
    let grace = create_from_account(&server, "Grace").await.unwrap();
    let started = start_account_sign_in(&server, ON_SERVER).await.unwrap();
    hub.approve_sign_in(grace);
    let signed_in = sign_in_with_account(&server, &started.handle, SIGN_IN_CODE)
        .await
        .unwrap();

    hub.stop();

    let device = server.client_config_as(&signed_in.token);
    let not_confirmed =
        reauthenticate_with_account(device.clone(), &started.handle, SIGN_IN_CODE).await;
    assert_eq!(not_confirmed.unwrap_err().code, ErrorCode::Unavailable);
    let still = SignInServiceClient::new(HttpClient::plaintext(), device)
        .who_am_i(WhoAmIRequest::default())
        .await;
    assert!(still.is_ok(), "{still:?}");
    let refused = start_account_sign_in(&server, ON_SERVER).await.unwrap_err();
    assert_eq!(refused.code, ErrorCode::Unavailable);
    let message = refused.message.as_deref().unwrap_or_default();
    assert!(message.contains("needs the Hub"), "{refused:?}");
    assert!(
        !message.to_lowercase().contains("password"),
        "a User created from an Account has none: {refused:?}"
    );
    let before = users(server.client_config())
        .list_users(ListUsersRequest::default())
        .await
        .unwrap()
        .into_owned()
        .users
        .len();
    let not_created = create_from_account(&server, "Hedy").await.unwrap_err();
    assert_eq!(not_created.code, ErrorCode::Unavailable);
    let after = users(server.client_config())
        .list_users(ListUsersRequest::default())
        .await
        .unwrap()
        .into_owned()
        .users
        .len();
    assert_eq!(after, before);
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn cannot_sign_in(server: &TestServer) -> Vec<i64> {
    let listed = users(server.client_config())
        .list_users(ListUsersRequest::default())
        .await
        .unwrap();
    listed.into_owned().cannot_sign_in_user_ids
}

#[tokio::test]
async fn a_user_with_no_password_whose_account_unlinked_them_is_listed_as_unable_to_sign_in() {
    let (server, hub) = linked_server().await;
    let grace = create_from_account(&server, "Grace").await.unwrap();
    hub.link_user(grace);
    assert_eq!(cannot_sign_in(&server).await, Vec::<i64>::new());

    // As when Grace's Account is deleted on the Hub. The Server asks again a minute later, or at
    // once after a restart.
    hub.unlink_user_on_hub(grace);
    let restarted = TestServer::builder()
        .config(hub.config())
        .restart(server)
        .await;

    assert_eq!(cannot_sign_in(&restarted).await, [grace]);
}

#[tokio::test]
async fn a_user_who_signs_in_only_with_their_account_cant_unlink_from_it_but_can_cancel_a_code() {
    let (server, hub) = linked_server().await;
    let grace = create_from_account(&server, "Grace").await.unwrap();
    // Linked before signing in, since signing in has the Server ask the Hub whether she is, and
    // keep the answer for a minute.
    hub.link_user(grace);
    let started = start_account_sign_in(&server, ON_SERVER).await.unwrap();
    hub.approve_sign_in(grace);
    let signed_in = sign_in_with_account(&server, &started.handle, SIGN_IN_CODE)
        .await
        .unwrap();
    let device = server.client_config_as(&signed_in.token);
    let unlink = async || {
        users(device.clone())
            .unlink_user(UnlinkUserRequest::default())
            .await
    };

    assert_refused(
        unlink().await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_LAST_SIGN_IN_METHOD,
    );
    assert_eq!(hub.unlinked_users(), Vec::<i64>::new());
    // Unlinked on the Hub since, and Linking again, they can still stop showing their new code.
    hub.unlink_user_on_hub(grace);
    let again = start_account_sign_in(&server, ON_SERVER).await.unwrap();
    hub.approve_sign_in(grace);
    reauthenticate_with_account(device.clone(), &again.handle, SIGN_IN_CODE)
        .await
        .unwrap();
    users(device.clone())
        .start_user_link(StartUserLinkRequest::default())
        .await
        .unwrap();
    unlink().await.unwrap();
    assert_eq!(hub.unlinked_users(), [grace]);
}

#[tokio::test]
async fn a_sign_in_token_the_hub_signed_for_another_server_is_refused() {
    let (server, hub) = linked_server().await;
    let grace = create_from_account(&server, "Grace").await.unwrap();
    let started = start_account_sign_in(&server, ON_SERVER).await.unwrap();
    hub.sign_in_for("another server");
    hub.approve_sign_in(grace);

    let refused = sign_in_with_account(&server, &started.handle, SIGN_IN_CODE).await;

    assert_eq!(refused.unwrap_err().code, ErrorCode::Unavailable);
}

/// Longer than the bound on a look at a Link, shorter than forever.
const PAST_THE_BOUND: Duration = Duration::from_secs(20);

#[tokio::test]
async fn server_info_offers_passwords_alone_while_the_hub_never_answers() {
    let hub = FakeHub::start().await;
    let server = TestServer::builder().config(hub.config()).start().await;
    settings(&server)
        .start_server_link(StartServerLinkRequest::default())
        .await
        .unwrap();
    hub.enter_code();
    hub.hang();

    let offered = tokio::time::timeout(PAST_THE_BOUND, offers_account_sign_in(&server)).await;

    assert_eq!(offered.ok(), Some(false));
}

#[tokio::test]
async fn the_users_list_answers_while_the_hub_never_answers_and_lists_no_one_unable_to_sign_in() {
    let (server, hub) = linked_server().await;
    create_from_account(&server, "Grace").await.unwrap();
    hub.hang();

    let listed = tokio::time::timeout(PAST_THE_BOUND, cannot_sign_in(&server)).await;

    assert_eq!(listed.ok(), Some(Vec::new()));
}

#[tokio::test]
async fn reauthenticating_with_an_account_past_the_limit_is_refused_without_asking_the_hub() {
    let (server, hub) = linked_server().await;
    let grace = create_from_account(&server, "Grace").await.unwrap();
    let started = start_account_sign_in(&server, ON_SERVER).await.unwrap();
    hub.approve_sign_in(grace);
    let signed_in = sign_in_with_account(&server, &started.handle, SIGN_IN_CODE)
        .await
        .unwrap();
    let device = server.client_config_as(&signed_in.token);
    let hedy = create_from_account(&server, "Hedy").await.unwrap();
    // Each counts toward the limit on Grace's name: never approved, or approved by Hedy's Account.
    for _ in 0..5 {
        assert_refused(
            reauthenticate_with_account(device.clone(), &started.handle, SIGN_IN_CODE).await,
            ErrorCode::NotFound,
            RefusalReason::REFUSAL_REASON_ACCOUNT_SIGN_IN_NOT_FOUND,
        );
        hub.approve_sign_in(hedy);
        assert_refused(
            reauthenticate_with_account(device.clone(), &started.handle, SIGN_IN_CODE).await,
            ErrorCode::Unauthenticated,
            RefusalReason::REFUSAL_REASON_WRONG_ACCOUNT,
        );
    }
    let asked = hub.sign_in_polls();
    hub.approve_sign_in(grace);

    let blocked = reauthenticate_with_account(device, &started.handle, SIGN_IN_CODE).await;

    assert_refused(
        blocked,
        ErrorCode::ResourceExhausted,
        RefusalReason::REFUSAL_REASON_TOO_MANY_ATTEMPTS,
    );
    assert_eq!(hub.sign_in_polls(), asked);
}

#[tokio::test]
async fn the_last_admin_who_signs_in_with_a_password_cant_be_demoted_or_deleted() {
    let (server, _hub) = linked_server().await;
    let grace = create_from_account(&server, "Grace").await.unwrap();
    let admin = SignInServiceClient::new(HttpClient::plaintext(), server.client_config())
        .who_am_i(WhoAmIRequest::default())
        .await
        .unwrap()
        .into_owned()
        .user
        .into_option()
        .unwrap()
        .id;
    let set = async |user_id, permissions: &[Permission]| {
        users(server.client_config())
            .set_permissions(SetPermissionsRequest {
                user_id,
                permissions: permissions.iter().map(|&p| p.into()).collect(),
                ..Default::default()
            })
            .await
    };
    set(grace, &[Permission::PERMISSION_ADMIN]).await.unwrap();

    assert_refused(
        set(admin, &[Permission::PERMISSION_REQUEST]).await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_LAST_ADMIN,
    );
    assert_refused(
        users(server.client_config())
            .delete_user(DeleteUserRequest {
                user_id: admin,
                ..Default::default()
            })
            .await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_LAST_ADMIN,
    );
    // An Admin who signs in only with their Account may go.
    set(grace, &[Permission::PERMISSION_REQUEST]).await.unwrap();
}

#[tokio::test]
async fn the_hub_is_told_the_server_url_and_port_the_admin_set_and_the_server_shows_what_it_found()
{
    let hub = FakeHub::start().await;
    let server = TestServer::builder().config(hub.config()).start().await;
    hub.link(&server).await;
    set_server_url(&server, "https://nama.example.com").await;

    let restarted = TestServer::builder()
        .config(Config {
            forwarded_port: NonZeroU16::new(OPEN_PORT),
            ..hub.config()
        })
        .restart(server)
        .await;

    // The test Server listens on 127.0.0.1, which no other device on its LAN can reach.
    until_told(&hub, "https://nama.example.com/", OPEN_PORT).await;
    let found = async {
        while !link(&restarted).await.unwrap().reachable {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    };
    tokio::time::timeout(server::WAIT, found).await.unwrap();
    let shown = link(&restarted).await.unwrap();
    assert_eq!(shown.account, EMAIL);
    assert_eq!(
        shown.public_address,
        format!("http://{PUBLIC_IP}:{OPEN_PORT}")
    );
    assert_eq!(shown.names, [OWN]);
}

#[tokio::test]
async fn the_hub_is_told_a_new_server_url_as_soon_as_it_is_saved() {
    let hub = FakeHub::start().await;
    let server = TestServer::builder().config(hub.config()).start().await;
    // Linked after the first telling, at startup, so the next one isn't due for a minute.
    hub.link(&server).await;

    set_server_url(&server, "https://nama.example.com").await;

    until_told(&hub, "https://nama.example.com/", 0).await;
}

/// Waits until `hub` was last told the Server URL `url` and the public port `port`, well within
/// the minute between tellings.
async fn until_told(hub: &FakeHub, url: &str, port: u16) {
    let told = || hub.last_told().map(|told| (told.url, told.public_port));
    let waited = tokio::time::timeout(server::WAIT, async {
        while told() != Some((url.to_owned(), u32::from(port))) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    assert!(waited.is_ok(), "told {:?}", told());
}
