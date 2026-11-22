#!/usr/bin/env bash
# Usage: buf-breaking.sh <base>
# Runs `buf breaking` on the client API, proto/nama/v1/, and the Hub API, proto/nama/hub/v1/,
# against <base>'s; the Plugin contract, proto/nama/plugin/, has its own version (ADR 0041). A
# break passes only when proto/BREAKING.md has more entries than at <base>; otherwise it fails as
# buf does.
# The rules this enforces: crates/nama-proto/AGENTS.md and crates/nama-hub-proto/AGENTS.md.
set -euo pipefail

base=$(git rev-parse --verify "${1:?usage: buf-breaking.sh <base>}^{commit}")
cd "$(git rev-parse --show-toplevel)/proto"

# buf exits 100 for compile errors on either side as well as for breaks; no entry excuses those.
buf build
buf build "../.git#ref=$base,subdir=proto"
status=0
buf breaking --path nama/v1 --path nama/hub/v1 --against "../.git#ref=$base,subdir=proto" || status=$?
if [ "$status" -ne 100 ]; then
  exit "$status"
fi

# Entries are counted, not compared (ADR 0032).
entry='^## [0-9]{4}-[0-9]{2}-[0-9]{2}, #[0-9]+'
entries_before=$(git show "$base:proto/BREAKING.md" 2>/dev/null | grep -cE "$entry" || true)
entries_now=$(grep -cE "$entry" BREAKING.md 2>/dev/null || true)
if (( entries_now > entries_before )); then
  echo "These breaks are declared in proto/BREAKING.md, so they pass."
  exit 0
fi
echo "To break the client or Hub API on purpose, declare it in proto/BREAKING.md (crates/nama-proto/README.md)." >&2
exit "$status"
