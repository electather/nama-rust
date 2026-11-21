import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { PluginService } from "@/gen/nama/v1/plugin_pb";

/** Replaces a Plugin's Qualities, and is done once everything has been read again. */
export function useReplaceQualities() {
  const queryClient = useQueryClient();
  return useMutation(PluginService.method.replaceQualities, {
    // One at a time, in the order they were asked: each sends the whole list, so the last one
    // asked must be the last one the Server takes.
    scope: { id: "qualities" },
    onSuccess: async () => {
      await queryClient.invalidateQueries();
    },
  });
}
