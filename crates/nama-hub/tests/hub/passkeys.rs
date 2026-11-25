//! `AccountService`'s passkeys: adding, naming and removing them, signing in with one, and how
//! Sign-in Links bear on them (ADR 0100), against a real PostgreSQL.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/hub.rs"]
mod hub;
#[expect(dead_code, reason = "this binary uses only some of the helpers")]
#[path = "support/linking.rs"]
mod linking;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/mail.rs"]
mod mail;
#[path = "support/passkey.rs"]
mod passkey;
#[path = "support/refused.rs"]
mod refused;

use connectrpc::{ConnectError, ErrorCode, client::HttpClient};
use hub::TestHub;
use linking::{accounts, create, create_proven, hub_with_mail, session_token};
use mail::Inbox;
use nama_hub_proto::nama::hub::v1::{
    self as proto, AccountServiceClient, GetAccountRequest, ListPasskeysRequest, RefusalReason,
    RemovePasskeyRequest, SendSignInLinkRequest, SignInWithLinkRequest, StartAddingPasskeyRequest,
    StartPasskeySignInRequest,
};
use passkey::Passkey;
use refused::assert_refused;
use sqlx::PgPool;

/// The Hub's pages' Account, signed in with the session `token`.
fn signed_in(hub: &TestHub, token: &str) -> AccountServiceClient<HttpClient> {
    let config = hub
        .client_config()
        .with_default_header("cookie", format!("nama_hub_session={token}"));
    AccountServiceClient::new(HttpClient::plaintext(), config)
}

/// Adds `passkey`, named `name`, to the Account `token` is a session of.
async fn add_passkey(
    hub: &TestHub,
    token: &str,
    passkey: &mut Passkey,
    name: &str,
) -> Result<proto::Passkey, ConnectError> {
    let client = signed_in(hub, token);
    let started = client
        .start_adding_passkey(StartAddingPasskeyRequest::default())
        .await?
        .into_owned();
    let added = client
        .add_passkey(passkey.created(name, &started))
        .await?
        .into_owned();
    Ok(added.passkey.unwrap())
}

/// Signs in with `passkey`, and returns the session token.
async fn sign_in_with(hub: &TestHub, passkey: &Passkey) -> Result<String, ConnectError> {
    let client = accounts(hub);
    let started = client
        .start_passkey_sign_in(StartPasskeySignInRequest::default())
        .await?
        .into_owned();
    let signed_in = client
        .sign_in_with_passkey(passkey.signed(&started))
        .await?;
    Ok(session_token(signed_in.headers()))
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn passkey_names(hub: &TestHub, token: &str) -> Vec<String> {
    let listed = signed_in(hub, token)
        .list_passkeys(ListPasskeysRequest::default())
        .await
        .unwrap()
        .into_owned();
    listed
        .passkeys
        .into_iter()
        .map(|passkey| passkey.name)
        .collect()
}

async fn remove_passkey(hub: &TestHub, token: &str, id: &str) -> Result<(), ConnectError> {
    signed_in(hub, token)
        .remove_passkey(RemovePasskeyRequest {
            passkey_id: id.to_owned(),
            ..Default::default()
        })
        .await?;
    Ok(())
}

/// Uses the next Sign-in Link sent to `email`, and returns the session it signed in.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn use_a_link(hub: &TestHub, inbox: &mut Inbox, email: &str) -> String {
    accounts(hub)
        .send_sign_in_link(SendSignInLinkRequest {
            email: email.to_owned(),
            continue_to: "/".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();
    let used = accounts(hub)
        .sign_in_with_link(SignInWithLinkRequest {
            token: inbox.next_link(email).await,
            ..Default::default()
        })
        .await
        .unwrap();
    session_token(used.headers())
}

#[sqlx::test(migrations = false)]
async fn a_passkey_added_by_a_fresh_session_signs_in_without_an_email_or_password(db: PgPool) {
    let hub = TestHub::start(db, None).await;
    let token = create(&hub, "ada@example.com").await;
    let mut passkey = Passkey::new();

    let added = add_passkey(&hub, &token, &mut passkey, "  Phone ")
        .await
        .unwrap();
    let passkey_session = sign_in_with(&hub, &passkey).await.unwrap();

    assert_eq!(added.name, "Phone");
    assert_eq!(passkey_names(&hub, &token).await, ["Phone"]);
    assert_ne!(passkey_session, token);
    let account = signed_in(&hub, &passkey_session)
        .get_account(GetAccountRequest::default())
        .await
        .unwrap()
        .into_owned();
    assert_eq!(account.account.unwrap().email, "ada@example.com");
}

#[sqlx::test(migrations = false)]
async fn one_client_is_handed_only_so_many_passkey_challenges(db: PgPool) {
    let hub = TestHub::start(db, None).await;
    let client = accounts(&hub);

    let mut refused = None;
    for _ in 0..100 {
        if let Err(err) = client
            .start_passkey_sign_in(StartPasskeySignInRequest::default())
            .await
        {
            refused = Some(err);
            break;
        }
    }

    assert_refused(
        refused.map_or(Ok(()), Err),
        ErrorCode::ResourceExhausted,
        RefusalReason::REFUSAL_REASON_TOO_MANY_ATTEMPTS,
    );
}

#[sqlx::test(migrations = false)]
async fn a_used_challenge_an_unknown_passkey_a_forged_signature_or_no_user_handle_is_refused(
    db: PgPool,
) {
    let hub = TestHub::start(db, None).await;
    let token = create(&hub, "ada@example.com").await;
    let mut passkey = Passkey::new();
    add_passkey(&hub, &token, &mut passkey, "Phone")
        .await
        .unwrap();
    let client = accounts(&hub);
    let start = || async {
        client
            .start_passkey_sign_in(StartPasskeySignInRequest::default())
            .await
            .unwrap()
            .into_owned()
    };
    let answer = passkey.signed(&start().await);
    client.sign_in_with_passkey(answer.clone()).await.unwrap();

    let used_again = client.sign_in_with_passkey(answer).await;
    let unknown = sign_in_with(&hub, &Passkey::new()).await;
    let mut forged = passkey.signed(&start().await);
    forged.authenticator_data[32] = 0;
    let mut no_user_handle = passkey.signed(&start().await);
    no_user_handle.user_handle.clear();

    for refused in [
        used_again.map(drop),
        unknown.map(drop),
        client.sign_in_with_passkey(forged).await.map(drop),
        client.sign_in_with_passkey(no_user_handle).await.map(drop),
    ] {
        assert_refused(
            refused,
            ErrorCode::Unauthenticated,
            RefusalReason::REFUSAL_REASON_PASSKEY_NOT_ACCEPTED,
        );
    }
}

#[sqlx::test(migrations = false)]
async fn an_account_with_a_password_removes_its_only_passkey_which_then_cant_sign_in(db: PgPool) {
    let hub = TestHub::start(db, None).await;
    let token = create(&hub, "ada@example.com").await;
    let mut passkey = Passkey::new();
    let added = add_passkey(&hub, &token, &mut passkey, "Phone")
        .await
        .unwrap();

    remove_passkey(&hub, &token, &added.id).await.unwrap();

    assert_eq!(passkey_names(&hub, &token).await, Vec::<String>::new());
    assert_refused(
        sign_in_with(&hub, &passkey).await,
        ErrorCode::Unauthenticated,
        RefusalReason::REFUSAL_REASON_PASSKEY_NOT_ACCEPTED,
    );
}

#[sqlx::test(migrations = false)]
async fn without_a_password_the_last_passkey_stays_unless_the_hub_sends_sign_in_links(db: PgPool) {
    let (with_mail, mut inbox) = hub_with_mail(db.clone()).await;
    let without_mail = TestHub::start(db, None).await;
    // The link takes the Account over, which removes its password (ADR 0100).
    let token = create_proven(&with_mail, &mut inbox, "ada@example.com").await;
    let first = add_passkey(&with_mail, &token, &mut Passkey::new(), "Phone")
        .await
        .unwrap();
    let last = add_passkey(&with_mail, &token, &mut Passkey::new(), "Laptop")
        .await
        .unwrap();

    remove_passkey(&without_mail, &token, &first.id)
        .await
        .unwrap();
    assert_refused(
        remove_passkey(&without_mail, &token, &last.id).await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_LAST_SIGN_IN_METHOD,
    );
    remove_passkey(&with_mail, &token, &last.id).await.unwrap();

    assert_eq!(
        passkey_names(&with_mail, &token).await,
        Vec::<String>::new()
    );
}

#[sqlx::test(migrations = false)]
async fn a_sign_in_link_taking_over_an_unproven_account_removes_its_passkeys(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let token = create(&hub, "ada@example.com").await;
    let mut passkey = Passkey::new();
    add_passkey(&hub, &token, &mut passkey, "Phone")
        .await
        .unwrap();

    let owner = use_a_link(&hub, &mut inbox, "ada@example.com").await;
    let mut owners = Passkey::new();
    add_passkey(&hub, &owner, &mut owners, "Owner's")
        .await
        .unwrap();
    use_a_link(&hub, &mut inbox, "ada@example.com").await;

    assert_eq!(passkey_names(&hub, &owner).await, ["Owner's"]);
    assert_refused(
        sign_in_with(&hub, &passkey).await,
        ErrorCode::Unauthenticated,
        RefusalReason::REFUSAL_REASON_PASSKEY_NOT_ACCEPTED,
    );
    sign_in_with(&hub, &owners).await.unwrap();
}

#[sqlx::test(migrations = false)]
async fn a_passkey_name_breaking_the_rules_is_refused(db: PgPool) {
    let hub = TestHub::start(db, None).await;
    let token = create(&hub, "ada@example.com").await;

    for name in ["   ".to_owned(), "x".repeat(65)] {
        let refused = add_passkey(&hub, &token, &mut Passkey::new(), &name).await;

        let refusal = assert_refused(
            refused,
            ErrorCode::InvalidArgument,
            RefusalReason::REFUSAL_REASON_PASSKEY_NAME_LENGTH,
        );
        assert_eq!(refusal.field, "name");
        assert_eq!(refusal.params["minLength"], "1");
        assert_eq!(refusal.params["maxLength"], "64");
    }
}

#[sqlx::test(migrations = false)]
async fn each_account_sees_and_removes_only_its_own_passkeys(db: PgPool) {
    let hub = TestHub::start(db, None).await;
    let ada = create(&hub, "ada@example.com").await;
    let bea = create(&hub, "bea@example.com").await;
    let mut adas = Passkey::new();
    let added = add_passkey(&hub, &ada, &mut adas, "Phone").await.unwrap();
    let start_adding = |token| {
        let client = signed_in(&hub, token);
        async move {
            client
                .start_adding_passkey(StartAddingPasskeyRequest::default())
                .await
                .unwrap()
                .into_owned()
        }
    };

    let bea_starts = start_adding(&bea).await;
    let ada_starts = start_adding(&ada).await;
    let removed_by_bea = remove_passkey(&hub, &bea, &added.id).await;
    let taken_by_bea = add_passkey(&hub, &bea, &mut adas, "Also mine").await;

    assert_eq!(passkey_names(&hub, &bea).await, Vec::<String>::new());
    assert_eq!(bea_starts.exclude_credential_ids, Vec::<Vec<u8>>::new());
    assert_eq!(
        ada_starts.exclude_credential_ids,
        [adas.credential_id.clone()]
    );
    assert_ne!(ada_starts.user_id, bea_starts.user_id);
    assert_refused(
        removed_by_bea,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_PASSKEY_NOT_FOUND,
    );
    assert_refused(
        taken_by_bea,
        ErrorCode::Unauthenticated,
        RefusalReason::REFUSAL_REASON_PASSKEY_NOT_ACCEPTED,
    );
    assert_eq!(passkey_names(&hub, &ada).await, ["Phone"]);
}

#[sqlx::test(migrations = false)]
async fn changing_passkeys_needs_a_session(db: PgPool) {
    let hub = TestHub::start(db, None).await;
    let client = signed_in(&hub, "not-a-session");

    for refused in [
        client
            .start_adding_passkey(StartAddingPasskeyRequest::default())
            .await
            .map(drop),
        client
            .list_passkeys(ListPasskeysRequest::default())
            .await
            .map(drop),
        remove_passkey(&hub, "not-a-session", "").await,
    ] {
        assert_refused(
            refused,
            ErrorCode::Unauthenticated,
            RefusalReason::REFUSAL_REASON_SIGN_IN_REQUIRED,
        );
    }
}
