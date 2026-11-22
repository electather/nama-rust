## Signing

Signing the tokens Servers verify (ADR 0011, ADR 0098): the Hub's Ed25519 keys and the key set it publishes.

- **Only for Servers.** A token is signed only for a Server to verify, such as while the Hub is unreachable. What the Hub checks itself, a session or a Linked Server's credential, is opaque (ADR 0098).
- **Keys come from the config only**, `NAMA_HUB_SIGNING_KEYS`, never PostgreSQL. The first signs, and all are published at `/.well-known/jwks.json`, each with its RFC 7638 thumbprint as `kid`, so a key can be rotated (ADR 0098, "Consequences").

### Neighbours

- Uses: nothing.
- Used by: `links`, which signs the token a Server gets once Linked, `api`, which serves the key set, and `main`, which reads the keys.
