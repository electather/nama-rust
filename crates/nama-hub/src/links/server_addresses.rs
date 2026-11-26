//! A Linked Server's Server Addresses, the one owner of what counts as one: storing those it
//! reports, probing its public port, listing them best first, and whether a sign-in may return to
//! one. Every check parses with WHATWG's parser, as browsers read the Hub's links to them and the
//! address its page sends a person back to.
//!
//! A sign-in never returns to the public address or the Relay's, though the list offers both: the
//! Server refuses such a return address itself before it asks the Hub (`nama-server`'s
//! `server_addresses::is_reported`), as it can't know its public IP, and the Relay's address is a
//! name the Hub gives it. Only its Server URL and LAN addresses are return addresses.

use std::net::SocketAddr;

use reqwest::Url;

use super::{Error, LinkedServer, LinkedServerId, Links, ListedServer, names};
use crate::accounts::AccountId;

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
    /// Its router's public IP address with the port that reaches it, if it has one: the request's
    /// own IP, never one the Server names (ADR 0057).
    pub(crate) public: Option<SocketAddr>,
    /// The Relay it dials out to, if any (ADR 0057).
    pub(crate) relay: Option<SocketAddr>,
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
            r#"SELECT id, name, server_url, public_address, relay_address, lan_addresses,
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
            .map(|server| ListedServer {
                id: LinkedServerId(server.id),
                name: server.name,
                linked_at: server.linked_at,
                addresses: [
                    // Clients add `/` and a path to an address, and a Server URL may end in one.
                    server.server_url.trim_end_matches('/').to_owned(),
                    server.public_address,
                    // A Server the Hub can't reach may have its Relay's as its Server URL too.
                    if server.relay_address == server.server_url {
                        String::new()
                    } else {
                        server.relay_address
                    },
                ]
                .into_iter()
                .filter(|address| !address.is_empty())
                .chain(server.lan_addresses)
                .collect(),
                url: server.server_url,
            })
            .collect())
    }

    /// Replaces the Server Addresses `server` reported before with its Server URL, LAN addresses
    /// and Relay at once, then with its public address once the Hub has tried to connect there,
    /// and names it after them all.
    pub(crate) async fn set_addresses(
        &self,
        server: &LinkedServer,
        reported: ReportedAddresses<'_>,
    ) -> Result<Checked, Error> {
        let ReportedAddresses {
            url,
            lan_addresses,
            public,
            relay,
        } = reported;
        let addresses_valid = lan_addresses.len() <= MAX_LAN_ADDRESSES
            && (url.is_empty() || is_address(url))
            && lan_addresses.iter().all(|address| is_address(address));
        if !addresses_valid {
            return Err(Error::AddressInvalid);
        }
        // Stored before the probe, which can take seconds, so a sign-in returning to the new
        // Server URL meanwhile isn't refused by `is_return_address`.
        sqlx::query!(
            "UPDATE linked_servers SET server_url = $1, lan_addresses = $2, relay_address = $3
             WHERE id = $4",
            url,
            lan_addresses,
            self.relay_address(server.id, relay),
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
        let public_address =
            reached.map_or_else(String::new, |address| format!("http://{address}"));
        // Stored before naming too, so a DNS provider that is down holds back only the names.
        self.store_public_address(server.id, &public_address)
            .await?;
        let names = self
            .write_names(
                server.id,
                reached.map(|address| address.ip()),
                lan_addresses,
                relay.map(|relay| relay.ip()),
            )
            .await?;
        // Stored again after naming, so a Server Unlinked meanwhile is found gone here and its
        // names go again; no transaction waits on the DNS provider holding one of the pool's
        // connections.
        if !self
            .store_public_address(server.id, &public_address)
            .await?
        {
            self.forget_names(server.id).await?;
        }
        Ok(Checked {
            public: probed.map(|(address, reachable)| PublicAddress {
                address: format!("http://{address}"),
                reachable,
            }),
            names,
        })
    }

    /// Whether `return_url` may be where a sign-in to `server` returns: on its Server URL or one
    /// of its LAN addresses, never its public address or Relay's (module docs).
    pub(super) async fn is_return_address(
        &self,
        server: LinkedServerId,
        return_url: &str,
    ) -> Result<bool, sqlx::Error> {
        let addresses = sqlx::query!(
            "SELECT server_url, lan_addresses FROM linked_servers WHERE id = $1",
            server.0,
        )
        .fetch_one(&self.db)
        .await?;
        let return_addresses = std::iter::once(addresses.server_url.as_str())
            .chain(addresses.lan_addresses.iter().map(String::as_str));
        Ok(is_on_server(return_url, return_addresses))
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

    /// Where clients reach `server` through `relay`, the Relay it dials out to, such as
    /// `https://relay.<own name>:443`: empty without one, or while the Hub names no Server, since
    /// only a name its certificate covers passes through the Relay.
    fn relay_address(&self, server: LinkedServerId, relay: Option<SocketAddr>) -> String {
        match (&self.names, relay) {
            (Some(server_names), Some(relay)) => {
                let own = names::own_name(server, &server_names.zone);
                format!("https://{}:{}", names::relay_name(&own), relay.port())
            }
            _ => String::new(),
        }
    }
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
    let Ok(parsed) = Url::parse(url) else {
        return false;
    };
    is_web_url(url)
        && addresses.into_iter().any(|address| {
            Url::parse(address).is_ok_and(|address| address.origin() == parsed.origin())
        })
}

/// Whether the Hub's page may send a person to `url`: an `http` or `https` URL of up to
/// [`RETURN_ADDRESS_LENGTH`] characters. Any other scheme, such as `javascript:`, would run on the
/// Hub's origin.
pub(super) fn is_web_url(url: &str) -> bool {
    url.len() <= RETURN_ADDRESS_LENGTH && is_http(url)
}

/// Whether `url` is an absolute `http` or `https` URL with a host, as a browser reads it.
fn is_http(url: &str) -> bool {
    Url::parse(url)
        .is_ok_and(|url| matches!(url.scheme(), "http" | "https") && url.host().is_some())
}

#[cfg(test)]
#[path = "server_addresses_tests.rs"]
mod tests;
