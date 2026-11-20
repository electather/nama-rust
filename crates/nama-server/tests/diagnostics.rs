//! The spans a ConnectRPC call is traced in, as an in-memory exporter receives them through the
//! Server's own Diagnostics layer, and the web app's spans the Server forwards to its collector
//! (ADR 0054).

#[path = "support/http.rs"]
#[expect(dead_code, reason = "this binary uses only some of the helpers")]
mod http;
#[path = "support/server.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod server;

use std::{
    sync::{Arc, Mutex, OnceLock, PoisonError},
    time::{Duration, Instant},
};

use axum::{Router, body::Bytes, extract::State, http::HeaderMap, routing::post};
use connectrpc::{ConnectError, client::HttpClient};
use http::{Reply, send};
use http_body_util::Full;
use hyper::{Method, Request};
use nama_proto::nama::v1::{
    GetServerInfoRequest, ServerServiceClient, SessionDelivery, SignInRequest, SignInResponse,
    SignInServiceClient,
};
use nama_server::config::Config;
use opentelemetry::trace::{SpanId, Status, TraceId};
use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider, SpanData};
use server::{PASSWORD, TestServer, WAIT};
use tokio::{net::TcpListener, task::JoinHandle};
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
    assert!(
        children.contains(&"write queued"),
        "no span for the write's wait in {children:?}"
    );
    // The span naming a Built-in on stdout would repeat the Plugin call span.
    assert!(spans.iter().all(|span| span.name != "plugin"), "{spans:#?}");
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

/// Where the web app posts its spans.
const TRACES: &str = "/diagnostics/v1/traces";

/// What the web app's OTLP exporter sends: OTLP/HTTP's JSON encoding.
const OTLP_JSON: &str = "application/json";

/// Each body a [`FakeCollector`] received, with its content type.
type Received = Arc<Mutex<Vec<(String, Bytes)>>>;

/// A stand-in for an OpenTelemetry Collector, recording each body posted to its traces path,
/// until dropped.
struct FakeCollector {
    endpoint: String,
    received: Received,
    server: JoinHandle<()>,
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl FakeCollector {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let received = Received::default();
        let app = Router::new()
            .route("/v1/traces", post(record))
            .with_state(Arc::clone(&received));
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self {
            endpoint,
            received,
            server,
        }
    }

    fn received(&self) -> Vec<(String, Bytes)> {
        self.received
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Drop for FakeCollector {
    fn drop(&mut self) {
        self.server.abort();
    }
}

async fn record(State(received): State<Received>, headers: HeaderMap, body: Bytes) {
    let content_type = headers
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    received
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push((content_type, body));
}

/// A Server exporting its Diagnostics to `collector`.
async fn exporting_to(collector: &FakeCollector) -> TestServer {
    TestServer::builder()
        .config(Config {
            otlp_endpoint: Some(collector.endpoint.clone()),
            ..Config::default()
        })
        .start()
        .await
}

/// Posts `body` to the web app's traces path as OTLP JSON, as the Device holding `token`, if any.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn post_traces(server: &TestServer, token: Option<&str>, body: Vec<u8>) -> Reply {
    let request = Request::builder()
        .method(Method::POST)
        .uri(TRACES)
        .header("content-type", OTLP_JSON)
        .body(Full::new(Bytes::from(body)))
        .unwrap();
    send(server, request, token).await
}

#[expect(clippy::unwrap_used, reason = "a test fails by panicking")]
async fn exports_diagnostics(server: &TestServer) -> bool {
    ServerServiceClient::new(HttpClient::plaintext(), server.anonymous_client_config())
        .get_server_info(GetServerInfoRequest::default())
        .await
        .unwrap()
        .into_owned()
        .exports_diagnostics
}

#[tokio::test]
async fn without_an_endpoint_the_server_says_so_and_the_traces_path_is_not_found() {
    let server = TestServer::builder().start().await;

    assert!(!exports_diagnostics(&server).await);
    let reply = post_traces(&server, Some(&server.admin_token), b"{}".to_vec()).await;
    assert_eq!(reply.status, 404);
}

#[tokio::test]
async fn a_signed_in_device_s_spans_reach_the_collector_unchanged() {
    let collector = FakeCollector::start().await;
    let server = exporting_to(&collector).await;
    let spans = br#"{"resourceSpans":[]}"#.to_vec();

    assert!(exports_diagnostics(&server).await);
    let reply = post_traces(&server, Some(&server.admin_token), spans.clone()).await;

    assert_eq!(reply.status, 200);
    assert_eq!(
        collector.received(),
        [(OTLP_JSON.to_owned(), Bytes::from(spans))]
    );
}

#[tokio::test]
async fn the_traces_path_refuses_a_caller_not_signed_in_and_a_body_over_256_kib() {
    let collector = FakeCollector::start().await;
    let server = exporting_to(&collector).await;

    let anonymous = post_traces(&server, None, b"{}".to_vec()).await;
    let too_big = post_traces(
        &server,
        Some(&server.admin_token),
        vec![b' '; 256 * 1024 + 1],
    )
    .await;
    let just_fits = post_traces(&server, Some(&server.admin_token), vec![b' '; 256 * 1024]).await;

    assert_eq!(anonymous.status, 401);
    assert_eq!(too_big.status, 413);
    assert_eq!(just_fits.status, 200);
    assert_eq!(collector.received().len(), 1);
}

#[tokio::test]
async fn a_device_posting_more_than_60_times_a_minute_is_refused_and_others_are_not() {
    let collector = FakeCollector::start().await;
    let server = exporting_to(&collector).await;
    let other = server.user("Sam", &[]).await;

    for _ in 0..60 {
        let reply = post_traces(&server, Some(&server.admin_token), b"{}".to_vec()).await;
        assert_eq!(reply.status, 200);
    }
    let refused = post_traces(&server, Some(&server.admin_token), b"{}".to_vec()).await;
    let others = post_traces(&server, Some(&other.token), b"{}".to_vec()).await;

    assert_eq!(refused.status, 429);
    assert_eq!(others.status, 200);
    assert_eq!(collector.received().len(), 61);
}
