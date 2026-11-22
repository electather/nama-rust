//! The Server's Server Addresses (GLOSSARY.md), which the enabled Plugin that links the Server,
//! such as the Hub Plugin, is told so the service can hand them to clients (ADR 0094).

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    time::Duration,
};

use nama_plugin_api::server_link::{ServerAddresses, ServerLinker};
use tokio::task::JoinHandle;

use crate::{plugins::ExtensionPoints, server_settings::ServerUrl};

/// How often the Plugin is told, so it hears of a new Server URL or LAN address, or tells a service
/// it was just enabled or Linked to, within this long. It passes on only a change.
const EVERY: Duration = Duration::from_mins(1);

/// Tells the enabled Plugin that links the Server its Server Addresses now and every minute: the
/// Server URL `url` holds, and the LAN address of `listening`. Runs until the task is aborted. A
/// Plugin that fails shows it on its page.
pub(crate) fn tell_every_minute(
    points: ExtensionPoints,
    url: ServerUrl,
    listening: SocketAddr,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticks = tokio::time::interval(EVERY);
        loop {
            ticks.tick().await;
            let addresses = ServerAddresses {
                url: url.url(),
                lan: lan_address(listening).into_iter().collect(),
            };
            points
                .ask_first::<dyn ServerLinker, _, _, _, _>(async |linker| {
                    linker.set_addresses(addresses.clone()).await
                })
                .await;
        }
    })
}

/// Where other devices on the LAN reach the Server listening on `listening`: that address, or,
/// listening on every address, the one its traffic to the internet leaves from. `None` on loopback.
// shortcut: one IPv4 address, the default route's; list every interface's once a Server on several
// networks needs them all.
fn lan_address(listening: SocketAddr) -> Option<String> {
    let ip = if listening.ip().is_unspecified() {
        default_route_address()?
    } else {
        listening.ip()
    };
    (!ip.is_loopback()).then(|| format!("http://{}", SocketAddr::new(ip, listening.port())))
}

/// The address the Server's traffic to the internet leaves from, if it has a route there.
fn default_route_address() -> Option<IpAddr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    // Connecting a UDP socket sends nothing: it only picks the route, and with it the address it
    // leaves from. 192.0.2.1 is for documentation (RFC 5737), so it is never on the LAN itself.
    socket.connect((Ipv4Addr::new(192, 0, 2, 1), 9)).ok()?;
    Some(socket.local_addr().ok()?.ip())
}

#[cfg(test)]
#[path = "server_addresses_tests.rs"]
mod tests;
