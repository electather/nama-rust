//! Admins creating, listing and changing the Role of Users, Users changing their own password, and
//! Users and Admins listing and revoking Devices, through the generated `UserService` client, over a real socket and against a
//! real SQLite database.

#[path = "support/server.rs"]
mod server;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use connectrpc::{
    ConnectError, ErrorCode,
    client::{ClientConfig, HttpClient, ServerStream},
};
use nama_proto::nama::v1::{
    __buffa::view::SubscribeResponseView, ChangePasswordRequest, CreateUserRequest, Device,
    EventServiceClient, ListDevicesRequest, ListUsersRequest, RevokeDeviceRequest, Role,
    SessionDelivery, SetRoleRequest, SignInRequest, SignInResponse, SignInServiceClient,
    SubscribeRequest, SubscribeResponse, User, UserServiceClient, WhoAmIRequest,
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

    /// Creates a User named `name` with `role`, signs a Device in as them, and returns their ID
    /// and that Device's session token.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    async fn signed_in_user(&self, name: &str, role: Role) -> (i64, String) {
        let id = self.create(name, PASSWORD, role).await.unwrap();
        let signed_in = self.sign_in_device(name, PASSWORD, "Phone").await.unwrap();
        (id, signed_in.token)
    }

    /// The ID of the Admin the Server was set up with.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    async fn first_admin_id(&self) -> i64 {
        self.list().await.unwrap().first().unwrap().id
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

/// Changes the caller's password from `current` to `new`, through `client`.
async fn change_password(
    client: &UserServiceClient<HttpClient>,
    current: &str,
    new: &str,
) -> Result<(), ConnectError> {
    client
        .change_password(ChangePasswordRequest {
            current_password: current.to_owned(),
            new_password: new.to_owned(),
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

#[tokio::test]
async fn a_changed_password_signs_in_and_signs_the_users_other_devices_out() {
    let fixture = Fixture::start().await;
    let (phone, laptop) = fixture.grace_on_phone_and_laptop().await;
    let mut laptop_events = subscribe(fixture.server.client_config_as(&laptop.token)).await;
    let new_password = "a brand new password";

    change_password(&fixture.client_as(&phone.token), PASSWORD, new_password)
        .await
        .unwrap();

    assert!(fixture.signed_in(&phone.token).await.unwrap());
    assert!(!fixture.signed_in(&laptop.token).await.unwrap());
    assert!(ended(&mut laptop_events).await, "the stream is still open");
    assert_eq!(code(fixture.sign_in("Grace", new_password).await), None);
    let old = fixture.sign_in("Grace", PASSWORD).await;
    assert_eq!(code(old), Some(ErrorCode::Unauthenticated));
}

#[tokio::test]
async fn a_wrong_current_password_is_unauthenticated_and_changes_nothing() {
    let fixture = Fixture::start().await;
    let (phone, laptop) = fixture.grace_on_phone_and_laptop().await;

    let changed = change_password(
        &fixture.client_as(&phone.token),
        "not the password",
        "a brand new password",
    )
    .await;

    assert_eq!(code(changed), Some(ErrorCode::Unauthenticated));
    assert!(fixture.signed_in(&laptop.token).await.unwrap());
    assert_eq!(code(fixture.sign_in("Grace", PASSWORD).await), None);
}

#[tokio::test]
async fn the_11th_failed_change_for_a_user_is_blocked_even_with_the_right_password() {
    let fixture = Fixture::start().await;
    let (phone, _laptop) = fixture.grace_on_phone_and_laptop().await;
    let grace = fixture.client_as(&phone.token);
    for _ in 0..10 {
        let wrong = change_password(&grace, "not the password", "a brand new password").await;
        assert_eq!(code(wrong), Some(ErrorCode::Unauthenticated));
    }

    let blocked = change_password(&grace, PASSWORD, "a brand new password").await;
    let signing_in = fixture.sign_in("grace", PASSWORD).await;

    assert_eq!(code(blocked), Some(ErrorCode::ResourceExhausted));
    assert_eq!(code(signing_in), Some(ErrorCode::ResourceExhausted));
}

#[tokio::test]
async fn a_new_password_outside_the_rules_is_refused_and_the_old_one_still_works() {
    let fixture = Fixture::start().await;
    let (phone, laptop) = fixture.grace_on_phone_and_laptop().await;

    let changed = change_password(&fixture.client_as(&phone.token), PASSWORD, "1234567").await;

    assert_eq!(code(changed), Some(ErrorCode::InvalidArgument));
    assert!(fixture.signed_in(&laptop.token).await.unwrap());
    assert_eq!(code(fixture.sign_in("Grace", PASSWORD).await), None);
}
