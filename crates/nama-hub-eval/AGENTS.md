## nama-hub-eval

- **MovieLens is downloaded each run** and kept in memory: never committed, never written to a database, never in a release (`dist = false`). Its licence is for research only (ADR 0095).
- **Score the Hub's own code.** The harness calls `nama-hub`'s library for vectors and Similar answers; it trains nothing of its own that the Hub could serve.
