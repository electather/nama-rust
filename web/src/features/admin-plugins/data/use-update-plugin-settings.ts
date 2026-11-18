import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { PluginService } from "@/gen/nama/v1/plugin_pb";
import { pluginsKey } from "@/lib/plugins";

/** Saves a Plugin's settings, and refreshes the Plugins once it succeeds. */
export function useUpdatePluginSettings() {
  const queryClient = useQueryClient();
  return useMutation(PluginService.method.updatePluginSettings, {
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: pluginsKey });
    },
  });
}
