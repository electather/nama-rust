import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { pluginsKey } from "@/features/admin-plugins/data/plugins-key";
import { PluginService } from "@/gen/nama/v1/plugin_pb";

/** Saves a Plugin's settings, one save at a time, and refreshes the Plugins once it succeeds. */
export function useUpdatePluginSettings() {
  const queryClient = useQueryClient();
  return useMutation(PluginService.method.updatePluginSettings, {
    // One at a time, in the order they were asked: each save sends every setting, so the last
    // one asked must be the last one the Server takes.
    scope: { id: "plugin-settings" },
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: pluginsKey });
    },
  });
}
