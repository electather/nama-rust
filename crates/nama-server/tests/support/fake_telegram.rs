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
    bot: Arc<Bot>,
    server: JoinHandle<()>,
}

/// What the fake bot has been sent and told.
#[derive(Default)]
struct Bot {
    /// The body of each message sent with the right token, in order, delivered or not.
    sent: Mutex<Vec<Value>>,
    /// Each press of Start on a deep link: its code, and the chat pressing it.
    started: Mutex<Vec<(String, i64)>>,
    /// The chats whose User blocked the bot.
    blocked: Mutex<Vec<i64>>,
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
        let bot = Arc::default();
        let app = Router::new()
            .route("/{bot}/{method}", post(answer))
            .with_state(Arc::clone(&bot));
        let server = tokio::spawn(async move {
            let _: Result<(), _> = axum::serve(listener, app).await;
        });
        Self { url, bot, server }
    }

    /// Someone in the chat `chat_id` opens the bot's deep link with `code` and presses Start.
    pub(crate) fn press_start(&self, code: &str, chat_id: i64) {
        lock(&self.bot.started).push((code.to_owned(), chat_id));
    }

    /// The User in the chat `chat_id` blocks the bot, so messages to it are refused.
    pub(crate) fn block(&self, chat_id: i64) {
        lock(&self.bot.blocked).push(chat_id);
    }

    /// Where it serves, as the Bot API URL setting names it.
    pub(crate) fn url(&self) -> &str {
        &self.url
    }

    /// Stops answering, so connecting to it is refused, as for a Telegram that can't be reached.
    pub(crate) fn go_down(&self) {
        self.server.abort();
    }

    /// The body of each message sent with the right token so far, in order, delivered or not.
    pub(crate) fn sent(&self) -> Vec<Value> {
        lock(&self.bot.sent).clone()
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

async fn answer(
    State(bot): State<Arc<Bot>>,
    Path((token, method)): Path<(String, String)>,
    body: Bytes,
) -> Response {
    let token = token.strip_prefix("bot").unwrap_or_default();
    if token
        .split_once(':')
        .is_none_or(|(id, _)| id.parse::<u64>().is_err())
    {
        return fixture(StatusCode::NOT_FOUND, "not-found.json");
    }
    if token != BOT_TOKEN {
        return fixture(StatusCode::UNAUTHORIZED, "unauthorized.json");
    }
    match method.as_str() {
        "sendMessage" => {
            let message: Value = serde_json::from_slice(&body).unwrap_or_default();
            let chat_id = message.get("chat_id").and_then(Value::as_str);
            let blocked = lock(&bot.blocked)
                .iter()
                .any(|blocked| chat_id == Some(&blocked.to_string()));
            lock(&bot.sent).push(message);
            if blocked {
                fixture(StatusCode::FORBIDDEN, "forbidden.json")
            } else {
                fixture(StatusCode::OK, "sendMessage.json")
            }
        }
        "getMe" => fixture(StatusCode::OK, "getMe.json"),
        "getUpdates" => updates(&bot),
        _ => fixture(StatusCode::NOT_FOUND, "not-found.json"),
    }
}

/// `getUpdates.json`'s first update, then its second once per press of Start, with that press's
/// code and chat.
#[expect(
    clippy::indexing_slicing,
    reason = "a test helper fails its test by panicking"
)]
fn updates(bot: &Bot) -> Response {
    let mut answer: Value =
        serde_json::from_slice(&recorded("getUpdates.json")).unwrap_or_default();
    let start = answer["result"][1].take();
    let mut updates = vec![answer["result"][0].take()];
    for (code, chat_id) in lock(&bot.started).iter() {
        let mut update = start.clone();
        update["message"]["text"] = format!("/start {code}").into();
        update["message"]["chat"]["id"] = (*chat_id).into();
        updates.push(update);
    }
    answer["result"] = updates.into();
    json(StatusCode::OK, answer.to_string().into_bytes())
}

/// The recorded response `file`, with `status`.
fn fixture(status: StatusCode, file: &str) -> Response {
    json(status, recorded(file))
}

fn json(status: StatusCode, body: Vec<u8>) -> Response {
    (status, [(header::CONTENT_TYPE, "application/json")], body).into_response()
}

/// The contents of the recorded response `file`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn recorded(file: &str) -> Vec<u8> {
    let fixtures =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins/telegram/fixtures");
    std::fs::read(fixtures.join(file)).unwrap()
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
