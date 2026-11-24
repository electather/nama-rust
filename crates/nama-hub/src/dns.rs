//! The zone Linked Servers are named in, which a DNS provider serves and the Hub writes through its
//! API (ADR 0104): a true external, so a trait with the Cloudflare adapter here and a fake in the
//! tests (ADR 0099).

use std::{fmt, future::Future, net::IpAddr, pin::Pin, sync::Arc, time::Duration};

use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

/// The zone Linked Servers are named in, and what writes its records.
#[derive(Clone)]
pub struct ServerNames {
    /// The zone, such as `namadirect.net`, in lower case.
    pub zone: String,
    /// What writes its records.
    pub records: Arc<dyn Records>,
}

impl fmt::Debug for ServerNames {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServerNames")
            .field("zone", &self.zone)
            .finish_non_exhaustive()
    }
}

/// A name and the IP address it resolves to: an A record for IPv4, AAAA for IPv6.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Record {
    /// The full name, such as `192-168-1-10.abc.namadirect.net`.
    pub name: String,
    /// Where it points.
    pub address: IpAddr,
}

/// Records on their way to the zone.
pub type Writing<'a> = Pin<Box<dyn Future<Output = Result<(), DnsError>> + Send + 'a>>;

/// Something that writes a zone's address records.
pub trait Records: Send + Sync {
    /// Makes `records` the only A and AAAA records at `name` and the names under it, removing
    /// them all when it is empty. Records of other types stay, such as an ACME challenge's TXT.
    fn replace(&self, name: &str, records: Vec<Record>) -> Writing<'_>;

    /// Makes `values` the only TXT records at `name` itself, removing them all when it is empty.
    fn replace_texts(&self, name: &str, values: Vec<String>) -> Writing<'_>;
}

/// Why records weren't written.
#[derive(Debug, thiserror::Error)]
pub enum DnsError {
    /// The DNS provider couldn't be reached, or didn't answer in time.
    #[error("could not reach the DNS provider: {0}")]
    Request(#[from] reqwest::Error),
    /// The DNS provider answered with an error.
    #[error("the DNS provider refused the records with {status}: {body}")]
    Refused {
        /// Its HTTP status.
        status: StatusCode,
        /// What it said.
        body: String,
    },
    /// The HTTPS client couldn't be set up.
    #[error("could not set up TLS: {0}")]
    Tls(#[from] rustls::Error),
}

/// Writes records through Cloudflare's API.
pub struct Cloudflare {
    client: reqwest::Client,
    /// The zone's DNS records under [`CLOUDFLARE_URL`], or a stand-in's in tests.
    url: String,
    api_token: String,
}

/// Cloudflare's API.
const CLOUDFLARE_URL: &str = "https://api.cloudflare.com/client/v4";

/// How long one call may take before it counts as failed.
const CALL_TIMEOUT: Duration = Duration::from_secs(30);

/// How long resolvers keep a record, short so a new public IP address takes over soon.
const TTL_SECS: u32 = 60;

impl Cloudflare {
    /// Writes with the API token `api_token`, which may edit the DNS of the zone whose id is
    /// `zone_id`.
    ///
    /// # Errors
    ///
    /// If the HTTPS client can't be set up.
    pub fn new(api_token: String, zone_id: &str) -> Result<Self, DnsError> {
        let client = crate::https::client()?.timeout(CALL_TIMEOUT).build()?;
        Ok(Self {
            client,
            url: format!("{CLOUDFLARE_URL}/zones/{zone_id}/dns_records"),
            api_token,
        })
    }

    /// The address records at `name` and under it.
    async fn listed(&self, name: &str) -> Result<Vec<Listed>, DnsError> {
        // A Server's names hold a few dozen records at most, well within one page. A name holds
        // only letters, digits, dots and hyphens, so it needs no escaping.
        let response = self
            .client
            .get(format!("{}?name.endswith={name}&per_page=1000", self.url))
            .bearer_auth(&self.api_token)
            .send()
            .await?;
        let listing: Listing = succeeded(response).await?.json().await?;
        let under = format!(".{name}");
        Ok(listing
            .result
            .into_iter()
            .filter(|listed| matches!(listed.kind.as_str(), "A" | "AAAA"))
            // The filter also matches a name that merely ends the same, such as `x<name>`.
            .filter(|listed| listed.name == name || listed.name.ends_with(&under))
            .collect())
    }

    /// The TXT records at `name` itself.
    async fn listed_texts(&self, name: &str) -> Result<Vec<Listed>, DnsError> {
        let response = self
            .client
            .get(format!("{}?type=TXT&name.exact={name}", self.url))
            .bearer_auth(&self.api_token)
            .send()
            .await?;
        let listing: Listing = succeeded(response).await?.json().await?;
        Ok(listing.result)
    }

    /// Applies `batch`, unless it changes nothing.
    async fn apply(&self, batch: Batch<'_>) -> Result<(), DnsError> {
        if batch.deletes.is_empty() && batch.posts.is_empty() {
            return Ok(());
        }
        let response = self
            .client
            .post(format!("{}/batch", self.url))
            .bearer_auth(&self.api_token)
            .json(&batch)
            .send()
            .await?;
        succeeded(response).await?;
        Ok(())
    }
}

/// `response`, if it succeeded.
async fn succeeded(response: reqwest::Response) -> Result<reqwest::Response, DnsError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let body = response.text().await.unwrap_or_default();
    Err(DnsError::Refused { status, body })
}

/// What Cloudflare's list of DNS records answers.
#[derive(Deserialize)]
struct Listing {
    result: Vec<Listed>,
}

/// One DNS record Cloudflare lists.
#[derive(Deserialize)]
struct Listed {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    name: String,
    content: String,
}

impl Listed {
    fn is(&self, record: &Record) -> bool {
        self.kind == record_type(record.address)
            && self.name == record.name
            && self.content == record.address.to_string()
    }
}

/// The body of Cloudflare's batch of DNS record changes, which it applies all or none of.
#[derive(Serialize)]
struct Batch<'a> {
    deletes: Vec<Deleted<'a>>,
    posts: Vec<Posted<'a>>,
}

#[derive(Serialize)]
struct Deleted<'a> {
    id: &'a str,
}

#[derive(Serialize)]
struct Posted<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    name: &'a str,
    content: String,
    ttl: u32,
    /// Off, so a name resolves to the Server itself rather than to Cloudflare's proxy.
    proxied: bool,
}

fn record_type(address: IpAddr) -> &'static str {
    if address.is_ipv4() { "A" } else { "AAAA" }
}

impl Records for Cloudflare {
    fn replace(&self, name: &str, records: Vec<Record>) -> Writing<'_> {
        let name = name.to_owned();
        Box::pin(async move {
            let listed = self.listed(&name).await?;
            let deletes: Vec<Deleted<'_>> = listed
                .iter()
                .filter(|listed| !records.iter().any(|record| listed.is(record)))
                .map(|listed| Deleted { id: &listed.id })
                .collect();
            let posts: Vec<Posted<'_>> = records
                .iter()
                .filter(|record| !listed.iter().any(|listed| listed.is(record)))
                .map(|record| Posted {
                    kind: record_type(record.address),
                    name: &record.name,
                    content: record.address.to_string(),
                    ttl: TTL_SECS,
                    proxied: false,
                })
                .collect();
            self.apply(Batch { deletes, posts }).await
        })
    }

    fn replace_texts(&self, name: &str, values: Vec<String>) -> Writing<'_> {
        let name = name.to_owned();
        Box::pin(async move {
            let listed = self.listed_texts(&name).await?;
            let deletes = (listed.iter())
                .map(|listed| Deleted { id: &listed.id })
                .collect();
            let posts = (values.into_iter())
                .map(|value| Posted {
                    kind: "TXT",
                    name: &name,
                    content: value,
                    ttl: TTL_SECS,
                    proxied: false,
                })
                .collect();
            self.apply(Batch { deletes, posts }).await
        })
    }
}

// The API token is a secret, so it is never printed.
impl fmt::Debug for Cloudflare {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cloudflare")
            .field("url", &self.url)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[path = "dns_tests.rs"]
mod tests;
