# Forwarded headers are trusted only from configured proxies

A Server is usually reached over plain HTTP on a LAN, and sometimes through a reverse proxy that terminates HTTPS. It honours `X-Forwarded-For` and `X-Forwarded-Proto` only when the request's peer is in `trusted_proxies` in `nama.toml`, which is empty by default. Behind an untrusted proxy every client would share the proxy's address for rate limits, and anyone could claim HTTPS. So sign-in limits, a Device's `ip_address` and the session cookie's `Secure` flag all come from the peer unless that peer is a trusted proxy.

## Considered Options

- **Always trust forwarded headers**: rejected. Any client on the LAN could pick its own address and dodge per-IP limits.
- **Set `Secure` from a configured public URL**: rejected. The same Server is often reached both as `https://nama.example.com` and as `http://192.168.1.5`, and a browser drops a `Secure` cookie over plain HTTP, so `Secure` is decided per request.
