#!/usr/bin/env bash
# Reruns the factory after an infra failure (exit 1: Claude usage limit, Docker, auth) every RETRY_SECS
# (default 1h). Stops on a clean exit (queue drained, MAX_ISSUES, STOP) or Ctrl-C (130).
# Run from the repo root: `.sandcastle/supervise.sh`. Env (MAX_PARALLEL, MAX_ISSUES, …) passes through.
set -u
cd "$(dirname "$0")/.."
RETRY_SECS=${RETRY_SECS:-3600}
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
  [ "$code" -ne 1 ] && { echo "$(date -Is) supervisor: factory exited $code, done"; exit "$code"; }
  echo "$(date -Is) supervisor: factory exited 1 (infra), retrying in ${RETRY_SECS}s"
  for ((i = 0; i < RETRY_SECS; i += 60)); do
    [ -e .sandcastle/STOP ] && { echo "$(date -Is) supervisor: STOP found, done"; exit 0; }
    sleep 60
  done
done
