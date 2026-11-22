//! `AccountService`: creating an Account, and signing it in and out, against a real PostgreSQL.

#[path = "support/hub.rs"]
mod hub;
#[path = "support/refused.rs"]
mod refused;

use connectrpc::{
    ConnectError, ErrorCode,
    client::{HttpClient, UnaryResponse},
};
use hub::TestHub;
use nama_hub_proto::nama::hub::v1::{
    AccountServiceClient, CreateAccountRequest, GetAccountRequest, RefusalReason, SignInRequest,
    SignOutRequest,
};
use refused::assert_refused;
use sqlx::PgPool;

const PASSWORD: &str = "correct horse battery";

/// A client of `hub` whose requests come from `ip`, as `hub`'s reverse proxy says.
fn accounts_from(hub: &TestHub, ip: &str) -> AccountServiceClient<HttpClient> {
    let config = hub
        .client_config()
        .with_default_header("x-forwarded-for", ip.to_owned());
    AccountServiceClient::new(HttpClient::plaintext(), config)
}

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
    let hub = TestHub::start(db).await;

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
    let hub = TestHub::start(db).await;
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
    let hub = TestHub::start(db).await;
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
    let hub = TestHub::start(db).await;
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
    let hub = TestHub::start(db).await;
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
    let hub = TestHub::start(db).await;
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
    let hub = TestHub::start(db).await;
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
    let hub = TestHub::start(db).await;
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
    let hub = TestHub::start(db).await;
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

    let retry_after = blocked.as_ref().map(drop).unwrap_err().response_headers()["retry-after"]
        .to_str()
        .unwrap()
        .to_owned();
    let refusal = assert_refused(
        blocked,
        ErrorCode::ResourceExhausted,
        RefusalReason::REFUSAL_REASON_TOO_MANY_ATTEMPTS,
    );
    assert_eq!(refusal.params["retryAfterSeconds"], retry_after);
    assert!((1..=900).contains(&retry_after.parse::<u64>().unwrap()));
}

/// How many Accounts one client IP may create in the window.
const CREATIONS_PER_IP: usize = 10;

#[sqlx::test(migrations = false)]
async fn too_many_creations_from_one_ip_are_refused_without_creating_the_account(db: PgPool) {
    let hub = TestHub::start(db).await;
    let client = accounts_from(&hub, "203.0.113.1");
    for i in 0..CREATIONS_PER_IP {
        client
            .create_account(create_request(&format!("{i}@example.com"), PASSWORD))
            .await
            .unwrap();
    }

    let blocked = client
        .create_account(create_request("ada@example.com", PASSWORD))
        .await;

    let refusal = assert_refused(
        blocked,
        ErrorCode::ResourceExhausted,
        RefusalReason::REFUSAL_REASON_TOO_MANY_ATTEMPTS,
    );
    assert!((1..=900).contains(&refusal.params["retryAfterSeconds"].parse::<u64>().unwrap()));
    accounts_from(&hub, "198.51.100.1")
        .create_account(create_request("ada@example.com", PASSWORD))
        .await
        .unwrap();
}

#[sqlx::test(migrations = false)]
async fn creations_refused_for_a_taken_email_count_towards_the_limit(db: PgPool) {
    let hub = TestHub::start(db).await;
    let client = accounts_from(&hub, "203.0.113.1");
    client
        .create_account(create_request("ada@example.com", PASSWORD))
        .await
        .unwrap();
    for _ in 1..CREATIONS_PER_IP {
        let taken = client
            .create_account(create_request("ada@example.com", PASSWORD))
            .await;
        assert_eq!(taken.unwrap_err().code, ErrorCode::AlreadyExists);
    }

    let blocked = client
        .create_account(create_request("bea@example.com", PASSWORD))
        .await;

    assert_refused(
        blocked,
        ErrorCode::ResourceExhausted,
        RefusalReason::REFUSAL_REASON_TOO_MANY_ATTEMPTS,
    );
}

#[sqlx::test(migrations = false)]
async fn a_session_the_hub_cant_look_up_isnt_taken_for_a_sign_out(db: PgPool) {
    let hub = TestHub::start(db).await;
    let token = create(&hub, "ada@example.com").await;
    hub.lose_database().await;

    let err = signed_in_email(&hub, &token).await.unwrap_err();

    assert_ne!(err.code, ErrorCode::Unauthenticated, "{err:?}");
}
