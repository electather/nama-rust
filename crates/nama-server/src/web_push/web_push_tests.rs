use std::sync::{Arc, Mutex};

use axum::{Router, http::HeaderMap, http::StatusCode, routing::post};
use ring::signature::{ECDSA_P256_SHA256_FIXED, UnparsedPublicKey};

use super::*;

fn decoded(base64: &str) -> Vec<u8> {
    URL_SAFE_NO_PAD.decode(base64).unwrap()
}

#[test]
fn seal_gives_rfc_8291s_example_message() {
    let shared = decoded("kyrL1jIIOHEzg3sM2ZWRHDRB62YACZhhSlknJ672kSs");
    let our_public = decoded(
        "BP4z9KsN6nGRTbVYI_c7VJSPQTBtkgcy27mlmlMoZIIgDll6e3vCYLocInmYWAmS6TlzAC8wEqKK6PBru3jl7A8",
    );
    let salt = decoded("DGv6ra1nlYgDCS1FRnbzlw").try_into().unwrap();

    let sealed = seal(
        &shared,
        &browser("https://push.example.net/"),
        &our_public,
        salt,
        b"When I grow up, I want to be a watermelon",
    )
    .unwrap();

    assert_eq!(
        URL_SAFE_NO_PAD.encode(sealed),
        "DGv6ra1nlYgDCS1FRnbzlwAAEABBBP4z9KsN6nGRTbVYI_c7VJSPQTBtkgcy27mlmlMoZIIgDll6e3vCYLocInmYWAmS\
         6TlzAC8wEqKK6PBru3jl7A_yl95bQpu6cVPTpK4Mqgkf1CXztLVBSt2Ks3oZwbuwXPXLWyouBWLVWGNWQexSgSxsj_\
         Qulcy4a-fN"
    );
}

#[test]
fn a_subscription_needs_an_https_endpoint_and_a_browsers_keys() {
    let valid = browser("https://push.example.net/send/abc");
    let new = |endpoint: &str, p256dh: &[u8], auth: &[u8]| {
        Subscription::new(endpoint.to_owned(), p256dh.to_vec(), auth.to_vec())
    };

    assert_eq!(
        new(&valid.endpoint, &valid.p256dh, &valid.auth),
        Ok(valid.clone())
    );
    assert_eq!(
        new("http://push.example.net/", &valid.p256dh, &valid.auth),
        Err(Invalid::Endpoint)
    );
    assert_eq!(
        new("not a url", &valid.p256dh, &valid.auth),
        Err(Invalid::Endpoint)
    );
    assert_eq!(
        new(&valid.endpoint, &valid.p256dh[1..], &valid.auth),
        Err(Invalid::P256dh)
    );
    assert_eq!(
        new(&valid.endpoint, &[7; 65], &valid.auth),
        Err(Invalid::P256dh)
    );
    assert_eq!(
        new(&valid.endpoint, &valid.p256dh, &[1; 15]),
        Err(Invalid::Auth)
    );
}

#[test]
fn a_key_reads_back_from_its_pkcs8_with_the_same_public_key() {
    let pkcs8 = VapidKey::new_pkcs8().unwrap();

    let key = VapidKey::from_pkcs8(&pkcs8).unwrap();

    assert_eq!(key.public_key().len(), 65);
    assert_eq!(
        VapidKey::from_pkcs8(&pkcs8).unwrap().public_key(),
        key.public_key()
    );
    assert!(VapidKey::from_pkcs8(b"not a key").is_err());
}

/// What a stub push service was sent: each request's headers and body.
pub(crate) type Received = Arc<Mutex<Vec<(HeaderMap, Vec<u8>)>>>;

/// A push service at the URL returned that answers `status` and keeps what it's sent.
pub(crate) async fn push_service(status: StatusCode) -> (String, Received) {
    let received = Received::default();
    let keeping = Arc::clone(&received);
    let app = Router::new().route(
        "/push/{id}",
        post(
            move |headers: HeaderMap, body: axum::body::Bytes| async move {
                keeping.lock().unwrap().push((headers, body.to_vec()));
                status
            },
        ),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/push/1", listener.local_addr().unwrap());
    tokio::spawn(axum::serve(listener, app).into_future());
    (url, received)
}

/// A push service at the URL returned that takes each connection and never answers.
pub(crate) async fn silent_push_service() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/push/silent", listener.local_addr().unwrap());
    tokio::spawn(async move {
        let mut held = Vec::new();
        loop {
            held.push(listener.accept().await.unwrap());
        }
    });
    url
}

/// A new key.
pub(crate) fn new_key() -> VapidKey {
    VapidKey::from_pkcs8(&VapidKey::new_pkcs8().unwrap()).unwrap()
}

/// A subscription at `endpoint` with RFC 8291's example browser keys.
pub(crate) fn browser(endpoint: &str) -> Subscription {
    Subscription {
        endpoint: endpoint.to_owned(),
        p256dh: decoded(
            "BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcxaOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4",
        ),
        auth: decoded("BTBZMqHH6r4Tts7J_aSIgg"),
    }
}

#[tokio::test]
async fn a_push_is_encrypted_and_signed_for_the_push_services_origin() {
    let key = new_key();
    let public_key = key.public_key().to_vec();
    let (endpoint, received) = push_service(StatusCode::CREATED).await;
    let pusher = Pusher::new(key).unwrap();

    let answer = pusher.push(&browser(&endpoint), b"hello").await.unwrap();

    assert_eq!(answer, Pushed::Delivered);
    let (headers, body) = received.lock().unwrap().pop().unwrap();
    assert_eq!(headers["content-encoding"], "aes128gcm");
    assert_eq!(headers["ttl"], KEPT_FOR_SECONDS.to_string().as_str());
    // Salt, record size, key id length, the key, and "hello", its delimiter and the tag.
    assert_eq!(body.len(), 16 + 4 + 1 + 65 + 5 + 1 + 16);
    let origin = Url::parse(&endpoint)
        .unwrap()
        .origin()
        .ascii_serialization();
    assert_valid_vapid(
        headers["authorization"].to_str().unwrap(),
        &origin,
        &public_key,
    );
}

#[tokio::test]
async fn a_push_service_answering_404_or_410_says_the_subscription_is_gone() {
    for status in [StatusCode::NOT_FOUND, StatusCode::GONE] {
        let (endpoint, _) = push_service(status).await;

        let pushed = Pusher::new(new_key())
            .unwrap()
            .push(&browser(&endpoint), b"hi")
            .await;

        assert_eq!(pushed.unwrap(), Pushed::Gone);
    }
    let (endpoint, _) = push_service(StatusCode::TOO_MANY_REQUESTS).await;
    let pushed = Pusher::new(new_key())
        .unwrap()
        .push(&browser(&endpoint), b"hi")
        .await;
    assert!(matches!(
        pushed,
        Err(PushError::Refused(StatusCode::TOO_MANY_REQUESTS))
    ));
}

/// Asserts `authorization` is RFC 8292's, signed by `public_key` for `origin` and not expired.
pub(crate) fn assert_valid_vapid(authorization: &str, origin: &str, public_key: &[u8]) {
    let (token, k) = (authorization.strip_prefix("vapid t="))
        .and_then(|rest| rest.split_once(", k="))
        .unwrap();
    assert_eq!(decoded(k), public_key);
    let (signed, signature) = token.rsplit_once('.').unwrap();
    UnparsedPublicKey::new(&ECDSA_P256_SHA256_FIXED, public_key)
        .verify(signed.as_bytes(), &decoded(signature))
        .unwrap();
    let (header, claims) = signed.split_once('.').unwrap();
    let header: serde_json::Value = serde_json::from_slice(&decoded(header)).unwrap();
    assert_eq!(header["alg"], "ES256");
    let claims: serde_json::Value = serde_json::from_slice(&decoded(claims)).unwrap();
    assert_eq!(claims["aud"], origin);
    let expires = claims["exp"].as_i64().unwrap();
    assert!(expires > now() && expires <= now() + 24 * 60 * 60);
    assert!(claims["sub"].as_str().unwrap().starts_with("https://"));
}
