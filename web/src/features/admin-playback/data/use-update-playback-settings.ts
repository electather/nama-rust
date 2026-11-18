import { createConnectQueryKey, useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { ServerService } from "@/gen/nama/v1/server_pb";

const playbackSettingsKey = createConnectQueryKey({
  schema: ServerService.method.getPlaybackSettings,
  cardinality: "finite",
});

/** Saves the Playback settings, then refetches them. */
export function useUpdatePlaybackSettings() {
  const queryClient = useQueryClient();
  return useMutation(ServerService.method.updatePlaybackSettings, {
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: playbackSettingsKey });
    },
  });
}
