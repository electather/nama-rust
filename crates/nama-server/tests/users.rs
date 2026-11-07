//! Admins creating and listing Users through the generated `UserService` client, over a real
//! socket and against a real SQLite database.

#[path = "support/server.rs"]
mod server;

use connectrpc::{ConnectError, ErrorCode, client::HttpClient};
use nama_proto::nama::v1::{
    CreateUserRequest, ListUsersRequest, Role, SessionDelivery, SignInRequest, SignInServiceClient,
    User, UserServiceClient,
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

    /// Signs a new Device in as the User named `name` with `password`, and returns that User.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    async fn sign_in(&self, name: &str, password: &str) -> Result<User, ConnectError> {
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
        Ok(response.into_owned().user.into_option().unwrap())
    }
}

/// The code a call failed with, or `None` if it succeeded.
fn code<T>(result: Result<T, ConnectError>) -> Option<ErrorCode> {
    result.err().map(|err| err.code)
}

#[tokio::test]
async fn a_user_an_admin_creates_can_sign_in_and_is_listed() {
    let fixture = Fixture::start().await;

    let id = fixture
        .create("  Grace  ", PASSWORD, Role::ROLE_MEMBER)
        .await
        .unwrap();
    let signed_in = fixture.sign_in("grace", PASSWORD).await.unwrap();
    let users = fixture.list().await.unwrap();

    assert_eq!(signed_in.id, id);
    let names: Vec<_> = users.iter().map(|user| user.name.as_str()).collect();
    assert_eq!(names, ["Admin", "Grace"]);
    let grace = users.iter().find(|user| user.id == id).unwrap();
    assert_eq!(grace.role, Role::ROLE_MEMBER);
    assert!(grace.created_at > 1_700_000_000, "{}", grace.created_at);
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
    let signed_in = fixture.sign_in(&name, &password).await.unwrap();
    assert_eq!(signed_in.role, Role::ROLE_ADMIN);
}
