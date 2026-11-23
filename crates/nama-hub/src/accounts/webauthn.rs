//! The Hub as a WebAuthn relying party: the challenges it hands out, and the checks a relying party
//! makes on a browser's answer (W3C Web Authentication, §7), for the two algorithms the Hub asks
//! for, ES256 and RS256, verified with `ring`.
//!
//! The Hub asks for no attestation, so nothing signs a new passkey: it takes the public key as the
//! browser gives it, and only the challenge ties it to the request that asked for it.
//!
//! ponytail: waiting challenges live in this process's memory (ADR 0096), as the sign-in rate
//! limits do; they move to PostgreSQL once the Hub runs as replicas.

use std::{
    collections::{HashMap, hash_map::Entry},
    net::IpAddr,
    sync::{Mutex, MutexGuard, PoisonError},
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::signature::{
    ECDSA_P256_SHA256_ASN1, RSA_PKCS1_2048_8192_SHA256, UnparsedPublicKey, VerificationAlgorithm,
};
use serde::Deserialize;
use sha2::{Digest as _, Sha256};
use url::Url;

use super::{
    AccountId, Error,
    rate_limits::{Blocked, Caller, counted_ip},
};

/// COSE's id (RFC 9053) for ECDSA with P-256 and SHA-256.
const ES256: i32 = -7;
/// COSE's id (RFC 9864) for RSASSA-PKCS1-v1_5 with SHA-256.
const RS256: i32 = -257;

/// How long a challenge waits for its answer.
const CHALLENGE_LIFETIME: Duration = Duration::from_mins(5);

/// The most challenges that may wait at once, so a flood of requests can't fill the Hub's memory.
const MOST_WAITING: usize = 10_000;

/// The most challenges one client, as `counted_ip` counts it, may have waiting, so it would take
/// 500 of them to fill the Hub's. Rotating through a /48's /64s is bounded only by `MOST_WAITING`;
/// stateless, HMAC-signed challenges would close that, should it matter (#2049).
const WAITING_PER_CLIENT: usize = 20;

/// A P-256 `SubjectPublicKeyInfo` up to its uncompressed point, the same in every one.
const P256_SPKI_PREFIX: &[u8] = &[
    0x30, 0x59, 0x30, 0x13, 0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01, 0x06, 0x08, 0x2a,
    0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07, 0x03, 0x42, 0x00,
];

/// The DER `AlgorithmIdentifier` of an RSA `SubjectPublicKeyInfo`: `rsaEncryption`, `NULL`.
const RSA_ALGORITHM: &[u8] = &[
    0x30, 0x0d, 0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01, 0x05, 0x00,
];

/// The authenticator data flag saying a person was there, such as by touching a key.
const USER_PRESENT: u8 = 0x01;

/// What a challenge was handed out for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Ceremony {
    SignIn,
    /// Adding a passkey to this Account.
    Add(AccountId),
}

impl Ceremony {
    /// The `type` the browser's client data names for it.
    const fn client_data_type(self) -> &'static str {
        match self {
            Self::SignIn => "webauthn.get",
            Self::Add(_) => "webauthn.create",
        }
    }
}

/// The Hub at its public address, as passkeys know it.
#[derive(Debug)]
pub(super) struct RelyingParty {
    /// WebAuthn's RP ID: the public address's host.
    pub(super) id: String,
    /// The public address's origin, which every answer must come from.
    origin: String,
    waiting: Mutex<Waiting>,
}

/// The challenges waiting for their answer, and how many each client holds.
#[derive(Debug, Default)]
struct Waiting {
    challenges: HashMap<[u8; 32], Handed>,
    held: HashMap<IpAddr, usize>,
}

/// What a waiting challenge was handed out for, when, and to which client.
#[derive(Debug, Clone, Copy)]
struct Handed {
    ceremony: Ceremony,
    at: Instant,
    client: IpAddr,
}

/// What the browser says it signed, `clientDataJSON` (§5.8.1).
#[derive(Deserialize)]
struct ClientData {
    #[serde(rename = "type")]
    kind: String,
    challenge: String,
    origin: String,
    #[serde(rename = "crossOrigin", default)]
    cross_origin: bool,
}

impl RelyingParty {
    /// The relying party at `public_url`, or `None` for a URL without a host.
    pub(super) fn new(public_url: &Url) -> Option<Self> {
        Some(Self {
            id: public_url.host_str()?.to_owned(),
            origin: public_url.origin().ascii_serialization(),
            waiting: Mutex::default(),
        })
    }

    /// A fresh challenge for `ceremony`, handed to `caller`, which waits for its answer from when
    /// it asked. Refused while `caller` holds its share of waiting challenges, or all wait.
    pub(super) fn challenge(&self, ceremony: Ceremony, caller: Caller) -> Result<[u8; 32], Error> {
        let client = counted_ip(caller.ip);
        let mut challenge = [0; 32];
        getrandom::fill(&mut challenge)?;
        let mut waiting = self.lock();
        if waiting.challenges.len() >= MOST_WAITING || waiting.held_by(client) >= WAITING_PER_CLIENT
        {
            waiting.drop_expired(caller.now);
        }
        if let Some(retry_after) = waiting.wait(client, caller.now) {
            return Err(Blocked { retry_after }.into());
        }
        if waiting.challenges.len() >= MOST_WAITING {
            return Err(Error::ChallengesBusy);
        }
        waiting.insert(
            challenge,
            Handed {
                ceremony,
                at: caller.now,
                client,
            },
        );
        Ok(challenge)
    }

    /// Whether `client_data_json` answers, from the Hub's origin, a challenge handed out for
    /// `ceremony` that is still waiting at `now`. Using a challenge up whatever the answer, so
    /// each is tried once.
    pub(super) fn answers(
        &self,
        client_data_json: &[u8],
        ceremony: Ceremony,
        now: Instant,
    ) -> bool {
        let Ok(client_data) = serde_json::from_slice::<ClientData>(client_data_json) else {
            return false;
        };
        let Some(challenge) = URL_SAFE_NO_PAD
            .decode(&client_data.challenge)
            .ok()
            .and_then(|bytes| <[u8; 32]>::try_from(bytes).ok())
        else {
            return false;
        };
        let waited = self.lock().remove(&challenge);
        client_data.kind == ceremony.client_data_type()
            && client_data.origin == self.origin
            && !client_data.cross_origin
            && waited.is_some_and(|handed| handed.ceremony == ceremony && !expired(handed.at, now))
    }

    /// Whether `signature` is `public_key`'s, of `authenticator_data` for this relying party with
    /// a person present, and `client_data_json` (§7.2, steps 15 to 21).
    pub(super) fn signed(
        &self,
        public_key: &UnparsedPublicKey<&[u8]>,
        authenticator_data: &[u8],
        client_data_json: &[u8],
        signature: &[u8],
    ) -> bool {
        let Some((rp_id_hash, [flags, ..])) = authenticator_data.split_first_chunk::<32>() else {
            return false;
        };
        let signed = [authenticator_data, &Sha256::digest(client_data_json)].concat();
        rp_id_hash[..] == Sha256::digest(&self.id)[..]
            && flags & USER_PRESENT != 0
            && public_key.verify(&signed, signature).is_ok()
    }

    fn lock(&self) -> MutexGuard<'_, Waiting> {
        // Each change keeps `held` counting `challenges`, so a panic elsewhere doesn't spoil it.
        self.waiting.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Waiting {
    fn held_by(&self, client: IpAddr) -> usize {
        self.held.get(&client).copied().unwrap_or_default()
    }

    /// How long `client`, at its share at `now`, must wait for its oldest challenge to expire.
    /// Expects expired challenges dropped.
    fn wait(&self, client: IpAddr, now: Instant) -> Option<Duration> {
        if self.held_by(client) < WAITING_PER_CLIENT {
            return None;
        }
        let oldest = self
            .challenges
            .values()
            .filter(|handed| handed.client == client)
            .map(|handed| handed.at)
            .min()?;
        Some(CHALLENGE_LIFETIME.saturating_sub(now.saturating_duration_since(oldest)))
    }

    fn insert(&mut self, challenge: [u8; 32], handed: Handed) {
        self.challenges.insert(challenge, handed);
        *self.held.entry(handed.client).or_default() += 1;
    }

    fn remove(&mut self, challenge: &[u8; 32]) -> Option<Handed> {
        let handed = self.challenges.remove(challenge)?;
        release(&mut self.held, handed.client);
        Some(handed)
    }

    fn drop_expired(&mut self, now: Instant) {
        let held = &mut self.held;
        self.challenges.retain(|_, handed| {
            let expired = expired(handed.at, now);
            if expired {
                release(held, handed.client);
            }
            !expired
        });
    }
}

/// Takes one off `client`'s count, dropping it at zero so `held` doesn't keep every client seen.
fn release(held: &mut HashMap<IpAddr, usize>, client: IpAddr) {
    if let Entry::Occupied(mut count) = held.entry(client) {
        *count.get_mut() -= 1;
        if *count.get() == 0 {
            count.remove();
        }
    }
}

/// `spki`, a DER `SubjectPublicKeyInfo`, as `ring` checks `algorithm`'s signatures with it, if it is
/// a key for ES256 or RS256.
pub(super) fn public_key(spki: &[u8], algorithm: i32) -> Option<UnparsedPublicKey<&[u8]>> {
    let (verification, key): (&'static dyn VerificationAlgorithm, _) = match algorithm {
        ES256 => (
            &ECDSA_P256_SHA256_ASN1,
            spki.strip_prefix(P256_SPKI_PREFIX)
                .filter(|point| point.len() == 65)?,
        ),
        RS256 => (&RSA_PKCS1_2048_8192_SHA256, rsa_public_key(spki)?),
        _ => return None,
    };
    Some(UnparsedPublicKey::new(verification, key))
}

/// The DER `RSAPublicKey` inside an RSA `SubjectPublicKeyInfo`, which is what `ring` reads.
fn rsa_public_key(spki: &[u8]) -> Option<&[u8]> {
    let ([], info) = der(spki, 0x30)? else {
        return None;
    };
    let ([], bits) = der(info.strip_prefix(RSA_ALGORITHM)?, 0x03)? else {
        return None;
    };
    // A BIT STRING's first byte counts its unused bits, which a key has none of.
    bits.strip_prefix(&[0])
}

/// What follows a DER value tagged `tag` at the start of `bytes`, and its contents, for a length
/// under 64 KiB, as a key's is.
fn der(bytes: &[u8], tag: u8) -> Option<(&[u8], &[u8])> {
    let (&found, bytes) = bytes.split_first()?;
    let (&first, bytes) = bytes.split_first()?;
    let (length, bytes) = match first {
        0..=0x7f => (usize::from(first), bytes),
        0x81 => bytes
            .split_first()
            .map(|(&length, rest)| (usize::from(length), rest))?,
        0x82 => bytes
            .split_first_chunk()
            .map(|(&length, rest)| (usize::from(u16::from_be_bytes(length)), rest))?,
        _ => return None,
    };
    let (contents, rest) = bytes.split_at_checked(length)?;
    (found == tag).then_some((rest, contents))
}

fn expired(handed_out_at: Instant, now: Instant) -> bool {
    now.saturating_duration_since(handed_out_at) >= CHALLENGE_LIFETIME
}

#[cfg(test)]
#[path = "webauthn_tests.rs"]
mod tests;
