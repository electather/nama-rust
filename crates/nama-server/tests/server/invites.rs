//! Users who manage Users making, listing and revoking Invites, and someone without a User
//! accepting one, through the generated clients, over a real socket and against a real SQLite
//! database.

use crate::support::{refused, server};

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use connectrpc::{ConnectError, ErrorCode, client::HttpClient};
use nama_proto::nama::v1::{
    AcceptInviteRequest, AcceptInviteResponse, CreateInviteRequest, CreateInviteResponse,
    DeleteUserRequest, Invite, ListInvitesRequest, ListUsersRequest, Permission, PlaybackLimit,
    RefusalReason, RevokeInviteRequest, ServerServiceClient, ServerSettings, SessionDelivery,
    SetDefaultPermissionsRequest, SetPermissionsRequest, SignInServiceClient,
    UpdateServerSettingsRequest, UserServiceClient, WhoAmIRequest,
};
use refused::assert_refused;
use server::{PASSWORD, TestServer};

const SERVER_URL: &str = "https://nama.example.com";

/// A Server with a Server URL and an Admin signed in.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn start(builder: server::Builder) -> TestServer {
    let server = builder.start().await;
    set_server_url(&server, SERVER_URL).await.unwrap();
    server
}

async fn set_server_url(server: &TestServer, url: &str) -> Result<(), ConnectError> {
    let client = ServerServiceClient::new(HttpClient::plaintext(), server.client_config());
    client
        .update_server_settings(UpdateServerSettingsRequest {
            settings: ServerSettings {
                url: url.to_owned(),
                ..Default::default()
            }
            .into(),
            ..Default::default()
        })
        .await?;
    Ok(())
}

fn users(as_who: connectrpc::client::ClientConfig) -> UserServiceClient<HttpClient> {
    UserServiceClient::new(HttpClient::plaintext(), as_who)
}

async fn create_invite(
    client: &UserServiceClient<HttpClient>,
    note: &str,
    permissions: &[Permission],
) -> Result<CreateInviteResponse, ConnectError> {
    let response = client
        .create_invite(CreateInviteRequest {
            note: note.to_owned(),
            permissions: permissions.iter().map(|&p| p.into()).collect(),
            ..Default::default()
        })
        .await?;
    Ok(response.into_owned())
}

/// The token at the end of an Invite's link.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn token_of(created: &CreateInviteResponse) -> String {
    created.url.rsplit('/').next().unwrap().to_owned()
}

/// Accepts the Invite with `token` from a Device with no session, as someone opening its link in a
/// fresh browser would.
async fn accept(
    server: &TestServer,
    token: &str,
    name: &str,
) -> Result<AcceptInviteResponse, ConnectError> {
    let client =
        SignInServiceClient::new(HttpClient::plaintext(), server.anonymous_client_config());
    let response = client
        .accept_invite(AcceptInviteRequest {
            token: token.to_owned(),
            name: name.to_owned(),
            password: PASSWORD.to_owned(),
            delivery: SessionDelivery::SESSION_DELIVERY_TOKEN.into(),
            ..Default::default()
        })
        .await?;
    Ok(response.into_owned())
}

/// How far the clock, while the test hasn't moved it, is from the start of the Unix second
/// `second`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn until_second(second: i64) -> Duration {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    // Whole milliseconds, as `db::now_ms` reads it, so moving the clock by this lands on `second`.
    let now = Duration::from_millis(now.as_millis().try_into().unwrap());
    Duration::from_secs(second.try_into().unwrap())
        .checked_sub(now)
        .unwrap()
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn list(client: &UserServiceClient<HttpClient>) -> (Vec<Invite>, bool) {
    let listed = client
        .list_invites(ListInvitesRequest::default())
        .await
        .unwrap()
        .into_owned();
    (listed.invites, listed.server_url_set)
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn set_permissions(server: &TestServer, user: i64, permissions: &[Permission]) {
    users(server.client_config())
        .set_permissions(SetPermissionsRequest {
            user_id: user,
            permissions: permissions.iter().map(|&p| p.into()).collect(),
            ..Default::default()
        })
        .await
        .unwrap();
}

fn permissions_of(permissions: &[buffa::EnumValue<Permission>]) -> Vec<Permission> {
    permissions
        .iter()
        .filter_map(buffa::EnumValue::as_known)
        .collect()
}

fn assert_invite_not_found<T: std::fmt::Debug>(result: Result<T, ConnectError>) {
    let refusal = assert_refused(
        result,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_INVITE_NOT_FOUND,
    );
    assert_eq!(refusal.field, "token");
}

#[tokio::test]
async fn an_invite_token_carries_128_random_bits() {
    let server = start(TestServer::builder()).await;
    let client = users(server.client_config());

    let first = token_of(&create_invite(&client, "", &[]).await.unwrap());
    let second = token_of(&create_invite(&client, "", &[]).await.unwrap());

    // 16 bytes, base64url-encoded without padding.
    assert_eq!(first.len(), 22, "{first}");
    assert_ne!(first, second);
}

#[tokio::test]
async fn accepting_an_invite_creates_a_user_signed_in_with_its_permissions() {
    let server = start(TestServer::builder()).await;
    let picked = [Permission::ManageLibraries, Permission::Request];
    let created = create_invite(&users(server.client_config()), "Mum", &picked)
        .await
        .unwrap();

    let accepted = accept(&server, &token_of(&created), "Mum").await.unwrap();

    assert!(
        created.url.starts_with(&format!("{SERVER_URL}/invite/")),
        "{}",
        created.url
    );
    let user = accepted.user.into_option().unwrap();
    assert_eq!(user.name, "Mum");
    assert_eq!(permissions_of(&user.permissions), picked);
    let signed_in = SignInServiceClient::new(
        HttpClient::plaintext(),
        server.client_config_as(&accepted.token),
    );
    let who = signed_in.who_am_i(WhoAmIRequest::default()).await.unwrap();
    assert_eq!(who.into_owned().user.into_option().unwrap().id, user.id);
}

#[tokio::test]
async fn an_invite_works_once() {
    let server = start(TestServer::builder()).await;
    let admin = users(server.client_config());
    let created = create_invite(&admin, "", &[]).await.unwrap();
    accept(&server, &token_of(&created), "Mum").await.unwrap();

    let again = accept(&server, &token_of(&created), "Dad").await;

    assert_invite_not_found(again);
    assert_eq!(list(&admin).await.0, Vec::new());
}

#[tokio::test]
async fn an_invite_stops_working_7_days_after_it_was_made() {
    let server = start(TestServer::builder()).await;
    let admin = users(server.client_config());
    let early = create_invite(&admin, "", &[]).await.unwrap();
    let late = create_invite(&admin, "", &[]).await.unwrap();
    let early_expiry = early.invite.as_option().unwrap().expires_at;
    let late_expiry = late.invite.as_option().unwrap().expires_at;

    // The real clock keeps running, so land on the start of the last second `early` works, giving
    // the accept that whole second.
    server.advance_clock(until_second(early_expiry - 1));
    let within = accept(&server, &token_of(&early), "Mum").await;
    // From the last second `early` works to `late`'s expiry, which may be a second after `early`'s.
    let to_late_expiry = late_expiry - early_expiry + 1;
    server.advance_clock(Duration::from_secs(to_late_expiry.try_into().unwrap()));
    let after = accept(&server, &token_of(&late), "Dad").await;

    within.unwrap();
    assert_invite_not_found(after);
    assert_eq!(list(&admin).await.0, Vec::new());
}

#[tokio::test]
async fn a_revoked_invite_stops_working() {
    let server = start(TestServer::builder()).await;
    let admin = users(server.client_config());
    let created = create_invite(&admin, "", &[]).await.unwrap();
    let id = created.invite.as_option().unwrap().id;

    admin
        .revoke_invite(RevokeInviteRequest {
            invite_id: id,
            ..Default::default()
        })
        .await
        .unwrap();
    let again = admin
        .revoke_invite(RevokeInviteRequest {
            invite_id: id,
            ..Default::default()
        })
        .await;

    assert_invite_not_found(accept(&server, &token_of(&created), "Mum").await);
    let refusal = assert_refused(
        again,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_INVITE_NOT_FOUND,
    );
    assert_eq!(refusal.field, "invite_id");
}

#[tokio::test]
async fn an_invite_dies_with_its_creator() {
    let server = start(TestServer::builder()).await;
    let creator = server.user("Ann", &[Permission::ManageUsers]).await;
    let created = create_invite(&users(creator.client_config()), "", &[])
        .await
        .unwrap();

    users(server.client_config())
        .delete_user(DeleteUserRequest {
            user_id: creator.id,
            ..Default::default()
        })
        .await
        .unwrap();

    assert_invite_not_found(accept(&server, &token_of(&created), "Mum").await);
}

#[tokio::test]
async fn an_invite_stops_working_for_good_once_its_creator_cant_manage_users() {
    let server = start(TestServer::builder()).await;
    let creator = server
        .user("Ann", &[Permission::ManageUsers, Permission::Request])
        .await;
    let created = create_invite(&users(creator.client_config()), "", &[])
        .await
        .unwrap();

    set_permissions(&server, creator.id, &[Permission::Request]).await;
    let listed = list(&users(server.client_config())).await.0;
    set_permissions(&server, creator.id, &[Permission::ManageUsers]).await;

    assert_eq!(listed, Vec::new());
    assert_invite_not_found(accept(&server, &token_of(&created), "Mum").await);
}

#[tokio::test]
async fn an_invite_never_grants_admin_or_a_permission_its_creator_lacks() {
    let server = start(TestServer::builder()).await;
    let creator = server.user("Ann", &[Permission::ManageUsers]).await;

    let admin = create_invite(&users(server.client_config()), "", &[Permission::Admin]).await;
    let lacking = create_invite(
        &users(creator.client_config()),
        "",
        &[Permission::ManageLibraries],
    )
    .await;

    for (refused, permission) in [
        (admin, "PERMISSION_ADMIN"),
        (lacking, "PERMISSION_MANAGE_LIBRARIES"),
    ] {
        let refusal = assert_refused(
            refused,
            ErrorCode::PermissionDenied,
            RefusalReason::REFUSAL_REASON_PERMISSION_REQUIRED,
        );
        assert_eq!(refusal.field, "permissions");
        assert_eq!(refusal.params["permission"], permission);
    }
}

#[tokio::test]
async fn a_creator_who_loses_a_permission_cant_pass_it_on() {
    let server = start(TestServer::builder()).await;
    let creator = server
        .user(
            "Ann",
            &[
                Permission::ManageUsers,
                Permission::Request,
                Permission::ViewRequests,
            ],
        )
        .await;
    let picked = [Permission::Request, Permission::ViewRequests];
    let created = create_invite(&users(creator.client_config()), "", &picked)
        .await
        .unwrap();

    set_permissions(
        &server,
        creator.id,
        &[Permission::ManageUsers, Permission::ViewRequests],
    )
    .await;
    let listed = list(&users(server.client_config())).await.0;
    let accepted = accept(&server, &token_of(&created), "Mum").await.unwrap();

    assert_eq!(
        permissions_of(&listed[0].permissions),
        [Permission::ViewRequests]
    );
    let user = accepted.user.into_option().unwrap();
    assert_eq!(
        permissions_of(&user.permissions),
        [Permission::ViewRequests]
    );
}

#[tokio::test]
async fn a_taken_name_keeps_the_invite_valid() {
    let server = start(TestServer::builder()).await;
    let created = create_invite(&users(server.client_config()), "", &[])
        .await
        .unwrap();
    server.user_with_default_permissions("Mum").await;

    let taken = accept(&server, &token_of(&created), "mum").await;
    let other = accept(&server, &token_of(&created), "Mother").await;

    assert_refused(
        taken,
        ErrorCode::AlreadyExists,
        RefusalReason::REFUSAL_REASON_USER_NAME_TAKEN,
    );
    other.unwrap();
}

#[tokio::test]
async fn pending_invites_are_listed_with_their_note_creator_permissions_and_expiry() {
    let server = start(TestServer::builder()).await;
    let creator = server.user("Ann", &[Permission::ManageUsers]).await;
    let created = create_invite(
        &users(creator.client_config()),
        "  Mum ",
        &[Permission::ManageUsers],
    )
    .await
    .unwrap();

    let (listed, server_url_set) = list(&users(server.client_config())).await;

    assert!(server_url_set);
    let made = created.invite.into_option().unwrap();
    assert_eq!(listed, vec![made.clone()]);
    assert_eq!(made.note, "Mum");
    assert_eq!(made.creator_name, "Ann");
    assert_eq!(permissions_of(&made.permissions), [Permission::ManageUsers]);
    assert_eq!(made.expires_at - made.created_at, 7 * 24 * 60 * 60);
}

#[tokio::test]
async fn without_a_server_url_no_invite_is_made() {
    let server = TestServer::builder().start().await;
    let admin = users(server.client_config());

    let refused = create_invite(&admin, "", &[]).await;

    assert_refused(
        refused,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_SERVER_URL_REQUIRED,
    );
    assert_eq!(list(&admin).await, (Vec::new(), false));
}

#[tokio::test]
async fn an_invite_outlives_the_server_url_it_was_made_under() {
    let server = start(TestServer::builder()).await;
    let created = create_invite(&users(server.client_config()), "", &[])
        .await
        .unwrap();

    set_server_url(&server, "").await.unwrap();

    accept(&server, &token_of(&created), "Mum").await.unwrap();
}

#[tokio::test]
async fn guessing_invite_tokens_is_slowed_down_like_signing_in() {
    let server = start(TestServer::builder()).await;

    let mut last = Ok(());
    for _ in 0..=50 {
        last = accept(&server, "guess", "Mum").await.map(|_| ());
    }

    assert_refused(
        last,
        ErrorCode::ResourceExhausted,
        RefusalReason::REFUSAL_REASON_TOO_MANY_ATTEMPTS,
    );
}

#[tokio::test]
async fn a_user_made_by_an_invite_gets_the_default_cap_which_a_user_manager_changes() {
    let server = start(TestServer::builder()).await;
    let ann = server.user("Ann", &[Permission::ManageUsers]).await;
    let anns = users(ann.client_config());
    (users(server.client_config()).set_default_permissions(SetDefaultPermissionsRequest {
        remote_playback_limit: PlaybackLimit::PLAYBACK_LIMIT_720P.into(),
        ..Default::default()
    }))
    .await
    .unwrap();
    let created = create_invite(&anns, "", &[]).await.unwrap();
    let mum = (accept(&server, &token_of(&created), "Mum")
        .await
        .unwrap()
        .user)
        .id;
    let cap_of_mum = async || {
        let listed = anns.list_users(ListUsersRequest::default()).await.unwrap();
        (listed.into_owned().users.into_iter())
            .find(|user| user.id == mum)
            .unwrap()
            .remote_playback_limit
    };

    let invited = cap_of_mum().await;
    (anns.set_permissions(SetPermissionsRequest {
        user_id: mum,
        remote_playback_limit: PlaybackLimit::PLAYBACK_LIMIT_480P.into(),
        ..Default::default()
    }))
    .await
    .unwrap();
    let changed = cap_of_mum().await;

    assert_eq!(invited, PlaybackLimit::PLAYBACK_LIMIT_720P);
    assert_eq!(changed, PlaybackLimit::PLAYBACK_LIMIT_480P);
}
