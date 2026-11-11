#!/usr/bin/env bash
# Keeps the factory running: after a clean exit (queue drained, MAX_ISSUES) it reruns it every IDLE_SECS
# (default 5 min) to pick up issues labelled since; after an infra failure (exit 1: Claude usage limit, Docker,
# auth) every RETRY_SECS (default 1h). Stops on STOP or Ctrl-C (130).
# Run from the repo root: `.sandcastle/supervise.sh`. Env (MAX_PARALLEL, MAX_ISSUES, …) passes through.
set -u
cd "$(dirname "$0")/.."
RETRY_SECS=${RETRY_SECS:-3600}
# A rerun, not a factory that waits: each run starts with a fresh view of the queue and of other machines' claims.
IDLE_SECS=${IDLE_SECS:-300}
# Fetch and push over HTTPS with gh's token: an SSH agent such as 1Password stops signing once it locks,
# so every overnight run died on `git fetch`.
export GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0='url.https://github.com/.insteadOf' GIT_CONFIG_VALUE_0='git@github.com:'
lock=~/.cache/nama-sandcastle/supervisor.lock
mkdir -p "$(dirname "$lock")"
# One supervisor per machine; main.ts's own pid file already keeps a second factory out.
exec 9>"$lock"
flock -n 9 || { echo "Another supervisor is already running."; exit 1; }

while :; do
  echo "$(date -Is) supervisor: starting factory"
  mise x -- npm --prefix .sandcastle run factory
  code=$?
  [ -e .sandcastle/STOP ] && { echo "$(date -Is) supervisor: STOP found, done"; exit 0; }
  case $code in
    0) pause=$IDLE_SECS ;;
    1) pause=$RETRY_SECS; echo "$(date -Is) supervisor: factory exited 1 (infra), retrying in ${pause}s" ;;
    *) echo "$(date -Is) supervisor: factory exited $code, done"; exit "$code" ;;
  esac
  for ((i = 0; i < pause; i += 60)); do
    [ -e .sandcastle/STOP ] && { echo "$(date -Is) supervisor: STOP found, done"; exit 0; }
    sleep 60
  done
done
