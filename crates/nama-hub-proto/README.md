# nama-hub-proto

Rust types and ConnectRPC stubs for the Hub API, generated from [`proto/nama/hub/`](../../proto/nama/hub) (ADR 0007). `nama.hub.v1` is the module `nama_hub_proto::nama::hub::v1`, holding the messages, the service traits the Hub implements, such as `HubService`, and the clients the Server's Kernel calls them with, such as `HubServiceClient`.

The code under `src/generated/` comes from the same `buf generate` as `nama-proto`'s; [`crates/nama-proto/README.md`](../nama-proto/README.md) says how to run it and how to break the schema on purpose.
