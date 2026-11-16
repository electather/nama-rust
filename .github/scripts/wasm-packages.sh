#!/usr/bin/env bash
# Prints the `--package` arguments for linting the wasm32 crates: the contract's messages, the
# test Plugin, and every package under plugins/ built on nama-pdk (Installed Plugins and their
# shared libraries), so a Plugin's folder is all it takes to be linted.
set -euo pipefail
cd "$(dirname "$0")/../.."

{
  echo nama-plugin-proto
  echo nama-test-plugin
  for manifest in plugins/*/Cargo.toml; do
    if grep -q '^nama-pdk' "$manifest"; then
      sed -n 's/^name = "\(.*\)"$/\1/p' "$manifest" | head -n 1
    fi
  done | sort
} | sed 's/^/--package /' | paste -sd ' ' -
