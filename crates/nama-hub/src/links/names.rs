//! A Linked Server's names in the Server-names zone (ADR 0057, ADR 0104): whether the Hub names
//! Servers at all, a Server's own name, and every write to and removal of the records under it,
//! its certificate challenge's included. Only [`Names`] knows the order that keeps them from
//! outliving the Link: no transaction is held across a DNS provider's call (#2056), so each write
//! checks the Server is still Linked before and after, and removes what it wrote if not. Which
//! records go under the own name is the Server Addresses' (`server_addresses.rs`).

use std::future::Future;

use sqlx::PgPool;

use super::{Error, LinkedServerId, server_addresses::ServerAddresses};
use crate::dns::ServerNames;

/// The most certificate challenge values a Server may publish at once: one for its own name, one
/// for the wildcard under it.
pub(crate) const MAX_CHALLENGE_VALUES: usize = 2;

/// How long a DNS-01 challenge's value is: a SHA-256 digest in unpadded base64url (RFC 8555).
const CHALLENGE_VALUE_LENGTH: usize = 43;

/// Names Linked Servers in the Server-names zone, or names none without one.
#[derive(Clone)]
pub(super) struct Names {
    db: PgPool,
    zone: Option<ServerNames>,
}

impl Names {
    /// Names Servers in `zone`, checking on `db` that they are still Linked, or names none.
    pub(super) fn new(db: PgPool, zone: Option<ServerNames>) -> Self {
        Self { db, zone }
    }

    /// `server`'s own name, or [`Error::NamesOff`] while the Hub names no Server.
    pub(super) fn own_name(&self, server: LinkedServerId) -> Result<String, Error> {
        let zone = self.zone.as_ref().ok_or(Error::NamesOff)?;
        Ok(own_name(server, &zone.zone))
    }

    /// Makes the records of `addresses` the only ones under `server`'s own name, and answers the
    /// names it is told, its own first: none while the Hub names no Server, or once `server` is
    /// found Unlinked, whose names are then gone.
    pub(super) async fn publish_report(
        &self,
        server: LinkedServerId,
        addresses: &ServerAddresses<'_>,
    ) -> Result<Vec<String>, Error> {
        let Some(zone) = &self.zone else {
            return Ok(Vec::new());
        };
        if !self.is_linked(server).await? {
            return Ok(Vec::new());
        }
        let own = own_name(server, &zone.zone);
        zone.records.replace(&own, addresses.records()).await?;
        if !self.is_linked(server).await? {
            self.forget(server).await?;
            return Ok(Vec::new());
        }
        Ok(addresses.names())
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
        let zone = self.zone.as_ref().ok_or(Error::NamesOff)?;
        if !self.is_linked(server).await? {
            return Ok(());
        }
        let own = own_name(server, &zone.zone);
        (zone.records)
            .replace_texts(&challenge_name(&own), values)
            .await?;
        if !self.is_linked(server).await? {
            self.forget(server).await?;
        }
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
    pub(super) async fn forget_deleted(&self, servers: Vec<LinkedServerId>) {
        for server in servers {
            if let Err(err) = self.forget(server).await {
                tracing::error!(%server, "the names of a deleted Account's Server stay: {err}");
            }
        }
    }

    /// Removes every record under `server`'s own name, its certificate challenge's included.
    async fn forget(&self, server: LinkedServerId) -> Result<(), Error> {
        let Some(zone) = &self.zone else {
            return Ok(());
        };
        let own = own_name(server, &zone.zone);
        zone.records.replace(&own, Vec::new()).await?;
        (zone.records)
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

/// `server`'s own name in `zone`: its id in hex, which never changes.
fn own_name(server: LinkedServerId, zone: &str) -> String {
    format!("{}.{zone}", server.0.simple())
}

/// Where `own`, a Server's own name, holds the TXT records of the DNS-01 challenge for its
/// certificate (RFC 8555, section 8.4).
fn challenge_name(own: &str) -> String {
    format!("_acme-challenge.{own}")
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
