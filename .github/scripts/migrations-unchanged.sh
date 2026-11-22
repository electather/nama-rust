#!/usr/bin/env bash
# Usage: migrations-unchanged.sh <base>
# Fails when HEAD edits, renames or deletes a migration of any crate's, crates/*/migrations/, that
# <base> has: an installed database records each migration's checksum, and the Server and the Hub
# refuse one that changed.
# The rule this enforces: crates/nama-server/AGENTS.md and crates/nama-hub/AGENTS.md.
set -euo pipefail

base=$(git rev-parse --verify "${1:?usage: migrations-unchanged.sh <base>}^{commit}")
changed=$(git diff --name-status --find-renames --diff-filter=MDR "$base" HEAD -- 'crates/*/migrations/*')
if [ -n "$changed" ]; then
  echo "$changed"
  echo "::error::a migration already on the base branch was edited, renamed or deleted; put the" \
    "change in a new migration instead (sqlx migrate add --timestamp --source <its folder> <name>)"
  exit 1
fi
