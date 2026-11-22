use std::sync::Mutex;

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
