#!/usr/bin/env bash
# Keeps the factory running: after a clean exit (queue drained, MAX_ISSUES) it reruns it every IDLE_SECS
# (default 5 min) to pick up issues labelled since; after an infra failure (exit 1: Claude usage limit, Docker,
# auth) every RETRY_SECS (default 1h). Stops on STOP or Ctrl-C (130).
# SIGHUP (`.sandcastle/handover.sh`) swaps in the code checked out now without waiting: the running factory drains
# (SIGUSR1: it finishes what it holds and starts nothing) while a new one starts at once and takes everything else.
# Run from the repo root: `.sandcastle/supervise.sh`. Env (MAX_PARALLEL, MAX_ISSUES, …) passes through.
set -u
cd "$(dirname "$0")/.."
RETRY_SECS=${RETRY_SECS:-3600}
# A rerun, not a factory that waits: each run starts with a fresh view of the queue and of other machines' claims.
IDLE_SECS=${IDLE_SECS:-300}
# Fetch and push over HTTPS with gh's token: an SSH agent such as 1Password stops signing once it locks,
# so every overnight run died on `git fetch`.
export GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0='url.https://github.com/.insteadOf' GIT_CONFIG_VALUE_0='git@github.com:'
cache=~/.cache/nama-sandcastle
lock=$cache/supervisor.lock
factory_pid=$cache/factory.pid # main.ts's: the active run's node process
mkdir -p "$cache"
# One supervisor per machine; main.ts's own pid file already keeps a second active factory out.
exec 9>"$lock"
flock -n 9 || { echo "Another supervisor is already running."; exit 1; }
echo $$ >"$cache/supervisor.pid"

handover=
trap 'handover=1' HUP
say() { echo "$(date -Is) supervisor: $*"; }
# A draining run is a child too: wait for it, or ending the unit would kill it mid-work.
finish() {
  say "$1"
  wait
  exit "$2"
}
start() {
  say "starting factory"
  mise x -- npm --prefix .sandcastle run factory &
  child=$!
}
# Tells the active run to drain, then waits until it lets go of the pid file, which the new run needs.
drain() {
  local pid
  pid=$(cat "$factory_pid" 2>/dev/null) || return 0
  kill -USR1 "$pid" 2>/dev/null || return 0
  for ((i = 0; i < 30; i++)); do
    [ -e "$factory_pid" ] || { say "pid $pid is draining"; return 0; }
    sleep 1
  done
  say "pid $pid didn't start draining within 30s: keeping it, no new run"
  return 1
}

while :; do
  start
  # `wait` returns early when HUP arrives; only the newest run's exit decides what comes next.
  while :; do
    wait "$child"
    code=$?
    [ -z "$handover" ] && break
    handover=
    say "handover"
    drain && start
  done
  [ -e .sandcastle/STOP ] && finish "STOP found, done" 0
  case $code in
    0) pause=$IDLE_SECS ;;
    1) pause=$RETRY_SECS; say "factory exited 1 (infra), retrying in ${pause}s" ;;
    *) finish "factory exited $code, done" "$code" ;;
  esac
  for ((i = 0; i < pause; i += 60)); do
    [ -e .sandcastle/STOP ] && finish "STOP found, done" 0
    [ -n "$handover" ] && { handover=; say "handover: starting the new code now"; break; }
    sleep 60 &
    wait $!
  done
done
