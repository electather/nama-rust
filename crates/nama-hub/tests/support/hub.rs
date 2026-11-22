//! A Hub serving the API on a random local port, on a database `#[sqlx::test]` made for the test,
//! for tests that call it the way clients do.

use std::net::SocketAddr;

use connectrpc::client::ClientConfig;
use nama_hub::{db, signing::Keys};
use sqlx::PgPool;
use tokio::{net::TcpListener, sync::oneshot};

/// The Hub's one signing key, made with `openssl genpkey -algorithm ed25519`.
const SIGNING_KEY: &str = "-----BEGIN PRIVATE KEY-----
MC4CAQAwBQYDK2VwBCIEIJzJDsQn3SpQXPoFGl74lUI4L+huggYWuNcSt9vaCwTS
-----END PRIVATE KEY-----
";

/// A Hub serving until it is dropped.
#[derive(Debug)]
pub(crate) struct TestHub {
    addr: SocketAddr,
    db: PgPool,
    _stop: oneshot::Sender<()>,
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl TestHub {
    /// Migrates `db`, as the Hub does at startup, and serves the Hub on it.
    pub(crate) async fn start(db: PgPool) -> Self {
        db::migrate(&db).await.unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (stop, stopped) = oneshot::channel::<()>();
        let keys = Keys::from_pems(SIGNING_KEY).unwrap();
        tokio::spawn(nama_hub::serve(listener, db.clone(), keys, &[], async {
            let _: Result<(), oneshot::error::RecvError> = stopped.await;
        }));
        Self {
            addr,
            db,
            _stop: stop,
        }
    }

    /// Client settings that point at the Hub and carry no sign-in.
    pub(crate) fn client_config(&self) -> ClientConfig {
        ClientConfig::new(format!("http://{}", self.addr).parse().unwrap())
    }

    /// Closes the Hub's connections to PostgreSQL, as if it went down.
    pub(crate) async fn lose_database(&self) {
        self.db.close().await;
    }
}
