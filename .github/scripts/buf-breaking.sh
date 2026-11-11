#!/usr/bin/env bash
# Usage: buf-breaking.sh <base>
# Runs `buf breaking` on the working tree's proto/ against <base>'s. A break passes only when
# proto/BREAKING.md has more entries than at <base>; otherwise it fails as buf does.
# The rule this enforces: crates/nama-proto/AGENTS.md.
set -euo pipefail

base=$(git rev-parse --verify "${1:?usage: buf-breaking.sh <base>}^{commit}")
cd "$(git rev-parse --show-toplevel)/proto"

# buf exits 100 for compile errors on either side as well as for breaks; no entry excuses those.
buf build
buf build "../.git#ref=$base,subdir=proto"
status=0
buf breaking --against "../.git#ref=$base,subdir=proto" || status=$?
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
echo "To break the schema on purpose, declare it in proto/BREAKING.md (crates/nama-proto/README.md)." >&2
exit "$status"
