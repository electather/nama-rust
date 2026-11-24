//! Web Push's protocol: a message encrypted for one browser (RFC 8291), sent to its push service
//! (RFC 8030) with the Server's VAPID signature (RFC 8292). No third party of Nama's
//! takes part: the push service is the browser's own. And each User's subscriptions
//! (`subscriptions.rs`).

use std::{sync::Arc, time::Duration};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::{
    aead::{AES_128_GCM, Aad, LessSafeKey, Nonce, UnboundKey},
    agreement::{self, ECDH_P256, EphemeralPrivateKey, UnparsedPublicKey},
    error::{KeyRejected, Unspecified},
    hkdf::{self, HKDF_SHA256},
    rand::{SecureRandom, SystemRandom},
    signature::{ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair, KeyPair},
};
use thiserror::Error;
use url::Url;

use crate::{db::now, diagnostics};

mod subscriptions;
#[cfg(test)]
mod testing;

pub(crate) use subscriptions::{push as push_to_subscriptions, subscribe, unsubscribe};
#[cfg(test)]
pub(crate) use testing::{assert_valid_vapid, browser, push_service};

/// How long a push service keeps a push for a browser that's offline, in seconds.
const KEPT_FOR_SECONDS: u32 = 7 * 24 * 60 * 60;

/// How long a push service has to answer.
const PUSH_TIMEOUT: Duration = Duration::from_secs(10);

/// How long a VAPID signature holds; RFC 8292 allows at most 24 hours.
const SIGNATURE_LASTS_SECONDS: i64 = 12 * 60 * 60;

/// The length of an uncompressed P-256 point, as browsers and VAPID send public keys.
const PUBLIC_KEY_LEN: u8 = 65;

/// The length of a browser's authentication secret.
const AUTH_LEN: usize = 16;

/// The size of the one record a push holds (RFC 8188); push services take 4096 bytes at most.
const RECORD_SIZE: u32 = 4096;

/// The Server's VAPID key, which signs every push. It has no `Debug`, so no log shows it.
pub(crate) struct VapidKey(EcdsaKeyPair);

impl VapidKey {
    /// A new key, in the PKCS#8 form [`Self::from_pkcs8`] reads.
    ///
    /// # Errors
    ///
    /// If the system's random source fails.
    pub(crate) fn new_pkcs8() -> Result<Vec<u8>, Unspecified> {
        let rng = SystemRandom::new();
        let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng)?;
        Ok(pkcs8.as_ref().to_vec())
    }

    /// The key [`Self::new_pkcs8`] made.
    ///
    /// # Errors
    ///
    /// If `pkcs8` isn't a P-256 key.
    pub(crate) fn from_pkcs8(pkcs8: &[u8]) -> Result<Self, KeyRejected> {
        let rng = SystemRandom::new();
        EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, pkcs8, &rng).map(Self)
    }

    /// The public key, an uncompressed P-256 point: a browser's `applicationServerKey`.
    pub(crate) fn public_key(&self) -> &[u8] {
        self.0.public_key().as_ref()
    }

    /// The `Authorization` header for a push to `endpoint` (RFC 8292).
    fn authorization(&self, endpoint: &Url) -> Result<String, Unspecified> {
        let header = URL_SAFE_NO_PAD.encode(r#"{"typ":"JWT","alg":"ES256"}"#);
        let claims = serde_json::json!({
            "aud": endpoint.origin().ascii_serialization(),
            "exp": now() + SIGNATURE_LASTS_SECONDS,
            // Whom the push service contacts about this Server's pushes; an Admin sets nothing.
            "sub": env!("CARGO_PKG_REPOSITORY"),
        });
        let signed = format!("{header}.{}", URL_SAFE_NO_PAD.encode(claims.to_string()));
        let signature = self.0.sign(&SystemRandom::new(), signed.as_bytes())?;
        Ok(format!(
            "vapid t={signed}.{}, k={}",
            URL_SAFE_NO_PAD.encode(signature),
            URL_SAFE_NO_PAD.encode(self.public_key())
        ))
    }
}

/// A browser's push subscription: where its push service takes pushes for it, and its keys. All
/// three are its secrets, so it has no `Debug`.
pub(crate) struct Subscription {
    pub(crate) endpoint: String,
    /// The browser's P-256 public key, uncompressed.
    pub(crate) p256dh: Vec<u8>,
    /// The browser's authentication secret.
    pub(crate) auth: Vec<u8>,
}

/// Which part of a subscription a client sent isn't one a browser makes.
#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum Invalid {
    /// Not an https URL: a push service is always reached over https, and the Server mustn't POST
    /// to whatever else a client names.
    #[error("the endpoint isn't an https URL")]
    Endpoint,
    #[error("p256dh isn't an uncompressed P-256 public key")]
    P256dh,
    #[error("auth isn't the browser's authentication secret")]
    Auth,
}

impl Subscription {
    /// The subscription a client sent, once each part is one a browser makes.
    ///
    /// # Errors
    ///
    /// The first part, in field order, that isn't.
    pub(crate) fn new(endpoint: String, p256dh: Vec<u8>, auth: Vec<u8>) -> Result<Self, Invalid> {
        let is_https =
            Url::parse(&endpoint).is_ok_and(|url| url.scheme() == "https" && url.host().is_some());
        if !is_https {
            return Err(Invalid::Endpoint);
        }
        if p256dh.len() != usize::from(PUBLIC_KEY_LEN) || p256dh.first() != Some(&4) {
            return Err(Invalid::P256dh);
        }
        if auth.len() != AUTH_LEN {
            return Err(Invalid::Auth);
        }
        Ok(Self {
            endpoint,
            p256dh,
            auth,
        })
    }
}

/// What a push service did with a push.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Pushed {
    /// It took it for the browser.
    Delivered,
    /// The subscription is gone (404 or 410), so it'll never take another.
    Gone,
}

/// Why a push failed.
#[derive(Debug, Error)]
pub(crate) enum PushError {
    /// The endpoint isn't a URL, its keys aren't the browser's, or signing failed.
    #[error("could not encrypt or sign a push for the subscription")]
    Unsendable,
    #[error("could not reach the push service: {0}")]
    Unreachable(reqwest::Error),
    #[error("the push service answered {0}")]
    Refused(reqwest::StatusCode),
}

/// Pushes messages signed with the Server's VAPID key. Clones share the key and the connections.
#[derive(Clone)]
pub(crate) struct Pusher {
    client: reqwest::Client,
    key: Arc<VapidKey>,
}

impl Pusher {
    /// Pushes signed with `key`.
    ///
    /// # Errors
    ///
    /// If the HTTP client can't be built.
    pub(crate) fn new(key: VapidKey) -> Result<Self, reqwest::Error> {
        diagnostics::use_ring_for_tls();
        let client = reqwest::Client::builder().timeout(PUSH_TIMEOUT).build()?;
        Ok(Self {
            client,
            key: Arc::new(key),
        })
    }

    /// Pushes `message`, encrypted, to the browser subscribed as `to`.
    ///
    /// # Errors
    ///
    /// If it can't be encrypted or signed, the push service can't be reached in
    /// [`PUSH_TIMEOUT`], or it answers anything but success, 404 or 410. Its URL isn't in the
    /// error, as an endpoint is the subscription's secret.
    pub(crate) async fn push(
        &self,
        to: &Subscription,
        message: &[u8],
    ) -> Result<Pushed, PushError> {
        let endpoint = Url::parse(&to.endpoint).map_err(|_failed| PushError::Unsendable)?;
        let authorization =
            (self.key.authorization(&endpoint)).map_err(|_failed| PushError::Unsendable)?;
        let body = encrypt(to, message).map_err(|_failed| PushError::Unsendable)?;
        let response = (self.client.post(endpoint))
            .header("Authorization", authorization)
            .header("Content-Encoding", "aes128gcm")
            .header("TTL", KEPT_FOR_SECONDS)
            .body(body)
            .send()
            .await
            .map_err(|err| PushError::Unreachable(err.without_url()))?;
        match response.status() {
            status if status.is_success() => Ok(Pushed::Delivered),
            reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::GONE => Ok(Pushed::Gone),
            status => Err(PushError::Refused(status)),
        }
    }
}

/// `plaintext` encrypted for `to` under a new key of the Server's own (RFC 8291).
fn encrypt(to: &Subscription, plaintext: &[u8]) -> Result<Vec<u8>, Unspecified> {
    let rng = SystemRandom::new();
    let ours = EphemeralPrivateKey::generate(&ECDH_P256, &rng)?;
    let our_public = ours.compute_public_key()?;
    let mut salt = [0; 16];
    rng.fill(&mut salt)?;
    let theirs = UnparsedPublicKey::new(&ECDH_P256, &to.p256dh);
    agreement::agree_ephemeral(ours, &theirs, |shared| {
        seal(shared, to, our_public.as_ref(), salt, plaintext)
    })?
}

/// RFC 8291's `aes128gcm` body for `plaintext`, from the ECDH secret `shared` the Server's
/// `our_public` key and `to`'s make, and `salt`.
fn seal(
    shared: &[u8],
    to: &Subscription,
    our_public: &[u8],
    salt: [u8; 16],
    plaintext: &[u8],
) -> Result<Vec<u8>, Unspecified> {
    let key_info = [b"WebPush: info\0".as_slice(), &to.p256dh, our_public].concat();
    let mut ikm = [0; 32];
    hkdf(&to.auth, shared, &key_info, &mut ikm)?;
    let (mut cek, mut nonce) = ([0; 16], [0; 12]);
    hkdf(&salt, &ikm, b"Content-Encoding: aes128gcm\0", &mut cek)?;
    hkdf(&salt, &ikm, b"Content-Encoding: nonce\0", &mut nonce)?;
    // One record, so the last-record delimiter and no padding.
    let mut record = [plaintext, &[2]].concat();
    let key = LessSafeKey::new(UnboundKey::new(&AES_128_GCM, &cek)?);
    key.seal_in_place_append_tag(
        Nonce::assume_unique_for_key(nonce),
        Aad::empty(),
        &mut record,
    )?;
    let header = [&salt[..], &RECORD_SIZE.to_be_bytes(), &[PUBLIC_KEY_LEN]].concat();
    Ok([&header, our_public, &record].concat())
}

/// Fills `out` with HKDF-SHA-256 of `ikm` under `salt` and `info`.
fn hkdf(salt: &[u8], ikm: &[u8], info: &[u8], out: &mut [u8]) -> Result<(), Unspecified> {
    struct Len(usize);
    impl hkdf::KeyType for Len {
        fn len(&self) -> usize {
            self.0
        }
    }
    let prk = hkdf::Salt::new(HKDF_SHA256, salt).extract(ikm);
    prk.expand(&[info], Len(out.len()))?.fill(out)
}

#[cfg(test)]
#[path = "web_push/web_push_tests.rs"]
mod tests;
