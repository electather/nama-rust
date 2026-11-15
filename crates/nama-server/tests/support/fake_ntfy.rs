//! A local stand-in for an ntfy server, recording each message published to it, so the ntfy
//! Plugin's tests need no ntfy.

use std::{
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, Instant},
};

use axum::{
    Router,
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::post,
};
use tokio::{net::TcpListener, task::JoinHandle};

use crate::server::WAIT;

/// One message published to the fake ntfy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Published {
    /// The topic it was published to.
    pub(crate) topic: String,
    /// Its `Title`, `Click` and `Authorization` headers, each `None` if it had none.
    pub(crate) title: Option<String>,
    pub(crate) click: Option<String>,
    pub(crate) authorization: Option<String>,
    /// Its body, the message's text.
    pub(crate) body: String,
}

/// A fake ntfy server serving on a local port until dropped.
pub(crate) struct FakeNtfy {
    url: String,
    published: Arc<Mutex<Vec<Published>>>,
    server: JoinHandle<()>,
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl FakeNtfy {
    /// Starts serving on a free local port.
    pub(crate) async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let published = Arc::default();
        let app = Router::new()
            .route("/{topic}", post(publish))
            .with_state(Arc::clone(&published));
        let server = tokio::spawn(async move {
            let _: Result<(), _> = axum::serve(listener, app).await;
        });
        Self {
            url,
            published,
            server,
        }
    }

    /// Where it serves, as the ntfy server setting names it.
    pub(crate) fn url(&self) -> &str {
        &self.url
    }

    /// Waits until `count` messages have been published, and returns them all, in order.
    pub(crate) async fn wait_for_messages(&self, count: usize) -> Vec<Published> {
        let started = Instant::now();
        loop {
            let published = lock(&self.published).clone();
            if published.len() >= count {
                return published;
            }
            assert!(
                started.elapsed() < WAIT,
                "ntfy got {published:?}, not {count} messages"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}

impl Drop for FakeNtfy {
    /// Stops serving: dropping a task's handle alone would leave it running.
    fn drop(&mut self) {
        self.server.abort();
    }
}

async fn publish(
    State(published): State<Arc<Mutex<Vec<Published>>>>,
    Path(topic): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> StatusCode {
    let header = |name: &str| {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
    };
    lock(&published).push(Published {
        topic,
        title: header("title"),
        click: header("click"),
        authorization: header("authorization"),
        body: String::from_utf8_lossy(&body).into_owned(),
    });
    StatusCode::OK
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
