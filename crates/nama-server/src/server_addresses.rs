//! The Server's Server Addresses (GLOSSARY.md), which the enabled Plugin that links the Server,
//! such as the Hub Plugin, is told so the service can hand them to clients (ADR 0094), and the
//! port it opens on the router for clients outside the LAN while that Plugin has it Linked
//! (ADR 0057).

use std::{
    future::Future,
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    num::NonZeroU16,
    sync::Arc,
    time::Duration,
};

use nama_plugin_api::{
    PluginError, PluginId,
    server_link::{Link, ServerAddresses, ServerLinker},
};
use tokio::{sync::oneshot, task::JoinHandle};
use url::Url;

use crate::{plugins::ExtensionPoints, port_mapping, server_settings::ServerUrl};

/// How often the Plugin is told, so it hears of a new Server URL or LAN address, or tells a service
/// it was just enabled or Linked to, within this long. It passes on only a change.
const EVERY: Duration = Duration::from_mins(1);

/// How many times the Plugin is told between opening the port on the router again: half its
/// [`port_mapping::LEASE`], so it never closes while the Server runs.
const OPEN_PORT_EVERY: u64 = port_mapping::LEASE.as_secs() / 2 / EVERY.as_secs();

/// Tells the enabled Plugin that links the Server its Server Addresses now and every minute: the
/// Server URL `url` holds, the LAN address of `listening`, and the public port: `forwarded`, the
/// one the Admin forwarded, or else one the Server opens on the router, only while such a Plugin
/// has it Linked. Runs until the task is aborted; each call runs in the Plugin's scope, so
/// disabling it or shutting down cancels and joins the call. A Plugin that fails shows it on its
/// page.
pub(crate) fn tell_every_minute(
    points: ExtensionPoints,
    url: ServerUrl,
    listening: SocketAddr,
    forwarded: Option<NonZeroU16>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticks = tokio::time::interval(EVERY);
        let mut opened = OpenedPort::default();
        loop {
            ticks.tick().await;
            let Some(plugin) = points.ranked::<dyn ServerLinker>().into_iter().next() else {
                // So enabling it again opens the port at once, whose lease may have run out since.
                opened = OpenedPort::default();
                continue;
            };
            let lan = lan_address(listening);
            let public_port = match forwarded {
                Some(port) => Some(port.get()),
                None if linked(&points, &plugin).await => opened.port(lan).await,
                None => {
                    // Shut while the Server isn't Linked, and opened at once when it is.
                    opened = OpenedPort::default();
                    None
                }
            };
            tell(&points, &plugin, addresses(&url, lan, public_port)).await;
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

/// The port opened on the router, kept between ticks.
#[derive(Default)]
struct OpenedPort {
    /// The port open on the router's public IP address, if it opened one.
    port: Option<u16>,
    /// Ticks left until the port is opened again; 0 opens it on the next.
    ticks_left: u64,
}

impl OpenedPort {
    /// The port open on the router to the Server at its LAN address `lan`, opened again every
    /// [`OPEN_PORT_EVERY`] calls. NAT-PMP and UPnP's port forwarding are IPv4 only.
    async fn port(&mut self, lan: Option<SocketAddr>) -> Option<u16> {
        if self.ticks_left == 0 {
            self.port = match lan {
                Some(SocketAddr::V4(lan)) => port_mapping::open(lan).await,
                _ => None,
            };
            self.ticks_left = OPEN_PORT_EVERY;
        }
        self.ticks_left -= 1;
        self.port
    }
}

/// Tells `plugin` `addresses`, and waits until it answered or its scope was cancelled.
async fn tell(points: &ExtensionPoints, plugin: &PluginId, addresses: ServerAddresses) {
    ask_in_scope(points, plugin, move |linker| {
        let addresses = addresses.clone();
        async move { linker.set_addresses(addresses).await }
    })
    .await;
}

/// Whether `plugin` has the Server Linked. Not, if it fails to say.
async fn linked(points: &ExtensionPoints, plugin: &PluginId) -> bool {
    let link = ask_in_scope(points, plugin, |linker| async move { linker.link().await }).await;
    matches!(link, Some(Link::Linked { .. }))
}

/// What `call` gets from `plugin`, the first enabled one that links the Server, asked in its
/// scope. `None` once it failed, didn't answer within [`EVERY`], or its scope was cancelled.
async fn ask_in_scope<R, F>(
    points: &ExtensionPoints,
    plugin: &PluginId,
    call: impl Fn(Arc<dyn ServerLinker>) -> F + Send + Sync + 'static,
) -> Option<R>
where
    R: Send + 'static,
    F: Future<Output = Result<R, PluginError>> + Send,
{
    let (asking, asked) = (points.clone(), plugin.clone());
    let (told, answered) = oneshot::channel();
    let spawned = points.spawn_in(plugin, async move {
        let answer = asking
            .ask(&asked, |linker: Arc<dyn ServerLinker>| {
                // Bounded, so a Plugin that never answers can't stop every later call.
                let calling = tokio::time::timeout(EVERY, call(linker));
                async move {
                    calling
                        .await
                        .unwrap_or_else(|_elapsed| Err("it didn't answer in time".into()))
                }
            })
            .await;
        let _: Result<(), _> = told.send(answer.and_then(|answer| answer.result.ok()));
    });
    if !spawned {
        return None;
    }
    // A cancelled scope drops the sender, which ends the wait as well.
    answered.await.ok().flatten()
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
