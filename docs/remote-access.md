# Reaching a Server from outside the home

A Server Linked to the Hub gets its own HTTPS names from it (ADR 0057), which Settings shows: its own name, such as `<id>.namadirect.net`, and `192-168-1-10.<id>.namadirect.net` for each of its LAN addresses. It gets a certificate for them from Let's Encrypt, agreeing to its Subscriber Agreement, and keeps it, with its key, which never leaves the Server, in `certificate.pem` in its data directory, renewing it a month before it expires. It serves HTTPS on its own port, 7980, beside plain HTTP, which the LAN keeps using, even while the certificate can't be renewed, such as while the Hub is down. Its own name reaches it from outside only while the Hub can reach its public port: one the Server opens on the router with UPnP or NAT-PMP, or the one you forward and name in `forwarded_port` in `nama.toml`. Then, unless you set a Server URL, `https://<id>.namadirect.net:<port>` is the Server URL.

## A Relay

Where the Hub can't reach the Server's port, such as behind CGNAT, a Linked Server can reach clients through a Relay you run on a machine with a public IP address, such as a small VPS. The Relay passes TLS through without decrypting it, routing each connection by the name it asks for, so it never sees what crosses it; the Server still answers with its own certificate. The Server dials out to the Relay and shows it a token from the Hub naming the Server, so nothing needs forwarding at home.

1. On the public machine, run the Relay, naming the Hub the Server is Linked to, and open its port, 443 unless you pick another with `--listen`:

   ```sh
   nama-server relay --hub https://hub.example
   ```

   or with Docker, `docker run -d -p 443:443 -e NAMA_HUB_URL=https://hub.example ghcr.io/electather/nama-server relay`.

2. Name it in the Server's `nama.toml` by its public IP address and port, and restart the Server:

   ```toml
   relay = "198.51.100.4:443"
   ```

The Hub then gives the Server the name `relay.<id>.namadirect.net`, pointing at the Relay, and offers `https://relay.<id>.namadirect.net:443` to clients after the Server's direct addresses, before plain `http://<lan-ip>`. While the Hub can't reach the Server's own port, that is its Server URL, unless you set one. Every byte of a stream through it crosses the Relay's machine twice, so it counts against that machine's bandwidth.

## Without the Hub

Where none of that can work, or the Server isn't Linked, an Admin opens a Server to the internet themselves. Pick one of the two setups below. For these, don't forward port 7980 to the internet yourself: over its plain HTTP, passwords and session cookies cross it in plain text.

| | Caddy | Tailscale |
|---|---|---|
| Needs | A public IP you can forward ports on (no CGNAT) and a domain | Nothing; works behind CGNAT |
| Who can connect | Anyone with the link, TVs and browsers included | Only devices running Tailscale and signed in to your tailnet |

## Caddy with a domain

1. Point a DNS name, such as `nama.example.com`, at your home's public IP.
2. Forward ports 80 and 443 on your router to the machine running Caddy.
3. Give Caddy this `Caddyfile`, with the Server's LAN address. Caddy gets the certificate itself.

   ```caddyfile
   nama.example.com {
       reverse_proxy 192.168.1.5:7980
   }
   ```

4. Add Caddy's address to `trusted_proxies` in the Server's `nama.toml` and restart the Server, so sign-in limits count each real client and the session cookie is `Secure`:

   ```toml
   trusted_proxies = ["192.168.1.4"]
   ```

5. In the web app, open Settings and set the Server URL to `https://nama.example.com`, so links in messages point there.

If your provider puts you behind CGNAT (your router's WAN address starts with `100.64`–`100.127`, or differs from what a "what is my IP" site shows), port forwarding can't work. Use Tailscale, or run Caddy on a small VPS that reaches home over Tailscale or WireGuard.

## Tailscale

1. Install Tailscale on the Server's machine and on every device that should connect.
2. On the Server's machine, serve the Server over HTTPS inside your tailnet:

   ```sh
   tailscale serve --bg http://127.0.0.1:7980
   ```

3. Set the Server URL to the `https://<machine>.<tailnet>.ts.net` address that `tailscale serve` printed.

To let family in, share the machine with their Tailscale accounts from the Tailscale admin console. Devices that can't run Tailscale, such as most smart TVs and consoles, can't connect this way. Tailscale doesn't document sending `X-Forwarded-For`, so every remote client reaches the Server from `127.0.0.1` and they share one sign-in limit.

## Tunnels

Tunnel services work behind CGNAT too. Check their terms before streaming through one: Cloudflare's, for example, have restricted serving video through its free plans, so streams can be throttled or the account suspended. The web app alone is fine.
