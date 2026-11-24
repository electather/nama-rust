use axum::http::StatusCode;
use url::Url;

use super::{
    testing::{assert_valid_vapid, browser, decoded, new_key, push_service},
    *,
};

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
    // Only what's wrong, as a subscription holds secrets and shows none.
    let new = |endpoint: &str, p256dh: &[u8], auth: &[u8]| {
        Subscription::new(endpoint.to_owned(), p256dh.to_vec(), auth.to_vec()).err()
    };

    assert_eq!(new(&valid.endpoint, &valid.p256dh, &valid.auth), None);
    assert_eq!(
        new("http://push.example.net/", &valid.p256dh, &valid.auth),
        Some(Invalid::Endpoint)
    );
    assert_eq!(
        new("not a url", &valid.p256dh, &valid.auth),
        Some(Invalid::Endpoint)
    );
    assert_eq!(
        new(&valid.endpoint, &valid.p256dh[1..], &valid.auth),
        Some(Invalid::P256dh)
    );
    assert_eq!(
        new(&valid.endpoint, &[7; 65], &valid.auth),
        Some(Invalid::P256dh)
    );
    assert_eq!(
        new(&valid.endpoint, &valid.p256dh, &[1; 15]),
        Some(Invalid::Auth)
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
