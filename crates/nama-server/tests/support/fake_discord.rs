//! A local stand-in for Discord's webhooks, recording each message posted to one, so the Discord
//! Plugin's tests need no Discord.

use std::{
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

use crate::server::WAIT;

/// One message posted to a webhook of the fake Discord.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Posted {
    /// The path it was posted to, such as `/api/webhooks/1/ada`.
    pub(crate) path: String,
    /// Its body's `content`, the message's text.
    pub(crate) content: String,
}

#[derive(Default)]
struct Webhooks {
    posted: Mutex<Vec<Posted>>,
    gone: Vec<String>,
}

/// A fake Discord serving on a local port until dropped.
pub(crate) struct FakeDiscord {
    url: String,
    webhooks: Arc<Webhooks>,
    server: JoinHandle<()>,
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl FakeDiscord {
    /// Starts serving on a free local port, answering 404 ("Unknown Webhook") to a webhook whose
    /// token is one of `gone`.
    pub(crate) async fn start(gone: &[&str]) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let webhooks = Arc::new(Webhooks {
            gone: gone.iter().map(|token| (*token).to_owned()).collect(),
            ..Webhooks::default()
        });
        let app = Router::new()
            .route("/api/webhooks/{id}/{token}", post(execute))
            .with_state(Arc::clone(&webhooks));
        let server = tokio::spawn(async move {
            let _: Result<(), _> = axum::serve(listener, app).await;
        });
        Self {
            url,
            webhooks,
            server,
        }
    }

    /// Where it serves, as the Discord URL setting names it.
    pub(crate) fn url(&self) -> &str {
        &self.url
    }

    /// Waits until `count` messages have been posted, and returns them all, in order.
    pub(crate) async fn wait_for_messages(&self, count: usize) -> Vec<Posted> {
        let started = Instant::now();
        loop {
            let posted = lock(&self.webhooks.posted).clone();
            if posted.len() >= count {
                return posted;
            }
            assert!(
                started.elapsed() < WAIT,
                "Discord got {posted:?}, not {count} messages"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}

impl Drop for FakeDiscord {
    /// Stops serving: dropping a task's handle alone would leave it running.
    fn drop(&mut self) {
        self.server.abort();
    }
}

async fn execute(
    State(webhooks): State<Arc<Webhooks>>,
    Path((id, token)): Path<(String, String)>,
    body: Bytes,
) -> Response {
    let body: Value = serde_json::from_slice(&body).unwrap_or_default();
    lock(&webhooks.posted).push(Posted {
        path: format!("/api/webhooks/{id}/{token}"),
        content: body
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
    });
    if webhooks.gone.contains(&token) {
        let unknown = r#"{"message": "Unknown Webhook", "code": 10015}"#;
        let json = [(header::CONTENT_TYPE, "application/json")];
        (StatusCode::NOT_FOUND, json, unknown).into_response()
    } else {
        StatusCode::NO_CONTENT.into_response()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
