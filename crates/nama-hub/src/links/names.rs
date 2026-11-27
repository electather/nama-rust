//! A Linked Server's own name in the Server-names zone, and its certificate challenge's (ADR 0057,
//! ADR 0104), which need no Server Addresses. The names under it are its Server Addresses' own
//! (`server_addresses.rs`).

use super::LinkedServerId;

/// `server`'s own name in `zone`: its id in hex, which never changes.
pub(super) fn own_name(server: LinkedServerId, zone: &str) -> String {
    format!("{}.{zone}", server.0.simple())
}

/// Where `own`, a Server's own name, holds the TXT records of the DNS-01 challenge for its
/// certificate (RFC 8555, section 8.4).
pub(super) fn challenge_name(own: &str) -> String {
    format!("_acme-challenge.{own}")
}

#[cfg(test)]
#[path = "names_tests.rs"]
mod tests;
