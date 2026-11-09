#!/usr/bin/env bash
# Prints one JSON line per dependency that fails `cargo deny check advisories` in the current
# directory: {key, id, package, version, title, url}, sorted by key. `key` (advisory and crate)
# is what ci.yml compares between a PR and its base, and what advisories.yml files issues by.
# Exits non-zero only when cargo-deny couldn't run, never because it found something.
set -euo pipefail

out=$(mktemp)
trap 'rm -f "$out"' EXIT
cargo deny --format json check advisories 2> "$out" > /dev/null || true

# A run that crashed (a bad deny.toml, `cargo metadata` failing) writes no summary, and reading
# no findings from it would pass the check.
if ! jq -R 'fromjson? | select(.type == "summary")' "$out" | grep -q .; then
  cat "$out" >&2
  echo "cargo deny check advisories wrote no summary" >&2
  exit 1
fi

jq -cR 'fromjson? | select(.type == "diagnostic" and .fields.severity == "error") | .fields
  | (.advisory.id // .code) as $id | .graphs[0].Krate as $crate
  | {key: "\($id) \($crate.name)", id: $id, package: $crate.name, version: $crate.version,
     title: (.advisory.title // .message),
     url: (if .advisory.id then "https://rustsec.org/advisories/\(.advisory.id)" else null end)}' \
  "$out" | jq -sc 'unique_by(.key)[]'
