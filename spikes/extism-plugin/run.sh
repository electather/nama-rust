#!/bin/sh
# Builds the guest twice (v1 and v2) and runs the host's experiments against both.
set -eu
cd "$(dirname "$0")"
for version in v1 v2; do
  features=""
  [ "$version" = v2 ] && features="--features v2"
  cargo build -p guest --target wasm32-unknown-unknown --release $features
  cp target/wasm32-unknown-unknown/release/guest.wasm "target/guest-$version.wasm"
done
cargo run -p host --release -- target/guest-v1.wasm target/guest-v2.wasm
