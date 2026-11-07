//! Where a request came from. A reverse proxy's `X-Forwarded-For` and `X-Forwarded-Proto` are
//! believed only when the request's peer is one of the configured trusted proxies (ADR 0018).

use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
};

use axum::{
    extract::{ConnectInfo, Request, State},
    http::HeaderMap,
    middleware::Next,
    response::Response,
};

use crate::config::IpRange;

/// Where a request came from, as far as the Server trusts: what rate limits count by and whether
/// the session cookie can be `Secure`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RequestSource {
    /// The client's IP address.
    pub(crate) client_ip: IpAddr,
    /// Whether the client's request arrived over HTTPS.
    pub(crate) https: bool,
}

/// Adds the [`RequestSource`] to `request`'s extensions. A request served without its peer's
/// address gets none.
pub(crate) async fn identify_source(
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
    let trusted = |ip: IpAddr| trusted_proxies.iter().any(|proxies| proxies.contains(ip));
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
        .map_while(|hop| hop.trim().parse::<IpAddr>().ok());
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
