import { useMutation } from "@connectrpc/connect-query";

import { useRefreshPlugins } from "@/features/admin-plugins/data/use-refresh-plugins";
import { PluginService } from "@/gen/nama/v1/plugin_pb";

/** Disables a Plugin, and refreshes the Plugins once it settles. */
export function useDisablePlugin() {
  return useMutation(PluginService.method.disablePlugin, { onSettled: useRefreshPlugins() });
}
