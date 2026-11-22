//! Signing tokens for Servers to verify (ADR 0011, ADR 0098): Ed25519 keys from the config, the
//! first of which signs, all published as a JSON Web Key Set.

use std::fmt;

use base64::{
    Engine as _, engine::general_purpose::STANDARD, engine::general_purpose::URL_SAFE_NO_PAD,
};
use ring::signature::{Ed25519KeyPair, KeyPair as _};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest as _, Sha256};

const PEM_BEGIN: &str = "-----BEGIN PRIVATE KEY-----";
const PEM_END: &str = "-----END PRIVATE KEY-----";

/// The Hub's signing keys: the first signs, and every one is published.
pub struct Keys {
    signer: Ed25519KeyPair,
    signer_kid: String,
    jwks: String,
}

// The private keys are secret, so only the signing key's id is printed.
impl fmt::Debug for Keys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Keys")
            .field("signer_kid", &self.signer_kid)
            .finish_non_exhaustive()
    }
}

/// Why the configured keys can't be used.
#[derive(Debug, thiserror::Error)]
pub enum KeysError {
    /// No key at all.
    #[error("NAMA_HUB_SIGNING_KEYS holds no `{PEM_BEGIN}` block")]
    None,
    /// A block that doesn't end.
    #[error("key {0} in NAMA_HUB_SIGNING_KEYS has no `{PEM_END}` line")]
    Unterminated(usize),
    /// A block that isn't base64.
    #[error("key {0} in NAMA_HUB_SIGNING_KEYS isn't base64")]
    NotBase64(usize),
    /// A block that holds some other key, or none.
    #[error("key {0} in NAMA_HUB_SIGNING_KEYS isn't an Ed25519 PKCS#8 key: {1}")]
    NotEd25519(usize, ring::error::KeyRejected),
}

impl Keys {
    /// The keys in `pems`, PKCS#8 PEM blocks one after another, such as
    /// `openssl genpkey -algorithm ed25519` writes. Anything between blocks is ignored.
    ///
    /// # Errors
    ///
    /// If `pems` holds no key, or a block isn't an Ed25519 key; each error numbers the key from 1.
    pub fn from_pems(pems: &str) -> Result<Self, KeysError> {
        let mut pairs = Vec::new();
        let mut rest = pems;
        while let Some((_, block)) = rest.split_once(PEM_BEGIN) {
            let number = pairs.len() + 1;
            let (body, after) = block
                .split_once(PEM_END)
                .ok_or(KeysError::Unterminated(number))?;
            let der = STANDARD
                .decode(body.split_whitespace().collect::<String>())
                .map_err(|_not_base64| KeysError::NotBase64(number))?;
            let pair = Ed25519KeyPair::from_pkcs8_maybe_unchecked(&der)
                .map_err(|err| KeysError::NotEd25519(number, err))?;
            pairs.push(pair);
            rest = after;
        }
        let published: Vec<_> = pairs.iter().map(jwk).collect();
        let jwks = json!({ "keys": published }).to_string();
        let mut pairs = pairs.into_iter();
        let signer = pairs.next().ok_or(KeysError::None)?;
        Ok(Self {
            signer_kid: thumbprint(&signer),
            signer,
            jwks,
        })
    }

    /// `claims` as a JWT signed with `EdDSA` by the first key, naming it in the header's `kid`.
    pub(crate) fn sign(&self, claims: &impl Serialize) -> String {
        let header = json!({ "alg": "EdDSA", "typ": "JWT", "kid": self.signer_kid });
        let signed = format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(header.to_string()),
            // Plain data, which always serializes.
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(claims).unwrap_or_default()),
        );
        let signature = self.signer.sign(signed.as_bytes());
        format!("{signed}.{}", URL_SAFE_NO_PAD.encode(signature))
    }

    /// Every key's public half, as the JSON Web Key Set served at `/.well-known/jwks.json`.
    pub(crate) fn jwks(&self) -> &str {
        &self.jwks
    }
}

/// `pair`'s public key as a JSON Web Key (RFC 8037), named by its thumbprint.
fn jwk(pair: &Ed25519KeyPair) -> serde_json::Value {
    json!({
        "kty": "OKP",
        "crv": "Ed25519",
        "x": public_x(pair),
        "kid": thumbprint(pair),
        "alg": "EdDSA",
        "use": "sig",
    })
}

/// The key's RFC 7638 thumbprint: the SHA-256 of its required members, in this exact order and
/// spacing, as the RFC defines it.
fn thumbprint(pair: &Ed25519KeyPair) -> String {
    let members = format!(
        r#"{{"crv":"Ed25519","kty":"OKP","x":"{}"}}"#,
        public_x(pair)
    );
    URL_SAFE_NO_PAD.encode(Sha256::digest(members))
}

fn public_x(pair: &Ed25519KeyPair) -> String {
    URL_SAFE_NO_PAD.encode(pair.public_key())
}

#[cfg(test)]
#[path = "signing/signing_tests.rs"]
mod tests;
