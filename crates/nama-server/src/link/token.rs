//! Checking a token the Hub signed (ADR 0011, ADR 0098): an `EdDSA` JWT, against the Hub's key set.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::signature::{ED25519, UnparsedPublicKey};
use serde::Deserialize;

/// How far past its `exp` a token still passes, for a Server whose clock runs behind the Hub's.
const LEEWAY_SECS: u64 = 60;

/// What a token the Hub signed for a Linked Server says.
#[derive(Debug, Deserialize)]
pub(super) struct Claims {
    /// Whom it names: the Linked Server, or for a sign-in the User's id on the Server.
    #[serde(default)]
    pub(super) sub: String,
    /// Its Account's email, in the token a newly Linked Server gets.
    #[serde(default)]
    pub(super) email: String,
    exp: u64,
}

/// Why a token didn't pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Rejected {
    /// The key set has no key by the id the token names: fetch it again.
    UnknownKey,
    /// Malformed, badly signed or expired, with why.
    Invalid(&'static str),
}

#[derive(Deserialize)]
struct Header {
    alg: String,
    kid: String,
}

#[derive(Deserialize)]
struct KeySet {
    keys: Vec<Key>,
}

#[derive(Deserialize)]
struct Key {
    kid: String,
    crv: String,
    x: String,
}

/// `token`'s claims, if one of `key_set`'s keys signed it and it is unexpired at `now`, in seconds
/// since the Unix epoch.
pub(super) fn verify(key_set: &[u8], token: &str, now: u64) -> Result<Claims, Rejected> {
    let mut parts = token.split('.');
    let (Some(header), Some(claims), Some(signature), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(Rejected::Invalid("a token has three parts"));
    };
    let decoded = |part: &str| URL_SAFE_NO_PAD.decode(part).ok();
    let header: Header = decoded(header)
        .and_then(|header| serde_json::from_slice(&header).ok())
        .ok_or(Rejected::Invalid("its header isn't JSON"))?;
    // Only EdDSA: a token naming another algorithm, such as `none`, never passes.
    if header.alg != "EdDSA" {
        return Err(Rejected::Invalid("it isn't signed with EdDSA"));
    }
    let key_set: KeySet = serde_json::from_slice(key_set)
        .map_err(|_unreadable| Rejected::Invalid("the key set isn't JSON"))?;
    let key = key_set
        .keys
        .into_iter()
        .find(|key| key.kid == header.kid && key.crv == "Ed25519")
        .ok_or(Rejected::UnknownKey)?;
    let public = decoded(&key.x).ok_or(Rejected::Invalid("the key isn't base64"))?;
    let signature = decoded(signature).ok_or(Rejected::Invalid("its signature isn't base64"))?;
    let signed = token.rsplit_once('.').map_or("", |(signed, _)| signed);
    UnparsedPublicKey::new(&ED25519, public)
        .verify(signed.as_bytes(), &signature)
        .map_err(|_unspecified| Rejected::Invalid("its signature doesn't match"))?;
    let claims: Claims = decoded(claims)
        .and_then(|claims| serde_json::from_slice(&claims).ok())
        .ok_or(Rejected::Invalid("its claims aren't JSON"))?;
    if claims.exp.saturating_add(LEEWAY_SECS) < now {
        return Err(Rejected::Invalid("it expired"));
    }
    Ok(claims)
}

#[cfg(test)]
#[path = "token_tests.rs"]
mod tests;
