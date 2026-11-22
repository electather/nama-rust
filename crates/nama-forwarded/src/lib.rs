//! Where a request to the Server or the Hub came from. A reverse proxy's `X-Forwarded-For` and
//! `X-Forwarded-Proto` are believed only when the request's peer is one of the configured trusted
//! proxies (ADR 0018).

use std::{
    fmt,
    net::{IpAddr, SocketAddr},
    str::FromStr,
    sync::Arc,
};

use axum::{
    extract::{ConnectInfo, Request, State},
    http::HeaderMap,
    middleware::Next,
    response::Response,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Where a request came from, as far as the binary serving it trusts: what rate limits count by
/// and whether the session cookie can be `Secure`.
#[derive(Debug, Clone, Copy)]
pub struct RequestSource {
    /// The client's IP address.
    pub client_ip: IpAddr,
    /// Whether the client's request arrived over HTTPS.
    pub https: bool,
}

/// Adds the [`RequestSource`] to `request`'s extensions. A request served without its peer's
/// address gets none, so serve with `into_make_service_with_connect_info::<SocketAddr>()`.
pub async fn identify_source(
    State(trusted_proxies): State<Arc<[IpRange]>>,
    mut request: Request,
    next: Next,
) -> Response {
    let peer = request.extensions().get::<ConnectInfo<SocketAddr>>();
    if let Some(&ConnectInfo(peer)) = peer {
        let source = request_source(peer.ip(), request.headers(), &trusted_proxies);
        request.extensions_mut().insert(source);
    }
    next.run(request).await
}

/// Where a request from `peer` with `headers` came from. Only a peer in `trusted_proxies` can say,
/// in forwarded headers, that it relays someone else's request, or one that reached it over HTTPS.
fn request_source(peer: IpAddr, headers: &HeaderMap, trusted_proxies: &[IpRange]) -> RequestSource {
    // A dual-stack socket reports an IPv4 peer mapped into IPv6; it's counted as the IPv4 address.
    let peer = peer.to_canonical();
    let trusted = |ip: IpAddr| trusted_proxies.iter().any(|range| range.contains(ip));
    if !trusted(peer) {
        return RequestSource {
            client_ip: peer,
            https: false,
        };
    }
    RequestSource {
        client_ip: forwarded_client(peer, headers, trusted),
        https: forwarded_https(headers),
    }
}

/// The client a trusted `peer` relays for: the rightmost `X-Forwarded-For` hop that isn't itself a
/// trusted proxy. The hops left of it were written by the client, so could be anything. A hop that
/// isn't an IP address ends the walk at the last trusted one.
fn forwarded_client(peer: IpAddr, headers: &HeaderMap, trusted: impl Fn(IpAddr) -> bool) -> IpAddr {
    let hops = headers
        .get_all("x-forwarded-for")
        .iter()
        .rev()
        .flat_map(|value| value.to_str().unwrap_or_default().rsplit(','))
        .map_while(|hop| {
            hop.trim()
                .parse::<IpAddr>()
                .ok()
                .map(|hop| hop.to_canonical())
        });
    let mut client = peer;
    for hop in hops {
        client = hop;
        if !trusted(hop) {
            break;
        }
    }
    client
}

/// Whether a trusted proxy says the request reached it over HTTPS. Only the last
/// `X-Forwarded-Proto` value counts: that is the one the proxy in front of the Server set.
fn forwarded_https(headers: &HeaderMap) -> bool {
    headers
        .get_all("x-forwarded-proto")
        .iter()
        .next_back()
        .and_then(|value| value.to_str().ok()?.rsplit(',').next())
        .is_some_and(|proto| proto.trim().eq_ignore_ascii_case("https"))
}

/// An IP address, or a CIDR range such as `10.0.0.0/8`, written as such in the config.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "String", into = "String")]
#[schemars(inline)]
pub struct IpRange {
    network: IpAddr,
    prefix_len: u8,
}

impl IpRange {
    /// Whether `ip` is in the range. An IPv4 address mapped into IPv6, as a dual-stack socket
    /// reports an IPv4 peer, counts as that IPv4 address.
    #[must_use]
    pub fn contains(&self, ip: IpAddr) -> bool {
        let (network, width) = bits(self.network);
        let (ip, ip_width) = bits(ip.to_canonical());
        let host_bits = u32::from(width - self.prefix_len);
        width == ip_width && (network ^ ip).checked_shr(host_bits).unwrap_or(0) == 0
    }
}

/// `ip`'s bits, right-aligned, and how many there are.
fn bits(ip: IpAddr) -> (u128, u8) {
    match ip {
        IpAddr::V4(ip) => (u32::from(ip).into(), 32),
        IpAddr::V6(ip) => (ip.into(), 128),
    }
}

/// A trusted proxy that isn't an IP address or a CIDR range.
#[derive(Debug, thiserror::Error)]
#[error("`{0}` is not an IP address or a CIDR range")]
pub struct InvalidIpRange(String);

impl FromStr for IpRange {
    type Err = InvalidIpRange;

    fn from_str(range: &str) -> Result<Self, Self::Err> {
        let invalid = || InvalidIpRange(range.to_owned());
        let (network, prefix_len) = range
            .split_once('/')
            .map_or((range, None), |(network, len)| (network, Some(len)));
        let network = network.parse::<IpAddr>().ok().ok_or_else(invalid)?;
        let width = bits(network).1;
        let prefix_len = match prefix_len {
            None => width,
            Some(len) => len
                .parse()
                .ok()
                .filter(|&len| len <= width)
                .ok_or_else(invalid)?,
        };
        // `contains` compares an IPv4 address mapped into IPv6 as that IPv4 address, so a range
        // within the mapped block becomes the IPv4 range it maps, its prefix less the 96 bits
        // of the mapping.
        let canonical = network.to_canonical();
        Ok(match prefix_len.checked_sub(96) {
            Some(prefix_len) if canonical != network => Self {
                network: canonical,
                prefix_len,
            },
            _ => Self {
                network,
                prefix_len,
            },
        })
    }
}

impl TryFrom<String> for IpRange {
    type Error = InvalidIpRange;

    fn try_from(range: String) -> Result<Self, Self::Error> {
        range.parse()
    }
}

impl From<IpRange> for String {
    fn from(range: IpRange) -> Self {
        range.to_string()
    }
}

impl fmt::Display for IpRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.network, self.prefix_len)
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
