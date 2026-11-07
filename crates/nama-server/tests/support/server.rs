//! A Server serving the API on a random local port, for tests that call it the way clients do.

use std::{net::SocketAddr, sync::Arc};

use connectrpc::client::{ClientConfig, HttpClient};
use nama_plugin_local_auth::LocalAuth;
use nama_proto::nama::v1::{SessionDelivery, SetupRequest, SignInServiceClient};
use nama_server::{
    config::Config,
    db::Database,
    plugins::{EnableError, Registry},
    users::SetupCode,
};
use tempfile::TempDir;
use tokio::{net::TcpListener, sync::oneshot};

/// The password of every User a test creates.
pub(crate) const PASSWORD: &str = "correct horse battery staple";

/// A Server with its own empty data directory and an Admin signed in, serving until dropped.
pub(crate) struct TestServer {
    pub(crate) addr: SocketAddr,
    /// The session token of the Admin's Device.
    pub(crate) admin_token: String,
    _stop: oneshot::Sender<()>,
    _data_dir: TempDir,
}

impl TestServer {
    /// Starts serving `config` on a free port, with a fresh database and the Plugins enabled in
    /// `plugins` plus `local-auth`, if it isn't already, and signs an Admin in.
    pub(crate) async fn start(config: Config, plugins: Registry) -> Self {
        Self::start_with_db(config, plugins).await.0
    }

    /// Like [`TestServer::start`], also returning the Server's database, for a test that reads or
    /// changes what the API can't.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    pub(crate) async fn start_with_db(config: Config, plugins: Registry) -> (Self, Arc<Database>) {
        let enabled = plugins.enable(Arc::new(LocalAuth));
        // Already enabled such as by `Registry::with_builtins`.
        assert!(
            matches!(enabled, Ok(()) | Err(EnableError::AlreadyEnabled(_))),
            "{enabled:?}"
        );
        let data_dir = tempfile::tempdir().unwrap();
        let config = Config {
            data_dir: data_dir.path().to_owned(),
            ..config
        };
        let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let setup_code = SetupCode::generate().unwrap();
        let (stop, stopped) = oneshot::channel::<()>();
        tokio::spawn(nama_server::serve(
            listener,
            &config,
            Arc::clone(&db),
            plugins,
            Some(setup_code.clone()),
            async {
                let _: Result<(), oneshot::error::RecvError> = stopped.await;
            },
        ));
        let admin_token = set_up(anonymous_config(addr), &setup_code).await;
        let server = Self {
            addr,
            admin_token,
            _stop: stop,
            _data_dir: data_dir,
        };
        (server, db)
    }

    /// Client settings that point at this Server and call it as the Admin.
    pub(crate) fn client_config(&self) -> ClientConfig {
        self.client_config_as(&self.admin_token)
    }

    /// Client settings that point at this Server and present `token` as a bearer token.
    pub(crate) fn client_config_as(&self, token: &str) -> ClientConfig {
        self.anonymous_client_config()
            .with_default_header("authorization", format!("Bearer {token}"))
    }

    /// Client settings that point at this Server and carry no session.
    pub(crate) fn anonymous_client_config(&self) -> ClientConfig {
        anonymous_config(self.addr)
    }
}

/// Client settings that point at the Server on `addr` and carry no session.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn anonymous_config(addr: SocketAddr) -> ClientConfig {
    ClientConfig::new(format!("http://{addr}").parse().unwrap())
}

/// Creates the first Admin with `setup_code` and returns their Device's session token.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn set_up(config: ClientConfig, setup_code: &SetupCode) -> String {
    SignInServiceClient::new(HttpClient::plaintext(), config)
        .setup(SetupRequest {
            setup_code: setup_code.to_string(),
            name: "Admin".to_owned(),
            password: PASSWORD.to_owned(),
            device_name: "Test".to_owned(),
            delivery: SessionDelivery::SESSION_DELIVERY_TOKEN.into(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .token
}
