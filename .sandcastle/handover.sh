#!/usr/bin/env bash
# Replaces the running factory with the code checked out now, without draining it first: the old run finishes
# what it holds (its builds, triages and PRs to their merge) and starts nothing, while a new run starts at once and
# takes everything else. The two share the machine's slots. Needs the supervisor (supervise.sh) running.
set -eu
pid=$(cat ~/.cache/nama-sandcastle/supervisor.pid 2>/dev/null) || { echo "No supervisor is running."; exit 1; }
kill -HUP "$pid" || { echo "Supervisor $pid isn't running."; exit 1; }
echo "Handover asked of supervisor $pid; follow it in its log (journalctl --user -u nama-factory -f)."
