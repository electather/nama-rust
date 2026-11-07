//! Admins creating and listing Users, and Users and Admins listing and revoking Devices, through
//! the generated `UserService` client, over a real socket and against a real SQLite database.

#[path = "support/server.rs"]
mod server;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use connectrpc::{
    ConnectError, ErrorCode,
    client::{ClientConfig, HttpClient, ServerStream},
};
use nama_proto::nama::v1::{
    __buffa::view::SubscribeResponseView, CreateUserRequest, Device, EventServiceClient,
    ListDevicesRequest, ListUsersRequest, RevokeDeviceRequest, Role, SessionDelivery,
    SignInRequest, SignInResponse, SignInServiceClient, SubscribeRequest, SubscribeResponse, User,
    UserServiceClient, WhoAmIRequest,
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
        let response = self.sign_in_device(name, password, "Phone").await?;
        Ok(response.user.into_option().unwrap())
    }

    /// Signs a new Device named `device_name` in as the User named `name` with `password`.
    async fn sign_in_device(
        &self,
        name: &str,
        password: &str,
        device_name: &str,
    ) -> Result<SignInResponse, ConnectError> {
        let client = SignInServiceClient::new(
            HttpClient::plaintext(),
            self.server.anonymous_client_config(),
        );
        let response = client
            .sign_in(SignInRequest {
                name: name.to_owned(),
                password: password.to_owned(),
                device_name: device_name.to_owned(),
                delivery: SessionDelivery::SESSION_DELIVERY_TOKEN.into(),
                ..Default::default()
            })
            .await?;
        Ok(response.into_owned())
    }

    /// Creates a Member named Grace and signs two of her Devices in, a Phone and then a Laptop.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    async fn grace_on_phone_and_laptop(&self) -> (SignInResponse, SignInResponse) {
        self.create("Grace", PASSWORD, Role::ROLE_MEMBER)
            .await
            .unwrap();
        let phone = self.sign_in_device("Grace", PASSWORD, "Phone").await;
        let laptop = self.sign_in_device("Grace", PASSWORD, "Laptop").await;
        (phone.unwrap(), laptop.unwrap())
    }

    /// A `UserService` client calling with the session `token`.
    fn client_as(&self, token: &str) -> UserServiceClient<HttpClient> {
        UserServiceClient::new(HttpClient::plaintext(), self.server.client_config_as(token))
    }

    /// Whether the session `token` still works.
    async fn signed_in(&self, token: &str) -> Result<bool, ConnectError> {
        let client =
            SignInServiceClient::new(HttpClient::plaintext(), self.server.client_config_as(token));
        match client.who_am_i(WhoAmIRequest::default()).await {
            Ok(_) => Ok(true),
            Err(err) if err.code == ErrorCode::Unauthenticated => Ok(false),
            Err(err) => Err(err),
        }
    }
}

/// Lists the Devices of the User `user_id`, or the caller's own, through `client`.
async fn list_devices(
    client: &UserServiceClient<HttpClient>,
    user_id: Option<i64>,
) -> Result<Vec<Device>, ConnectError> {
    let response = client
        .list_devices(ListDevicesRequest {
            user_id,
            ..Default::default()
        })
        .await?;
    Ok(response.into_owned().devices)
}

/// Revokes the Device `device_id` of the User `user_id`, or of the caller, through `client`.
async fn revoke_device(
    client: &UserServiceClient<HttpClient>,
    user_id: Option<i64>,
    device_id: i64,
) -> Result<(), ConnectError> {
    client
        .revoke_device(RevokeDeviceRequest {
            user_id,
            device_id,
            ..Default::default()
        })
        .await?;
    Ok(())
}

/// The IDs of the User and the Device in a sign-in response, in that order.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn ids(signed_in: &SignInResponse) -> (i64, i64) {
    let user = signed_in.user.as_option().unwrap().id;
    let device = signed_in.device.as_option().unwrap().id;
    (user, device)
}

/// How long a test waits for a stream before failing.
const WAIT: Duration = Duration::from_secs(10);

/// A subscription to the Server's events.
type Subscription = ServerStream<hyper::body::Incoming, SubscribeResponseView<'static>>;

/// Subscribes to every event with `config`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn subscribe(config: ClientConfig) -> Subscription {
    EventServiceClient::new(HttpClient::plaintext(), config)
        .subscribe(SubscribeRequest::default())
        .await
        .unwrap()
}

/// Whether `subscription` ended, rather than failing or carrying on.
#[expect(
    clippy::expect_used,
    reason = "a test helper fails its test by panicking"
)]
async fn ended(subscription: &mut Subscription) -> bool {
    let next = tokio::time::timeout(WAIT, subscription.message::<SubscribeResponse>())
        .await
        .expect("timed out waiting for the stream");
    matches!(next, Ok(None))
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
    let signed_in = fixture.sign_in("grace", PASSWORD).await.unwrap();
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
    let signed_in = fixture.sign_in(&name, &password).await.unwrap();
    assert_eq!(signed_in.role, Role::ROLE_ADMIN);
}

#[tokio::test]
async fn a_user_lists_their_devices_with_the_one_calling_marked_current() {
    let fixture = Fixture::start().await;
    let before = now();
    let (phone, laptop) = fixture.grace_on_phone_and_laptop().await;
    let after = now();

    let devices = list_devices(&fixture.client_as(&phone.token), None)
        .await
        .unwrap();

    let listed: Vec<_> = devices
        .iter()
        .map(|device| (device.id, device.name.as_str(), device.is_current))
        .collect();
    assert_eq!(
        listed,
        [
            (ids(&phone).1, "Phone", true),
            (ids(&laptop).1, "Laptop", false)
        ]
    );
    for device in &devices {
        assert!((before..=after).contains(&device.created_at), "{device:?}");
        assert!(
            (before..=after).contains(&device.last_seen_at),
            "{device:?}"
        );
    }
}

#[tokio::test]
async fn a_revoked_devices_token_is_refused_at_once_and_its_stream_ends() {
    let fixture = Fixture::start().await;
    let (phone, laptop) = fixture.grace_on_phone_and_laptop().await;
    let mut laptop_events = subscribe(fixture.server.client_config_as(&laptop.token)).await;
    let grace = fixture.client_as(&phone.token);

    revoke_device(&grace, None, ids(&laptop).1).await.unwrap();

    assert!(!fixture.signed_in(&laptop.token).await.unwrap());
    assert!(ended(&mut laptop_events).await, "the stream is still open");
    assert!(fixture.signed_in(&phone.token).await.unwrap());
    let left: Vec<_> = list_devices(&grace, None)
        .await
        .unwrap()
        .into_iter()
        .map(|device| device.name)
        .collect();
    assert_eq!(left, ["Phone"]);
}

#[tokio::test]
async fn revoking_your_current_device_signs_you_out_and_clears_a_cookie() {
    let fixture = Fixture::start().await;
    let (phone, _laptop) = fixture.grace_on_phone_and_laptop().await;
    let cookie_config = fixture
        .server
        .anonymous_client_config()
        .with_default_header("cookie", format!("nama_session={}", phone.token));
    let grace = UserServiceClient::new(HttpClient::plaintext(), cookie_config);

    let revoked = grace
        .revoke_device(RevokeDeviceRequest {
            device_id: ids(&phone).1,
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(!fixture.signed_in(&phone.token).await.unwrap());
    let cookie = revoked
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(cookie.starts_with("nama_session=;"), "{cookie}");
    assert!(cookie.contains("Max-Age=0"), "{cookie}");
}

#[tokio::test]
async fn an_admin_lists_and_revokes_another_users_device() {
    let fixture = Fixture::start().await;
    let (phone, laptop) = fixture.grace_on_phone_and_laptop().await;
    let (grace, laptop_id) = ids(&laptop);

    let listed = list_devices(&fixture.client, Some(grace)).await.unwrap();
    revoke_device(&fixture.client, Some(grace), laptop_id)
        .await
        .unwrap();

    let names: Vec<_> = listed.iter().map(|device| device.name.as_str()).collect();
    assert_eq!(names, ["Phone", "Laptop"]);
    assert!(listed.iter().all(|device| !device.is_current));
    assert!(!fixture.signed_in(&laptop.token).await.unwrap());
    assert!(fixture.signed_in(&phone.token).await.unwrap());
}

#[tokio::test]
async fn a_member_touching_another_users_devices_gets_not_found() {
    let fixture = Fixture::start().await;
    let (phone, _laptop) = fixture.grace_on_phone_and_laptop().await;
    let (grace_id, _) = ids(&phone);
    let admin_devices = list_devices(&fixture.client, None).await.unwrap();
    let (admin_id, admin_device) = (fixture.list().await.unwrap()[0].id, admin_devices[0].id);
    let grace = fixture.client_as(&phone.token);

    let listed = list_devices(&grace, Some(admin_id)).await;
    let listed_nobody = list_devices(&grace, Some(admin_id + 1000)).await;
    let revoked_theirs = revoke_device(&grace, Some(admin_id), admin_device).await;
    let revoked_as_own = revoke_device(&grace, None, admin_device).await;
    let revoked_as_own_by_id = revoke_device(&grace, Some(grace_id), admin_device).await;

    for refused in [code(listed), code(listed_nobody)] {
        assert_eq!(refused, Some(ErrorCode::NotFound));
    }
    for refused in [revoked_theirs, revoked_as_own, revoked_as_own_by_id] {
        assert_eq!(code(refused), Some(ErrorCode::NotFound));
    }
    assert!(
        fixture
            .signed_in(&fixture.server.admin_token)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn an_admin_asking_about_a_user_who_doesnt_exist_gets_not_found() {
    let fixture = Fixture::start().await;

    let listed = list_devices(&fixture.client, Some(1000)).await;
    let revoked = revoke_device(&fixture.client, Some(1000), 1).await;

    assert_eq!(code(listed), Some(ErrorCode::NotFound));
    assert_eq!(code(revoked), Some(ErrorCode::NotFound));
}
