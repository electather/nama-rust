//! A passkey the way a browser and its authenticator hold one: an ES256 key that answers the Hub's
//! challenges as `navigator.credentials` would, from the test Hub's public address.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use nama_hub_proto::nama::hub::v1::{
    AddPasskeyRequest, SignInWithPasskeyRequest, StartAddingPasskeyResponse,
    StartPasskeySignInResponse,
};
use ring::{
    rand::{SecureRandom as _, SystemRandom},
    signature::{ECDSA_P256_SHA256_ASN1_SIGNING, EcdsaKeyPair, KeyPair as _},
};
use sha2::{Digest as _, Sha256};

use super::hub::PUBLIC_URL;

/// A P-256 `SubjectPublicKeyInfo` up to its uncompressed point.
const P256_SPKI_PREFIX: [u8; 26] = [
    0x30, 0x59, 0x30, 0x13, 0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01, 0x06, 0x08, 0x2a,
    0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07, 0x03, 0x42, 0x00,
];

/// The authenticator data flags for a person present and verified.
const PRESENT_AND_VERIFIED: u8 = 0x05;

/// A passkey, and the Account's user handle once it is added to one.
#[derive(Debug)]
pub(crate) struct Passkey {
    key: EcdsaKeyPair,
    pub(crate) credential_id: Vec<u8>,
    user_handle: Vec<u8>,
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl Passkey {
    pub(crate) fn new() -> Self {
        let rng = SystemRandom::new();
        let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, &rng).unwrap();
        let key = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, pkcs8.as_ref(), &rng)
            .unwrap();
        let mut credential_id = vec![0; 16];
        rng.fill(&mut credential_id).unwrap();
        Self {
            key,
            credential_id,
            user_handle: Vec::new(),
        }
    }

    /// What a browser sends after creating this passkey, named `name`, for `started`.
    pub(crate) fn created(
        &mut self,
        name: &str,
        started: &StartAddingPasskeyResponse,
    ) -> AddPasskeyRequest {
        self.user_handle.clone_from(&started.user_id);
        AddPasskeyRequest {
            name: name.to_owned(),
            credential_id: self.credential_id.clone(),
            client_data_json: client_data("webauthn.create", &started.challenge),
            public_key: [&P256_SPKI_PREFIX[..], self.key.public_key().as_ref()].concat(),
            public_key_algorithm: -7,
            ..Default::default()
        }
    }

    /// What a browser sends after this passkey signed `started`'s challenge.
    pub(crate) fn signed(&self, started: &StartPasskeySignInResponse) -> SignInWithPasskeyRequest {
        let client_data_json = client_data("webauthn.get", &started.challenge);
        let authenticator_data = [
            &Sha256::digest(&started.rp_id)[..],
            &[PRESENT_AND_VERIFIED, 0, 0, 0, 1],
        ]
        .concat();
        let signed = [&authenticator_data[..], &Sha256::digest(&client_data_json)].concat();
        let signature = self.key.sign(&SystemRandom::new(), &signed).unwrap();
        SignInWithPasskeyRequest {
            credential_id: self.credential_id.clone(),
            client_data_json,
            authenticator_data,
            signature: signature.as_ref().to_vec(),
            user_handle: self.user_handle.clone(),
            ..Default::default()
        }
    }
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn client_data(kind: &str, challenge: &[u8]) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "type": kind,
        "challenge": URL_SAFE_NO_PAD.encode(challenge),
        "origin": PUBLIC_URL,
        "crossOrigin": false,
    }))
    .unwrap()
}
