//! The Server's Server Addresses (GLOSSARY.md), which the enabled Plugin that links the Server,
//! such as the Hub Plugin, is told so the service can hand them to clients (ADR 0094), and the
//! port it opens on the router for clients outside the LAN while that Plugin has it Linked
//! (ADR 0057).

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    num::NonZeroU16,
    time::Duration,
};

use nama_plugin_api::server_link::ServerAddresses;
use tokio::{sync::watch, task::JoinHandle};
use url::Url;

use crate::{link::Linking, port_mapping, server_settings::ServerUrl};

/// How often the Plugin is told, so it hears of a new LAN address, or tells a service it was just
/// enabled or Linked to, within this long. It passes on only a change.
const EVERY: Duration = Duration::from_mins(1);

/// How many times the Plugin is told between opening the port on the router again: half its
/// [`port_mapping::LEASE`], so it never closes while the Server runs. Saving the Server URL tells it
/// between minutes too, which only opens the port again sooner.
const OPEN_PORT_EVERY: u64 = port_mapping::LEASE.as_secs() / 2 / EVERY.as_secs();

/// Tells the enabled Plugin that links the Server its Server Addresses now, every minute, and as
/// soon as `url_saves` wakes, since the service refuses a sign-in returning anywhere else: the
/// Server URL `url` holds, the LAN address of `listening`, and the public port: `forwarded`,
/// the one the Admin forwarded, or else one the Server opens on the router, only while such a
/// Plugin has it Linked. Runs until the task is aborted; `linking` asks in the Plugin's scope, so
/// disabling it or shutting down cancels and joins the call. A Plugin that fails shows it on its
/// page.
pub(crate) fn keep_telling(
    linking: Linking,
    url: ServerUrl,
    mut url_saves: watch::Receiver<()>,
    listening: SocketAddr,
    forwarded: Option<NonZeroU16>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticks = tokio::time::interval(EVERY);
        let mut opened = OpenedPort::default();
        loop {
            tokio::select! {
                _ = ticks.tick() => {}
                // Err once the settings are dropped: the arm sits out and the minute still ticks.
                Ok(()) = url_saves.changed() => {}
            }
            let Some(plugin) = linking.linking_plugin() else {
                // So enabling it again opens the port at once, whose lease may have run out since.
                opened = OpenedPort::default();
                continue;
            };
            let lan = lan_address(listening);
            let public_port = match forwarded {
                Some(port) => Some(port.get()),
                None if linking.is_server_linked_by(&plugin).await => opened.port(lan).await,
                None => {
                    // Shut while the Server isn't Linked, and opened at once when it is.
                    opened = OpenedPort::default();
                    None
                }
            };
            (linking.tell_addresses(&plugin, addresses(&url, lan, public_port))).await;
        }
    })
}

/// Whether `address` has the scheme, host and port of one of the Server Addresses the Plugin is
/// told but the public one: the Server URL `url` holds, or the LAN address of `listening`. The Hub
/// sends a sign-in with an Account back only to those, as `nama-hub`'s `is_on_server` checks.
pub(crate) fn is_reported(address: &str, url: &ServerUrl, listening: SocketAddr) -> bool {
    let Ok(address) = Url::parse(address) else {
        return false;
    };
    let ServerAddresses { url, lan, .. } = addresses(url, lan_address(listening), None);
    std::iter::once(url).chain(lan).any(|reported| {
        Url::parse(&reported).is_ok_and(|reported| reported.origin() == address.origin())
    })
}

/// The Server Addresses the Plugin is told: the Server URL `url` holds, the LAN address `lan`, and
/// the public port.
fn addresses(
    url: &ServerUrl,
    lan: Option<SocketAddr>,
    public_port: Option<u16>,
) -> ServerAddresses {
    ServerAddresses {
        url: as_sent(&url.url()),
        lan: lan.map(|lan| format!("http://{lan}")).into_iter().collect(),
        public_port,
    }
}

/// The port opened on the router, kept between tellings.
#[derive(Default)]
struct OpenedPort {
    /// The port open on the router's public IP address, if it opened one.
    port: Option<u16>,
    /// Tellings left until the port is opened again; 0 opens it on the next.
    tellings_left: u64,
}

impl OpenedPort {
    /// The port open on the router to the Server at its LAN address `lan`, opened again every
    /// [`OPEN_PORT_EVERY`] calls. NAT-PMP and UPnP's port forwarding are IPv4 only.
    async fn port(&mut self, lan: Option<SocketAddr>) -> Option<u16> {
        if self.tellings_left == 0 {
            self.port = match lan {
                Some(SocketAddr::V4(lan)) => port_mapping::open(lan).await,
                _ => None,
            };
            self.tellings_left = OPEN_PORT_EVERY;
        }
        self.tellings_left -= 1;
        self.port
    }
}

/// The Server URL `url` as `url` serialises it, or empty for none: ASCII throughout, so the service,
/// which may parse URLs more strictly than the Server, takes every one the Server does.
fn as_sent(url: &str) -> String {
    Url::parse(url).map_or_else(|_none| String::new(), String::from)
}

/// Where other devices on the LAN reach the Server listening on `listening`: that address, or,
/// listening on every address, the one its traffic to the internet leaves from, with its port.
/// `None` on loopback.
// ponytail: one IPv4 address, the default route's, which in a Docker bridge network is the
// container's and unreachable from the LAN; list every interface's, or let the Admin name one,
// once a Server needs it (#1831 ranks and checks what clients can reach).
fn lan_address(listening: SocketAddr) -> Option<SocketAddr> {
    let ip = if listening.ip().is_unspecified() {
        default_route_address()?
    } else {
        listening.ip()
    };
    (!ip.is_loopback()).then(|| SocketAddr::new(ip, listening.port()))
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
