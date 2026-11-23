//! `AccountService.DeleteAccount`: an Account deleting itself, and what its Linked Servers see
//! after, against a real PostgreSQL.

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

use connectrpc::{ConnectError, ErrorCode, client::HttpClient};
use hub::TestHub;
use linking::*;
use nama_hub_proto::nama::hub::v1::{
    AccountServiceClient, DeleteAccountRequest, GetAccountRequest, GetUserLinkRequest,
    RefusalReason,
};
use refused::assert_refused;
use sqlx::PgPool;

/// The Hub's pages' Account, signed in with the session `token`.
fn signed_in(hub: &TestHub, token: &str) -> AccountServiceClient<HttpClient> {
    let config = hub
        .client_config()
        .with_default_header("cookie", format!("nama_hub_session={token}"));
    AccountServiceClient::new(HttpClient::plaintext(), config)
}

/// Deletes the Account signed in with the session `token`, and returns the `Set-Cookie` sent.
async fn delete(hub: &TestHub, token: &str) -> Result<String, ConnectError> {
    let deleted = signed_in(hub, token)
        .delete_account(DeleteAccountRequest::default())
        .await?;
    let cookie = deleted.headers().get("set-cookie");
    Ok(cookie
        .and_then(|cookie| cookie.to_str().ok())
        .unwrap_or_default()
        .to_owned())
}

#[sqlx::test(migrations = false)]
async fn deleting_an_account_signs_it_out_and_unlinks_its_users(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let grace = create_proven(&hub, &mut inbox, "grace@example.com").await;
    let server = linked(&hub, &ada, "Living room").await;
    let code = start_user(&hub, &server, 7).await.unwrap();
    approve_user(&hub, &grace, &code).await.unwrap();

    let cookie = delete(&hub, &grace).await.unwrap();

    assert!(cookie.starts_with("nama_hub_session=;"), "{cookie}");
    assert!(cookie.contains("Max-Age=0"), "{cookie}");
    assert_eq!(user_account(&hub, &server, 7).await, "");
    assert_refused(
        (signed_in(&hub, &grace))
            .get_account(GetAccountRequest::default())
            .await,
        ErrorCode::Unauthenticated,
        RefusalReason::REFUSAL_REASON_SIGN_IN_REQUIRED,
    );
    assert_eq!(server_names(&hub, &ada).await, ["Living room"]);
}

#[sqlx::test(migrations = false)]
async fn deleting_the_account_a_server_linked_with_unlinks_the_server(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let server = linked(&hub, &ada, "Living room").await;

    delete(&hub, &ada).await.unwrap();

    assert_refused(
        linked_server(&hub, &server)
            .get_user_link(GetUserLinkRequest {
                user_id: 7,
                ..Default::default()
            })
            .await,
        ErrorCode::Unauthenticated,
        RefusalReason::REFUSAL_REASON_SERVER_NOT_LINKED,
    );
}

#[sqlx::test(migrations = false)]
async fn deleting_an_account_needs_a_session(db: PgPool) {
    let hub = TestHub::start(db, None).await;

    assert_refused(
        delete(&hub, "not-a-session").await,
        ErrorCode::Unauthenticated,
        RefusalReason::REFUSAL_REASON_SIGN_IN_REQUIRED,
    );
}
