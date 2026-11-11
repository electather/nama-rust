#!/usr/bin/env bash
# Usage: buf-breaking.sh <base>
# Runs `buf breaking` on the working tree's proto/ against <base>'s. A break passes only when
# proto/BREAKING.md has more entries than at <base> (ADR 0032); otherwise it fails as buf does.
set -euo pipefail

base=$(git rev-parse --verify "${1:?usage: buf-breaking.sh <base>}^{commit}")
cd "$(git rev-parse --show-toplevel)/proto"

# buf exits 100 for compile errors as well as breaks, and no entry may excuse those.
buf build
status=0
buf breaking --against "../.git#ref=$base,subdir=proto" || status=$?
if [ "$status" -ne 100 ]; then
  exit "$status"
fi

# Entries are counted, not compared, so editing or deleting one never passes a break; an entry
# added in the change that deletes another fails too.
entry='^## [0-9]{4}-[0-9]{2}-[0-9]{2}, #[0-9]+'
entries_before=$(git show "$base:proto/BREAKING.md" 2>/dev/null | grep -cE "$entry" || true)
entries_now=$(grep -cE "$entry" BREAKING.md 2>/dev/null || true)
if (( entries_now > entries_before )); then
  echo "These breaks are declared in proto/BREAKING.md, so they pass."
  exit 0
fi
echo "To break the schema on purpose, declare it in proto/BREAKING.md (crates/nama-proto/README.md)." >&2
exit "$status"
