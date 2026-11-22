//! A Hub serving the API on a random local port, on a database `#[sqlx::test]` made for the test,
//! for tests that call it the way clients do, forwarding what Servers share to a fake backend.

use std::{
    net::SocketAddr,
    sync::{Arc, Mutex, PoisonError},
};

use axum::body::Bytes;
use connectrpc::client::ClientConfig;
use nama_hub::{
    db,
    diagnostics::{Backend, Forwarding, Signal},
    mail::Mail,
    signing::Keys,
};
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
    shared: Arc<SharedBatches>,
    _stop: oneshot::Sender<()>,
}

/// The fake Diagnostics backend: each batch it was sent, oldest first.
#[derive(Debug, Default)]
struct SharedBatches(Mutex<Vec<(Signal, Bytes)>>);

impl Backend for SharedBatches {
    fn forward(&self, signal: Signal, batch: Bytes) -> Forwarding<'_> {
        let mut batches = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        batches.push((signal, batch));
        Box::pin(async { Ok(()) })
    }
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl TestHub {
    /// Migrates `db`, as the Hub does at startup, and serves the Hub on it, sending mail through
    /// `mail`, if any, and trusting this machine as its reverse proxy, so a test can say in
    /// `X-Forwarded-For` which client a request is from.
    pub(crate) async fn start(db: PgPool, mail: Option<Mail>) -> Self {
        db::migrate(&db).await.unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (stop, stopped) = oneshot::channel::<()>();
        let keys = Keys::from_pems(SIGNING_KEY).unwrap();
        let shared = Arc::new(SharedBatches::default());
        let backend: Arc<dyn Backend> = Arc::<SharedBatches>::clone(&shared);
        tokio::spawn(nama_hub::serve(
            listener,
            db.clone(),
            keys,
            &["127.0.0.1".parse().unwrap()],
            mail,
            Some(backend),
            async {
                let _: Result<(), oneshot::error::RecvError> = stopped.await;
            },
        ));
        Self {
            addr,
            db,
            shared,
            _stop: stop,
        }
    }

    /// Client settings that point at the Hub and carry no sign-in.
    pub(crate) fn client_config(&self) -> ClientConfig {
        ClientConfig::new(format!("http://{}", self.addr).parse().unwrap())
    }

    /// Each batch the Hub forwarded to its Diagnostics backend, oldest first.
    pub(crate) fn shared(&self) -> Vec<(Signal, Bytes)> {
        self.shared
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Closes the Hub's connections to PostgreSQL, as if it went down.
    pub(crate) async fn lose_database(&self) {
        self.db.close().await;
    }
}
