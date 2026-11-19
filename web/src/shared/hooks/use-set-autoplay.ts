import { createConnectQueryKey, useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { PlaybackService } from "@/gen/nama/v1/playback_pb";

// The query key of the autoplay choice, to refetch after saving it.
const autoplayKey = createConnectQueryKey({
  schema: PlaybackService.method.getAutoplay,
  cardinality: "finite",
});

/** Saves whether the next Episode plays by itself, then refetches it. */
export function useSetAutoplay() {
  const queryClient = useQueryClient();
  return useMutation(PlaybackService.method.setAutoplay, {
    onSettled: async () => {
      await queryClient.invalidateQueries({ queryKey: autoplayKey });
    },
  });
}
