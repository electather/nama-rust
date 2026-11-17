#!/usr/bin/env bash
# Usage: migrations-unchanged.sh <base>
# Fails when HEAD edits, renames or deletes a Server migration that <base> has: an installed
# database records each migration's checksum, and a Server refuses one that changed.
# The rule this enforces: crates/nama-server/AGENTS.md.
set -euo pipefail

base=$(git rev-parse --verify "${1:?usage: migrations-unchanged.sh <base>}^{commit}")
dir=crates/nama-server/migrations/
changed=$(git diff --name-status --find-renames --diff-filter=MDR "$base" HEAD -- "$dir")
if [ -n "$changed" ]; then
  echo "$changed"
  echo "::error::a migration already on the base branch was edited, renamed or deleted; put the" \
    "change in a new migration instead (sqlx migrate add --timestamp --source $dir <name>)"
  exit 1
fi
