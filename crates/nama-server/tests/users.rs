//! Admins creating, listing and changing the Role of Users through the generated `UserService`
//! client, over a real socket and against a real SQLite database.

#[path = "support/server.rs"]
mod server;

use std::time::{SystemTime, UNIX_EPOCH};

use connectrpc::{ConnectError, ErrorCode, client::HttpClient};
use nama_proto::nama::v1::{
    CreateUserRequest, ListUsersRequest, Role, SessionDelivery, SetRoleRequest, SignInRequest,
    SignInServiceClient, User, UserServiceClient, WhoAmIRequest,
};
use nama_server::{config::Config, plugins::Registry};
use server::{PASSWORD, TestServer};

/// A Server with an Admin signed in, and a client for its `UserService` calling as them.
struct Fixture {
    client: UserServiceClient<HttpClient>,
    server: TestServer,
}

impl Fixture {
    async fn start() -> Self {
        let server = TestServer::start(Config::default(), Registry::new()).await;
        let client = UserServiceClient::new(HttpClient::plaintext(), server.client_config());
        Self { client, server }
    }

    /// Creates a User named `name` with `password` and `role`, and returns their ID.
    async fn create(&self, name: &str, password: &str, role: Role) -> Result<i64, ConnectError> {
        let response = self
            .client
            .create_user(CreateUserRequest {
                name: name.to_owned(),
                password: password.to_owned(),
                role: role.into(),
                ..Default::default()
            })
            .await?;
        Ok(response.into_owned().id)
    }

    /// Every User on the Server.
    async fn list(&self) -> Result<Vec<User>, ConnectError> {
        let response = self.client.list_users(ListUsersRequest::default()).await?;
        Ok(response.into_owned().users)
    }

    /// Signs a new Device in as the User named `name` with `password`, and returns that User and
    /// the Device's session token.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    async fn sign_in(&self, name: &str, password: &str) -> Result<(User, String), ConnectError> {
        let client = SignInServiceClient::new(
            HttpClient::plaintext(),
            self.server.anonymous_client_config(),
        );
        let response = client
            .sign_in(SignInRequest {
                name: name.to_owned(),
                password: password.to_owned(),
                device_name: "Phone".to_owned(),
                delivery: SessionDelivery::SESSION_DELIVERY_TOKEN.into(),
                ..Default::default()
            })
            .await?;
        let response = response.into_owned();
        Ok((response.user.into_option().unwrap(), response.token))
    }

    /// Creates a User named `name` with `role`, signs a Device in as them, and returns their ID
    /// and that Device's session token.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    async fn signed_in_user(&self, name: &str, role: Role) -> (i64, String) {
        let id = self.create(name, PASSWORD, role).await.unwrap();
        let (_, token) = self.sign_in(name, PASSWORD).await.unwrap();
        (id, token)
    }

    /// A client for `UserService` presenting the session `token`.
    fn client_as(&self, token: &str) -> UserServiceClient<HttpClient> {
        UserServiceClient::new(HttpClient::plaintext(), self.server.client_config_as(token))
    }

    /// The ID of the Admin the Server was set up with.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    async fn first_admin_id(&self) -> i64 {
        self.list().await.unwrap().first().unwrap().id
    }
}

/// Gives the User `user_id` the Role `role`, calling with `client`.
async fn set_role(
    client: &UserServiceClient<HttpClient>,
    user_id: i64,
    role: Role,
) -> Result<(), ConnectError> {
    client
        .set_role(SetRoleRequest {
            user_id,
            role: role.into(),
            ..Default::default()
        })
        .await?;
    Ok(())
}

/// The IDs of the Users who are Admins, listed by `client`, which must be an Admin's.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn admin_ids(client: &UserServiceClient<HttpClient>) -> Vec<i64> {
    let users = client
        .list_users(ListUsersRequest::default())
        .await
        .unwrap();
    users
        .into_owned()
        .users
        .into_iter()
        .filter(|user| user.role == Role::ROLE_ADMIN)
        .map(|user| user.id)
        .collect()
}

/// Whether `client` may call an Admin RPC: `Ok` if so, or the code it was refused with.
async fn admin_rpc(client: &UserServiceClient<HttpClient>) -> Result<(), ErrorCode> {
    match client.list_users(ListUsersRequest::default()).await {
        Ok(_) => Ok(()),
        Err(err) => Err(err.code),
    }
}

/// Now, in seconds since the Unix epoch, as the Server stores times.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn now() -> i64 {
    let since_epoch = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    i64::try_from(since_epoch.as_secs()).unwrap()
}

/// The code a call failed with, or `None` if it succeeded.
fn code<T>(result: Result<T, ConnectError>) -> Option<ErrorCode> {
    result.err().map(|err| err.code)
}

#[tokio::test]
async fn a_user_an_admin_creates_can_sign_in_and_is_listed() {
    let fixture = Fixture::start().await;

    let before = now();
    let id = fixture
        .create("  Grace  ", PASSWORD, Role::ROLE_MEMBER)
        .await
        .unwrap();
    let after = now();
    let (signed_in, _) = fixture.sign_in("grace", PASSWORD).await.unwrap();
    let users = fixture.list().await.unwrap();

    assert_eq!(signed_in.id, id);
    let names: Vec<_> = users.iter().map(|user| user.name.as_str()).collect();
    assert_eq!(names, ["Admin", "Grace"]);
    let grace = users.iter().find(|user| user.id == id).unwrap();
    assert_eq!(grace.role, Role::ROLE_MEMBER);
    assert!(
        (before..=after).contains(&grace.created_at),
        "{before} <= {} <= {after}",
        grace.created_at
    );
}

#[tokio::test]
async fn a_name_another_user_has_in_any_case_already_exists() {
    let fixture = Fixture::start().await;
    fixture
        .create("Grace", PASSWORD, Role::ROLE_MEMBER)
        .await
        .unwrap();

    let taken = fixture.create("gRACE", PASSWORD, Role::ROLE_ADMIN).await;
    let the_admins = fixture.create("ADMIN", PASSWORD, Role::ROLE_MEMBER).await;

    assert_eq!(code(taken), Some(ErrorCode::AlreadyExists));
    assert_eq!(code(the_admins), Some(ErrorCode::AlreadyExists));
}

#[tokio::test]
async fn a_name_password_or_role_outside_the_rules_is_refused_and_nobody_is_created() {
    let fixture = Fixture::start().await;
    let too_long_name = "x".repeat(65);
    let too_long_password = "x".repeat(1025);

    let refusals = [
        ("   ", PASSWORD, Role::ROLE_MEMBER),
        (too_long_name.as_str(), PASSWORD, Role::ROLE_MEMBER),
        ("Grace", "1234567", Role::ROLE_MEMBER),
        ("Grace", too_long_password.as_str(), Role::ROLE_MEMBER),
        ("Grace", PASSWORD, Role::ROLE_UNSPECIFIED),
    ];
    for (name, password, role) in refusals {
        let refused = fixture.create(name, password, role).await;

        assert_eq!(
            code(refused),
            Some(ErrorCode::InvalidArgument),
            "{name:.8} / {password:.8} / {role:?}"
        );
    }
    assert_eq!(fixture.list().await.unwrap().len(), 1);
}

#[tokio::test]
async fn the_longest_name_and_password_the_rules_allow_are_accepted() {
    let fixture = Fixture::start().await;
    let name = "é".repeat(64);
    let password = "x".repeat(1024);

    let created = fixture.create(&name, &password, Role::ROLE_ADMIN).await;

    assert!(created.is_ok(), "{created:?}");
    let (signed_in, _) = fixture.sign_in(&name, &password).await.unwrap();
    assert_eq!(signed_in.role, Role::ROLE_ADMIN);
}

#[tokio::test]
async fn a_promoted_member_may_call_admin_rpcs_on_their_next_request() {
    let fixture = Fixture::start().await;
    let (grace, token) = fixture.signed_in_user("Grace", Role::ROLE_MEMBER).await;
    let as_grace = fixture.client_as(&token);
    assert_eq!(admin_rpc(&as_grace).await, Err(ErrorCode::PermissionDenied));

    set_role(&fixture.client, grace, Role::ROLE_ADMIN)
        .await
        .unwrap();

    assert_eq!(admin_rpc(&as_grace).await, Ok(()));
}

#[tokio::test]
async fn a_demoted_admin_loses_admin_rpcs_on_their_next_request_but_stays_signed_in() {
    let fixture = Fixture::start().await;
    let (grace, token) = fixture.signed_in_user("Grace", Role::ROLE_ADMIN).await;
    let as_grace = fixture.client_as(&token);
    assert_eq!(admin_rpc(&as_grace).await, Ok(()));

    set_role(&fixture.client, grace, Role::ROLE_MEMBER)
        .await
        .unwrap();

    assert_eq!(admin_rpc(&as_grace).await, Err(ErrorCode::PermissionDenied));
    let who = SignInServiceClient::new(
        HttpClient::plaintext(),
        fixture.server.client_config_as(&token),
    )
    .who_am_i(WhoAmIRequest::default())
    .await
    .unwrap()
    .into_owned();
    assert_eq!(who.user.role, Role::ROLE_MEMBER);
}

#[tokio::test]
async fn demoting_the_last_admin_is_refused() {
    let fixture = Fixture::start().await;
    let admin = fixture.first_admin_id().await;

    let demoted = set_role(&fixture.client, admin, Role::ROLE_MEMBER).await;

    assert_eq!(code(demoted), Some(ErrorCode::FailedPrecondition));
    assert_eq!(admin_ids(&fixture.client).await, [admin]);
}

#[tokio::test]
async fn an_admin_may_demote_themselves_while_another_admin_exists() {
    let fixture = Fixture::start().await;
    let admin = fixture.first_admin_id().await;
    let (grace, token) = fixture.signed_in_user("Grace", Role::ROLE_ADMIN).await;

    let demoted = set_role(&fixture.client, admin, Role::ROLE_MEMBER).await;

    assert_eq!(code(demoted), None);
    assert_eq!(admin_ids(&fixture.client_as(&token)).await, [grace]);
}

#[tokio::test]
async fn two_admins_demoting_each_other_at_once_leave_exactly_one_admin() {
    let fixture = Fixture::start().await;
    let admin = fixture.first_admin_id().await;
    let (grace, token) = fixture.signed_in_user("Grace", Role::ROLE_ADMIN).await;
    let as_grace = fixture.client_as(&token);

    let (grace_demoted, admin_demoted) = tokio::join!(
        set_role(&fixture.client, grace, Role::ROLE_MEMBER),
        set_role(&as_grace, admin, Role::ROLE_MEMBER),
    );

    // Whichever lands second finds its caller already demoted, or itself the last Admin.
    let refused = |demoted: &Result<(), ConnectError>| {
        matches!(
            demoted.as_ref().map_err(|err| err.code),
            Err(ErrorCode::PermissionDenied | ErrorCode::FailedPrecondition)
        )
    };
    let survivor = match (&grace_demoted, &admin_demoted) {
        (Ok(()), lost) if refused(lost) => admin,
        (lost, Ok(())) if refused(lost) => grace,
        both => panic!("exactly one demotion should succeed: {both:?}"),
    };
    let as_survivor = if survivor == admin {
        &fixture.client
    } else {
        &as_grace
    };
    assert_eq!(admin_ids(as_survivor).await, [survivor]);
}

#[tokio::test]
async fn changing_the_role_of_an_unknown_user_is_not_found() {
    let fixture = Fixture::start().await;

    let changed = set_role(&fixture.client, 999, Role::ROLE_ADMIN).await;

    assert_eq!(code(changed), Some(ErrorCode::NotFound));
}
