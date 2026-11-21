import { isMessage } from "@bufbuild/protobuf";
import { type Query, type QueryCache, useQueryClient } from "@tanstack/react-query";
import { useCallback } from "react";

import { planKey } from "@/features/player/data/use-plan";
import { PlaybackPlanSchema } from "@/gen/nama/v1/playback_pb";

/**
 * Drops the plan `planId` from the cache once nothing shows it, so the Version's next player asks
 * for a new one. Call it only once the plan's `<video>` is gone.
 */
export function useForgetPlan() {
  const queryClient = useQueryClient();
  return useCallback(
    (planId: bigint) => {
      const cache = queryClient.getQueryCache();
      const plans = cache.findAll({
        queryKey: planKey,
        predicate: ({ state: { data } }) =>
          isMessage(data, PlaybackPlanSchema) && data.id === planId,
      });
      for (const plan of plans) {
        removeOnceUnseen(cache, plan);
      }
    },
    [queryClient],
  );
}

// An Admin's stop, or a player that gave up, still shows the plan its `<video>` left: removed then,
// the next render would ask the Server for a new plan and play it.
function removeOnceUnseen(cache: QueryCache, query: Query) {
  if (query.getObserversCount() === 0) {
    cache.remove(query);
    return;
  }
  const unsubscribe = cache.subscribe((event) => {
    if (event.query === query && query.getObserversCount() === 0) {
      unsubscribe();
      cache.remove(query);
    }
  });
}
