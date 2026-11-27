//! A Linked Server's names in the Server-names zone (ADR 0057, ADR 0104): whether the Hub names
//! Servers at all, a Server's own name and the addresses of the names under it, and every write to
//! and removal of its records, its certificate challenge's included. Only [`Names`] knows the order
//! that keeps them from outliving the Link: no transaction is held across a DNS provider's call
//! (#2056), so each write checks the Server is still Linked before and after, and removes what it
//! wrote if not.

use std::{
    collections::BTreeSet,
    iter,
    net::{IpAddr, Ipv4Addr},
};

use reqwest::Url;
use sqlx::PgPool;

use super::{Error, LinkedServerId};
use crate::dns::{DnsError, Record, ServerNames};

/// The most certificate challenge values a Server may publish at once: one for its own name, one
/// for the wildcard under it.
pub(crate) const MAX_CHALLENGE_VALUES: usize = 2;

/// How long a DNS-01 challenge's value is: a SHA-256 digest in unpadded base64url (RFC 8555).
const CHALLENGE_VALUE_LENGTH: usize = 43;

/// Names Linked Servers in the Server-names zone, or names none without one.
#[derive(Clone)]
pub(super) struct Names {
    db: PgPool,
    server_names: Option<ServerNames>,
}

impl Names {
    /// Names Servers in `server_names`' zone, checking on `db` that they are still Linked, or
    /// names none without it.
    pub(super) fn new(db: PgPool, server_names: Option<ServerNames>) -> Self {
        Self { db, server_names }
    }

    /// `server`'s own name, which answers the addresses of the names under it, or none while the
    /// Hub names no Server.
    pub(super) fn own(&self, server: LinkedServerId) -> Option<OwnName> {
        let server_names = self.server_names.as_ref()?;
        Some(OwnName::new(server, &server_names.zone))
    }

    /// `server`'s own name, or [`Error::NamesOff`] while the Hub names no Server.
    pub(super) fn own_name(&self, server: LinkedServerId) -> Result<String, Error> {
        let own = self.own(server).ok_or(Error::NamesOff)?;
        Ok(own.0)
    }

    /// Makes the records of a report the only ones under `server`'s own name: the own name at
    /// `public`, the IP the Hub reached, each private IPv4 address in `lan_addresses` at a name
    /// carrying it, and the Relay's name at `relay`. Answers the names, its own first: none while
    /// the Hub names no Server, or once `server` is found Unlinked, whose names are then gone.
    pub(super) async fn publish_report(
        &self,
        server: LinkedServerId,
        public: Option<IpAddr>,
        lan_addresses: &[String],
        relay: Option<IpAddr>,
    ) -> Result<Vec<String>, Error> {
        let Some(server_names) = &self.server_names else {
            return Ok(Vec::new());
        };
        let own = OwnName::new(server, &server_names.zone);
        let records = own.records(public, lan_addresses, relay);
        let names = own.names(&records);
        let write = async { server_names.records.replace(&own.0, records).await };
        let linked = self.write_while_linked(server, write).await?;
        Ok(if linked { names } else { Vec::new() })
    }

    /// Makes `values` the only TXT records at `_acme-challenge.<own name>`, `server`'s DNS-01
    /// challenge for a certificate for its names, or removes them when it is empty. Refused as
    /// [`Error::ChallengeInvalid`] past [`MAX_CHALLENGE_VALUES`] or for a value that isn't one.
    pub(super) async fn publish_challenge(
        &self,
        server: LinkedServerId,
        values: Vec<String>,
    ) -> Result<(), Error> {
        if values.len() > MAX_CHALLENGE_VALUES || !values.iter().all(|value| is_challenge(value)) {
            return Err(Error::ChallengeInvalid);
        }
        let server_names = self.server_names.as_ref().ok_or(Error::NamesOff)?;
        let challenge = challenge_name(&OwnName::new(server, &server_names.zone));
        let write = async { server_names.records.replace_texts(&challenge, values).await };
        self.write_while_linked(server, write).await?;
        Ok(())
    }

    /// Unlinks `server` through `removal`, its names gone first: while they can't be removed,
    /// `removal` doesn't run and `server` stays Linked (ADR 0104). They are removed again after,
    /// for a publish that wrote them in between, and only logged if they can't be.
    pub(super) async fn unlink(
        &self,
        server: LinkedServerId,
        removal: impl Future<Output = Result<(), sqlx::Error>>,
    ) -> Result<(), Error> {
        self.forget(server).await?;
        removal.await?;
        if let Err(err) = self.forget(server).await {
            tracing::error!(%server, "an Unlinked Server's names may stay: {err}");
        }
        Ok(())
    }

    /// Removes the names of `servers`, which deleting their Account Unlinked. A failure is only
    /// logged, as the deletion can't wait for the DNS provider (ADR 0104).
    pub(super) async fn forget_deleted(&self, servers: impl IntoIterator<Item = LinkedServerId>) {
        for server in servers {
            if let Err(err) = self.forget(server).await {
                tracing::error!(%server, "the names of a deleted Account's Server stay: {err}");
            }
        }
    }

    /// Runs `write` only while `server` is Linked, and removes everything under its own name if it
    /// was Unlinked meanwhile, answering whether it still is. The check after the write is the one
    /// that keeps names from outliving the Link; the one before only spares a write for a Server
    /// Unlinked while its report's public port was probed.
    async fn write_while_linked(
        &self,
        server: LinkedServerId,
        write: impl Future<Output = Result<(), DnsError>>,
    ) -> Result<bool, Error> {
        if !self.is_linked(server).await? {
            return Ok(false);
        }
        write.await?;
        if !self.is_linked(server).await? {
            self.forget(server).await?;
            return Ok(false);
        }
        Ok(true)
    }

    /// Removes every record under `server`'s own name, its certificate challenge's included.
    async fn forget(&self, server: LinkedServerId) -> Result<(), Error> {
        let Some(server_names) = &self.server_names else {
            return Ok(());
        };
        let own = OwnName::new(server, &server_names.zone);
        server_names.records.replace(&own.0, Vec::new()).await?;
        (server_names.records)
            .replace_texts(&challenge_name(&own), Vec::new())
            .await?;
        Ok(())
    }

    async fn is_linked(&self, server: LinkedServerId) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT FROM linked_servers WHERE id = $1) AS "linked!""#,
            server.0,
        )
        .fetch_one(&self.db)
        .await
    }
}

/// A Linked Server's own name in the Server-names zone, which answers where the names the Hub
/// gives under it reach the Server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct OwnName(String);

impl OwnName {
    /// `server`'s own name in `zone`: its id in hex, which never changes.
    pub(super) fn new(server: LinkedServerId, zone: &str) -> Self {
        Self(format!("{}.{zone}", server.0.simple()))
    }

    /// Where clients reach the Server through its Relay listening on `port`, such as
    /// `https://relay.<own name>:443`. Only a name its certificate covers passes through the
    /// Relay, which is why a Hub naming no Server gives a Relay no address.
    pub(super) fn relay_address(&self, port: u16) -> String {
        format!("https://{}:{port}", self.relay_name())
    }

    /// Where clients reach the LAN address `lan` over HTTPS: at the name the Hub gives it, on its
    /// port, or none for one that gets no name.
    pub(super) fn lan_name_address(&self, lan: &str) -> Option<String> {
        let lan = Url::parse(lan).ok()?;
        let name = self.lan_name(lan_ip(&lan)?);
        Some(format!("https://{name}:{}", lan.port_or_known_default()?))
    }

    /// The records of a report, as [`Names::publish_report`] takes it.
    fn records(
        &self,
        public: Option<IpAddr>,
        lan_addresses: &[String],
        relay: Option<IpAddr>,
    ) -> Vec<Record> {
        // A set, as two LAN addresses may differ only in their port.
        let lan: BTreeSet<Ipv4Addr> = (lan_addresses.iter())
            .filter_map(|address| lan_ip(&Url::parse(address).ok()?))
            .collect();
        let own_record = public.map(|public| Record {
            name: self.0.clone(),
            address: public,
        });
        let lan_records = lan.into_iter().map(|ip| Record {
            name: self.lan_name(ip),
            address: IpAddr::V4(ip),
        });
        let relay_record = relay.map(|relay| Record {
            name: self.relay_name(),
            address: relay,
        });
        own_record
            .into_iter()
            .chain(lan_records)
            .chain(relay_record)
            .collect()
    }

    /// The names a Server is told for `records`, its own first, whether the Hub reached it or not.
    fn names(&self, records: &[Record]) -> Vec<String> {
        let under = (records.iter())
            .map(|record| record.name.clone())
            .filter(|name| *name != self.0);
        iter::once(self.0.clone()).chain(under).collect()
    }

    /// The name at which clients reach the Server through its Relay (ADR 0057). The certificate
    /// for the wildcard under the own name covers it, so the Relay passes TLS through.
    fn relay_name(&self) -> String {
        format!("relay.{}", self.0)
    }

    /// The name carrying `ip`, such as `192-168-1-10.<own name>`.
    fn lan_name(&self, ip: Ipv4Addr) -> String {
        format!("{}.{}", ip.to_string().replace('.', "-"), self.0)
    }
}

/// Where `own`, a Server's own name, holds the TXT records of the DNS-01 challenge for its
/// certificate (RFC 8555, section 8.4).
fn challenge_name(own: &OwnName) -> String {
    format!("_acme-challenge.{}", own.0)
}

/// The host of the LAN address `address`, if it is a private IPv4 address: a public one could
/// point a name at someone else's host.
// ponytail: IPv6 LAN addresses get no name; add unique local ones once Servers report them.
fn lan_ip(address: &Url) -> Option<Ipv4Addr> {
    let ip = address.host_str()?.parse::<Ipv4Addr>().ok()?;
    ip.is_private().then_some(ip)
}

/// Whether `value` is a DNS-01 challenge's value. Only base64url, so it can't break out of the TXT
/// record it is written into.
fn is_challenge(value: &str) -> bool {
    value.len() == CHALLENGE_VALUE_LENGTH
        && (value.bytes()).all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

#[cfg(test)]
#[path = "names_tests.rs"]
mod tests;
