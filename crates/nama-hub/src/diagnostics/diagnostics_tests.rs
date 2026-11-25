use std::sync::Mutex;

use opentelemetry_proto::tonic::{
    collector::{logs::v1::ExportLogsServiceRequest, trace::v1::ExportTraceServiceRequest},
    logs::v1::ResourceLogs,
    trace::v1::ResourceSpans,
};

use super::*;

/// A backend keeping what it is sent.
#[derive(Default)]
struct Kept(Mutex<Vec<(Signal, Bytes)>>);

impl Backend for Kept {
    fn forward(&self, signal: Signal, batch: Bytes) -> Forwarding<'_> {
        self.0.lock().unwrap().push((signal, batch));
        Box::pin(async { Ok(()) })
    }
}

fn server(n: u128) -> LinkedServerId {
    LinkedServerId::from(uuid::Uuid::from_u128(n))
}

#[tokio::test]
async fn a_batch_reaches_the_backend_unchanged() {
    let backend = Arc::new(Kept::default());
    let shared = Shared::new(Some(Arc::<Kept>::clone(&backend)));

    shared
        .take(
            server(1),
            Signal::Logs,
            Bytes::from_static(b"batch"),
            Instant::now(),
        )
        .await
        .unwrap();

    assert_eq!(
        *backend.0.lock().unwrap(),
        [(Signal::Logs, Bytes::from_static(b"batch"))]
    );
}

#[tokio::test]
async fn a_server_over_its_bytes_a_minute_is_refused_until_the_minute_is_up_and_others_are_not() {
    let shared = Shared::new(None);
    let now = Instant::now();
    let batch = || Bytes::from(vec![0; BYTES_PER_WINDOW / 2]);
    for _ in 0..2 {
        shared
            .take(server(1), Signal::Traces, batch(), now)
            .await
            .unwrap();
    }

    let over = shared.take(server(1), Signal::Traces, batch(), now).await;
    let other = shared.take(server(2), Signal::Traces, batch(), now).await;
    let next_minute = (shared.take(server(1), Signal::Traces, batch(), now + WINDOW)).await;

    assert!(matches!(over, Err(NotTaken::TooMuch)), "{over:?}");
    assert!(other.is_ok(), "{other:?}");
    assert!(next_minute.is_ok(), "{next_minute:?}");
}

#[test]
fn headers_are_read_as_otel_exporter_otlp_headers_writes_them() {
    let parsed = parse_headers("authorization=Basic abc, x-scope = 1 ,").unwrap();

    assert_eq!(parsed["authorization"], "Basic abc");
    assert_eq!(parsed["x-scope"], "1");
    assert!(matches!(parse_headers("no-equals"), Err(OtlpError::Header)));
}

/// The `service.instance.id` values of each resource in `request`.
fn instance_ids(resources: impl IntoIterator<Item = Option<Resource>>) -> Vec<Vec<String>> {
    resources
        .into_iter()
        .map(|resource| {
            resource
                .unwrap_or_default()
                .attributes
                .into_iter()
                .filter(|attribute| attribute.key == INSTANCE_ID)
                .filter_map(|attribute| {
                    let Value::StringValue(id) = attribute.value?.value? else {
                        return None;
                    };
                    Some(id)
                })
                .collect()
        })
        .collect()
}

#[test]
fn every_resource_in_a_batch_names_the_server_in_place_of_what_it_claimed() {
    let claimed = KeyValue {
        key: INSTANCE_ID.to_owned(),
        value: Some(AnyValue {
            value: Some(Value::StringValue("someone else".to_owned())),
        }),
        ..Default::default()
    };
    let spans = ExportTraceServiceRequest {
        resource_spans: vec![
            ResourceSpans {
                resource: Some(Resource {
                    attributes: vec![claimed],
                    ..Default::default()
                }),
                ..Default::default()
            },
            ResourceSpans::default(),
        ],
    };
    let logs = ExportLogsServiceRequest {
        resource_logs: vec![ResourceLogs::default()],
    };

    let spans = naming(server(1), Signal::Traces, &spans.encode_to_vec()).unwrap();
    let logs = naming(server(1), Signal::Logs, &logs.encode_to_vec()).unwrap();

    let id = server(1).to_string();
    let spans = ExportTraceServiceRequest::decode(spans)
        .unwrap()
        .resource_spans;
    let logs = ExportLogsServiceRequest::decode(logs)
        .unwrap()
        .resource_logs;
    assert_eq!(
        instance_ids(spans.into_iter().map(|spans| spans.resource)),
        [[id.as_str()], [id.as_str()]]
    );
    assert_eq!(
        instance_ids(logs.into_iter().map(|logs| logs.resource)),
        [[id.as_str()]]
    );
}

#[test]
fn a_batch_that_isnt_an_export_request_isnt_named() {
    naming(server(1), Signal::Logs, b"\xff\xff").unwrap_err();
}
