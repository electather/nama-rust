use axum::{
    Router,
    http::{HeaderMap, StatusCode},
    routing::post,
};
use serde_json::{Value, json};
use tokio::{net::TcpListener, sync::mpsc};

use super::{Email, MailError, Mailer, Resend};

/// A Resend that sends to a local stand-in answering `status`, and what the stand-in received.
async fn resend_answering(
    status: StatusCode,
) -> (Resend, mpsc::UnboundedReceiver<(HeaderMap, Value)>) {
    let (received, receiving) = mpsc::unbounded_channel();
    let app = Router::new().route(
        "/emails",
        post(move |headers: HeaderMap, body: String| {
            let received = received.clone();
            async move {
                let _ignored = received.send((headers, serde_json::from_str(&body).unwrap()));
                (status, "{\"message\":\"nope\"}")
            }
        }),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await });
    let mut resend =
        Resend::new("re_key".to_owned(), "Nama Hub <hub@example.com>".to_owned()).unwrap();
    resend.url = format!("http://{addr}/emails");
    (resend, receiving)
}

fn email() -> Email {
    Email {
        to: "ada@example.com".to_owned(),
        subject: "Hello".to_owned(),
        text: "Plain text".to_owned(),
    }
}

#[tokio::test]
async fn resend_gets_the_email_as_plain_text_with_the_api_key() {
    let (resend, mut received) = resend_answering(StatusCode::OK).await;

    resend.send(email()).await.unwrap();

    let (headers, body) = received.recv().await.unwrap();
    assert_eq!(headers["authorization"], "Bearer re_key");
    assert_eq!(
        body,
        json!({
            "from": "Nama Hub <hub@example.com>",
            "to": ["ada@example.com"],
            "subject": "Hello",
            "text": "Plain text",
        })
    );
}

#[tokio::test]
async fn an_email_resend_refuses_fails_with_what_it_said() {
    let (resend, _received) = resend_answering(StatusCode::UNPROCESSABLE_ENTITY).await;

    let err = resend.send(email()).await.unwrap_err();

    assert!(
        matches!(&err, MailError::Refused { status, body }
            if *status == StatusCode::UNPROCESSABLE_ENTITY && body.contains("nope")),
        "{err:?}"
    );
}
