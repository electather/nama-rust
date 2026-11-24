use axum::{
    Router,
    extract::RawQuery,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use serde_json::{Value, json};
use tokio::{net::TcpListener, sync::mpsc};

use super::{Cloudflare, DnsError, Record, Records};

/// What the stand-in received: the list's query and headers, or a batch's body.
#[derive(Debug)]
enum Received {
    Listed { query: String, headers: HeaderMap },
    Batch(Value),
}

/// A Cloudflare that writes to a local stand-in, which lists `listed` as the zone's records and
/// answers a batch with `status`, and what the stand-in received.
async fn cloudflare_listing(
    listed: Value,
    status: StatusCode,
) -> (Cloudflare, mpsc::UnboundedReceiver<Received>) {
    let (received, receiving) = mpsc::unbounded_channel();
    let batched = received.clone();
    let app = Router::new()
        .route(
            "/zones/zone-1/dns_records",
            get(move |RawQuery(query): RawQuery, headers: HeaderMap| {
                let _ignored = received.send(Received::Listed {
                    query: query.unwrap_or_default(),
                    headers,
                });
                let listing = json!({ "success": true, "result": listed });
                async move { listing.to_string() }
            }),
        )
        .route(
            "/zones/zone-1/dns_records/batch",
            post(move |body: String| {
                let _ignored = batched.send(Received::Batch(serde_json::from_str(&body).unwrap()));
                async move { (status, "{\"errors\":[{\"message\":\"nope\"}]}") }
            }),
        );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await });
    let mut cloudflare = Cloudflare::new("cf_token".to_owned(), "zone-1").unwrap();
    cloudflare.url = format!("http://{addr}/zones/zone-1/dns_records");
    (cloudflare, receiving)
}

fn record(name: &str, address: &str) -> Record {
    Record {
        name: name.to_owned(),
        address: address.parse().unwrap(),
    }
}

fn listed(id: &str, kind: &str, name: &str, content: &str) -> Value {
    json!({ "id": id, "type": kind, "name": name, "content": content })
}

#[tokio::test]
async fn cloudflare_adds_the_missing_address_records_under_the_name_and_deletes_the_stale_ones() {
    let (cloudflare, mut received) = cloudflare_listing(
        json!([
            listed("old-own", "A", "abc.namadirect.net", "203.0.113.7"),
            listed(
                "kept",
                "A",
                "192-168-1-10.abc.namadirect.net",
                "192.168.1.10"
            ),
            listed("gone", "A", "10-0-0-5.abc.namadirect.net", "10.0.0.5"),
            listed(
                "challenge",
                "TXT",
                "_acme-challenge.abc.namadirect.net",
                "token"
            ),
            listed("neighbour", "A", "1-2.xabc.namadirect.net", "10.0.0.9"),
        ]),
        StatusCode::OK,
    )
    .await;

    cloudflare
        .replace(
            "abc.namadirect.net",
            vec![
                record("abc.namadirect.net", "2001:db8::7"),
                record("192-168-1-10.abc.namadirect.net", "192.168.1.10"),
            ],
        )
        .await
        .unwrap();

    let Some(Received::Listed { query, headers }) = received.recv().await else {
        panic!("the zone's records weren't listed first");
    };
    assert!(
        query.contains("name.endswith=abc.namadirect.net"),
        "{query}"
    );
    assert_eq!(headers["authorization"], "Bearer cf_token");
    let Some(Received::Batch(batch)) = received.recv().await else {
        panic!("no batch was written");
    };
    assert_eq!(
        batch,
        json!({
            "deletes": [{ "id": "old-own" }, { "id": "gone" }],
            "posts": [{
                "type": "AAAA",
                "name": "abc.namadirect.net",
                "content": "2001:db8::7",
                "ttl": 60,
                "proxied": false,
            }],
        })
    );
}

#[tokio::test]
async fn cloudflare_writes_nothing_when_the_records_are_as_wanted() {
    let (cloudflare, mut received) = cloudflare_listing(
        json!([listed("own", "A", "abc.namadirect.net", "203.0.113.7")]),
        StatusCode::OK,
    )
    .await;

    cloudflare
        .replace(
            "abc.namadirect.net",
            vec![record("abc.namadirect.net", "203.0.113.7")],
        )
        .await
        .unwrap();

    assert!(matches!(
        received.recv().await,
        Some(Received::Listed { .. })
    ));
    assert!(received.try_recv().is_err(), "a batch was written");
}

#[tokio::test]
async fn records_cloudflare_refuses_fail_with_what_it_said() {
    let (cloudflare, _received) = cloudflare_listing(json!([]), StatusCode::BAD_REQUEST).await;

    let err = cloudflare
        .replace(
            "abc.namadirect.net",
            vec![record("abc.namadirect.net", "203.0.113.7")],
        )
        .await
        .unwrap_err();

    assert!(
        matches!(&err, DnsError::Refused { status, body }
            if *status == StatusCode::BAD_REQUEST && body.contains("nope")),
        "{err:?}"
    );
}

#[tokio::test]
async fn cloudflare_replaces_the_txt_records_at_exactly_the_name() {
    let (cloudflare, mut received) = cloudflare_listing(
        json!([listed(
            "old",
            "TXT",
            "_acme-challenge.abc.namadirect.net",
            "\"stale\""
        )]),
        StatusCode::OK,
    )
    .await;

    cloudflare
        .replace_texts(
            "_acme-challenge.abc.namadirect.net",
            vec!["new-value".to_owned()],
        )
        .await
        .unwrap();

    let Some(Received::Listed { query, .. }) = received.recv().await else {
        panic!("the name's records weren't listed first");
    };
    assert!(
        query.contains("type=TXT&name.exact=_acme-challenge.abc.namadirect.net"),
        "{query}"
    );
    let Some(Received::Batch(batch)) = received.recv().await else {
        panic!("no batch was written");
    };
    assert_eq!(
        batch,
        json!({
            "deletes": [{ "id": "old" }],
            "posts": [{
                "type": "TXT",
                "name": "_acme-challenge.abc.namadirect.net",
                "content": "new-value",
                "ttl": 60,
                "proxied": false,
            }],
        })
    );
}
