//! `AccountService`'s sessions: listing where an Account is signed in, and signing out one or all
//! of the others, against a real PostgreSQL.

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
use linking::{accounts, create, session_token};
use nama_hub_proto::nama::hub::v1::{
    AccountServiceClient, GetAccountRequest, ListSessionsRequest, RefusalReason, SignInRequest,
    SignOutOtherSessionsRequest, SignOutSessionRequest,
};
use refused::assert_refused;
use sqlx::PgPool;

const FIREFOX_ON_LINUX: &str =
    "Mozilla/5.0 (X11; Linux x86_64; rv:133.0) Gecko/20100101 Firefox/133.0";
const CHROME_ON_WINDOWS: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                                 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

/// A browser sending `user_agent`, signed in with the session `token`.
fn browser(hub: &TestHub, token: &str, user_agent: &str) -> AccountServiceClient<HttpClient> {
    let config = hub
        .client_config()
        .with_default_header("cookie", format!("nama_hub_session={token}"))
        .with_default_header("user-agent", user_agent.to_owned());
    AccountServiceClient::new(HttpClient::plaintext(), config)
}

/// Signs in to the Account `email` again, a session of its own, and returns its token.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn sign_in(hub: &TestHub, email: &str) -> String {
    let signed_in = accounts(hub)
        .sign_in(SignInRequest {
            email: email.to_owned(),
            password: "correct horse battery".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();
    session_token(signed_in.headers())
}

/// Each live session of the Account `token` is a session of, as its name, its id and whether it's
/// `token`'s, in the order listed.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn sessions(hub: &TestHub, token: &str) -> Vec<(String, String, bool)> {
    let listed = browser(hub, token, FIREFOX_ON_LINUX)
        .list_sessions(ListSessionsRequest::default())
        .await
        .unwrap()
        .into_owned();
    (listed.sessions.into_iter())
        .map(|session| (session.name, session.id, session.is_current))
        .collect()
}

/// The email of the Account the session `token` is of, or why it's refused.
async fn signed_in_email(hub: &TestHub, token: &str) -> Result<String, ConnectError> {
    let account = browser(hub, token, FIREFOX_ON_LINUX)
        .get_account(GetAccountRequest::default())
        .await?
        .into_owned();
    Ok(account.account.unwrap().email)
}

async fn sign_out_session(
    hub: &TestHub,
    token: &str,
    session_id: &str,
) -> Result<(), ConnectError> {
    browser(hub, token, FIREFOX_ON_LINUX)
        .sign_out_session(SignOutSessionRequest {
            session_id: session_id.to_owned(),
            ..Default::default()
        })
        .await
        .map(drop)
}

fn assert_signed_out(refused: Result<String, ConnectError>) {
    assert_refused(
        refused,
        ErrorCode::Unauthenticated,
        RefusalReason::REFUSAL_REASON_SIGN_IN_REQUIRED,
    );
}

#[sqlx::test(migrations = false)]
async fn an_account_lists_its_own_sessions_by_browser_this_one_first(db: PgPool) {
    let hub = TestHub::start(db, None).await;
    let ada = create(&hub, "ada@example.com").await;
    let adas_other = sign_in(&hub, "ada@example.com").await;
    let bea = create(&hub, "bea@example.com").await;
    browser(&hub, &adas_other, CHROME_ON_WINDOWS)
        .get_account(GetAccountRequest::default())
        .await
        .unwrap();

    let listed = sessions(&hub, &ada).await;

    let names: Vec<_> = (listed.iter())
        .map(|(name, _, current)| (name.as_str(), *current))
        .collect();
    assert_eq!(
        names,
        [("Firefox on Linux", true), ("Chrome on Windows", false)]
    );
    assert_eq!(sessions(&hub, &bea).await.len(), 1);
}

#[sqlx::test(migrations = false)]
async fn signing_out_a_session_ends_it_at_once_and_nothing_else(db: PgPool) {
    let hub = TestHub::start(db, None).await;
    let ada = create(&hub, "ada@example.com").await;
    let ending = sign_in(&hub, "ada@example.com").await;
    let staying = sign_in(&hub, "ada@example.com").await;
    let bea = create(&hub, "bea@example.com").await;
    let id_of = |listed: &[(String, String, bool)], current: bool| {
        listed
            .iter()
            .find(|(_, _, is_current)| *is_current == current)
            .unwrap()
            .1
            .clone()
    };
    let endings_id = id_of(&sessions(&hub, &ending).await, true);
    let adas_id = id_of(&sessions(&hub, &ada).await, true);
    let beas_id = id_of(&sessions(&hub, &bea).await, true);

    sign_out_session(&hub, &ada, &endings_id).await.unwrap();
    sign_out_session(&hub, &ada, &beas_id).await.unwrap();
    sign_out_session(&hub, &ada, &adas_id).await.unwrap();
    sign_out_session(&hub, &ada, "not-a-uuid").await.unwrap();

    assert_signed_out(signed_in_email(&hub, &ending).await);
    for (token, email) in [
        (&ada, "ada@example.com"),
        (&staying, "ada@example.com"),
        (&bea, "bea@example.com"),
    ] {
        assert_eq!(signed_in_email(&hub, token).await.unwrap(), email);
    }
    assert_eq!(sessions(&hub, &ada).await.len(), 2);
}

#[sqlx::test(migrations = false)]
async fn signing_out_everywhere_else_ends_every_other_session_of_the_account_only(db: PgPool) {
    let hub = TestHub::start(db, None).await;
    let ada = create(&hub, "ada@example.com").await;
    let others = [
        sign_in(&hub, "ada@example.com").await,
        sign_in(&hub, "ada@example.com").await,
    ];
    let bea = create(&hub, "bea@example.com").await;

    browser(&hub, &ada, FIREFOX_ON_LINUX)
        .sign_out_other_sessions(SignOutOtherSessionsRequest::default())
        .await
        .unwrap();

    for other in &others {
        assert_signed_out(signed_in_email(&hub, other).await);
    }
    assert_eq!(
        signed_in_email(&hub, &ada).await.unwrap(),
        "ada@example.com"
    );
    assert_eq!(
        signed_in_email(&hub, &bea).await.unwrap(),
        "bea@example.com"
    );
    assert_eq!(sessions(&hub, &ada).await.len(), 1);
}

#[sqlx::test(migrations = false)]
async fn listing_or_signing_out_sessions_needs_a_session(db: PgPool) {
    let hub = TestHub::start(db, None).await;
    let client = browser(&hub, "not-a-session", FIREFOX_ON_LINUX);

    for refused in [
        client
            .list_sessions(ListSessionsRequest::default())
            .await
            .map(drop),
        sign_out_session(&hub, "not-a-session", "").await,
        client
            .sign_out_other_sessions(SignOutOtherSessionsRequest::default())
            .await
            .map(drop),
    ] {
        assert_refused(
            refused,
            ErrorCode::Unauthenticated,
            RefusalReason::REFUSAL_REASON_SIGN_IN_REQUIRED,
        );
    }
}
