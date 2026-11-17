# Servers get per-Server certificates through the Hub; Relays are run by Admins

People choose Plex over self-hosted servers mostly for easy remote access (#928). TVs can't run a VPN, CGNAT stops port forwarding, and browsers block a Hub page from calling a plain-HTTP Server (ADR 0010). So the project runs a DNS zone on a domain used only for Server names, and each Linked Server gets its own names there with a wildcard certificate. The Server gets that certificate through an ACME DNS-01 challenge, with the Hub publishing the TXT record for it, so the Server holds its own private key and the Hub can't read its traffic. A name can carry a LAN IP in its label, the way `*.plex.direct` does, so a browser at home also gets HTTPS. Clients connect straight to the Server when they can, using a port the Server opens with UPnP or NAT-PMP or one the Admin forwards, and the Hub probes whether that port can be reached from outside. A Relay is only the fallback, and the Admin runs it for now. It passes TLS through without decrypting it and routes on SNI. The Server dials out to the Relay and has to show a Hub-signed token (ADR 0011). A Server that isn't Linked keeps working over plain HTTP or behind the Admin's own proxy (ADR 0002).

## Considered Options

- **Documented reverse proxy, Tailscale or tunnel only**: rejected as the end state. Behind CGNAT and on TVs it doesn't work, and it stays a weekend project for the Admin. It ships first anyway, as a one-page recipe.
- **A Relay that the project runs and that carries all remote traffic**: rejected. The project would pay for every remote stream's bandwidth, and the Relay would become a party the user has to trust.
- **Hole punching**: rejected. Browsers can only do it through WebRTC.

## Consequences

- The domain has to be on the Public Suffix List before M5 ships. Otherwise every Server is same-site to every other, and `SameSite=Strict` (ADR 0015) stops separating them.
- Some routers block DNS answers that point at private IPs, so plain `http://<lan-ip>` stays the last Server Address.
- The goal is a Relay that the project runs with a bitrate cap. Because the Relay only ever sees TLS bytes, getting there means throttling and hosting, not changing the design.
- A Linked Server whose Admin set no Server URL uses its public Hub name as its Server URL, so it can create Invites and Notifier links.
- When the Hub is down, certificates aren't renewed, so a long outage ends HTTPS through Server names. The LAN still works over plain HTTP.
