//! Users an Admin created from an Account, which it accepts, and signing in to a Linked Server
//! with an Account (ADR 0092), against a real PostgreSQL.

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
    AcceptUserInviteRequest, ApproveServerSignInRequest, GetServerSignInRequest, InviteUserRequest,
    ListLinkedServersRequest, ListUserInvitesRequest, PollServerSignInRequest, RefusalReason,
    StartServerSignInRequest, StartServerSignInResponse,
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
async fn unlinking_a_user_forgets_the_account_asked_to_accept_them(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let server = linked(&hub, &ada, "Ada's").await;
    invite(&hub, &server, 7, "ada@example.com").await;

    unlink_user(&hub, &server, 7).await;

    assert_eq!(invites(&hub, &ada).await, []);
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn start_sign_in(hub: &TestHub, credential: &str) -> StartServerSignInResponse {
    linked_server(hub, credential)
        .start_server_sign_in(StartServerSignInRequest {
            return_url: "https://nama.example/sign-in".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
}

async fn poll_sign_in(
    hub: &TestHub,
    credential: &str,
    device_code: &str,
) -> Result<String, connectrpc::ConnectError> {
    Ok(linked_server(hub, credential)
        .poll_server_sign_in(PollServerSignInRequest {
            device_code: device_code.to_owned(),
            ..Default::default()
        })
        .await?
        .into_owned()
        .token)
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
    let code = start_user(&hub, &server, 7).await.unwrap();
    approve_user(&hub, &bea, &code).await.unwrap();
    let started = start_sign_in(&hub, &server).await;
    assert_eq!(
        poll_sign_in(&hub, &server, &started.device_code)
            .await
            .unwrap(),
        ""
    );

    assert_eq!(
        sign_in_page(&hub, &bea, &started.id).await.unwrap(),
        ("Ada's".to_owned(), "Grace".to_owned())
    );
    let back = approve_sign_in(&hub, &bea, &started.id).await.unwrap();

    assert_eq!(back, "https://nama.example/sign-in");
    let token = poll_sign_in(&hub, &server, &started.device_code)
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
        poll_sign_in(&hub, &server, &started.device_code).await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_LINK_EXPIRED,
    );
    assert_refused(
        approve_sign_in(&hub, &bea, &started.id).await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_SERVER_SIGN_IN_NOT_FOUND,
    );
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
async fn an_account_with_two_users_on_a_server_signs_in_as_the_one_linked_first(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let server = linked(&hub, &ada, "Ada's").await;
    for user_id in [7, 8] {
        let code = start_user(&hub, &server, user_id).await.unwrap();
        approve_user(&hub, &ada, &code).await.unwrap();
    }
    let started = start_sign_in(&hub, &server).await;

    assert_eq!(
        sign_in_page(&hub, &ada, &started.id).await.unwrap(),
        ("Ada's".to_owned(), "Grace".to_owned())
    );
    approve_sign_in(&hub, &ada, &started.id).await.unwrap();
    let token = poll_sign_in(&hub, &server, &started.device_code)
        .await
        .unwrap();
    assert_eq!(verified_claims(&hub, &token).await["sub"], "7");
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
    approve_sign_in(&hub, &ada, &started.id).await.unwrap();

    assert_refused(
        poll_sign_in(&hub, &other, &started.device_code).await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_LINK_EXPIRED,
    );
}
