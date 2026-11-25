//! What a Linked Server shares: its OTLP/HTTP batches reach the backend naming it, against a real
//! PostgreSQL.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/hub.rs"]
mod hub;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/linking.rs"]
mod linking;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/mail.rs"]
mod mail;

use axum::{body::Bytes, http::StatusCode};
use connectrpc::client::{ClientTransport as _, HttpClient, full_body};
use hub::TestHub;
use linking::{create_proven, hub_with_mail, linked};
use nama_hub::diagnostics::Signal;
use opentelemetry_proto::tonic::{
    collector::{logs::v1::ExportLogsServiceRequest, trace::v1::ExportTraceServiceRequest},
    trace::v1::ResourceSpans,
};
use prost::Message as _;
use sqlx::PgPool;

/// A traces export request of one resource, whose schema URL is `padding` bytes long.
fn spans(padding: usize) -> Vec<u8> {
    ExportTraceServiceRequest {
        resource_spans: vec![ResourceSpans {
            schema_url: "x".repeat(padding),
            ..Default::default()
        }],
    }
    .encode_to_vec()
}

/// Posts `batch` to the Hub's OTLP/HTTP `/otlp/v1/{signal}`, as the Server presenting
/// `credential`, if any, and answers the Hub's status.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn share(hub: &TestHub, signal: &str, credential: Option<&str>, batch: &[u8]) -> StatusCode {
    let mut request = axum::http::Request::post(format!(
        "{}otlp/v1/{signal}",
        hub.client_config().base_uri()
    ))
    .header("content-type", "application/x-protobuf");
    if let Some(credential) = credential {
        request = request.header("authorization", format!("Bearer {credential}"));
    }
    let request = request
        .body(full_body(Bytes::copy_from_slice(batch)))
        .unwrap();
    HttpClient::plaintext()
        .send(request)
        .await
        .unwrap()
        .status()
}

#[sqlx::test(migrations = false)]
async fn a_linked_servers_shared_batches_reach_the_backend_naming_it_and_no_one_elses_do(
    db: PgPool,
) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let credential = linked(&hub, &ada, "Living room").await;
    let lines = ExportLogsServiceRequest::default().encode_to_vec();

    let anyone = share(&hub, "traces", None, &spans(0)).await;
    let unknown = share(&hub, "traces", Some("no such credential"), &spans(0)).await;
    let garbled = share(&hub, "traces", Some(&credential), b"\xff\xff").await;
    let shared_spans = share(&hub, "traces", Some(&credential), &spans(0)).await;
    let shared_lines = share(&hub, "logs", Some(&credential), &lines).await;

    assert_eq!(
        [anyone, unknown, garbled, shared_spans, shared_lines],
        [
            StatusCode::UNAUTHORIZED,
            StatusCode::UNAUTHORIZED,
            StatusCode::BAD_REQUEST,
            StatusCode::OK,
            StatusCode::OK
        ]
    );
    let shared = hub.shared();
    let signals: Vec<_> = shared.iter().map(|(signal, _)| *signal).collect();
    assert_eq!(signals, [Signal::Traces, Signal::Logs]);
    let forwarded = ExportTraceServiceRequest::decode(shared[0].1.clone()).unwrap();
    let keys: Vec<_> = forwarded.resource_spans[0]
        .resource
        .iter()
        .flat_map(|resource| &resource.attributes)
        .map(|attribute| attribute.key.as_str())
        .collect();
    assert_eq!(keys, ["service.instance.id"]);
}

#[sqlx::test(migrations = false)]
async fn a_linked_server_sharing_over_4_mib_a_minute_is_refused(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let credential = linked(&hub, &ada, "Living room").await;
    let batch = spans(3 * 512 * 1024);

    let mut answers = Vec::new();
    for _ in 0..3 {
        answers.push(share(&hub, "traces", Some(&credential), &batch).await);
    }

    assert_eq!(
        answers,
        [
            StatusCode::OK,
            StatusCode::OK,
            StatusCode::TOO_MANY_REQUESTS
        ]
    );
    assert_eq!(hub.shared().len(), 2);
}
