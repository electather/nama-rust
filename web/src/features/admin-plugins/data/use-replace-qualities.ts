import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { PluginService, type Quality } from "@/gen/nama/v1/plugin_pb";

/** Replaces a Plugin's Qualities, calling `onReplaced` with the saved ones, and refetches everything. */
export function useReplaceQualities(onReplaced: (qualities: Quality[]) => void) {
  const queryClient = useQueryClient();
  return useMutation(PluginService.method.replaceQualities, {
    onSuccess: async ({ qualities }) => {
      onReplaced(qualities);
      await queryClient.invalidateQueries();
    },
  });
}
