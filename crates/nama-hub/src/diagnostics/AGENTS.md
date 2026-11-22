## Diagnostics

The Diagnostics and Usage Analytics Linked Servers share with Nama's developers (ADR 0054).

- **Forwarded unchanged.** A batch goes to the backend byte for byte: what it may carry is the Server's rule ("Spans carry ids, not words" in `crates/nama-server/AGENTS.md`), so the Hub neither reads nor stores it.
- **Only Linked Servers share,** each limited to `BYTES_PER_WINDOW` a minute, so one gone wrong can't run up the backend's bill.
- **The backend is a true external** (ADR 0099): `Backend`, with `Otlp` here and a fake in the tests. Without one configured, what Servers share is dropped.

### Neighbours

- Uses: `links` (`LinkedServerId`).
- Used by: `api`, whose auth layer names the Linked Server, and `main`, which builds the backend from the config.
