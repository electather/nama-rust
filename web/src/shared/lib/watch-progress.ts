import type { WatchState } from "@/gen/nama/v1/watch_state_pb";

/** How far into a Movie or Episode you are, as a percentage: 0 unless started and not played. */
export function watchedPercent(state: WatchState | undefined) {
  if (!state || state.played || state.durationMs === 0n) {
    return 0;
  }
  return Number((state.positionMs * 100n) / state.durationMs);
}

/** Whether you've started it and not played it, so it resumes. */
export function isStarted(state: WatchState | undefined): state is WatchState {
  return state !== undefined && !state.played && state.positionMs > 0n;
}
