import { isMessage } from "@bufbuild/protobuf";
import { useQueryClient } from "@tanstack/react-query";
import { useCallback } from "react";

import { planKeyRoot } from "@/features/player/data/use-plan";
import { PlaybackPlanSchema } from "@/gen/nama/v1/playback_pb";

/**
 * Drops the plan `planId` from the cache, so the Version's next player asks for a new one. Call it
 * only once nothing shows the plan, or its player asks for it again.
 */
export function useForgetPlan() {
  const queryClient = useQueryClient();
  return useCallback(
    (planId: bigint) => {
      queryClient.removeQueries({
        queryKey: [planKeyRoot],
        predicate: ({ state: { data } }) =>
          isMessage(data, PlaybackPlanSchema) && data.id === planId,
      });
    },
    [queryClient],
  );
}
