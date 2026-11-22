//! `AccountService`: creating an Account, and signing it in, with its password or a Sign-in Link,
//! and out, against a real PostgreSQL.

#[path = "support/hub.rs"]
mod hub;
#[path = "support/mail.rs"]
mod mail;
#[path = "support/refused.rs"]
mod refused;

use connectrpc::{
    ConnectError, ErrorCode,
    client::{HttpClient, UnaryResponse},
};
use hub::TestHub;
use mail::{Inbox, fake_mail};
use nama_hub_proto::nama::hub::v1::{
    AccountServiceClient, CreateAccountRequest, GetAccountRequest, GetHubInfoRequest,
    GetSignInLinkRequest, GetSignInLinkResponse, HubServiceClient, RefusalReason,
    SendSignInLinkRequest, SignInRequest, SignInWithLinkRequest, SignInWithLinkResponse,
    SignOutRequest,
};
use refused::assert_refused;
use sqlx::PgPool;

const PASSWORD: &str = "correct horse battery";

/// A client of `hub` that presents the session `token`, if any, in its cookie, as a browser does.
fn accounts(hub: &TestHub, token: Option<&str>) -> AccountServiceClient<HttpClient> {
    let mut config = hub.client_config();
    if let Some(token) = token {
        config =
            config.with_default_header("cookie", format!("theme=dark; nama_hub_session={token}"));
    }
    AccountServiceClient::new(HttpClient::plaintext(), config)
}

fn create_request(email: &str, password: &str) -> CreateAccountRequest {
    CreateAccountRequest {
        email: email.to_owned(),
        password: password.to_owned(),
        ..Default::default()
    }
}

fn sign_in_request(email: &str, password: &str) -> SignInRequest {
    SignInRequest {
        email: email.to_owned(),
        password: password.to_owned(),
        ..Default::default()
    }
}

/// The token in the session cookie `response` sets, and the cookie's attributes in the order sent.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn session_cookie<T>(response: &UnaryResponse<T>) -> (String, Vec<String>) {
    let cookie = response
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap();
    let mut parts = cookie.split("; ");
    let token = parts
        .next()
        .unwrap()
        .strip_prefix("nama_hub_session=")
        .unwrap();
    (token.to_owned(), parts.map(str::to_owned).collect())
}

/// Creates an Account with `email` and [`PASSWORD`], and returns its session token.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn create(hub: &TestHub, email: &str) -> String {
    let created = accounts(hub, None)
        .create_account(create_request(email, PASSWORD))
        .await
        .unwrap();
    session_cookie(&created).0
}

async fn signed_in_email(hub: &TestHub, token: &str) -> Result<String, ConnectError> {
    let account = accounts(hub, Some(token))
        .get_account(GetAccountRequest::default())
        .await?
        .into_owned();
    Ok(account.account.unwrap().email)
}

#[sqlx::test(migrations = false)]
async fn creating_an_account_signs_it_in_with_a_strict_http_only_cookie(db: PgPool) {
    let hub = TestHub::start(db, None).await;

    let created = accounts(&hub, None)
        .create_account(create_request(" Ada@example.com ", PASSWORD))
        .await
        .unwrap();

    let (token, attributes) = session_cookie(&created);
    assert_eq!(
        attributes,
        ["HttpOnly", "SameSite=Strict", "Path=/", "Max-Age=2592000"]
    );
    let account = created.into_owned().account.unwrap();
    assert_eq!(account.email, "Ada@example.com");
    let fetched = accounts(&hub, Some(&token))
        .get_account(GetAccountRequest::default())
        .await
        .unwrap()
        .into_owned()
        .account
        .unwrap();
    assert_eq!(fetched, account);
}

#[sqlx::test(migrations = false)]
async fn an_email_another_account_has_in_any_case_is_refused(db: PgPool) {
    let hub = TestHub::start(db, None).await;
    create(&hub, "ada@example.com").await;

    let taken = accounts(&hub, None)
        .create_account(create_request("ADA@Example.com", "another password"))
        .await;

    let refusal = assert_refused(
        taken,
        ErrorCode::AlreadyExists,
        RefusalReason::REFUSAL_REASON_EMAIL_TAKEN,
    );
    assert_eq!(refusal.field, "email");
}

#[sqlx::test(migrations = false)]
async fn an_email_or_a_password_breaking_the_rules_is_refused(db: PgPool) {
    let hub = TestHub::start(db, None).await;
    let client = accounts(&hub, None);

    let not_an_email = client
        .create_account(create_request("ada.example.com", PASSWORD))
        .await;
    let too_short = client
        .create_account(create_request("ada@example.com", "seven c"))
        .await;

    let email_refusal = assert_refused(
        not_an_email,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_EMAIL_INVALID,
    );
    assert_eq!(email_refusal.field, "email");
    let password_refusal = assert_refused(
        too_short,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_PASSWORD_LENGTH,
    );
    assert_eq!(password_refusal.field, "password");
    assert_eq!(password_refusal.params["minLength"], "8");
    assert_eq!(password_refusal.params["maxLength"], "1024");
    let too_long = client
        .create_account(create_request("ada@example.com", &"x".repeat(1025)))
        .await;
    assert_refused(
        too_long,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_PASSWORD_LENGTH,
    );
}

#[sqlx::test(migrations = false)]
async fn signing_in_with_the_password_starts_a_session_of_its_own(db: PgPool) {
    let hub = TestHub::start(db, None).await;
    let first = create(&hub, "ada@example.com").await;

    let signed_in = accounts(&hub, None)
        .sign_in(sign_in_request("ADA@example.com", PASSWORD))
        .await
        .unwrap();

    let (second, attributes) = session_cookie(&signed_in);
    assert_ne!(second, first);
    assert!(attributes.contains(&"SameSite=Strict".to_owned()));
    assert_eq!(
        signed_in_email(&hub, &second).await.unwrap(),
        "ada@example.com"
    );
}

#[sqlx::test(migrations = false)]
async fn a_wrong_password_or_an_unknown_email_is_refused_without_saying_which(db: PgPool) {
    let hub = TestHub::start(db, None).await;
    create(&hub, "ada@example.com").await;
    let client = accounts(&hub, None);

    for (email, password) in [
        ("ada@example.com", "not the password"),
        ("bea@example.com", PASSWORD),
    ] {
        let refused = client.sign_in(sign_in_request(email, password)).await;

        assert_refused(
            refused,
            ErrorCode::Unauthenticated,
            RefusalReason::REFUSAL_REASON_WRONG_EMAIL_OR_PASSWORD,
        );
    }
}

#[sqlx::test(migrations = false)]
async fn signing_out_ends_only_its_own_session_and_clears_the_cookie(db: PgPool) {
    let hub = TestHub::start(db, None).await;
    let first = create(&hub, "ada@example.com").await;
    let signed_in = accounts(&hub, None)
        .sign_in(sign_in_request("ada@example.com", PASSWORD))
        .await
        .unwrap();
    let second = session_cookie(&signed_in).0;

    let signed_out = accounts(&hub, Some(&first))
        .sign_out(SignOutRequest::default())
        .await
        .unwrap();

    let (cleared, attributes) = session_cookie(&signed_out);
    assert_eq!(cleared, "");
    assert!(attributes.contains(&"Max-Age=0".to_owned()));
    assert_refused(
        signed_in_email(&hub, &first).await,
        ErrorCode::Unauthenticated,
        RefusalReason::REFUSAL_REASON_SIGN_IN_REQUIRED,
    );
    assert_eq!(
        signed_in_email(&hub, &second).await.unwrap(),
        "ada@example.com"
    );
}

#[sqlx::test(migrations = false)]
async fn signing_out_or_reading_the_account_without_a_session_needs_sign_in(db: PgPool) {
    let hub = TestHub::start(db, None).await;
    let client = accounts(&hub, Some("not-a-session"));

    assert_refused(
        client.sign_out(SignOutRequest::default()).await,
        ErrorCode::Unauthenticated,
        RefusalReason::REFUSAL_REASON_SIGN_IN_REQUIRED,
    );
    assert_refused(
        client.get_account(GetAccountRequest::default()).await,
        ErrorCode::Unauthenticated,
        RefusalReason::REFUSAL_REASON_SIGN_IN_REQUIRED,
    );
}

#[sqlx::test(migrations = false)]
async fn each_session_reads_and_signs_out_only_its_own_account(db: PgPool) {
    let hub = TestHub::start(db, None).await;
    let ada = create(&hub, "ada@example.com").await;
    let bea = create(&hub, "bea@example.com").await;

    assert_eq!(
        signed_in_email(&hub, &ada).await.unwrap(),
        "ada@example.com"
    );
    assert_eq!(
        signed_in_email(&hub, &bea).await.unwrap(),
        "bea@example.com"
    );
    accounts(&hub, Some(&ada))
        .sign_out(SignOutRequest::default())
        .await
        .unwrap();
    assert_eq!(
        signed_in_email(&hub, &bea).await.unwrap(),
        "bea@example.com"
    );
}

#[sqlx::test(migrations = false)]
async fn too_many_failed_sign_ins_for_an_email_refuse_even_the_right_password(db: PgPool) {
    let hub = TestHub::start(db, None).await;
    create(&hub, "ada@example.com").await;
    let client = accounts(&hub, None);
    for _ in 0..10 {
        let wrong = client
            .sign_in(sign_in_request("ada@example.com", "not the password"))
            .await;
        assert_eq!(wrong.unwrap_err().code, ErrorCode::Unauthenticated);
    }

    let blocked = client
        .sign_in(sign_in_request("ADA@example.com", PASSWORD))
        .await;

    assert_too_many_attempts(blocked);
}

#[sqlx::test(migrations = false)]
async fn a_session_the_hub_cant_look_up_isnt_taken_for_a_sign_out(db: PgPool) {
    let hub = TestHub::start(db, None).await;
    let token = create(&hub, "ada@example.com").await;
    hub.lose_database().await;

    let err = signed_in_email(&hub, &token).await.unwrap_err();

    assert_ne!(err.code, ErrorCode::Unauthenticated, "{err:?}");
}

/// A Hub that emails Sign-in Links through the fake, and the fake's inbox.
async fn hub_with_mail(db: PgPool) -> (TestHub, Inbox) {
    let (mail, inbox) = fake_mail();
    (TestHub::start(db, Some(mail)).await, inbox)
}

async fn send_link(hub: &TestHub, email: &str, continue_to: &str) -> Result<(), ConnectError> {
    accounts(hub, None)
        .send_sign_in_link(SendSignInLinkRequest {
            email: email.to_owned(),
            continue_to: continue_to.to_owned(),
            ..Default::default()
        })
        .await?;
    Ok(())
}

async fn get_link(hub: &TestHub, token: &str) -> Result<GetSignInLinkResponse, ConnectError> {
    let link = accounts(hub, None)
        .get_sign_in_link(GetSignInLinkRequest {
            token: token.to_owned(),
            ..Default::default()
        })
        .await?;
    Ok(link.into_owned())
}

/// Uses the link `token`, from a browser presenting `session`, if any. Returns the response and the
/// new session's token.
async fn use_link(
    hub: &TestHub,
    token: &str,
    session: Option<&str>,
) -> Result<(SignInWithLinkResponse, String), ConnectError> {
    let used = accounts(hub, session)
        .sign_in_with_link(SignInWithLinkRequest {
            token: token.to_owned(),
            ..Default::default()
        })
        .await?;
    let (new_session, _) = session_cookie(&used);
    Ok((used.into_owned(), new_session))
}

#[sqlx::test(migrations = false)]
async fn without_mail_the_hub_offers_no_sign_in_links(db: PgPool) {
    let hub = TestHub::start(db, None).await;
    create(&hub, "ada@example.com").await;

    let info = HubServiceClient::new(HttpClient::plaintext(), hub.client_config())
        .get_hub_info(GetHubInfoRequest::default())
        .await
        .unwrap()
        .into_owned();

    assert!(!info.sign_in_links);
    assert_refused(
        send_link(&hub, "ada@example.com", "/").await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_SIGN_IN_LINKS_OFF,
    );
}

#[sqlx::test(migrations = false)]
async fn a_sign_in_link_names_its_account_then_signs_in_once_in_place_of_another(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    create(&hub, "ada@example.com").await;
    let bea = create(&hub, "bea@example.com").await;
    let info = HubServiceClient::new(HttpClient::plaintext(), hub.client_config())
        .get_hub_info(GetHubInfoRequest::default())
        .await
        .unwrap();
    assert!(info.into_owned().sign_in_links);

    send_link(&hub, " ADA@example.com ", "/servers?accept=1")
        .await
        .unwrap();
    let token = inbox.next_link("ada@example.com").await;

    for _ in 0..2 {
        let link = get_link(&hub, &token).await.unwrap();
        assert_eq!(link.account.unwrap().email, "ada@example.com");
    }
    let (used, session) = use_link(&hub, &token, Some(&bea)).await.unwrap();
    assert_eq!(used.account.unwrap().email, "ada@example.com");
    assert_eq!(used.continue_to, "/servers?accept=1");
    assert_eq!(
        signed_in_email(&hub, &session).await.unwrap(),
        "ada@example.com"
    );
    assert_refused(
        signed_in_email(&hub, &bea).await,
        ErrorCode::Unauthenticated,
        RefusalReason::REFUSAL_REASON_SIGN_IN_REQUIRED,
    );
    assert_refused(
        use_link(&hub, &token, None).await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_SIGN_IN_LINK_EXPIRED,
    );
    assert_refused(
        get_link(&hub, &token).await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_SIGN_IN_LINK_EXPIRED,
    );
}

#[sqlx::test(migrations = false)]
async fn asking_again_replaces_the_waiting_link(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    create(&hub, "ada@example.com").await;
    send_link(&hub, "ada@example.com", "/").await.unwrap();
    let first = inbox.next_link("ada@example.com").await;

    send_link(&hub, "ada@example.com", "/").await.unwrap();
    let second = inbox.next_link("ada@example.com").await;

    assert_refused(
        get_link(&hub, &first).await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_SIGN_IN_LINK_EXPIRED,
    );
    use_link(&hub, &second, None).await.unwrap();
}

#[sqlx::test(migrations = false)]
async fn asking_for_a_link_for_an_email_without_an_account_answers_the_same_and_sends_nothing(
    db: PgPool,
) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    create(&hub, "ada@example.com").await;

    send_link(&hub, "nobody@example.com", "/").await.unwrap();
    send_link(&hub, "ada@example.com", "/").await.unwrap();

    // The Hub sends in the order asked, so nobody's request was done with before Ada's email.
    inbox.next_link("ada@example.com").await;
    assert!(inbox.is_empty());
}

#[sqlx::test(migrations = false)]
async fn the_first_link_used_on_an_unproven_account_takes_it_over_and_later_ones_dont(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let created = create(&hub, "ada@example.com").await;
    send_link(&hub, "ada@example.com", "/").await.unwrap();
    let first = inbox.next_link("ada@example.com").await;
    assert!(!get_link(&hub, &first).await.unwrap().email_proven);

    let (_, proven) = use_link(&hub, &first, None).await.unwrap();

    assert_refused(
        signed_in_email(&hub, &created).await,
        ErrorCode::Unauthenticated,
        RefusalReason::REFUSAL_REASON_SIGN_IN_REQUIRED,
    );
    assert_refused(
        accounts(&hub, None)
            .sign_in(sign_in_request("ada@example.com", PASSWORD))
            .await,
        ErrorCode::Unauthenticated,
        RefusalReason::REFUSAL_REASON_WRONG_EMAIL_OR_PASSWORD,
    );
    send_link(&hub, "ada@example.com", "/").await.unwrap();
    let second = inbox.next_link("ada@example.com").await;
    assert!(get_link(&hub, &second).await.unwrap().email_proven);
    use_link(&hub, &second, None).await.unwrap();
    assert_eq!(
        signed_in_email(&hub, &proven).await.unwrap(),
        "ada@example.com"
    );
}

/// Asserts that `refused` was refused for too many attempts, saying how long to wait in both the
/// Refusal and the `Retry-After` header.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn assert_too_many_attempts<T>(refused: Result<T, ConnectError>) {
    let retry_after = refused.as_ref().map(drop).unwrap_err().response_headers()["retry-after"]
        .to_str()
        .unwrap()
        .to_owned();
    let refusal = assert_refused(
        refused,
        ErrorCode::ResourceExhausted,
        RefusalReason::REFUSAL_REASON_TOO_MANY_ATTEMPTS,
    );
    assert_eq!(refusal.params.get("retryAfterSeconds"), Some(&retry_after));
    assert!((1..=900).contains(&retry_after.parse::<u64>().unwrap()));
}

#[sqlx::test(migrations = false)]
async fn a_fourth_link_for_one_email_in_15_minutes_is_refused_account_or_not(db: PgPool) {
    let (hub, _inbox) = hub_with_mail(db).await;
    for _ in 0..3 {
        send_link(&hub, "nobody@example.com", "/").await.unwrap();
    }

    assert_too_many_attempts(send_link(&hub, "NOBODY@example.com", "/").await);
}

#[sqlx::test(migrations = false)]
async fn a_client_asking_for_a_21st_link_in_15_minutes_is_refused(db: PgPool) {
    let (hub, _inbox) = hub_with_mail(db).await;
    for i in 0..20 {
        send_link(&hub, &format!("{i}@example.com"), "/")
            .await
            .unwrap();
    }

    assert_too_many_attempts(send_link(&hub, "new@example.com", "/").await);
}
