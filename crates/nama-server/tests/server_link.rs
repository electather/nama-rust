//! Linking the Server, and a User, through the enabled Plugin that links it (ADR 0011, ADR 0094),
//! here an in-process one scripted by each test, and a Server without one, which has nothing to
//! Link. The Plugin is told the Server's Server Addresses, and shows whether the service reached it
//! from outside. Through it too, Users created from Accounts, signing in with an Account, and the
//! Admin who can always sign in without the Hub (ADR 0092).

#[path = "support/log.rs"]
mod log;
#[path = "support/refused.rs"]
mod refused;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::{
    future,
    num::NonZeroU16,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use connectrpc::{
    ConnectError, ErrorCode,
    client::{ClientConfig, HttpClient},
};
use log::Log;
use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    points::WatchStateChange,
    server_link::{
        AccountSignIn, Link, Reachability, ServerAddresses, ServerLinker, WatchlistEntry,
    },
};
use nama_proto::nama::v1::{
    self as proto, CreateUserFromAccountRequest, DeleteUserRequest, GetServerInfoRequest,
    GetServerLinkRequest, GetUserLinkRequest, ListUsersRequest, Permission, RefusalReason,
    ServerServiceClient, ServerSettings, SessionDelivery, SetPermissionsRequest,
    SignInServiceClient, SignInWithAccountRequest, StartAccountSignInRequest,
    StartServerLinkRequest, StartUserLinkRequest, UnlinkServerRequest, UnlinkUserRequest,
    UpdateServerSettingsRequest, UserServiceClient, WhoAmIRequest,
};
use nama_server::{config::Config, plugins::Registry};
use refused::assert_refused;
use server::TestServer;

/// A Plugin that links the Server, answering what its test scripts.
#[derive(Clone, Default)]
struct Scripted(Arc<Mutex<Script>>);

#[derive(Default)]
struct Script {
    /// What it answers `link`; `None` fails, as with the service unreachable.
    link: Option<Link>,
    /// The name it was last asked to start Linking as.
    started_as: Option<String>,
    /// What it answers `user_link` for every User; `None` while the Server isn't Linked.
    user_link: Option<Link>,
    /// The User it was last asked to start Linking, by id and name.
    user_started: Option<(i64, String)>,
    /// The User it was last asked to invite, by id, name and email.
    invited: Option<(i64, String, String)>,
    /// Where the sign-in with an Account it last started returns to.
    returns_to: Option<String>,
    /// The User an Account approved the sign-in as, until it is finished.
    approved: Option<i64>,
    /// The Server Addresses it was last told.
    addresses: Option<ServerAddresses>,
    /// The Users it was asked to Unlink, by id, in order.
    unlinked_users: Vec<i64>,
    /// Whether Unlinking a User fails, as with the service unreachable.
    unlink_user_fails: bool,
    /// Whether Unlinking a User never answers, as with the service slow to time out.
    unlink_user_hangs: bool,
}

impl Scripted {
    fn script(&self) -> MutexGuard<'_, Script> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn answer(&self) -> Result<Link, PluginError> {
        self.script()
            .link
            .clone()
            .ok_or_else(|| "the service is unreachable".into())
    }
}

impl Plugin for Scripted {
    fn manifest(&self) -> Manifest {
        Manifest {
            extension_points: vec![PointKey::provide::<dyn ServerLinker>()],
            ..Manifest::new(PluginId::from_static("linker"))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.provide::<dyn ServerLinker>(Priority::DEFAULT, Arc::new(self.clone()));
        Ok(())
    }
}

impl ServerLinker for Scripted {
    fn link(&self) -> BoxFuture<'_, Result<Link, PluginError>> {
        Box::pin(future::ready(self.answer()))
    }

    fn start_link<'a>(&'a self, server_name: &'a str) -> BoxFuture<'a, Result<Link, PluginError>> {
        self.script().started_as = Some(server_name.to_owned());
        let started = self.answer().map(|_| Link::Waiting {
            code: "BDFH-JKLM".to_owned(),
            url: "https://hub.example".to_owned(),
        });
        if let Ok(waiting) = &started {
            self.script().link = Some(waiting.clone());
        }
        Box::pin(future::ready(started))
    }

    fn unlink(&self) -> BoxFuture<'_, Result<(), PluginError>> {
        let unlinked = self.answer().map(|_| ());
        if unlinked.is_ok() {
            self.script().link = Some(Link::NotLinked);
        }
        Box::pin(future::ready(unlinked))
    }

    fn user_link(&self, _user_id: i64) -> BoxFuture<'_, Result<Option<Link>, PluginError>> {
        Box::pin(future::ready(Ok(self.script().user_link.clone())))
    }

    fn start_user_link<'a>(
        &'a self,
        user_id: i64,
        user_name: &'a str,
    ) -> BoxFuture<'a, Result<Link, PluginError>> {
        let waiting = Link::Waiting {
            code: "CDFG-HJKL".to_owned(),
            url: "https://hub.example".to_owned(),
        };
        let mut script = self.script();
        script.user_started = Some((user_id, user_name.to_owned()));
        script.user_link = Some(waiting.clone());
        Box::pin(future::ready(Ok(waiting)))
    }

    fn unlink_user(&self, user_id: i64) -> BoxFuture<'_, Result<(), PluginError>> {
        let mut script = self.script();
        script.unlinked_users.push(user_id);
        if script.unlink_user_hangs {
            return Box::pin(future::pending());
        }
        if script.unlink_user_fails {
            return Box::pin(future::ready(Err("the service is unreachable".into())));
        }
        script.user_link = Some(Link::NotLinked);
        Box::pin(future::ready(Ok(())))
    }

    fn invite_user<'a>(
        &'a self,
        user_id: i64,
        user_name: &'a str,
        email: &'a str,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        let invited = self.answer().map(|_| {
            self.script().invited = Some((user_id, user_name.to_owned(), email.to_owned()));
        });
        Box::pin(future::ready(invited))
    }

    fn start_account_sign_in<'a>(
        &'a self,
        return_url: &'a str,
    ) -> BoxFuture<'a, Result<AccountSignIn, PluginError>> {
        let started = self.answer().map(|_| {
            self.script().returns_to = Some(return_url.to_owned());
            AccountSignIn {
                url: "https://hub.example/server-sign-in/1".to_owned(),
                handle: "handle".to_owned(),
            }
        });
        Box::pin(future::ready(started))
    }

    fn finish_account_sign_in<'a>(
        &'a self,
        handle: &'a str,
        code: &'a str,
    ) -> BoxFuture<'a, Result<Option<i64>, PluginError>> {
        let finished = self.answer().map(|_| {
            let approved = self.script().approved.take();
            approved.filter(|_| handle == "handle" && code == "code")
        });
        Box::pin(future::ready(finished))
    }
    fn set_addresses(&self, addresses: ServerAddresses) -> BoxFuture<'_, Result<(), PluginError>> {
        self.script().addresses = Some(addresses);
        Box::pin(future::ready(Ok(())))
    }

    /// Pulling is `tests/sync.rs`'s.
    fn watch_states(
        &self,
        _user_id: i64,
    ) -> BoxFuture<'_, Result<Vec<WatchStateChange>, PluginError>> {
        Box::pin(future::ready(Ok(Vec::new())))
    }

    /// Syncing is `tests/sync.rs`'s.
    fn sync_watchlist(
        &self,
        _user_id: i64,
        _watchlist: Vec<WatchlistEntry>,
    ) -> BoxFuture<'_, Result<Vec<WatchlistEntry>, PluginError>> {
        Box::pin(future::ready(Ok(Vec::new())))
    }
}

async fn with_linker(linker: &Scripted) -> TestServer {
    TestServer::builder()
        .plugins(enabling(linker))
        .start()
        .await
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn enabling(linker: &Scripted) -> Registry {
    let plugins = Registry::new();
    plugins.enable(Arc::new(linker.clone())).unwrap();
    plugins
}

fn settings(server: &TestServer) -> ServerServiceClient<HttpClient> {
    ServerServiceClient::new(HttpClient::plaintext(), server.client_config())
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn set_server_url(server: &TestServer, url: &str) {
    settings(server)
        .update_server_settings(UpdateServerSettingsRequest {
            settings: ServerSettings {
                url: url.to_owned(),
                ..Default::default()
            }
            .into(),
            ..Default::default()
        })
        .await
        .unwrap();
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn link(server: &TestServer) -> Option<proto::ServerLink> {
    let got = settings(server)
        .get_server_link(GetServerLinkRequest::default())
        .await
        .unwrap();
    got.into_owned().link.into_option()
}

fn users(config: ClientConfig) -> UserServiceClient<HttpClient> {
    UserServiceClient::new(HttpClient::plaintext(), config)
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn user_link(config: ClientConfig) -> Option<proto::UserLink> {
    let got = users(config)
        .get_user_link(GetUserLinkRequest::default())
        .await
        .unwrap();
    got.into_owned().link.into_option()
}

/// Waits until `linker` was asked to Unlink exactly `expected`, since a delete doesn't wait for it.
async fn until_unlinked(linker: &Scripted, expected: &[i64]) {
    let waited = tokio::time::timeout(server::WAIT, async {
        while linker.script().unlinked_users != expected {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    assert!(
        waited.is_ok(),
        "Unlinked {:?}, not {expected:?}",
        linker.script().unlinked_users
    );
}

/// Deletes the User `user_id`, calling as `who`.
async fn delete(who: ClientConfig, user_id: i64) -> Result<(), ConnectError> {
    users(who)
        .delete_user(DeleteUserRequest {
            user_id,
            ..Default::default()
        })
        .await
        .map(drop)
}

#[tokio::test]
async fn a_server_without_a_plugin_that_links_it_has_nothing_to_link() {
    let server = TestServer::builder().start().await;
    set_server_url(&server, "https://nama.example").await;

    assert_eq!(link(&server).await, None);
    assert_refused(
        settings(&server)
            .start_server_link(StartServerLinkRequest::default())
            .await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_NO_SERVER_LINKER,
    );
    assert_refused(
        settings(&server)
            .unlink_server(UnlinkServerRequest::default())
            .await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_NO_SERVER_LINKER,
    );
    assert_eq!(user_link(server.client_config()).await, None);
    assert!(!offers_account_sign_in(&server).await);
    assert_refused(
        start_account_sign_in(&server, ON_SERVER).await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_NO_SERVER_LINKER,
    );
    assert_refused(
        create_from_account(&server, "Grace").await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_NO_SERVER_LINKER,
    );
    assert_refused(
        users(server.client_config())
            .start_user_link(StartUserLinkRequest::default())
            .await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_NO_SERVER_LINKER,
    );
    assert_refused(
        users(server.client_config())
            .unlink_user(UnlinkUserRequest::default())
            .await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_NO_SERVER_LINKER,
    );
}

#[tokio::test]
async fn a_users_link_is_unset_until_the_server_is_linked() {
    let linker = Scripted::default();
    let server = with_linker(&linker).await;

    assert_eq!(user_link(server.client_config()).await, None);
}

#[tokio::test]
async fn a_user_shows_the_code_the_plugin_starts_for_them_and_then_whose_they_are() {
    let linker = Scripted::default();
    linker.script().user_link = Some(Link::NotLinked);
    let server = with_linker(&linker).await;
    let grace = server.user_with_default_permissions("Grace").await;
    assert_eq!(
        user_link(grace.client_config()).await,
        Some(proto::UserLink::default())
    );

    let started = users(grace.client_config())
        .start_user_link(StartUserLinkRequest::default())
        .await
        .unwrap()
        .into_owned()
        .link
        .into_option();

    let waiting = proto::UserLink {
        code: "CDFG-HJKL".to_owned(),
        url: "https://hub.example".to_owned(),
        ..Default::default()
    };
    assert_eq!(started, Some(waiting.clone()));
    assert_eq!(
        linker.script().user_started,
        Some((grace.id, "Grace".to_owned()))
    );
    assert_eq!(user_link(grace.client_config()).await, Some(waiting));
    linker.script().user_link = Some(Link::Linked {
        account: "grace@example.com".to_owned(),
        reachability: None,
    });
    let shown = user_link(grace.client_config()).await.unwrap();
    assert_eq!(shown.account, "grace@example.com");

    users(grace.client_config())
        .unlink_user(UnlinkUserRequest::default())
        .await
        .unwrap();

    assert_eq!(
        user_link(grace.client_config()).await,
        Some(proto::UserLink::default())
    );
}

#[tokio::test]
async fn the_server_shows_the_code_its_plugin_starts_and_then_whose_it_is() {
    let linker = Scripted::default();
    linker.script().link = Some(Link::NotLinked);
    let server = with_linker(&linker).await;
    assert_eq!(link(&server).await, Some(proto::ServerLink::default()));

    let started = settings(&server)
        .start_server_link(StartServerLinkRequest::default())
        .await
        .unwrap()
        .into_owned()
        .link
        .into_option();

    let waiting = proto::ServerLink {
        code: "BDFH-JKLM".to_owned(),
        url: "https://hub.example".to_owned(),
        ..Default::default()
    };
    assert_eq!(started, Some(waiting.clone()));
    assert_eq!(linker.script().started_as.as_deref(), Some("Nama"));
    assert_eq!(link(&server).await, Some(waiting));
    linker.script().link = Some(Link::Linked {
        account: "ada@example.com".to_owned(),
        reachability: Some(Reachability {
            address: "http://203.0.113.7:7980".to_owned(),
            reachable: true,
        }),
    });
    let shown = link(&server).await.unwrap();
    assert_eq!(shown.account, "ada@example.com");
    assert_eq!(shown.code, "");
    assert_eq!(shown.public_address, "http://203.0.113.7:7980");
    assert!(shown.reachable);

    settings(&server)
        .unlink_server(UnlinkServerRequest::default())
        .await
        .unwrap();

    assert_eq!(link(&server).await, Some(proto::ServerLink::default()));
}

#[tokio::test]
async fn a_plugin_that_cant_reach_its_service_leaves_the_server_as_it_was() {
    let linker = Scripted::default();
    let server = with_linker(&linker).await;

    let started = settings(&server)
        .start_server_link(StartServerLinkRequest::default())
        .await;
    let unlinked = settings(&server)
        .unlink_server(UnlinkServerRequest::default())
        .await;

    assert_eq!(started.unwrap_err().code, ErrorCode::Unavailable);
    assert_eq!(unlinked.unwrap_err().code, ErrorCode::Unavailable);
}

/// A `return_url` on the Server URL [`linked_server`] sets.
const ON_SERVER: &str = "https://nama.example/sign-in";

/// A Server whose Plugin has it Linked to an Account, at the Server URL `https://nama.example`.
async fn linked_server(linker: &Scripted) -> TestServer {
    linker.script().link = Some(Link::Linked {
        account: "ada@example.com".to_owned(),
        reachability: None,
    });
    let server = with_linker(linker).await;
    set_server_url(&server, "https://nama.example").await;
    server
}

async fn create_from_account(server: &TestServer, name: &str) -> Result<i64, ConnectError> {
    Ok(users(server.client_config())
        .create_user_from_account(CreateUserFromAccountRequest {
            name: name.to_owned(),
            email: "grace@example.com".to_owned(),
            ..Default::default()
        })
        .await?
        .into_owned()
        .id)
}

fn sign_in(server: &TestServer) -> SignInServiceClient<HttpClient> {
    SignInServiceClient::new(HttpClient::plaintext(), server.anonymous_client_config())
}

async fn start_account_sign_in(
    server: &TestServer,
    return_url: &str,
) -> Result<proto::StartAccountSignInResponse, ConnectError> {
    Ok(sign_in(server)
        .start_account_sign_in(StartAccountSignInRequest {
            return_url: return_url.to_owned(),
            ..Default::default()
        })
        .await?
        .into_owned())
}

/// Finishes the sign-in `handle` stands for with the one-time code the service sent the person
/// back with, and returns the session token.
async fn sign_in_with_account(
    server: &TestServer,
    handle: &str,
    code: &str,
) -> Result<proto::SignInWithAccountResponse, ConnectError> {
    Ok(sign_in(server)
        .sign_in_with_account(SignInWithAccountRequest {
            handle: handle.to_owned(),
            code: code.to_owned(),
            delivery: SessionDelivery::SESSION_DELIVERY_TOKEN.into(),
            ..Default::default()
        })
        .await?
        .into_owned())
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn offers_account_sign_in(server: &TestServer) -> bool {
    ServerServiceClient::new(HttpClient::plaintext(), server.anonymous_client_config())
        .get_server_info(GetServerInfoRequest::default())
        .await
        .unwrap()
        .into_owned()
        .account_sign_in
}

#[tokio::test]
async fn a_user_created_from_an_account_signs_in_once_the_account_approves() {
    let linker = Scripted::default();
    let server = linked_server(&linker).await;

    let grace = create_from_account(&server, " Grace ").await.unwrap();

    assert_eq!(
        linker.script().invited,
        Some((grace, "Grace".to_owned(), "grace@example.com".to_owned()))
    );
    assert!(offers_account_sign_in(&server).await);
    let started = start_account_sign_in(&server, ON_SERVER).await.unwrap();
    assert_eq!(started.url, "https://hub.example/server-sign-in/1");
    assert_eq!(linker.script().returns_to.as_deref(), Some(ON_SERVER));
    assert_refused(
        start_account_sign_in(&server, "javascript:alert(1)").await,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_NOT_A_URL,
    );
    // Its Server URL is the one Server Address a Server listening on loopback reports.
    let off_server = assert_refused(
        start_account_sign_in(&server, "http://localhost:7980/sign-in").await,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_NOT_A_SERVER_ADDRESS,
    );
    assert_eq!(off_server.params["serverUrl"], "https://nama.example");
    assert_eq!(linker.script().returns_to.as_deref(), Some(ON_SERVER));
    assert_refused(
        sign_in_with_account(&server, &started.handle, "code").await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_ACCOUNT_SIGN_IN_NOT_FOUND,
    );
    linker.script().approved = Some(grace);
    let signed_in = sign_in_with_account(&server, &started.handle, "code")
        .await
        .unwrap();
    assert_eq!(signed_in.user.into_option().unwrap().name, "Grace");
    let who = SignInServiceClient::new(
        HttpClient::plaintext(),
        server.client_config_as(&signed_in.token),
    )
    .who_am_i(WhoAmIRequest::default())
    .await
    .unwrap()
    .into_owned();
    assert_eq!(who.user.into_option().unwrap().id, grace);
    // Whoever approved on the Hub may not hold this Device, as with a Sign-in Code (ADR 0014).
    assert_refused(
        users(server.client_config_as(&signed_in.token))
            .start_user_link(StartUserLinkRequest::default())
            .await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_RECENT_AUTHENTICATION_REQUIRED,
    );
}

#[tokio::test]
async fn while_the_hub_is_down_devices_stay_signed_in_but_an_account_cant_sign_in() {
    let linker = Scripted::default();
    let server = linked_server(&linker).await;
    let grace = create_from_account(&server, "Grace").await.unwrap();
    let started = start_account_sign_in(&server, ON_SERVER).await.unwrap();
    linker.script().approved = Some(grace);
    let signed_in = sign_in_with_account(&server, &started.handle, "code")
        .await
        .unwrap();

    linker.script().link = None;

    let still = SignInServiceClient::new(
        HttpClient::plaintext(),
        server.client_config_as(&signed_in.token),
    )
    .who_am_i(WhoAmIRequest::default())
    .await;
    assert!(still.is_ok(), "{still:?}");
    let refused = start_account_sign_in(&server, ON_SERVER).await.unwrap_err();
    assert_eq!(refused.code, ErrorCode::Unavailable);
    let message = refused.message.as_deref().unwrap_or_default();
    assert!(message.contains("needs the Hub"), "{refused:?}");
    assert!(
        !message.to_lowercase().contains("password"),
        "a User created from an Account has none: {refused:?}"
    );
    assert!(!offers_account_sign_in(&server).await);
    let before = users(server.client_config())
        .list_users(ListUsersRequest::default())
        .await
        .unwrap()
        .into_owned()
        .users
        .len();
    let not_created = create_from_account(&server, "Hedy").await.unwrap_err();
    assert_eq!(not_created.code, ErrorCode::Unavailable);
    let after = users(server.client_config())
        .list_users(ListUsersRequest::default())
        .await
        .unwrap()
        .into_owned()
        .users
        .len();
    assert_eq!(after, before);
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn cannot_sign_in(server: &TestServer) -> Vec<i64> {
    let listed = users(server.client_config())
        .list_users(ListUsersRequest::default())
        .await
        .unwrap();
    listed.into_owned().cannot_sign_in_user_ids
}

#[tokio::test]
async fn a_user_with_no_password_whose_account_unlinked_them_is_listed_as_unable_to_sign_in() {
    let linker = Scripted::default();
    let server = linked_server(&linker).await;
    let grace = create_from_account(&server, "Grace").await.unwrap();
    linker.script().user_link = Some(Link::Linked {
        account: "grace@example.com".to_owned(),
        reachability: None,
    });
    assert_eq!(cannot_sign_in(&server).await, Vec::<i64>::new());

    // As when Grace's Account is deleted on the Hub.
    linker.script().user_link = Some(Link::NotLinked);

    assert_eq!(cannot_sign_in(&server).await, [grace]);
}

#[tokio::test]
async fn the_last_admin_who_signs_in_with_a_password_cant_be_demoted_or_deleted() {
    let linker = Scripted::default();
    let server = linked_server(&linker).await;
    let grace = create_from_account(&server, "Grace").await.unwrap();
    let admin = SignInServiceClient::new(HttpClient::plaintext(), server.client_config())
        .who_am_i(WhoAmIRequest::default())
        .await
        .unwrap()
        .into_owned()
        .user
        .into_option()
        .unwrap()
        .id;
    let set = async |user_id, permissions: &[Permission]| {
        users(server.client_config())
            .set_permissions(SetPermissionsRequest {
                user_id,
                permissions: permissions.iter().map(|&p| p.into()).collect(),
                ..Default::default()
            })
            .await
    };
    set(grace, &[Permission::PERMISSION_ADMIN]).await.unwrap();

    assert_refused(
        set(admin, &[Permission::PERMISSION_REQUEST]).await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_LAST_ADMIN,
    );
    assert_refused(
        users(server.client_config())
            .delete_user(DeleteUserRequest {
                user_id: admin,
                ..Default::default()
            })
            .await,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_LAST_ADMIN,
    );
    // An Admin who signs in only with their Account may go.
    set(grace, &[Permission::PERMISSION_REQUEST]).await.unwrap();
}

#[tokio::test]
async fn the_plugin_is_told_the_server_url_and_port_the_admin_set() {
    let linker = Scripted::default();
    let server = with_linker(&linker).await;
    set_server_url(&server, "https://nama.example.com").await;

    let _restarted = TestServer::builder()
        .config(Config {
            forwarded_port: NonZeroU16::new(7443),
            ..Config::default()
        })
        .plugins(enabling(&linker))
        .restart(server)
        .await;

    // The test Server listens on 127.0.0.1, which no other device on its LAN can reach.
    let told = ServerAddresses {
        url: "https://nama.example.com/".to_owned(),
        lan: vec![],
        public_port: Some(7443),
    };
    let waited = tokio::time::timeout(server::WAIT, async {
        while linker.script().addresses.as_ref() != Some(&told) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    assert!(waited.is_ok(), "told {:?}", linker.script().addresses);
}

#[tokio::test]
async fn deleting_a_user_unlinks_them_once() {
    let linker = Scripted::default();
    linker.script().user_link = Some(Link::Linked {
        account: "grace@example.com".to_owned(),
        reachability: None,
    });
    let server = with_linker(&linker).await;
    let grace = server.user_with_default_permissions("Grace").await;

    delete(server.client_config(), grace.id).await.unwrap();

    until_unlinked(&linker, &[grace.id]).await;
}

#[tokio::test]
async fn deleting_a_user_doesnt_wait_for_the_plugin_to_unlink_them() {
    let linker = Scripted::default();
    linker.script().unlink_user_hangs = true;
    let server = with_linker(&linker).await;
    let grace = server.user_with_default_permissions("Grace").await;

    let deleted =
        tokio::time::timeout(server::WAIT, delete(server.client_config(), grace.id)).await;

    deleted.expect("the delete waited for the Plugin").unwrap();
    until_unlinked(&linker, &[grace.id]).await;
}

#[tokio::test]
async fn an_admin_deleting_themselves_is_unlinked() {
    let linker = Scripted::default();
    let server = with_linker(&linker).await;
    let ada = server.user("Ada", &[Permission::Admin]).await;

    delete(ada.client_config(), ada.id).await.unwrap();

    until_unlinked(&linker, &[ada.id]).await;
}

#[tokio::test]
async fn an_unlinked_user_is_deleted() {
    let linker = Scripted::default();
    linker.script().user_link = Some(Link::NotLinked);
    let server = with_linker(&linker).await;
    let grace = server.user_with_default_permissions("Grace").await;

    delete(server.client_config(), grace.id).await.unwrap();
}

#[tokio::test]
async fn a_refused_delete_unlinks_no_one() {
    let linker = Scripted::default();
    let server = with_linker(&linker).await;

    delete(server.client_config(), 9999).await.unwrap_err();

    assert_eq!(linker.script().unlinked_users, Vec::<i64>::new());
}

#[tokio::test]
async fn a_user_is_deleted_without_a_plugin_that_links_the_server() {
    let server = TestServer::builder().start().await;
    let grace = server.user_with_default_permissions("Grace").await;

    delete(server.client_config(), grace.id).await.unwrap();
}

#[tokio::test]
async fn a_plugin_that_cant_unlink_a_deleted_user_leaves_them_deleted_and_warns() {
    let log = Log::global();
    let linker = Scripted::default();
    linker.script().unlink_user_fails = true;
    let server = with_linker(&linker).await;
    let grace = server.user_with_default_permissions("Grace").await;

    delete(server.client_config(), grace.id).await.unwrap();

    let listed = users(server.client_config())
        .list_users(ListUsersRequest::default())
        .await
        .unwrap()
        .into_owned()
        .users;
    assert!(listed.iter().all(|user| user.id != grace.id));
    log.wait_for(
        &["could not tell the service a User was deleted", "linker"],
        1,
    )
    .await;
}
