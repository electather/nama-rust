//! A Linked Server's Server Addresses, the one owner of what counts as one: storing those it
//! reports, probing its public port, and [`ServerAddresses`], which alone lists them best first,
//! says whether a sign-in may return to one, and names them. Two addresses are the same when they
//! have the same origin, as WHATWG's parser reads them, since browsers read the Hub's links to them
//! and the address its page sends a person back to that way.
//!
//! A sign-in never returns to the public address, though the list offers it: the Server refuses
//! such a return address itself before it asks the Hub (`nama-server`'s
//! `server_addresses::is_reported`), as it can't know its public IP. Its Server URL, LAN addresses,
//! the HTTPS names the Hub gave those and its Relay's address are return addresses; the Hub makes
//! the last two itself, so a Server can't widen them by reporting anything.

use std::{
    collections::{BTreeSet, HashSet},
    iter,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};

use reqwest::Url;

use super::{Error, LinkedServer, LinkedServerId, Links, ListedServer, names};
use crate::{accounts::AccountId, dns::Record};

/// The most characters a Server Address may have.
pub(crate) const ADDRESS_LENGTH: usize = 2000;

/// The most characters a return address may have, more than [`ADDRESS_LENGTH`] for the path and
/// query it carries on top, as browsers handle about this much.
const RETURN_ADDRESS_LENGTH: usize = 2048;

/// The most LAN addresses a Server may report.
pub(crate) const MAX_LAN_ADDRESSES: usize = 16;

/// The Server Addresses a Server reports.
pub(crate) struct ReportedAddresses<'a> {
    /// The Server URL its Admin set, or empty for none.
    pub(crate) url: &'a str,
    /// Such as `http://192.168.1.10:7980`.
    pub(crate) lan_addresses: &'a [String],
    /// The port that reaches the Server from outside, as sent: 0 for none.
    pub(crate) public_port: u32,
    /// The Relay it dials out to as `<ip>:<port>`, as sent: empty for none (ADR 0057).
    pub(crate) relay: &'a str,
    /// The IP address the request came from (ADR 0018), the only one the public port is probed
    /// at, never one the Server names (ADR 0057).
    pub(crate) request_ip: IpAddr,
}

/// What the Hub found of a Server's addresses it reported.
pub(crate) struct Checked {
    /// Where it tried to reach the Server from outside, if the Server has a public port.
    pub(crate) public: Option<PublicAddress>,
    /// The Server's names, its own first, or none while the Hub names no Server.
    pub(crate) names: Vec<String>,
}

/// Where the Hub tried to reach a Server from outside, and whether it could.
#[derive(Default)]
pub(crate) struct PublicAddress {
    /// Such as `http://203.0.113.7:7980`.
    pub(crate) address: String,
    pub(crate) reachable: bool,
}

impl Links {
    /// The Linked Servers `account` owns or has a User on, oldest first.
    pub(crate) async fn list(&self, account: AccountId) -> Result<Vec<ListedServer>, sqlx::Error> {
        // Floored, not cast: `crates/nama-hub/AGENTS.md`, "Data".
        let servers = sqlx::query!(
            r#"SELECT id, name, server_url, public_address, relay, lan_addresses,
             floor(extract(epoch FROM created_at))::bigint AS "linked_at!"
             FROM linked_servers WHERE account_id = $1
             OR id IN (SELECT linked_server_id FROM user_links WHERE account_id = $1)
             ORDER BY created_at, id"#,
            account.0,
        )
        .fetch_all(&self.db)
        .await?;
        Ok(servers
            .into_iter()
            .map(|server| {
                let id = LinkedServerId(server.id);
                let addresses = ServerAddresses::new(
                    id,
                    self.zone(),
                    &server.server_url,
                    &server.lan_addresses,
                    stored_public(&server.public_address),
                    stored_relay(&server.relay),
                )
                .listed();
                ListedServer {
                    id,
                    name: server.name,
                    linked_at: server.linked_at,
                    addresses,
                    url: server.server_url,
                }
            })
            .collect())
    }

    /// Replaces the Server Addresses `server` reported before with its Server URL, LAN addresses
    /// and Relay at once, then with its public address once the Hub has tried to connect there,
    /// and names it after them all. Any part of the report that isn't valid is refused with
    /// [`Error::AddressInvalid`] before anything is stored.
    pub(crate) async fn set_addresses(
        &self,
        server: &LinkedServer,
        reported: ReportedAddresses<'_>,
    ) -> Result<Checked, Error> {
        let ReportedAddresses {
            url, lan_addresses, ..
        } = reported;
        let (public, relay) = checked_report(&reported)?;
        // Stored before the probe, which can take seconds, so a sign-in returning to the new
        // Server URL meanwhile isn't refused by `is_return_address`.
        sqlx::query!(
            "UPDATE linked_servers SET server_url = $1, lan_addresses = $2, relay = $3
             WHERE id = $4",
            url,
            lan_addresses,
            reported.relay,
            server.id.0,
        )
        .execute(&self.db)
        .await?;
        let probed = match public {
            Some(address) => Some((address, self.probe.reaches(address).await)),
            None => None,
        };
        let reached = probed
            .filter(|&(_, reachable)| reachable)
            .map(|(address, _)| address);
        let public_as_stored = reached.map_or_else(String::new, public_address);
        // Stored before naming too, so a DNS provider that is down holds back only the names.
        self.store_public_address(server.id, &public_as_stored)
            .await?;
        let addresses =
            ServerAddresses::new(server.id, self.zone(), url, lan_addresses, reached, relay);
        let names = self.write_names(server.id, &addresses).await?;
        // Stored again after naming, so a Server Unlinked meanwhile is found gone here and its
        // names go again; no transaction waits on the DNS provider holding one of the pool's
        // connections.
        if !self
            .store_public_address(server.id, &public_as_stored)
            .await?
        {
            self.forget_names(server.id).await?;
        }
        Ok(Checked {
            public: probed.map(|(address, reachable)| PublicAddress {
                address: public_address(address),
                reachable,
            }),
            names,
        })
    }

    /// Whether `return_url` may be where a sign-in to `server` returns (module docs).
    pub(super) async fn is_return_address(
        &self,
        server: LinkedServerId,
        return_url: &str,
    ) -> Result<bool, sqlx::Error> {
        let stored = sqlx::query!(
            "SELECT server_url, lan_addresses, public_address, relay FROM linked_servers
             WHERE id = $1",
            server.0,
        )
        .fetch_one(&self.db)
        .await?;
        let addresses = ServerAddresses::new(
            server,
            self.zone(),
            &stored.server_url,
            &stored.lan_addresses,
            stored_public(&stored.public_address),
            stored_relay(&stored.relay),
        );
        Ok(addresses.is_return_address(return_url))
    }

    /// The Server-names zone, or none while the Hub names no Server.
    fn zone(&self) -> Option<&str> {
        self.names.as_ref().map(|names| names.zone.as_str())
    }

    /// Stores `address` as `server`'s public address, and answers whether it is still Linked.
    async fn store_public_address(
        &self,
        server: LinkedServerId,
        address: &str,
    ) -> Result<bool, sqlx::Error> {
        let stored = sqlx::query!(
            "UPDATE linked_servers SET public_address = $1 WHERE id = $2",
            address,
            server.0,
        )
        .execute(&self.db)
        .await?;
        Ok(stored.rows_affected() > 0)
    }
}

/// A Linked Server's Server Addresses, made from the facts of its last report and nothing else, so
/// that its list, its return addresses and its names can't disagree (module docs).
pub(super) struct ServerAddresses<'a> {
    /// Its own name, or none while the Hub names no Server.
    own: Option<String>,
    /// The Server URL as sent, or empty for none.
    url: &'a str,
    /// As sent.
    lan_addresses: &'a [String],
    /// Where the Hub reached it from outside, if it did.
    public: Option<SocketAddr>,
    /// The Relay it dials out to, if any (ADR 0057).
    relay: Option<SocketAddr>,
}

impl<'a> ServerAddresses<'a> {
    /// `server`'s Server Addresses, named in `zone` unless it is none. A report makes them from
    /// what it just checked, and a read from one row, so the two can't differ.
    #[expect(
        clippy::too_many_arguments,
        reason = "the facts the Server Addresses are made from, each one of a different kind"
    )]
    pub(super) fn new(
        server: LinkedServerId,
        zone: Option<&str>,
        url: &'a str,
        lan_addresses: &'a [String],
        public: Option<SocketAddr>,
        relay: Option<SocketAddr>,
    ) -> Self {
        Self {
            own: zone.map(|zone| names::own_name(server, zone)),
            url,
            lan_addresses,
            public,
            relay,
        }
    }

    /// The addresses a client tries, best first: the Server URL, the public address, the Relay's
    /// address, then the LAN addresses, each kept only when no earlier one has its origin.
    pub(super) fn listed(&self) -> Vec<String> {
        // Clients add `/` and a path to an address, and a Server URL may end in one.
        let url = Some(self.url.trim_end_matches('/')).filter(|url| !url.is_empty());
        let mut origins = HashSet::new();
        (url.map(str::to_owned).into_iter())
            .chain(self.public.map(public_address))
            .chain(self.relay_address())
            .chain(self.lan_addresses.iter().cloned())
            .filter(|address| {
                Url::parse(address).is_ok_and(|parsed| origins.insert(parsed.origin()))
            })
            .collect()
    }

    /// Whether `return_url` has the origin of the Server URL, a LAN address, a LAN address's HTTPS
    /// name on its port or the Relay's address, never the public address (module docs).
    pub(super) fn is_return_address(&self, return_url: &str) -> bool {
        let made: Vec<String> = (self.relay_address().into_iter())
            .chain(self.lan_name_addresses())
            .collect();
        let sent = iter::once(self.url).chain(self.lan_addresses.iter().map(String::as_str));
        is_on_server(return_url, sent.chain(made.iter().map(String::as_str)))
    }

    /// The records under the Server's own name: the own name at the public IP the Hub reached, one
    /// name per private IPv4 LAN address, carrying it, and the Relay's name at the Relay's IP.
    pub(super) fn records(&self) -> Vec<Record> {
        let Some(own) = &self.own else {
            return Vec::new();
        };
        // A set, as two LAN addresses may differ only in their port.
        let lan: BTreeSet<Ipv4Addr> = (self.lan_addresses.iter())
            .filter_map(|address| lan_ip(&Url::parse(address).ok()?))
            .collect();
        let own_record = self.public.map(|public| Record {
            name: own.clone(),
            address: public.ip(),
        });
        let lan_records = lan.into_iter().map(|ip| Record {
            name: lan_name(own, ip),
            address: IpAddr::V4(ip),
        });
        let relay_record = self.relay.map(|relay| Record {
            name: relay_name(own),
            address: relay.ip(),
        });
        own_record
            .into_iter()
            .chain(lan_records)
            .chain(relay_record)
            .collect()
    }

    /// The names the Server is told, its own first: none while the Hub names no Server.
    pub(super) fn names(&self) -> Vec<String> {
        let Some(own) = &self.own else {
            return Vec::new();
        };
        let under = (self.records().into_iter())
            .map(|record| record.name)
            .filter(|name| name != own);
        iter::once(own.clone()).chain(under).collect()
    }

    /// Where clients reach the Server through its Relay, such as `https://relay.<own name>:443`:
    /// none while the Hub names no Server, since only a name its certificate covers passes through
    /// the Relay.
    fn relay_address(&self) -> Option<String> {
        let (own, relay) = (self.own.as_deref()?, self.relay?);
        Some(format!("https://{}:{}", relay_name(own), relay.port()))
    }

    /// Where clients reach each named LAN address over HTTPS: at the name [`Self::records`] give
    /// it, on its port.
    fn lan_name_addresses(&self) -> impl Iterator<Item = String> {
        let own = self.own.as_deref();
        self.lan_addresses.iter().filter_map(move |address| {
            let address = Url::parse(address).ok()?;
            let name = lan_name(own?, lan_ip(&address)?);
            Some(format!(
                "https://{name}:{}",
                address.port_or_known_default()?
            ))
        })
    }
}

/// How the Hub lists `public`, the public IP and port it reached, such as
/// `http://203.0.113.7:7980`.
fn public_address(public: SocketAddr) -> String {
    format!("http://{public}")
}

/// The public IP and port a row keeps as [`public_address`] wrote them, or none for empty.
fn stored_public(stored: &str) -> Option<SocketAddr> {
    stored.strip_prefix("http://")?.parse().ok()
}

/// The Relay a row keeps as the Server sent it, which [`parse_relay`] checked, or none for empty.
fn stored_relay(stored: &str) -> Option<SocketAddr> {
    stored.parse().ok()
}

/// The name under `own`, a Server's own name, at which clients reach it through its Relay (ADR
/// 0057). The certificate for the wildcard under `own` covers it, so the Relay passes TLS through.
fn relay_name(own: &str) -> String {
    format!("relay.{own}")
}

/// The name under `own`, a Server's own name, carrying `ip`, such as `192-168-1-10.<own>`.
fn lan_name(own: &str, ip: Ipv4Addr) -> String {
    format!("{}.{own}", ip.to_string().replace('.', "-"))
}

/// The host of the LAN address `address`, if it is a private IPv4 address: a public one could
/// point a name at someone else's host.
// ponytail: IPv6 LAN addresses get no name; add unique local ones once Servers report them.
fn lan_ip(address: &Url) -> Option<Ipv4Addr> {
    let ip = address.host_str()?.parse::<Ipv4Addr>().ok()?;
    ip.is_private().then_some(ip)
}

/// The public address to probe and the Relay `reported` holds, or [`Error::AddressInvalid`] for
/// any part of it that isn't valid, so nothing is stored from a bad report.
fn checked_report(
    reported: &ReportedAddresses<'_>,
) -> Result<(Option<SocketAddr>, Option<SocketAddr>), Error> {
    let addresses_valid = reported.lan_addresses.len() <= MAX_LAN_ADDRESSES
        && (reported.url.is_empty() || is_address(reported.url))
        && reported
            .lan_addresses
            .iter()
            .all(|address| is_address(address));
    if !addresses_valid {
        return Err(Error::AddressInvalid);
    }
    let public = match reported.public_port {
        0 => None,
        port => {
            let port = u16::try_from(port).map_err(|_too_big| Error::AddressInvalid)?;
            Some(SocketAddr::new(reported.request_ip.to_canonical(), port))
        }
    };
    Ok((public, parse_relay(reported.relay)?))
}

/// The Relay `relay` names, as `<ip>:<port>`, or `None` for empty. One no client could connect
/// to, such as on port 0 or a loopback address, is refused, since the Hub names it and offers it.
fn parse_relay(relay: &str) -> Result<Option<SocketAddr>, Error> {
    if relay.is_empty() {
        return Ok(None);
    }
    let relay: SocketAddr = relay
        .parse()
        .map_err(|_not_an_address| Error::AddressInvalid)?;
    let ip = relay.ip().to_canonical();
    if relay.port() == 0 || ip.is_unspecified() || ip.is_loopback() || ip.is_multicast() {
        return Err(Error::AddressInvalid);
    }
    Ok(Some(relay))
}

/// Whether `address` may be a Server Address: an absolute `http` or `https` URL with a host, so
/// the Hub's pages never link to a `javascript:` one a Server sent, of at most [`ADDRESS_LENGTH`]
/// characters. It is kept as sent; parsing only checks it.
fn is_address(address: &str) -> bool {
    address.len() <= ADDRESS_LENGTH && is_http(address)
}

/// Whether `url` is an `http` or `https` address short enough to keep, with the scheme, host and
/// port of one of the Server's `addresses`. The Hub's page sends the person there with the code
/// that finishes the sign-in, so an address anyone else chose would hand it to them, and any other
/// scheme, such as `javascript:`, would run on the Hub's origin.
fn is_on_server<'a>(url: &str, addresses: impl IntoIterator<Item = &'a str>) -> bool {
    is_web_url(url)
        && Url::parse(url).is_ok_and(|parsed| {
            addresses.into_iter().any(|address| {
                Url::parse(address).is_ok_and(|address| address.origin() == parsed.origin())
            })
        })
}

/// Whether the Hub's page may send a person to `url`: an `http` or `https` URL of up to
/// [`RETURN_ADDRESS_LENGTH`] characters. Any other scheme, such as `javascript:`, would run on the
/// Hub's origin.
pub(super) fn is_web_url(url: &str) -> bool {
    url.len() <= RETURN_ADDRESS_LENGTH && is_http(url)
}

/// Whether `url` is an absolute `http` or `https` URL with a host, as a browser reads it. It needs
/// `://` after its scheme: alone `https:nama.example` parses as `https://nama.example/`, but a link
/// to it on one of the Hub's `https` pages goes to a path on the Hub.
fn is_http(url: &str) -> bool {
    Url::parse(url).is_ok_and(|parsed| {
        matches!(parsed.scheme(), "http" | "https")
            && parsed.host().is_some()
            && url
                .split_once("://")
                .is_some_and(|(scheme, _)| scheme.eq_ignore_ascii_case(parsed.scheme()))
    })
}

#[cfg(test)]
#[path = "server_addresses_tests.rs"]
mod tests;
