//! A local stand-in for the Telegram Bot API serving the responses recorded in
//! `plugins/telegram/fixtures/`, so the Telegram Plugin's tests need no Telegram. See that
//! folder's `README.md` for which answers what.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, Instant},
};

use axum::{
    Router,
    body::Bytes,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use serde_json::Value;
use tokio::{net::TcpListener, task::JoinHandle};

/// The bot token the fake Telegram accepts.
pub(crate) const BOT_TOKEN: &str = "123456:test-telegram-token";

/// How long [`FakeTelegram::wait_for_messages`] waits.
const WAIT: Duration = Duration::from_secs(10);

/// A fake Telegram Bot API serving on a local port until dropped.
pub(crate) struct FakeTelegram {
    url: String,
    sent: Arc<Mutex<Vec<Value>>>,
    server: JoinHandle<()>,
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl FakeTelegram {
    /// Starts serving on a free local port.
    pub(crate) async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let sent = Arc::default();
        let app = Router::new()
            .route("/{bot}/sendMessage", post(send_message))
            .with_state(Arc::clone(&sent));
        let server = tokio::spawn(async move {
            let _: Result<(), _> = axum::serve(listener, app).await;
        });
        Self { url, sent, server }
    }

    /// Where it serves, as the Bot API URL setting names it.
    pub(crate) fn url(&self) -> &str {
        &self.url
    }

    /// Stops answering, so connecting to it is refused, as for a Telegram that can't be reached.
    pub(crate) fn go_down(&self) {
        self.server.abort();
    }

    /// The body of each message sent with the right token so far, in order.
    pub(crate) fn sent(&self) -> Vec<Value> {
        self.sent
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Waits until `count` messages have been sent, and returns them all.
    pub(crate) async fn wait_for_messages(&self, count: usize) -> Vec<Value> {
        let started = Instant::now();
        loop {
            let sent = self.sent();
            if sent.len() >= count {
                return sent;
            }
            assert!(
                started.elapsed() < WAIT,
                "Telegram got {sent:?}, not {count} messages"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}

impl Drop for FakeTelegram {
    /// Stops serving: dropping a task's handle alone would leave it running.
    fn drop(&mut self) {
        self.go_down();
    }
}

async fn send_message(
    State(sent): State<Arc<Mutex<Vec<Value>>>>,
    Path(bot): Path<String>,
    body: Bytes,
) -> Response {
    if bot.strip_prefix("bot") != Some(BOT_TOKEN) {
        return fixture(StatusCode::UNAUTHORIZED, "unauthorized.json");
    }
    let message = serde_json::from_slice(&body).unwrap_or_default();
    sent.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(message);
    fixture(StatusCode::OK, "sendMessage.json")
}

/// The recorded response `file`, with `status`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn fixture(status: StatusCode, file: &str) -> Response {
    let fixtures =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins/telegram/fixtures");
    let body = std::fs::read(fixtures.join(file)).unwrap();
    (status, [(header::CONTENT_TYPE, "application/json")], body).into_response()
}
