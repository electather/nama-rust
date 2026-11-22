# The Hub signs tokens only for Servers to verify; the ones it checks itself are opaque

ADR 0011 has the Hub sign short-lived JWTs so that a Server can trust a person arriving through the Hub even while the Hub is unreachable. That is the only reason the Hub signs anything. What the Hub verifies itself, such as a Linked Server's credential, a native app's Account token or a Hub web session, is a random opaque token that the Hub stores as a SHA-256 hash and revokes by deleting its row. So registering a Server through the device flow ends with one long-lived opaque credential for the Server, and Unlinking deletes it.

The Hub signs with Ed25519. Its private keys come only from its config, as a list: the first key signs, and the Hub publishes all of them at `/.well-known/jwks.json`, each with its RFC 7638 thumbprint as its `kid`. They are never stored in PostgreSQL.

## Considered Options

- **Access JWTs and refresh tokens for every caller, OAuth's usual shape**: rejected. A signed access token saves a database lookup per call, which pays off across many replicas, and the Hub runs one (ADR 0096). It also can't be revoked before it expires.
- **Keys generated at first start and stored in PostgreSQL**: rejected. Any copy of the database would then be able to sign anyone in as any Account on any Server.

## Consequences

- To rotate a key, put the new one first and deploy, then remove the old one once the longest signed token's lifetime and the time Servers cache the key set have both passed.
