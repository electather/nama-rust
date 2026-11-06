//! A Server serving the API on a random local port, for tests that call it the way clients do.

use std::{net::SocketAddr, sync::Arc};

use connectrpc::client::ClientConfig;
use nama_server::{config::Config, db::Database, plugins::Registry};
use tempfile::TempDir;
use tokio::{net::TcpListener, sync::oneshot};

/// A Server with its own empty data directory, serving until dropped.
pub(crate) struct TestServer {
    pub(crate) addr: SocketAddr,
    _stop: oneshot::Sender<()>,
    _data_dir: TempDir,
}

impl TestServer {
    /// Starts serving `config` on a free port, with a fresh database and the Plugins enabled in
    /// `plugins`.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    pub(crate) async fn start(config: Config, plugins: Registry) -> Self {
        let data_dir = tempfile::tempdir().unwrap();
        let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (stop, stopped) = oneshot::channel::<()>();
        tokio::spawn(nama_server::serve(
            listener,
            &config,
            db,
            plugins,
            None,
            async {
                let _: Result<(), oneshot::error::RecvError> = stopped.await;
            },
        ));
        Self {
            addr,
            _stop: stop,
            _data_dir: data_dir,
        }
    }

    /// Client settings that point at this Server.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    pub(crate) fn client_config(&self) -> ClientConfig {
        ClientConfig::new(format!("http://{}", self.addr).parse().unwrap())
    }
}
