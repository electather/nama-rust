use std::{
    net::{IpAddr, Ipv4Addr},
    time::Instant,
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::{
    rand::SystemRandom,
    signature::{ECDSA_P256_SHA256_ASN1_SIGNING, EcdsaKeyPair, KeyPair as _},
};
use sha2::{Digest as _, Sha256};
use url::Url;
use uuid::Uuid;

use super::{
    CHALLENGE_LIFETIME, Ceremony, ES256, MOST_WAITING, P256_SPKI_PREFIX, RS256, RelyingParty,
    WAITING_PER_CLIENT, public_key,
};
use crate::accounts::{AccountId, Caller, Error};

const RS256_SPKI: &[u8] = include_bytes!("../../tests/fixtures/rs256-passkey.spki.der");
/// `openssl dgst -sha256 -sign` of `RS256_SIGNED` by the key `RS256_SPKI` holds.
const RS256_SIGNATURE: &[u8] = include_bytes!("../../tests/fixtures/rs256-passkey.signature");
const RS256_SIGNED: &[u8] = b"signed by an RS256 passkey";

fn hub() -> RelyingParty {
    RelyingParty::new(&Url::parse("https://hub.example:8443/some/path").unwrap()).unwrap()
}

fn caller(ip: &str, now: Instant) -> Caller {
    Caller {
        ip: ip.parse().unwrap(),
        now,
    }
}

fn client_data(kind: &str, challenge: &[u8], origin: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "type": kind,
        "challenge": URL_SAFE_NO_PAD.encode(challenge),
        "origin": origin,
        "crossOrigin": false,
    }))
    .unwrap()
}

/// A P-256 passkey, as a browser gives its public key, and signing with it.
struct Es256Passkey {
    key: EcdsaKeyPair,
    spki: Vec<u8>,
}

impl Es256Passkey {
    fn new() -> Self {
        let rng = SystemRandom::new();
        let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, &rng).unwrap();
        let key = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, pkcs8.as_ref(), &rng)
            .unwrap();
        let spki = [P256_SPKI_PREFIX, key.public_key().as_ref()].concat();
        Self { key, spki }
    }

    fn sign(&self, authenticator_data: &[u8], client_data_json: &[u8]) -> Vec<u8> {
        let signed = [authenticator_data, &Sha256::digest(client_data_json)].concat();
        self.key
            .sign(&SystemRandom::new(), &signed)
            .unwrap()
            .as_ref()
            .to_vec()
    }
}

/// Authenticator data for `rp_id` with `flags` and a zero signature count.
fn authenticator_data(rp_id: &str, flags: u8) -> Vec<u8> {
    [&Sha256::digest(rp_id)[..], &[flags, 0, 0, 0, 0]].concat()
}

#[test]
fn the_relying_party_is_the_public_address_host_and_its_origin_the_scheme_host_and_port() {
    let hub = hub();

    assert_eq!(hub.id, "hub.example");
    assert_eq!(hub.origin, "https://hub.example:8443");
}

#[test]
fn a_challenge_is_answered_once_from_the_hubs_origin_for_its_ceremony() {
    let hub = hub();
    let now = Instant::now();
    let challenge = hub
        .challenge(Ceremony::SignIn, caller("203.0.113.1", now))
        .unwrap();
    let answer = client_data("webauthn.get", &challenge, "https://hub.example:8443");

    assert!(hub.answers(&answer, Ceremony::SignIn, now));
    assert!(!hub.answers(&answer, Ceremony::SignIn, now), "used up");
}

#[test]
fn an_answer_from_elsewhere_late_or_for_another_ceremony_uses_the_challenge_up_unanswered() {
    let hub = hub();
    let now = Instant::now();
    let account = Ceremony::Add(AccountId(Uuid::nil()));
    let other = Ceremony::Add(AccountId(Uuid::max()));
    let hub_origin = "https://hub.example:8443";
    for (handed_out_for, answered_as, kind, origin, at) in [
        (
            Ceremony::SignIn,
            Ceremony::SignIn,
            "webauthn.get",
            "https://evil.example",
            now,
        ),
        (
            Ceremony::SignIn,
            Ceremony::SignIn,
            "webauthn.create",
            hub_origin,
            now,
        ),
        (
            Ceremony::SignIn,
            Ceremony::SignIn,
            "webauthn.get",
            hub_origin,
            now + CHALLENGE_LIFETIME,
        ),
        (account, account, "webauthn.get", hub_origin, now),
        (account, other, "webauthn.create", hub_origin, now),
        (
            Ceremony::SignIn,
            account,
            "webauthn.create",
            hub_origin,
            now,
        ),
    ] {
        let challenge = hub
            .challenge(handed_out_for, caller("203.0.113.1", now))
            .unwrap();
        let answer = client_data(kind, &challenge, origin);

        assert!(!hub.answers(&answer, answered_as, at), "{kind} {origin}");
        assert!(
            !hub.answers(&answer, handed_out_for, now),
            "used up: {kind} {origin}"
        );
    }
    assert!(!hub.answers(b"not json", Ceremony::SignIn, now));
}

#[test]
fn challenges_past_the_most_that_wait_are_refused_until_some_expire() {
    let hub = hub();
    let now = Instant::now();
    let client = |index: usize| Caller {
        ip: IpAddr::V4(Ipv4Addr::from(
            u32::try_from(index / WAITING_PER_CLIENT).unwrap(),
        )),
        now,
    };
    for index in 0..MOST_WAITING {
        hub.challenge(Ceremony::SignIn, client(index)).unwrap();
    }

    let fresh = client(MOST_WAITING + WAITING_PER_CLIENT);
    assert!(matches!(
        hub.challenge(Ceremony::SignIn, fresh),
        Err(Error::ChallengesBusy)
    ));
    let later = Caller {
        now: now + CHALLENGE_LIFETIME,
        ..fresh
    };
    hub.challenge(Ceremony::SignIn, later).unwrap();
}

#[test]
fn a_client_at_its_share_of_waiting_challenges_is_blocked_while_another_is_not() {
    let hub = hub();
    let now = Instant::now();
    for _ in 0..WAITING_PER_CLIENT {
        hub.challenge(Ceremony::SignIn, caller("2001:db8::1", now))
            .unwrap();
    }

    let Err(Error::Blocked(blocked)) =
        hub.challenge(Ceremony::SignIn, caller("2001:db8::ffff", now))
    else {
        panic!("the same /64 is at its share");
    };
    assert_eq!(blocked.retry_after_secs(), CHALLENGE_LIFETIME.as_secs());
    hub.challenge(Ceremony::SignIn, caller("2001:db8:0:1::1", now))
        .unwrap();
    hub.challenge(
        Ceremony::Add(AccountId(Uuid::nil())),
        caller("198.51.100.1", now),
    )
    .unwrap();
}

#[test]
fn a_used_or_expired_challenge_frees_its_clients_share() {
    let hub = hub();
    let now = Instant::now();
    let first = hub
        .challenge(Ceremony::SignIn, caller("203.0.113.1", now))
        .unwrap();
    for _ in 1..WAITING_PER_CLIENT {
        hub.challenge(
            Ceremony::SignIn,
            caller("203.0.113.1", now + CHALLENGE_LIFETIME / 2),
        )
        .unwrap();
    }
    let answer = client_data("webauthn.get", &first, "https://hub.example:8443");
    assert!(hub.answers(&answer, Ceremony::SignIn, now));

    hub.challenge(Ceremony::SignIn, caller("203.0.113.1", now))
        .unwrap();
    hub.challenge(Ceremony::SignIn, caller("203.0.113.1", now))
        .unwrap_err();
    hub.challenge(
        Ceremony::SignIn,
        caller("203.0.113.1", now + CHALLENGE_LIFETIME),
    )
    .unwrap();
}

#[test]
fn an_es256_signature_counts_only_over_this_hubs_authenticator_data_with_a_person_present() {
    let hub = hub();
    let passkey = Es256Passkey::new();
    let key = public_key(&passkey.spki, ES256).unwrap();
    let client_data = b"{}";
    let present = authenticator_data("hub.example", 0x05);

    assert!(hub.signed(
        &key,
        &present,
        client_data,
        &passkey.sign(&present, client_data)
    ));
    let signature = passkey.sign(&present, client_data);
    assert!(
        !hub.signed(&key, &present, b"{ }", &signature),
        "other client data"
    );
    for data in [
        authenticator_data("evil.example", 0x05),
        authenticator_data("hub.example", 0x04),
        Sha256::digest("hub.example").to_vec(),
    ] {
        assert!(!hub.signed(&key, &data, client_data, &passkey.sign(&data, client_data)));
    }
}

#[test]
fn an_rs256_key_is_read_from_its_subject_public_key_info() {
    let key = public_key(RS256_SPKI, RS256).unwrap();

    key.verify(RS256_SIGNED, RS256_SIGNATURE).unwrap();
    assert!(key.verify(b"something else", RS256_SIGNATURE).is_err());
}

#[test]
fn a_key_for_another_algorithm_or_cut_short_is_refused() {
    let es256 = Es256Passkey::new().spki;

    assert!(public_key(&es256, RS256).is_none());
    assert!(public_key(RS256_SPKI, ES256).is_none());
    assert!(public_key(&es256, -8).is_none());
    assert!(public_key(&es256[..es256.len() - 1], ES256).is_none());
    assert!(public_key(&RS256_SPKI[..RS256_SPKI.len() - 1], RS256).is_none());
    assert!(public_key(&[RS256_SPKI, &[0]].concat(), RS256).is_none());
}
