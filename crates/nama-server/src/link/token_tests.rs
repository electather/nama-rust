use ring::signature::{Ed25519KeyPair, KeyPair as _};
use serde_json::{Value, json};

use super::*;

fn key() -> Ed25519KeyPair {
    Ed25519KeyPair::from_seed_unchecked(&[3; 32]).unwrap()
}

fn key_set(kid: &str) -> Vec<u8> {
    json!({ "keys": [{
        "kty": "OKP",
        "crv": "Ed25519",
        "x": URL_SAFE_NO_PAD.encode(key().public_key()),
        "kid": kid,
    }]})
    .to_string()
    .into_bytes()
}

fn token(header: &Value, claims: &Value) -> String {
    let signed = format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(header.to_string()),
        URL_SAFE_NO_PAD.encode(claims.to_string()),
    );
    format!(
        "{signed}.{}",
        URL_SAFE_NO_PAD.encode(key().sign(signed.as_bytes()))
    )
}

fn signed(claims: &Value) -> String {
    token(&json!({ "alg": "EdDSA", "kid": "k1" }), claims)
}

#[test]
fn a_token_signed_by_a_key_in_the_set_passes_until_it_expires() {
    let claims = json!({ "email": "ada@example.com", "exp": 1_000 });

    let passed = verify(&key_set("k1"), &signed(&claims), 1_000).unwrap();

    assert_eq!(passed.email, "ada@example.com");
    assert_eq!(
        verify(&key_set("k1"), &signed(&claims), 1_061).unwrap_err(),
        Rejected::Invalid("it expired")
    );
}

#[test]
fn a_token_naming_a_key_the_set_lacks_asks_for_the_set_again() {
    let claims = json!({ "email": "ada@example.com", "exp": 1_000 });

    let rejected = verify(&key_set("k2"), &signed(&claims), 0).unwrap_err();

    assert_eq!(rejected, Rejected::UnknownKey);
}

#[test]
fn a_changed_token_or_another_algorithm_fails() {
    let claims = json!({ "email": "ada@example.com", "exp": 1_000 });
    let good = signed(&claims);
    let (signed_part, signature) = good.rsplit_once('.').unwrap();
    let (header, _) = signed_part.split_once('.').unwrap();
    let other_claims =
        URL_SAFE_NO_PAD.encode(json!({ "email": "eve@example.com", "exp": 1_000 }).to_string());
    let changed = format!("{header}.{other_claims}.{signature}");
    let unsigned = format!(
        "{}.{}.",
        URL_SAFE_NO_PAD.encode(json!({ "alg": "none", "kid": "k1" }).to_string()),
        URL_SAFE_NO_PAD.encode(claims.to_string())
    );

    assert_eq!(
        verify(&key_set("k1"), &changed, 0).unwrap_err(),
        Rejected::Invalid("its signature doesn't match")
    );
    assert_eq!(
        verify(&key_set("k1"), &unsigned, 0).unwrap_err(),
        Rejected::Invalid("it isn't signed with EdDSA")
    );
    assert_eq!(
        verify(&key_set("k1"), "one.two", 0).unwrap_err(),
        Rejected::Invalid("a token has three parts")
    );
}
