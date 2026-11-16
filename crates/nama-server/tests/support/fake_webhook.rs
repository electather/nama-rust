//! A local stand-in for a webhook receiver, recording each body posted to it, so the Webhook
//! Plugin's tests need no outside service.

use std::{
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicU16, Ordering},
    },
    time::{Duration, Instant},
};

use axum::{
    Router,
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
};
use tokio::{net::TcpListener, task::JoinHandle};

use crate::server::WAIT;

/// One body posted to the fake webhook.
#[derive(Debug, Clone)]
pub(crate) struct Posted {
    /// Its `X-Nama-Signature` header, `None` if it had none.
    pub(crate) signature: Option<String>,
    /// Its body, as sent.
    pub(crate) body: Bytes,
}

/// What the fake webhook shares with its handler: what was posted, and the status it answers.
#[derive(Default)]
struct Shared {
    posted: Mutex<Vec<Posted>>,
    status: AtomicU16,
}

/// A fake webhook receiver serving on a local port until dropped, answering `200 OK` until told
/// otherwise.
pub(crate) struct FakeWebhook {
    url: String,
    shared: Arc<Shared>,
    server: JoinHandle<()>,
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl FakeWebhook {
    /// Starts serving on a free local port.
    pub(crate) async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/hook", listener.local_addr().unwrap());
        let shared = Arc::new(Shared {
            status: AtomicU16::new(StatusCode::OK.as_u16()),
            ..Shared::default()
        });
        let app = Router::new()
            .route("/hook", post(receive))
            .with_state(Arc::clone(&shared));
        let server = tokio::spawn(async move {
            let _: Result<(), _> = axum::serve(listener, app).await;
        });
        Self {
            url,
            shared,
            server,
        }
    }

    /// Where it serves, as the Webhook URL setting names it.
    pub(crate) fn url(&self) -> &str {
        &self.url
    }

    /// Answers every later POST with `status`.
    pub(crate) fn answer(&self, status: StatusCode) {
        self.shared.status.store(status.as_u16(), Ordering::Relaxed);
    }

    /// Waits until `count` bodies have been posted, and returns them all, in order.
    pub(crate) async fn wait_for_posts(&self, count: usize) -> Vec<Posted> {
        let started = Instant::now();
        loop {
            let posted = lock(&self.shared.posted).clone();
            if posted.len() >= count {
                return posted;
            }
            assert!(
                started.elapsed() < WAIT,
                "the webhook got {posted:?}, not {count} posts"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}

impl Drop for FakeWebhook {
    /// Stops serving: dropping a task's handle alone would leave it running.
    fn drop(&mut self) {
        self.server.abort();
    }
}

async fn receive(State(shared): State<Arc<Shared>>, headers: HeaderMap, body: Bytes) -> StatusCode {
    let signature = headers
        .get("x-nama-signature")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    lock(&shared.posted).push(Posted { signature, body });
    StatusCode::from_u16(shared.status.load(Ordering::Relaxed)).unwrap_or(StatusCode::OK)
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
