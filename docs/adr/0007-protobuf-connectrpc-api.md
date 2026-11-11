# Schema-first Protobuf + ConnectRPC for the API and Plugin messages

The `.proto` files in `proto/` are the single source of truth. Buf generates the Rust server stubs (`connectrpc`, which plugs into axum), the TypeScript client (`connect-es` plus `connect-query`, which wraps TanStack Query), the Swift and Kotlin clients, and the messages passed to and from Installed Plugins. Nobody hand-writes a client for any platform. Connect runs over plain HTTP with JSON or binary bodies, so browsers need no gRPC proxy. Media bytes are not RPC: direct-play range requests, HLS playlists and segments, and images are plain HTTP under `/media/...`.

## Considered Options

- **Code-first REST + OpenAPI (utoipa)**: rejected. Swift and Kotlin generators are uneven, and nothing pushes changes to stay backward compatible.
- **GraphQL**: rejected. Too much machinery for a media API, and it doesn't fit streaming media.

## Consequences

- `connectrpc` is pre-1.0, which is a known risk.
- Protobuf field-number rules make backward compatibility something every change has to consider. A change that breaks it on purpose declares the break (ADR 0032).
