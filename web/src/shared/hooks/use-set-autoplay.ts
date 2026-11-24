import type { Transport } from "@connectrpc/connect";
import { createConnectQueryKey, useMutation, useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { PlaybackService } from "@/gen/nama/v1/playback_pb";

function autoplayKey(transport: Transport) {
  return createConnectQueryKey({
    schema: PlaybackService.method.getAutoplay,
    cardinality: "finite",
    transport,
  });
}

/** Saves whether the next Episode plays by itself, then refetches it. */
export function useSetAutoplay() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  return useMutation(PlaybackService.method.setAutoplay, {
    onSettled: async () => {
      await queryClient.invalidateQueries({ queryKey: autoplayKey(transport) });
    },
  });
}
