//! A Linked Server's names in the zone of Server Names (ADR 0057, ADR 0104).

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

/// The records under `own`, a Server's own name: `own` itself at `public`, where the Hub reached
/// it, and one name per private IPv4 address among `lan_addresses`, carrying the address, such as
/// `192-168-1-10.<own>`.
pub(super) fn records(own: &str, public: Option<IpAddr>, lan_addresses: &[String]) -> Vec<Record> {
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
        .collect()
}

/// The names `records` give the Server, its own name `own` first.
pub(super) fn shown(own: &str, records: &[Record]) -> Vec<String> {
    let lan = records.iter().filter(|record| record.name != own);
    iter::once(own)
        .chain(lan.map(|record| record.name.as_str()))
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
