//! A Hub serving the API on a random local port, on a database `#[sqlx::test]` made for the test,
//! for tests that call it the way clients do, forwarding what Servers share to a fake backend and
//! naming Servers in a fake zone.

use std::{
    collections::BTreeMap,
    future,
    net::SocketAddr,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
};

use axum::body::Bytes;
use connectrpc::client::ClientConfig;
use nama_hub::{
    db,
    diagnostics::{Backend, Forwarding, Signal},
    dns::{DnsError, Record, Records, ServerNames, Writing},
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

/// The zone the Hub names Servers in.
pub(crate) const ZONE: &str = "namadirect.test";

/// The address the test Hub's passkeys are for, which a browser's answers name as their origin.
pub(crate) const PUBLIC_URL: &str = "https://hub.example";

/// A Hub serving until it is dropped.
#[derive(Debug)]
pub(crate) struct TestHub {
    addr: SocketAddr,
    db: PgPool,
    shared: Arc<SharedBatches>,
    zone: Arc<FakeZone>,
    _stop: oneshot::Sender<()>,
}

/// The fake DNS provider: the zone's address records, its TXT records by name, and whether it
/// fails, as if down.
#[derive(Debug, Default)]
struct FakeZone {
    records: Mutex<Vec<Record>>,
    texts: Mutex<BTreeMap<String, Vec<String>>>,
    down: AtomicBool,
}

impl FakeZone {
    /// A failure, while it is down.
    fn failure(&self) -> Option<Writing<'_>> {
        let refused = DnsError::Refused {
            status: reqwest::StatusCode::SERVICE_UNAVAILABLE,
            body: String::new(),
        };
        let down = self.down.load(Ordering::Relaxed);
        down.then(|| -> Writing<'_> { Box::pin(future::ready(Err(refused))) })
    }
}

impl Records for FakeZone {
    fn replace(&self, name: &str, records: Vec<Record>) -> Writing<'_> {
        if let Some(failure) = self.failure() {
            return failure;
        }
        let under = format!(".{name}");
        let mut zone = self.records.lock().unwrap_or_else(PoisonError::into_inner);
        zone.retain(|record| record.name != name && !record.name.ends_with(&under));
        zone.extend(records);
        Box::pin(future::ready(Ok(())))
    }

    fn replace_texts(&self, name: &str, values: Vec<String>) -> Writing<'_> {
        if let Some(failure) = self.failure() {
            return failure;
        }
        let mut texts = self.texts.lock().unwrap_or_else(PoisonError::into_inner);
        if values.is_empty() {
            texts.remove(name);
        } else {
            texts.insert(name.to_owned(), values);
        }
        Box::pin(future::ready(Ok(())))
    }
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
    /// Migrates `db`, as the Hub does at startup, and serves the Hub on it at the public address
    /// [`PUBLIC_URL`], sending mail through `mail`, if any, and trusting this machine as its
    /// reverse proxy, so a test can say in `X-Forwarded-For` which client a request is from.
    pub(crate) async fn start(db: PgPool, mail: Option<Mail>) -> Self {
        Self::start_at(db, mail, Some(PUBLIC_URL)).await
    }

    /// As [`TestHub::start`], but with no public address, as without `NAMA_HUB_PUBLIC_URL`.
    pub(crate) async fn start_without_public_address(db: PgPool) -> Self {
        Self::start_at(db, None, None).await
    }

    async fn start_at(db: PgPool, mail: Option<Mail>, public_url: Option<&str>) -> Self {
        db::migrate(&db).await.unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (stop, stopped) = oneshot::channel::<()>();
        let keys = Keys::from_pems(SIGNING_KEY).unwrap();
        let shared = Arc::new(SharedBatches::default());
        let backend: Arc<dyn Backend> = Arc::<SharedBatches>::clone(&shared);
        let zone = Arc::new(FakeZone::default());
        let names = ServerNames {
            zone: ZONE.to_owned(),
            records: Arc::<FakeZone>::clone(&zone),
        };
        let public_url = public_url.map(|url| url.parse().unwrap());
        tokio::spawn(nama_hub::serve(
            listener,
            db.clone(),
            keys,
            &["127.0.0.1".parse().unwrap()],
            mail,
            Some(names),
            Some(backend),
            public_url.as_ref(),
            async {
                let _: Result<(), oneshot::error::RecvError> = stopped.await;
            },
        ));
        Self {
            addr,
            db,
            shared,
            zone,
            _stop: stop,
        }
    }

    /// The address records in the zone the Hub names Servers in, sorted.
    pub(crate) fn zone(&self) -> Vec<Record> {
        let mut records = (self.zone.records.lock())
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        records.sort();
        records
    }

    /// The TXT records in the zone the Hub names Servers in, by name.
    pub(crate) fn texts(&self) -> BTreeMap<String, Vec<String>> {
        (self.zone.texts.lock())
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Has writing the zone fail while `down`, as if the DNS provider were, and work again after.
    pub(crate) fn set_zone_down(&self, down: bool) {
        self.zone.down.store(down, Ordering::Relaxed);
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
