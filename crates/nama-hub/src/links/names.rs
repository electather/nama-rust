//! A Linked Server's names in the Server-names zone (ADR 0057, ADR 0104).

use std::{
    collections::BTreeSet,
    iter,
    net::{IpAddr, Ipv4Addr},
};

use axum::http::Uri;

use super::LinkedServerId;
use crate::dns::Record;

/// `server`'s own name in `zone`: its id in hex, which never changes.
pub(super) fn own_name(server: LinkedServerId, zone: &str) -> String {
    format!("{}.{zone}", server.0.simple())
}

/// Where `own`, a Server's own name, holds the TXT records of the DNS-01 challenge for its
/// certificate (RFC 8555, section 8.4).
pub(super) fn challenge_name(own: &str) -> String {
    format!("_acme-challenge.{own}")
}

/// The name under `own`, a Server's own name, at which clients reach it through its Relay (ADR
/// 0057). The certificate for the wildcard under `own` covers it, so the Relay passes TLS through.
pub(super) fn relay_name(own: &str) -> String {
    format!("relay.{own}")
}

/// The records under `own`, a Server's own name: `own` itself at `public`, where the Hub reached
/// it, one name per private IPv4 address among `lan_addresses`, carrying the address, such as
/// `192-168-1-10.<own>`, and its [`relay_name`] at `relay`, the Relay it dials out to.
pub(super) fn records(
    own: &str,
    public: Option<IpAddr>,
    lan_addresses: &[String],
    relay: Option<IpAddr>,
) -> Vec<Record> {
    // A set, as two LAN addresses may differ only in their port.
    let lan: BTreeSet<Ipv4Addr> = lan_addresses
        .iter()
        .filter_map(|address| lan_ip(address))
        .collect();
    let own_record = public.map(|address| Record {
        name: own.to_owned(),
        address,
    });
    own_record
        .into_iter()
        .chain(lan.into_iter().map(|ip| Record {
            name: format!("{}.{own}", ip.to_string().replace('.', "-")),
            address: IpAddr::V4(ip),
        }))
        .chain(relay.map(|address| Record {
            name: relay_name(own),
            address,
        }))
        .collect()
}

/// The names `records` give the Server, its own name `own` first.
pub(super) fn shown(own: &str, records: &[Record]) -> Vec<String> {
    let under = records.iter().filter(|record| record.name != own);
    iter::once(own)
        .chain(under.map(|record| record.name.as_str()))
        .map(str::to_owned)
        .collect()
}

/// The host of the LAN address `address`, if it is a private IPv4 address: a public one could
/// point a name at someone else's host.
// ponytail: IPv6 LAN addresses get no name; add unique local ones once Servers report them.
fn lan_ip(address: &str) -> Option<Ipv4Addr> {
    let uri = address.parse::<Uri>().ok()?;
    let ip = uri.host()?.parse::<Ipv4Addr>().ok()?;
    ip.is_private().then_some(ip)
}

#[cfg(test)]
#[path = "names_tests.rs"]
mod tests;
