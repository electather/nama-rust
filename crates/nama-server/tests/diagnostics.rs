//! The spans a ConnectRPC call is traced in, as an in-memory exporter receives them through the
//! Server's own Diagnostics layer (ADR 0054).

#[path = "support/server.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod server;

use std::{
    sync::OnceLock,
    time::{Duration, Instant},
};

use connectrpc::{ConnectError, client::HttpClient};
use nama_proto::nama::v1::{SessionDelivery, SignInRequest, SignInResponse, SignInServiceClient};
use opentelemetry::trace::{SpanId, Status, TraceId};
use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider, SpanData};
use server::{PASSWORD, TestServer, WAIT};
use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _};

/// The span id the calls' `traceparent` names as their parent.
const PARENT: &str = "00f067aa0ba902b7";

/// Where every span this binary's Servers record is exported, through one global subscriber: a
/// thread-local one wouldn't see the spans the runtime's other threads end.
fn exporter() -> &'static InMemorySpanExporter {
    static EXPORTING: OnceLock<(SdkTracerProvider, InMemorySpanExporter)> = OnceLock::new();
    let (_, exporter) = EXPORTING.get_or_init(|| {
        let exporter = InMemorySpanExporter::default();
        let provider = SdkTracerProvider::builder()
            .with_simple_exporter(exporter.clone())
            .build();
        tracing_subscriber::registry()
            .with(nama_server::diagnostics::layer(&provider))
            .init();
        (provider, exporter)
    });
    exporter
}

/// Signs the Admin in with `password`, continuing the trace `trace` at [`PARENT`].
async fn sign_in(
    server: &TestServer,
    trace: &str,
    password: &str,
) -> Result<SignInResponse, ConnectError> {
    let traced = server
        .anonymous_client_config()
        .with_default_header("traceparent", format!("00-{trace}-{PARENT}-01"));
    SignInServiceClient::new(HttpClient::plaintext(), traced)
        .sign_in(SignInRequest {
            name: "Admin".to_owned(),
            password: password.to_owned(),
            device_name: "Phone".to_owned(),
            delivery: SessionDelivery::SESSION_DELIVERY_TOKEN.into(),
            ..Default::default()
        })
        .await
        .map(connectrpc::UnaryResponse::into_owned)
}

/// The spans of the trace `trace`, once its `SignIn` call's span has ended, and that span.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn sign_in_trace(trace: &str) -> (Vec<SpanData>, SpanData) {
    let trace = TraceId::from_hex(trace).unwrap();
    let started = Instant::now();
    loop {
        let spans: Vec<SpanData> = exporter()
            .get_finished_spans()
            .unwrap()
            .into_iter()
            .filter(|span| span.span_context.trace_id() == trace)
            .collect();
        if let Some(call) = spans
            .iter()
            .find(|span| span.name == "nama.v1.SignInService/SignIn")
        {
            let call = call.clone();
            return (spans, call);
        }
        assert!(started.elapsed() < WAIT, "no SignIn span in {spans:#?}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn a_call_is_one_trace_of_its_sql_and_plugin_calls_continuing_its_traceparent() {
    exporter();
    let server = TestServer::builder().start().await;
    let trace = "4bf92f3577b34da6a3ce929d0e0e4736";

    sign_in(&server, trace, PASSWORD).await.unwrap();

    let (spans, call) = sign_in_trace(trace).await;
    assert_eq!(call.parent_span_id, SpanId::from_hex(PARENT).unwrap());
    let ids: Vec<SpanId> = spans
        .iter()
        .map(|span| span.span_context.span_id())
        .collect();
    for span in spans.iter().filter(|span| span.name != call.name) {
        assert!(
            ids.contains(&span.parent_span_id),
            "{} is outside the call's tree",
            span.name
        );
    }
    let children: Vec<&str> = spans
        .iter()
        .filter(|span| span.parent_span_id == call.span_context.span_id())
        .map(|span| &*span.name)
        .collect();
    for sql in ["SELECT", "INSERT"] {
        assert!(
            children.iter().any(|name| name.starts_with(sql)),
            "no {sql} span in {children:?}"
        );
    }
    assert!(
        children.contains(&"plugin Authenticator"),
        "no Plugin call span in {children:?}"
    );
    for span in &spans {
        assert!(span.events.is_empty(), "{} carries logged lines", span.name);
    }
    let attributes = spans
        .iter()
        .flat_map(|span| span.attributes.iter().map(move |kv| (&span.name, kv)));
    for (span, attribute) in attributes {
        let value = attribute.value.as_str();
        for private in ["Admin", "Phone", PASSWORD, "127.0.0.1"] {
            assert!(
                !value.contains(private),
                "{span}'s {} holds {private}",
                attribute.key
            );
        }
    }
}

#[tokio::test]
async fn a_failed_call_marks_its_span_failed_with_its_connect_code() {
    exporter();
    let server = TestServer::builder().start().await;
    let trace = "0af7651916cd43dd8448eb211c80319c";

    let refused = sign_in(&server, trace, "not the password")
        .await
        .unwrap_err();

    let (_, call) = sign_in_trace(trace).await;
    assert_eq!(call.status, Status::error(""));
    let kind = call
        .attributes
        .iter()
        .find(|attribute| attribute.key.as_str() == "error.type")
        .map(|attribute| attribute.value.as_str().into_owned());
    assert_eq!(kind.as_deref(), Some(refused.code.as_str()));
}
