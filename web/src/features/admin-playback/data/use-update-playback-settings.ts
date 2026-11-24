import type { Transport } from "@connectrpc/connect";
import { createConnectQueryKey, useMutation, useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { ServerService } from "@/gen/nama/v1/server_pb";

function playbackSettingsKey(transport: Transport) {
  return createConnectQueryKey({
    schema: ServerService.method.getPlaybackSettings,
    cardinality: "finite",
    transport,
  });
}

/** Saves the Playback settings, then refetches them. */
export function useUpdatePlaybackSettings() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  return useMutation(ServerService.method.updatePlaybackSettings, {
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: playbackSettingsKey(transport) });
    },
  });
}
