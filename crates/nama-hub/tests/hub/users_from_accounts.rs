//! Users an Admin created from an Account, which it accepts, and signing in to a Linked Server
//! with an Account (ADR 0092), and the tokens the web app at the Hub trades at a Linked Server for a
//! Device (ADR 0105), against a real PostgreSQL.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/hub.rs"]
mod hub;
#[expect(dead_code, reason = "this binary uses only some of the helpers")]
#[path = "support/linking.rs"]
mod linking;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/mail.rs"]
mod mail;
#[path = "support/refused.rs"]
mod refused;

use connectrpc::ErrorCode;
use hub::TestHub;
use linking::*;
use nama_hub_proto::nama::hub::v1::{
    AcceptUserInviteRequest, ApproveServerSignInRequest, CreateServerTokenRequest,
    GetServerSignInRequest, InviteUserRequest, ListLinkedServersRequest, ListUserInvitesRequest,
    PollServerSignInRequest, RefusalReason, SetServerAddressesRequest, StartServerSignInRequest,
    StartServerSignInResponse,
};
use refused::assert_refused;
use sqlx::PgPool;
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn invite(hub: &TestHub, credential: &str, user_id: i64, email: &str) {
    linked_server(hub, credential)
        .invite_user(InviteUserRequest {
            user_id,
            name: "Grace".to_owned(),
            email: email.to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();
}

/// Each User waiting for the signed-in Account, as `<id> <User> on <Server>`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn invites(hub: &TestHub, token: &str) -> Vec<(String, String)> {
    let listed = links(hub, token)
        .list_user_invites(ListUserInvitesRequest::default())
        .await
        .unwrap()
        .into_owned();
    listed
        .invites
        .into_iter()
        .map(|invite| {
            (
                invite.id,
                format!("{} on {}", invite.name, invite.server_name),
            )
        })
        .collect()
}

async fn accept(hub: &TestHub, token: &str, id: &str) -> Result<(), connectrpc::ConnectError> {
    links(hub, token)
        .accept_user_invite(AcceptUserInviteRequest {
            id: id.to_owned(),
            ..Default::default()
        })
        .await?;
    Ok(())
}

#[sqlx::test(migrations = false)]
async fn a_user_created_from_an_account_is_linked_once_it_accepts(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let bea = create_proven(&hub, &mut inbox, "bea@example.com").await;
    let server = linked(&hub, &ada, "Ada's").await;

    invite(&hub, &server, 7, "BEA@example.com").await;
    invite(&hub, &server, 8, "nobody@example.com").await;
    let waiting = invites(&hub, &bea).await;
    assert_eq!(
        waiting.iter().map(|(_, user)| user).collect::<Vec<_>>(),
        ["Grace on Ada's"]
    );
    assert_eq!(invites(&hub, &ada).await, []);
    assert_eq!(user_account(&hub, &server, 7).await, "");
    let (id, _) = &waiting[0];
    assert_refused(
        accept(&hub, &ada, id).await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_USER_INVITE_NOT_FOUND,
    );

    accept(&hub, &bea, id).await.unwrap();

    assert_eq!(user_account(&hub, &server, 7).await, "bea@example.com");
    assert_eq!(linked_users(&hub, &bea).await, ["Grace on Ada's"]);
    assert_eq!(invites(&hub, &bea).await, []);
}

#[sqlx::test(migrations = false)]
async fn an_account_whose_email_isnt_proven_cant_accept_a_user(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let bea = create(&hub, "bea@example.com").await;
    let server = linked(&hub, &ada, "Ada's").await;
    invite(&hub, &server, 7, "bea@example.com").await;
    let (id, _) = &invites(&hub, &bea).await[0];

    assert_refused(
        accept(&hub, &bea, id).await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_EMAIL_NOT_PROVEN,
    );
    assert_eq!(user_account(&hub, &server, 7).await, "");
}

#[sqlx::test(migrations = false)]
async fn on_a_hub_without_mail_an_unproven_account_cant_accept_a_user(db: PgPool) {
    let (with_mail, mut inbox) = hub_with_mail(db.clone()).await;
    let without_mail = TestHub::start(db, None).await;
    let ada = create_proven(&with_mail, &mut inbox, "ada@example.com").await;
    let bea = create(&without_mail, "bea@example.com").await;
    let server = linked(&with_mail, &ada, "Ada's").await;
    invite(&without_mail, &server, 7, "bea@example.com").await;
    let (id, _) = &invites(&without_mail, &bea).await[0];

    assert_refused(
        accept(&without_mail, &bea, id).await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_SIGN_IN_LINKS_OFF,
    );
    assert_eq!(user_account(&without_mail, &server, 7).await, "");
}

#[sqlx::test(migrations = false)]
async fn unlinking_a_user_forgets_the_account_asked_to_accept_them(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let server = linked(&hub, &ada, "Ada's").await;
    invite(&hub, &server, 7, "ada@example.com").await;

    unlink_user(&hub, &server, 7).await;

    assert_eq!(invites(&hub, &ada).await, []);
}

/// The Server URL the Servers in these tests report, and their sign-ins return to.
const SERVER_URL: &str = "https://nama.example";

/// Starts a sign-in to the Server presenting `credential`, returning to `return_url`.
async fn start_sign_in_to(
    hub: &TestHub,
    credential: &str,
    return_url: &str,
) -> Result<StartServerSignInResponse, connectrpc::ConnectError> {
    let server = linked_server(hub, credential);
    server
        .set_server_addresses(SetServerAddressesRequest {
            url: SERVER_URL.to_owned(),
            lan_addresses: vec!["http://192.168.1.10:7980".to_owned()],
            ..Default::default()
        })
        .await?;
    Ok(server
        .start_server_sign_in(StartServerSignInRequest {
            return_url: return_url.to_owned(),
            ..Default::default()
        })
        .await?
        .into_owned())
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn start_sign_in(hub: &TestHub, credential: &str) -> StartServerSignInResponse {
    start_sign_in_to(hub, credential, &format!("{SERVER_URL}/sign-in"))
        .await
        .unwrap()
}

async fn poll_sign_in(
    hub: &TestHub,
    credential: &str,
    device_code: &str,
    code: &str,
) -> Result<String, connectrpc::ConnectError> {
    Ok(linked_server(hub, credential)
        .poll_server_sign_in(PollServerSignInRequest {
            device_code: device_code.to_owned(),
            code: code.to_owned(),
            ..Default::default()
        })
        .await?
        .into_owned()
        .token)
}

/// The one-time code in the address approving sends the person back to, which must be the
/// sign-in's return address on [`SERVER_URL`] with only the code added.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn code_in(back: &str) -> &str {
    back.strip_prefix(&format!("{SERVER_URL}/sign-in?code="))
        .unwrap()
}

/// The Server and the Account's User there that the sign-in `id` names, as its page shows them.
async fn sign_in_page(
    hub: &TestHub,
    token: &str,
    id: &str,
) -> Result<(String, String), connectrpc::ConnectError> {
    let got = links(hub, token)
        .get_server_sign_in(GetServerSignInRequest {
            id: id.to_owned(),
            ..Default::default()
        })
        .await?
        .into_owned();
    Ok((got.server_name, got.user_name))
}

async fn approve_sign_in(
    hub: &TestHub,
    token: &str,
    id: &str,
) -> Result<String, connectrpc::ConnectError> {
    Ok(links(hub, token)
        .approve_server_sign_in(ApproveServerSignInRequest {
            id: id.to_owned(),
            ..Default::default()
        })
        .await?
        .into_owned()
        .return_url)
}

#[sqlx::test(migrations = false)]
async fn an_account_signs_in_to_a_server_as_its_user_there(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let bea = create_proven(&hub, &mut inbox, "bea@example.com").await;
    let server = linked(&hub, &ada, "Ada's").await;
    let user_code = start_user(&hub, &server, 7).await.unwrap();
    approve_user(&hub, &bea, &user_code).await.unwrap();
    let started = start_sign_in(&hub, &server).await;
    assert_eq!(
        poll_sign_in(&hub, &server, &started.device_code, "")
            .await
            .unwrap(),
        ""
    );

    assert_eq!(
        sign_in_page(&hub, &bea, &started.id).await.unwrap(),
        ("Ada's".to_owned(), "Grace".to_owned())
    );
    let back = approve_sign_in(&hub, &bea, &started.id).await.unwrap();

    let code = code_in(&back);
    assert_eq!(code.len(), 64);
    let token = poll_sign_in(&hub, &server, &started.device_code, code)
        .await
        .unwrap();
    let claims = verified_claims(&hub, &token).await;
    assert_eq!(claims["sub"], "7");
    let servers = links(&hub, &ada)
        .list_linked_servers(ListLinkedServersRequest::default())
        .await
        .unwrap()
        .into_owned();
    assert_eq!(claims["aud"], servers.servers[0].id.as_str());
    assert_refused(
        poll_sign_in(&hub, &server, &started.device_code, code).await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_LINK_EXPIRED,
    );
    assert_refused(
        approve_sign_in(&hub, &bea, &started.id).await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_SERVER_SIGN_IN_NOT_FOUND,
    );
}

/// The token the Account signed in with `token` gets for the Linked Server `linked_server_id`.
async fn server_token(
    hub: &TestHub,
    token: &str,
    linked_server_id: &str,
) -> Result<String, connectrpc::ConnectError> {
    Ok(links(hub, token)
        .create_server_token(CreateServerTokenRequest {
            linked_server_id: linked_server_id.to_owned(),
            ..Default::default()
        })
        .await?
        .into_owned()
        .token)
}

#[sqlx::test(migrations = false)]
async fn an_account_gets_a_server_token_only_for_its_user_on_that_server(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let bea = create_proven(&hub, &mut inbox, "bea@example.com").await;
    let server = linked(&hub, &ada, "Ada's").await;
    linked(&hub, &ada, "Ada's other").await;
    let user_code = start_user(&hub, &server, 7).await.unwrap();
    approve_user(&hub, &bea, &user_code).await.unwrap();
    let servers = links(&hub, &ada)
        .list_linked_servers(ListLinkedServersRequest::default())
        .await
        .unwrap()
        .into_owned()
        .servers;
    let (with_bea, other) = (&servers[0].id, &servers[1].id);

    let token = server_token(&hub, &bea, with_bea).await.unwrap();

    let claims = verified_claims(&hub, &token).await;
    assert_eq!(claims["sub"], "7");
    assert_eq!(claims["aud"], with_bea.as_str());
    let again = verified_claims(&hub, &server_token(&hub, &bea, with_bea).await.unwrap()).await;
    assert_ne!(
        claims["jti"], again["jti"],
        "each token has its own id, so a Server takes it once"
    );
    for (account, linked_server_id) in [
        (&ada, with_bea.as_str()),
        (&bea, other),
        (&bea, "not a uuid"),
    ] {
        assert_refused(
            server_token(&hub, account, linked_server_id).await,
            ErrorCode::FailedPrecondition,
            RefusalReason::REFUSAL_REASON_NO_USER_ON_SERVER,
        );
    }
}

#[sqlx::test(migrations = false)]
async fn an_account_without_a_user_on_the_server_cant_sign_in_to_it(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let server = linked(&hub, &ada, "Ada's").await;
    let started = start_sign_in(&hub, &server).await;

    assert_eq!(
        sign_in_page(&hub, &ada, &started.id).await.unwrap(),
        ("Ada's".to_owned(), String::new())
    );
    assert_refused(
        approve_sign_in(&hub, &ada, &started.id).await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_NO_USER_ON_SERVER,
    );
    assert_refused(
        sign_in_page(&hub, &ada, "not a uuid").await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_SERVER_SIGN_IN_NOT_FOUND,
    );
    let refused = linked_server(&hub, &server)
        .start_server_sign_in(StartServerSignInRequest {
            return_url: "javascript:alert(1)".to_owned(),
            ..Default::default()
        })
        .await;
    assert_refused(
        refused,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_URL_INVALID,
    );
}

#[sqlx::test(migrations = false)]
async fn an_account_with_a_user_on_the_server_cant_accept_another_there(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let server = linked(&hub, &ada, "Ada's").await;
    let code = start_user(&hub, &server, 7).await.unwrap();
    approve_user(&hub, &ada, &code).await.unwrap();
    invite(&hub, &server, 8, "ada@example.com").await;
    let (id, _) = &invites(&hub, &ada).await[0];

    assert_refused(
        accept(&hub, &ada, id).await,
        ErrorCode::AlreadyExists,
        RefusalReason::REFUSAL_REASON_ACCOUNT_HAS_USER_ON_SERVER,
    );
    assert_eq!(user_account(&hub, &server, 8).await, "");
    // Still waiting, to accept once the other User is Unlinked.
    assert_eq!(invites(&hub, &ada).await.len(), 1);
}

#[sqlx::test(migrations = false)]
async fn another_linked_server_cant_collect_a_sign_in(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let server = linked(&hub, &ada, "Ada's").await;
    let other = linked(&hub, &ada, "Other").await;
    let code = start_user(&hub, &server, 7).await.unwrap();
    approve_user(&hub, &ada, &code).await.unwrap();
    let started = start_sign_in(&hub, &server).await;
    let back = approve_sign_in(&hub, &ada, &started.id).await.unwrap();

    assert_refused(
        poll_sign_in(&hub, &other, &started.device_code, code_in(&back)).await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_LINK_EXPIRED,
    );
}

#[sqlx::test(migrations = false)]
async fn a_sign_in_returns_only_to_one_of_the_servers_addresses(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let server = linked(&hub, &ada, "Ada's").await;

    for elsewhere in [
        "https://attacker.example/sign-in",
        "http://nama.example/sign-in",
        "http://192.168.1.10:7981/sign-in",
    ] {
        assert_refused(
            start_sign_in_to(&hub, &server, elsewhere).await,
            ErrorCode::InvalidArgument,
            RefusalReason::REFUSAL_REASON_URL_INVALID,
        );
    }
    let user_code = start_user(&hub, &server, 7).await.unwrap();
    approve_user(&hub, &ada, &user_code).await.unwrap();
    let started = start_sign_in_to(&hub, &server, "http://192.168.1.10:7980/sign-in")
        .await
        .unwrap();
    let back = approve_sign_in(&hub, &ada, &started.id).await.unwrap();
    assert!(
        back.starts_with("http://192.168.1.10:7980/sign-in?code="),
        "{back}"
    );
}

#[sqlx::test(migrations = false)]
async fn a_sign_in_polled_without_the_code_approving_sent_back_ends(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let server = linked(&hub, &ada, "Ada's").await;
    let user_code = start_user(&hub, &server, 7).await.unwrap();
    approve_user(&hub, &ada, &user_code).await.unwrap();

    for wrong in ["", "0".repeat(64).as_str()] {
        let started = start_sign_in(&hub, &server).await;
        let back = approve_sign_in(&hub, &ada, &started.id).await.unwrap();

        assert_refused(
            poll_sign_in(&hub, &server, &started.device_code, wrong).await,
            ErrorCode::NotFound,
            RefusalReason::REFUSAL_REASON_LINK_EXPIRED,
        );
        assert_refused(
            poll_sign_in(&hub, &server, &started.device_code, code_in(&back)).await,
            ErrorCode::NotFound,
            RefusalReason::REFUSAL_REASON_LINK_EXPIRED,
        );
    }
}
