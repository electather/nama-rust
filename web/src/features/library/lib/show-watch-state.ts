import { create } from "@bufbuild/protobuf";

import { WatchStateSchema } from "@/gen/nama/v1/watch_state_pb";

/**
 * A Show's Watch State as its poster marks it: played once every Episode is, else as far through
 * as the share of its Episodes played.
 */
export function showWatchState({ episodes, played }: { episodes: number; played: number }) {
  const state = {
    played: played >= episodes,
    positionMs: BigInt(played),
    durationMs: BigInt(episodes),
  };
  return played === 0 ? undefined : create(WatchStateSchema, state);
}
