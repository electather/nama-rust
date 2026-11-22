use ring::signature::{ED25519, UnparsedPublicKey};

use super::*;

/// RFC 8037's example key (appendix A.1), as the PKCS#8 PEM `openssl genpkey` writes.
fn rfc_8037_pem() -> String {
    let seed = URL_SAFE_NO_PAD
        .decode("nWGxne_9WmC6hEr0kuwsxERJxWl7MmkZcDusAxyuf2A")
        .unwrap();
    // The fixed DER prefix of a PKCS#8 v1 Ed25519 private key (RFC 8410).
    let mut der = vec![
        0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04,
        0x20,
    ];
    der.extend(seed);
    format!("{PEM_BEGIN}\n{}\n{PEM_END}\n", STANDARD.encode(der))
}

#[test]
fn each_key_is_published_by_its_rfc_7638_thumbprint() {
    let keys = Keys::from_pems(&rfc_8037_pem()).unwrap();

    let jwks: serde_json::Value = serde_json::from_str(keys.jwks()).unwrap();
    assert_eq!(
        jwks,
        json!({ "keys": [{
            "kty": "OKP",
            "crv": "Ed25519",
            "x": "11qYAYKxCrfVS_7TyWQHOg7hcvPapiMlrwIaaPcHURo",
            // RFC 8037, appendix A.3.
            "kid": "kPrK_qmxVWaYVA9wwBF6Iuo3vVzz7TxHCTwXBygrS4k",
            "alg": "EdDSA",
            "use": "sig",
        }]})
    );
}

#[test]
fn a_token_is_signed_by_the_first_key_and_names_it() {
    let other = "-----BEGIN PRIVATE KEY-----\n\
                 MC4CAQAwBQYDK2VwBCIEIJzJDsQn3SpQXPoFGl74lUI4L+huggYWuNcSt9vaCwTS\n\
                 -----END PRIVATE KEY-----\n";
    let keys = Keys::from_pems(&format!("{}{other}", rfc_8037_pem())).unwrap();

    let token = keys.sign(&json!({ "sub": "a server" }));

    let parts: Vec<&str> = token.split('.').collect();
    let header: serde_json::Value =
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[0]).unwrap()).unwrap();
    assert_eq!(
        header,
        json!({ "alg": "EdDSA", "typ": "JWT", "kid": "kPrK_qmxVWaYVA9wwBF6Iuo3vVzz7TxHCTwXBygrS4k" })
    );
    let claims: serde_json::Value =
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[1]).unwrap()).unwrap();
    assert_eq!(claims, json!({ "sub": "a server" }));
    let public = URL_SAFE_NO_PAD
        .decode("11qYAYKxCrfVS_7TyWQHOg7hcvPapiMlrwIaaPcHURo")
        .unwrap();
    UnparsedPublicKey::new(&ED25519, public)
        .verify(
            format!("{}.{}", parts[0], parts[1]).as_bytes(),
            &URL_SAFE_NO_PAD.decode(parts[2]).unwrap(),
        )
        .unwrap();
    let jwks: serde_json::Value = serde_json::from_str(keys.jwks()).unwrap();
    assert_eq!(jwks["keys"].as_array().unwrap().len(), 2);
}

#[test]
fn no_key_or_a_broken_one_is_refused() {
    assert!(matches!(Keys::from_pems(""), Err(KeysError::None)));
    assert!(matches!(
        Keys::from_pems(&format!("{}{PEM_BEGIN}\nAAAA\n", rfc_8037_pem())),
        Err(KeysError::Unterminated(2))
    ));
    assert!(matches!(
        Keys::from_pems(&format!("{PEM_BEGIN}\n!!\n{PEM_END}")),
        Err(KeysError::NotBase64(1))
    ));
    assert!(matches!(
        Keys::from_pems(&format!("{PEM_BEGIN}\nAAAA\n{PEM_END}")),
        Err(KeysError::NotEd25519(1, _))
    ));
}
