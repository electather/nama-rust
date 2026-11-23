//! Linking the Server, and a User, through the enabled Plugin that links it (ADR 0011, ADR 0094),
//! here an in-process one scripted by each test, and a Server without one, which has nothing to
//! Link. The Plugin is told the Server's Server Addresses, and shows whether the service reached it
//! from outside. Through it too, Users created from Accounts, signing in with an Account, and the
//! Admin who can always sign in without the Hub (ADR 0092).

#[path = "support/refused.rs"]
mod refused;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/scripted_linker.rs"]
mod scripted_linker;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::{num::NonZeroU16, time::Duration};

use connectrpc::{
    ConnectError, ErrorCode,
    client::{ClientConfig, HttpClient},
};
use nama_plugin_api::server_link::{Link, Reachability, ServerAddresses};
use nama_proto::nama::v1::{
    self as proto, CreateUserFromAccountRequest, DeleteUserRequest, GetServerInfoRequest,
    GetServerLinkRequest, GetUserLinkRequest, ListUsersRequest, Permission, ReauthenticateRequest,
    RefusalReason, ServerServiceClient, ServerSettings, SessionDelivery, SetPermissionsRequest,
    SignInServiceClient, SignInWithAccountRequest, StartAccountSignInRequest,
    StartServerLinkRequest, StartUserLinkRequest, UnlinkServerRequest, UnlinkUserRequest,
    UpdateServerSettingsRequest, WhoAmIRequest, reauthenticate_request::Proof,
};
use nama_server::config::Config;
use refused::assert_refused;
use scripted_linker::{Scripted, enabling, users, with_linker};
use server::TestServer;

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
async fn a_server_without_a_plugin_that_links_it_has_nothing_to_link() {
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
    let linker = Scripted::default();
    let server = with_linker(&linker).await;

    assert_eq!(user_link(server.client_config()).await, None);
}

#[tokio::test]
async fn a_user_shows_the_code_the_plugin_starts_for_them_and_then_whose_they_are() {
    let linker = Scripted::default();
    linker.script().user_link = Some(Link::NotLinked);
    let server = with_linker(&linker).await;
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
        code: "CDFG-HJKL".to_owned(),
        url: "https://hub.example".to_owned(),
        ..Default::default()
    };
    assert_eq!(started, Some(waiting.clone()));
    assert_eq!(
        linker.script().user_started,
        Some((grace.id, "Grace".to_owned()))
    );
    assert_eq!(user_link(grace.client_config()).await, Some(waiting));
    linker.script().user_link = Some(Link::Linked {
        account: "grace@example.com".to_owned(),
        reachability: None,
    });
    let shown = user_link(grace.client_config()).await.unwrap();
    assert_eq!(shown.account, "grace@example.com");

    users(grace.client_config())
        .unlink_user(UnlinkUserRequest::default())
        .await
        .unwrap();

    assert_eq!(
        user_link(grace.client_config()).await,
        Some(proto::UserLink::default())
    );
}

#[tokio::test]
async fn the_server_shows_the_code_its_plugin_starts_and_then_whose_it_is() {
    let linker = Scripted::default();
    linker.script().link = Some(Link::NotLinked);
    let server = with_linker(&linker).await;
    assert_eq!(link(&server).await, Some(proto::ServerLink::default()));

    let started = settings(&server)
        .start_server_link(StartServerLinkRequest::default())
        .await
        .unwrap()
        .into_owned()
        .link
        .into_option();

    let waiting = proto::ServerLink {
        code: "BDFH-JKLM".to_owned(),
        url: "https://hub.example".to_owned(),
        ..Default::default()
    };
    assert_eq!(started, Some(waiting.clone()));
    assert_eq!(linker.script().started_as.as_deref(), Some("Nama"));
    assert_eq!(link(&server).await, Some(waiting));
    linker.script().link = Some(Link::Linked {
        account: "ada@example.com".to_owned(),
        reachability: Some(Reachability {
            address: "http://203.0.113.7:7980".to_owned(),
            reachable: true,
        }),
    });
    let shown = link(&server).await.unwrap();
    assert_eq!(shown.account, "ada@example.com");
    assert_eq!(shown.code, "");
    assert_eq!(shown.public_address, "http://203.0.113.7:7980");
    assert!(shown.reachable);

    settings(&server)
        .unlink_server(UnlinkServerRequest::default())
        .await
        .unwrap();

    assert_eq!(link(&server).await, Some(proto::ServerLink::default()));
}

#[tokio::test]
async fn a_plugin_that_cant_reach_its_service_leaves_the_server_as_it_was() {
    let linker = Scripted::default();
    let server = with_linker(&linker).await;

    let started = settings(&server)
        .start_server_link(StartServerLinkRequest::default())
        .await;
    let unlinked = settings(&server)
        .unlink_server(UnlinkServerRequest::default())
        .await;

    assert_eq!(started.unwrap_err().code, ErrorCode::Unavailable);
    assert_eq!(unlinked.unwrap_err().code, ErrorCode::Unavailable);
}

/// A `return_url` on the Server URL [`linked_server`] sets.
const ON_SERVER: &str = "https://nama.example/sign-in";

/// A Server whose Plugin has it Linked to an Account, at the Server URL `https://nama.example`.
async fn linked_server(linker: &Scripted) -> TestServer {
    linker.script().link = Some(Link::Linked {
        account: "ada@example.com".to_owned(),
        reachability: None,
    });
    let server = with_linker(linker).await;
    set_server_url(&server, "https://nama.example").await;
    server
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
    let linker = Scripted::default();
    let server = linked_server(&linker).await;

    let grace = create_from_account(&server, " Grace ").await.unwrap();

    assert_eq!(
        linker.script().invited,
        Some((grace, "Grace".to_owned(), "grace@example.com".to_owned()))
    );
    assert!(offers_account_sign_in(&server).await);
    let started = start_account_sign_in(&server, ON_SERVER).await.unwrap();
    assert_eq!(started.url, "https://hub.example/server-sign-in/1");
    assert_eq!(linker.script().returns_to.as_deref(), Some(ON_SERVER));
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
    assert_eq!(linker.script().returns_to.as_deref(), Some(ON_SERVER));
    assert_refused(
        sign_in_with_account(&server, &started.handle, "code").await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_ACCOUNT_SIGN_IN_NOT_FOUND,
    );
    linker.script().approved = Some(grace);
    let signed_in = sign_in_with_account(&server, &started.handle, "code")
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
    linker.script().approved = Some(hedy);
    assert_refused(
        reauthenticate_with_account(device.clone(), &by_hedy.handle, "code").await,
        ErrorCode::Unauthenticated,
        RefusalReason::REFUSAL_REASON_WRONG_ACCOUNT,
    );
    let by_grace = start_account_sign_in(&server, ON_SERVER).await.unwrap();
    linker.script().approved = Some(grace);
    reauthenticate_with_account(device.clone(), &by_grace.handle, "code")
        .await
        .unwrap();
    start_user_link().await.unwrap();
}

#[tokio::test]
async fn with_the_hub_down_devices_stay_signed_in_but_accounts_cant_sign_in_or_reauthenticate() {
    let linker = Scripted::default();
    let server = linked_server(&linker).await;
    let grace = create_from_account(&server, "Grace").await.unwrap();
    let started = start_account_sign_in(&server, ON_SERVER).await.unwrap();
    linker.script().approved = Some(grace);
    let signed_in = sign_in_with_account(&server, &started.handle, "code")
        .await
        .unwrap();

    linker.script().link = None;

    let device = server.client_config_as(&signed_in.token);
    let not_confirmed = reauthenticate_with_account(device.clone(), &started.handle, "code").await;
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
    assert!(!offers_account_sign_in(&server).await);
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
    let linker = Scripted::default();
    let server = linked_server(&linker).await;
    let grace = create_from_account(&server, "Grace").await.unwrap();
    linker.script().user_link = Some(Link::Linked {
        account: "grace@example.com".to_owned(),
        reachability: None,
    });
    assert_eq!(cannot_sign_in(&server).await, Vec::<i64>::new());

    // As when Grace's Account is deleted on the Hub.
    linker.script().user_link = Some(Link::NotLinked);

    assert_eq!(cannot_sign_in(&server).await, [grace]);
}

#[tokio::test]
async fn a_user_who_signs_in_only_with_their_account_cant_unlink_from_it_but_can_cancel_a_code() {
    let linker = Scripted::default();
    let server = linked_server(&linker).await;
    let grace = create_from_account(&server, "Grace").await.unwrap();
    let started = start_account_sign_in(&server, ON_SERVER).await.unwrap();
    linker.script().approved = Some(grace);
    let signed_in = sign_in_with_account(&server, &started.handle, "code")
        .await
        .unwrap();
    linker.script().user_link = Some(Link::Linked {
        account: "grace@example.com".to_owned(),
        reachability: None,
    });
    let unlink = async || {
        users(server.client_config_as(&signed_in.token))
            .unlink_user(UnlinkUserRequest::default())
            .await
    };

    assert_refused(
        unlink().await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_LAST_SIGN_IN_METHOD,
    );
    assert_eq!(linker.script().unlinked_users, Vec::<i64>::new());
    // Unlinked on the Hub since, they can still stop showing the code they started to Link again.
    linker.script().user_link = Some(Link::Waiting {
        code: "CDFG-HJKL".to_owned(),
        url: "https://hub.example".to_owned(),
    });
    unlink().await.unwrap();
    assert_eq!(linker.script().unlinked_users, [grace]);
}

#[tokio::test]
async fn the_last_admin_who_signs_in_with_a_password_cant_be_demoted_or_deleted() {
    let linker = Scripted::default();
    let server = linked_server(&linker).await;
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
async fn the_plugin_is_told_the_server_url_and_port_the_admin_set() {
    let linker = Scripted::default();
    let server = with_linker(&linker).await;
    set_server_url(&server, "https://nama.example.com").await;

    let _restarted = TestServer::builder()
        .config(Config {
            forwarded_port: NonZeroU16::new(7443),
            ..Config::default()
        })
        .plugins(enabling(&linker))
        .restart(server)
        .await;

    // The test Server listens on 127.0.0.1, which no other device on its LAN can reach.
    let told = ServerAddresses {
        url: "https://nama.example.com/".to_owned(),
        lan: vec![],
        public_port: Some(7443),
    };
    let waited = tokio::time::timeout(server::WAIT, async {
        while linker.script().addresses.as_ref() != Some(&told) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    assert!(waited.is_ok(), "told {:?}", linker.script().addresses);
}
