//! Opening the Server's port on the LAN's router with UPnP or NAT-PMP, so clients outside the LAN
//! reach a Linked Server directly (ADR 0057). The router closes it once its lease runs out, so the
//! Server opens it again well before, and a Server that stopped leaves nothing open for long.

use std::{
    error::Error,
    net::{Ipv4Addr, SocketAddrV4},
    time::Duration,
};

use tokio::{net::UdpSocket, time::timeout};
use url::Url;

/// How long the router keeps the port open unless the Server opens it again.
pub(crate) const LEASE: Duration = Duration::from_hours(1);

/// How long each step waits for the router to answer.
const ANSWER_WITHIN: Duration = Duration::from_secs(3);

/// Where UPnP devices listen for searches (SSDP).
const SSDP: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(239, 255, 255, 250), 1900);

/// A search for routers that open ports, an Internet Gateway Device.
const SEARCH: &str = "M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\n\
                      MAN: \"ssdp:discover\"\r\nMX: 2\r\n\
                      ST: urn:schemas-upnp-org:device:InternetGatewayDevice:1\r\n\r\n";

/// The port NAT-PMP routers listen on (RFC 6886).
const NAT_PMP_PORT: u16 = 5351;

type Failed = Box<dyn Error + Send + Sync>;

/// Opens the port `server` listens on, under the same number, on the router's public IP address,
/// with UPnP or else NAT-PMP, for [`LEASE`]. `None` when the router does neither or refuses, as for
/// a port another device has.
// shortcut: asks only for the Server's own port number; ask for another once routers refuse it.
pub(crate) async fn open(server: SocketAddrV4) -> Option<u16> {
    let upnp = match open_with_upnp(server).await {
        Ok(port) => return Some(port),
        Err(failed) => failed,
    };
    match open_with_nat_pmp(server).await {
        Ok(port) => Some(port),
        Err(nat_pmp) => {
            tracing::debug!("the router opened no port: UPnP: {upnp}; NAT-PMP: {nat_pmp}");
            None
        }
    }
}

async fn open_with_upnp(server: SocketAddrV4) -> Result<u16, Failed> {
    let description = find_router(*server.ip()).await?;
    add_port_mapping(&description, server).await
}

/// The description of the first router on the LAN to answer a search, sent from `lan_ip`.
async fn find_router(lan_ip: Ipv4Addr) -> Result<Url, Failed> {
    let socket = UdpSocket::bind((lan_ip, 0)).await?;
    socket.send_to(SEARCH.as_bytes(), SSDP).await?;
    let mut answer = [0; 2048];
    let (length, from) = timeout(ANSWER_WITHIN, socket.recv_from(&mut answer)).await??;
    let location = location(std::str::from_utf8(
        answer.get(..length).unwrap_or_default(),
    )?)
    .ok_or("the router's answer names no description")?;
    let description = Url::parse(location)?;
    // So another device answering can't have the Server send requests anywhere else.
    if description.host_str() != Some(&from.ip().to_string()) {
        return Err("the router's description is on another host".into());
    }
    Ok(description)
}

/// Asks the router described at `description` to forward its port `server.port()` to `server`.
async fn add_port_mapping(description: &Url, server: SocketAddrV4) -> Result<u16, Failed> {
    let client = reqwest::Client::builder().timeout(ANSWER_WITHIN).build()?;
    let described = client.get(description.clone()).send().await?;
    let (control, service) =
        wan_connection(&described.error_for_status()?.text().await?, description)
            .ok_or("the router offers no WAN connection")?;
    client
        .post(control)
        .header("SOAPAction", format!("\"{service}#AddPortMapping\""))
        .header("Content-Type", "text/xml; charset=\"utf-8\"")
        .body(port_mapping_request(&service, server))
        .send()
        .await?
        .error_for_status()?;
    Ok(server.port())
}

/// The value of an SSDP answer's `LOCATION` header: where the device's description is.
fn location(answer: &str) -> Option<&str> {
    answer.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.trim()
            .eq_ignore_ascii_case("location")
            .then(|| value.trim())
    })
}

/// The control URL and type of the service in the router's `description`, found at `location`,
/// that opens ports.
fn wan_connection(description: &str, location: &Url) -> Option<(Url, String)> {
    let document = roxmltree::Document::parse(description).ok()?;
    document
        .descendants()
        .filter(|node| node.has_tag_name("service"))
        .find_map(|service| {
            let service_type = child_text(service, "serviceType")?;
            let version = service_type
                .strip_prefix("urn:schemas-upnp-org:service:WANIPConnection:")
                .or_else(|| {
                    service_type.strip_prefix("urn:schemas-upnp-org:service:WANPPPConnection:")
                })?;
            // So the type is safe to send back inside the request's XML.
            if version.is_empty() || !version.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            let control = location.join(child_text(service, "controlURL")?).ok()?;
            Some((control, service_type.to_owned()))
        })
}

fn child_text<'a>(node: roxmltree::Node<'a, '_>, name: &str) -> Option<&'a str> {
    let child = node.children().find(|child| child.has_tag_name(name))?;
    child.text().map(str::trim)
}

/// The SOAP request through which `service` forwards `server`'s port to it for [`LEASE`].
fn port_mapping_request(service: &str, server: SocketAddrV4) -> String {
    let (ip, port, lease) = (server.ip(), server.port(), LEASE.as_secs());
    format!(
        "<?xml version=\"1.0\"?>\
         <s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" \
         s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\"><s:Body>\
         <u:AddPortMapping xmlns:u=\"{service}\"><NewRemoteHost></NewRemoteHost>\
         <NewExternalPort>{port}</NewExternalPort><NewProtocol>TCP</NewProtocol>\
         <NewInternalPort>{port}</NewInternalPort><NewInternalClient>{ip}</NewInternalClient>\
         <NewEnabled>1</NewEnabled><NewPortMappingDescription>Nama</NewPortMappingDescription>\
         <NewLeaseDuration>{lease}</NewLeaseDuration></u:AddPortMapping></s:Body></s:Envelope>"
    )
}

async fn open_with_nat_pmp(server: SocketAddrV4) -> Result<u16, Failed> {
    // shortcut: reads only Linux's routing table, so elsewhere a Server opens its port with UPnP
    // alone; read the others' once a Server there needs NAT-PMP.
    let routes = tokio::fs::read_to_string("/proc/net/route").await?;
    let gateway = default_gateway(&routes).ok_or("no default route")?;
    let socket = UdpSocket::bind((*server.ip(), 0)).await?;
    socket.connect((gateway, NAT_PMP_PORT)).await?;
    socket.send(&nat_pmp_request(server.port())).await?;
    let mut answer = [0; 16];
    let length = timeout(ANSWER_WITHIN, socket.recv(&mut answer)).await??;
    Ok(nat_pmp_mapped(answer.get(..length).unwrap_or_default()).ok_or("the router refused")?)
}

/// The gateway of the default route in Linux's `/proc/net/route`.
fn default_gateway(routes: &str) -> Option<Ipv4Addr> {
    routes.lines().skip(1).find_map(|route| {
        let mut fields = route.split_whitespace().skip(1);
        let (destination, gateway) = (fields.next()?, fields.next()?);
        let gateway = u32::from_str_radix(gateway, 16).ok()?;
        // The table prints each address as a number in the host's byte order.
        (destination == "00000000" && gateway != 0).then(|| Ipv4Addr::from(gateway.to_ne_bytes()))
    })
}

/// A NAT-PMP request forwarding the router's TCP port `port` to the same port of the Server
/// sending it, for [`LEASE`] (RFC 6886, 3.3).
fn nat_pmp_request(port: u16) -> [u8; 12] {
    let lease = u32::try_from(LEASE.as_secs()).unwrap_or(u32::MAX);
    let mut request = [0; 12];
    // Version 0, then opcode 2: map a TCP port.
    request[1] = 2;
    request[4..6].copy_from_slice(&port.to_be_bytes());
    request[6..8].copy_from_slice(&port.to_be_bytes());
    request[8..12].copy_from_slice(&lease.to_be_bytes());
    request
}

/// The public port a NAT-PMP answer to [`nat_pmp_request`] forwards, if it succeeded.
fn nat_pmp_mapped(answer: &[u8]) -> Option<u16> {
    match answer {
        // Version 0, opcode 130 (2's answer), result 0 (success), then the epoch, the Server's
        // port and the public one.
        [0, 130, 0, 0, _, _, _, _, _, _, high, low, ..] => {
            Some(u16::from_be_bytes([*high, *low])).filter(|&port| port != 0)
        }
        _ => None,
    }
}

#[cfg(test)]
#[path = "port_mapping_tests.rs"]
mod tests;
